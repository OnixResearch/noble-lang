//! Bounded host observation record for the source-bound `test.send` adapter
//! (WORKER-10).
//!
//! The host, never the guest, decides and observes each send attempt. A
//! request is authorized against the host's own exact-plan grant table, then
//! admitted and started through `noble_kernel::authority` and handed to an
//! in-process synthetic peer. Completion and unknown outcomes are kernel
//! observations from the host's observation source; the guest learns none of
//! them. The host-selected terminal event cancels the invocation while an
//! admitted attempt still awaits its acknowledgement.
//!
//! Observation rows are retained up to a host-selected bound; overflow is
//! counted and marked as omitted scope, never dropped silently. The
//! diagnostic is bounded the same way. Payload Text is never exported: rows
//! carry correlation only.

use anyhow::{ensure, Context, Result};
use noble_kernel::authority as auth;
use serde_json::{json, Value};

const OPERATION: &str = "noble-test:worker-program/send@1.0.0#send";
const AUTHORITY_SOURCE: auth::SourceId = auth::SourceId(1);
const OBSERVATION_SOURCE: auth::SourceId = auth::SourceId(2);
const INVOCATION_SOURCE: auth::SourceId = auth::SourceId(3);
const POLICY: auth::PolicyRevision = auth::PolicyRevision(1);
const CHECKPOINT: auth::Checkpoint = auth::Checkpoint {
    sequence: 1,
    now: 10,
};
/// Every request costs at most one authorization and one admission request.
const KERNEL_REQUESTS_PER_SEND: u32 = 2;
/// Exact `(attempt, payload)` plans the host's authority source grants.
const GRANTS: [(u32, &str); 2] = [(1, "w10-a"), (3, "w10-c")];
/// Whether the synthetic peer's acknowledgement of the n-th admitted delivery
/// reaches the host.
const ACKNOWLEDGEMENTS: [bool; 2] = [true, false];

/// Host-selected observation bounds; nothing here is read from the guest.
#[derive(Clone, Copy)]
pub(super) struct Scenario {
    rows: usize,
    diagnostic_bytes: usize,
}

impl Scenario {
    /// The three WORKER-10 variants differ only in their output bounds.
    pub(super) fn for_mode(mode: &str) -> Option<Self> {
        let (rows, diagnostic_bytes) = match mode {
            "send-observe" => (16, 512),
            "send-observe-rows" => (6, 512),
            "send-observe-diagnostic" => (16, 48),
            _ => return None,
        };
        Some(Self {
            rows,
            diagnostic_bytes,
        })
    }
}

/// What the guest's import learns: nothing about the operation outcome.
pub(super) enum Status {
    /// The host recorded the request; the invocation continues.
    Recorded,
    /// The host's terminal event ended the invocation.
    Terminated,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Requested,
    Denied,
    Admitted,
    Completed,
    Unknown,
}

impl Kind {
    const fn name(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Denied => "denied",
            Self::Admitted => "admitted",
            Self::Completed => "completed",
            Self::Unknown => "externally-unknown",
        }
    }
}

#[derive(Clone, Copy)]
struct Row {
    sequence: u32,
    attempt: u32,
    kind: Kind,
    kernel: Option<auth::AttemptKey>,
}

/// The contiguous tail of observations that did not fit the row bound.
struct Omitted {
    first_sequence: u32,
    first_attempt: u32,
    last_attempt: u32,
    count: u32,
}

struct Invocation {
    owner: auth::OwnerContext,
    authority: auth::Authority,
    rows: Vec<Row>,
    omitted: Option<Omitted>,
    observations: u32,
    requests: u32,
    denied: u32,
    admitted: Vec<(u32, auth::AttemptKey)>,
    witnesses: Vec<auth::WitnessClaim>,
    deliveries: u32,
    cancelled: bool,
    task_cancel_acknowledged: Option<bool>,
}

/// One selected send policy records exactly one invocation.
pub(super) struct Recorder {
    scenario: Scenario,
    invocation: Option<Invocation>,
}

fn plan(owner: auth::OwnerContext, attempt: u32, text: &str) -> Result<auth::Plan> {
    ensure!(
        text.len() <= auth::MAX_ARGUMENT_BYTES - 4,
        "send payload exceeds the plan argument bound"
    );
    // The complete, unambiguous arguments: neither a digest nor a label can
    // stand in for the host's exact grant.
    let mut arguments = Vec::with_capacity(4 + text.len());
    arguments.extend_from_slice(&attempt.to_le_bytes());
    arguments.extend_from_slice(text.as_bytes());
    auth::Plan::new(auth::PlanDescription {
        operation: auth::OperationContract {
            name: OPERATION.as_bytes().to_vec(),
            effect: noble_kernel::contracts::TEST_SEND,
        },
        arguments,
        actor: auth::ActorId(1),
        owner,
        policy: POLICY,
        constraints: auth::Constraints {
            not_before: 1,
            expires_at: 20,
            quota: auth::QuotaId(1),
            units: 1,
            scope: 1,
        },
    })
    .map_err(|error| anyhow::anyhow!("send plan refused: {error:?}"))
}

/// The host's current facts for one attempt. The grant, when the host's
/// table has one, is the host's own independently built plan.
fn facts(owner: auth::OwnerContext, attempt: u32) -> Result<auth::CurrentFacts> {
    let grant = match GRANTS.iter().find(|(granted, _)| *granted == attempt) {
        Some((granted, payload)) => Some(auth::Grant {
            plan: plan(owner, *granted, payload)?,
            kind: auth::WITNESS_KIND,
            rights: auth::EXECUTE_RIGHT,
        }),
        None => None,
    };
    Ok(auth::CurrentFacts {
        source: AUTHORITY_SOURCE,
        checkpoint: CHECKPOINT,
        grant,
        credentials_valid: Some(true),
        revoked: Some(false),
        policy: Some(POLICY),
        quota_remaining: Some(GRANTS.len() as u64),
    })
}

fn attempt_name(attempt: u32) -> String {
    format!("attempt-{attempt}")
}

impl Recorder {
    pub(super) const fn new(scenario: Scenario) -> Self {
        Self {
            scenario,
            invocation: None,
        }
    }

    /// Open the record for the issued invocation of one installed Program.
    pub(super) fn begin(&mut self, instance: u64) -> Result<()> {
        ensure!(
            self.invocation.is_none(),
            "a selected send policy records one invocation"
        );
        let owner = auth::OwnerContext {
            instance,
            invocation: 1,
            generation: 1,
        };
        let authority = auth::Authority::new(auth::Profile {
            owner,
            witness_kind: auth::WITNESS_KIND,
            authority_source: AUTHORITY_SOURCE,
            observation_source: OBSERVATION_SOURCE,
            invocation_source: INVOCATION_SOURCE,
            checkpoint: CHECKPOINT,
            limits: Self::limits(),
        })
        .map_err(|error| anyhow::anyhow!("send authority refused: {error:?}"))?;
        self.invocation = Some(Invocation {
            owner,
            authority,
            rows: Vec::with_capacity(self.scenario.rows),
            omitted: None,
            observations: 0,
            requests: 0,
            denied: 0,
            admitted: Vec::with_capacity(GRANTS.len()),
            witnesses: Vec::with_capacity(GRANTS.len()),
            deliveries: 0,
            cancelled: false,
            task_cancel_acknowledged: None,
        });
        Ok(())
    }

    const fn limits() -> auth::Limits {
        auth::Limits {
            witnesses: GRANTS.len(),
            attempts: GRANTS.len(),
            requests: KERNEL_REQUESTS_PER_SEND * super::worker_program_guest::EFFECT_REQUEST_LIMIT,
        }
    }

    /// One guest request. `admissible` is the policy's issue check: the
    /// installed cell is issued, unrevoked, current, running and in bounds.
    pub(super) fn request(&mut self, text: &str, admissible: bool) -> Result<Status> {
        let rows = self.scenario.rows;
        let invocation = self
            .invocation
            .as_mut()
            .context("send request outside an issued invocation")?;
        invocation.request(rows, text, admissible)
    }

    pub(super) fn note_task_cancellation(&mut self, acknowledged: bool) -> Result<()> {
        let invocation = self
            .invocation
            .as_mut()
            .context("send cancellation outside an issued invocation")?;
        invocation.task_cancel_acknowledged = Some(acknowledged);
        Ok(())
    }

    /// The complete bounded record after the invocation ended. Every count,
    /// outcome and omission is derived from retained host or kernel state.
    pub(super) fn record(
        &self,
        guest_completed: bool,
        issued_allowance_retired: bool,
        build_context: Value,
        mut profile: Value,
    ) -> Result<Value> {
        let invocation = self
            .invocation
            .as_ref()
            .context("no issued send invocation was recorded")?;
        let outcomes = invocation.outcomes()?;
        let primary = invocation.primary_outcome(guest_completed);
        let name = format!("invocation-{}", invocation.owner.invocation);
        let completed = outcomes
            .iter()
            .filter(|(_, outcome)| *outcome == "completed")
            .count();
        let unknown: Vec<String> = outcomes
            .iter()
            .filter(|(_, outcome)| *outcome == "unknown")
            .map(|(attempt, _)| attempt_name(*attempt))
            .collect();
        let counters = invocation.authority.counters();
        let full = format!(
            "{name} {primary}: {} requests, {} denied, {} admitted, {completed} completed; outcome unknown for {}",
            invocation.requests,
            invocation.denied,
            counters.attempts_admitted,
            if unknown.is_empty() {
                "none".to_owned()
            } else {
                unknown.join(", ")
            }
        );
        let (diagnostic, diagnostic_omitted) = bounded(&full, self.scenario.diagnostic_bytes);
        profile["observation_rows"] = json!(self.scenario.rows);
        profile["diagnostic_bytes"] = json!(self.scenario.diagnostic_bytes);
        profile["authority_limits"] = json!({
            "witnesses": Self::limits().witnesses,
            "attempts": Self::limits().attempts,
            "requests": Self::limits().requests,
        });
        Ok(json!({
            "schema": "noble-worker-host-observation/v1",
            "invocation": name,
            "program_build_context": build_context,
            "execution_profile": profile,
            "primary_outcome": primary,
            "terminal_event": if invocation.cancelled { "cancellation" } else { "none" },
            "ownership_disposition": invocation.disposition(issued_allowance_retired, &outcomes)?,
            "counters": {
                "guest_requests": invocation.requests,
                "denied_requests": invocation.denied,
                "protected_operation_admissions": counters.attempts_admitted,
                "started_operations": counters.protected_operations,
                "completed_operations": completed,
                "approved_observations": counters.approved_observations,
                "observations_total": invocation.observations,
            },
            "operation_attempt_correlation": {
                "rows": invocation.rows.iter().map(|row| row.json(&name)).collect::<Vec<_>>(),
                "retained": invocation.rows.len(),
                "total": invocation.observations,
            },
            "omitted_observation_scope": invocation.omitted_scope(diagnostic_omitted),
            "unknown_operation_outcomes": unknown,
            "diagnostic": {
                "text": diagnostic,
                "bytes": diagnostic.len(),
                "limit": self.scenario.diagnostic_bytes,
                "truncated": diagnostic_omitted != 0,
            },
            "synthetic_peer": {"deliveries_received": invocation.deliveries},
        }))
    }
}

/// The longest prefix of `text` within `limit` bytes, and the omitted count.
fn bounded(text: &str, limit: usize) -> (&str, usize) {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], text.len() - end)
}

impl Row {
    fn json(&self, invocation: &str) -> Value {
        json!({
            "sequence": self.sequence,
            "invocation": invocation,
            "attempt": attempt_name(self.attempt),
            "observation": self.kind.name(),
            "kernel_attempt": self.kernel.map(|key| json!({
                "slot": key.slot,
                "generation": key.generation,
            })),
        })
    }
}

impl Invocation {
    fn request(&mut self, rows: usize, text: &str, admissible: bool) -> Result<Status> {
        self.requests = self
            .requests
            .checked_add(1)
            .context("send request count overflow")?;
        let attempt = self.requests;
        self.observe(rows, attempt, Kind::Requested, None)?;
        let proposed = match plan(self.owner, attempt, text) {
            Ok(proposed) if admissible => proposed,
            Ok(_) | Err(_) => return self.deny(rows, attempt),
        };
        let facts = facts(self.owner, attempt)?;
        let witness = match self
            .authority
            .authorize_from_trusted_host(proposed.clone(), &facts)
        {
            auth::Authorization::Authorized(witness) => witness,
            auth::Authorization::Denied(_) | auth::Authorization::Preflight(_) => {
                return self.deny(rows, attempt)
            }
        };
        self.witnesses.push(witness.claim());
        let admission = auth::AdmissionRequest {
            claim: witness.claim(),
            plan: proposed,
        };
        let execution = match self.authority.admit(Some(witness), &admission, &facts) {
            auth::Admission::Committed(execution) => execution,
            auth::Admission::Denied { witness, .. }
            | auth::Admission::Preflight { witness, .. } => {
                if let Some(witness) = witness {
                    self.authority
                        .retire_witness(witness)
                        .map_err(|error| anyhow::anyhow!("send witness retirement: {error:?}"))?;
                }
                return self.deny(rows, attempt);
            }
        };
        let key = execution.attempt();
        self.admitted.push((attempt, key));
        self.observe(rows, attempt, Kind::Admitted, Some(key))?;
        self.start(rows, attempt, execution)
    }

    fn deny(&mut self, rows: usize, attempt: u32) -> Result<Status> {
        self.denied = self
            .denied
            .checked_add(1)
            .context("send denial count overflow")?;
        self.observe(rows, attempt, Kind::Denied, None)?;
        Ok(Status::Recorded)
    }

    /// The one action allowed by `Started`: hand the payload to the peer.
    fn start(&mut self, rows: usize, attempt: u32, execution: auth::Execution) -> Result<Status> {
        let started = self
            .authority
            .start_execution(execution)
            .map_err(|error| anyhow::anyhow!("send start refused: {error:?}"))?;
        let delivery = usize::try_from(self.deliveries)?;
        self.deliveries = self
            .deliveries
            .checked_add(1)
            .context("send delivery count overflow")?;
        let acknowledged = ACKNOWLEDGEMENTS.get(delivery).copied().unwrap_or(false);
        if acknowledged {
            self.observe_outcome(&started, auth::ObservedOutcome::OperationSuccess)?;
            self.observe(rows, attempt, Kind::Completed, Some(started.attempt()))?;
            return Ok(Status::Recorded);
        }
        // Host-selected terminal event: cancel the invocation while this
        // attempt's acknowledgement is outstanding. The kernel keeps every
        // earlier attempt and still admits this attempt's observation.
        self.authority
            .cancel_invocation()
            .map_err(|error| anyhow::anyhow!("send cancellation refused: {error:?}"))?;
        self.cancelled = true;
        self.observe_outcome(&started, auth::ObservedOutcome::Unknown)?;
        self.observe(rows, attempt, Kind::Unknown, Some(started.attempt()))?;
        Ok(Status::Terminated)
    }

    fn observe_outcome(
        &mut self,
        started: &auth::Started,
        outcome: auth::ObservedOutcome,
    ) -> Result<()> {
        self.authority
            .observe_from_trusted_host(auth::ObservationDescription {
                attempt: started.attempt(),
                plan: started.plan().clone(),
                source: OBSERVATION_SOURCE,
                checkpoint: CHECKPOINT,
                outcome,
            })
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!("send observation refused: {error:?}"))
    }

    /// Retain the observation if the row bound allows; otherwise extend the
    /// marked omitted tail. Either way it is counted.
    fn observe(
        &mut self,
        rows: usize,
        attempt: u32,
        kind: Kind,
        kernel: Option<auth::AttemptKey>,
    ) -> Result<()> {
        self.observations = self
            .observations
            .checked_add(1)
            .context("send observation count overflow")?;
        if self.rows.len() < rows {
            self.rows.push(Row {
                sequence: self.observations,
                attempt,
                kind,
                kernel,
            });
            return Ok(());
        }
        match &mut self.omitted {
            Some(omitted) => {
                omitted.count = omitted
                    .count
                    .checked_add(1)
                    .context("omitted observation count overflow")?;
                omitted.last_attempt = attempt;
            }
            None => {
                self.omitted = Some(Omitted {
                    first_sequence: self.observations,
                    first_attempt: attempt,
                    last_attempt: attempt,
                    count: 1,
                })
            }
        }
        Ok(())
    }

    /// Each admitted attempt's outcome from the kernel's retained record:
    /// completed only with an approved success; started without a terminal
    /// observation is unknown, never assumed to have had no effect.
    fn outcomes(&self) -> Result<Vec<(u32, &'static str)>> {
        self.admitted
            .iter()
            .map(|(attempt, key)| {
                let snapshot = self
                    .authority
                    .attempt_snapshot(*key)
                    .map_err(|error| anyhow::anyhow!("send attempt snapshot: {error:?}"))?;
                let outcome = match (snapshot.started, snapshot.outcome) {
                    (_, Some(auth::ObservedOutcome::OperationSuccess)) => "completed",
                    (_, Some(auth::ObservedOutcome::OperationFailure)) => "failed",
                    (true, Some(auth::ObservedOutcome::Unknown) | None) => "unknown",
                    (false, Some(auth::ObservedOutcome::Unknown) | None) => "not-started",
                };
                Ok((*attempt, outcome))
            })
            .collect()
    }

    fn primary_outcome(&self, guest_completed: bool) -> &'static str {
        let kernel = self.authority.invocation_outcome();
        match (kernel, self.task_cancel_acknowledged, guest_completed) {
            (auth::InvocationOutcome::Cancelled, Some(true), false) => "cancelled",
            (auth::InvocationOutcome::Pending, None, true) => "normal",
            _ => "execution-refused",
        }
    }

    fn disposition(
        &self,
        issued_allowance_retired: bool,
        outcomes: &[(u32, &str)],
    ) -> Result<Value> {
        let mut states = [0u32; 3];
        for claim in &self.witnesses {
            let snapshot = self
                .authority
                .witness_snapshot(*claim)
                .map_err(|error| anyhow::anyhow!("send witness snapshot: {error:?}"))?;
            let index = match snapshot.state {
                auth::WitnessState::Live => 0,
                auth::WitnessState::Consumed => 1,
                auth::WitnessState::Retired => 2,
            };
            states[index] += 1;
        }
        let invocation = match self.authority.invocation_outcome() {
            auth::InvocationOutcome::Pending => "pending",
            auth::InvocationOutcome::Succeeded => "succeeded",
            auth::InvocationOutcome::Failed => "failed",
            auth::InvocationOutcome::Cancelled => "cancelled",
        };
        Ok(json!({
            "task_cancel_acknowledged": self.task_cancel_acknowledged,
            "issued_allowance_retired": issued_allowance_retired,
            "authority_invocation": invocation,
            "witnesses": {"live": states[0], "consumed": states[1], "retired": states[2]},
            "retained_attempts": self.admitted.iter().zip(outcomes).map(|((attempt, key), (_, outcome))| json!({
                "attempt": attempt_name(*attempt),
                "kernel_slot": key.slot,
                "outcome": outcome,
            })).collect::<Vec<_>>(),
        }))
    }

    fn omitted_scope(&self, diagnostic_bytes_omitted: usize) -> Value {
        let omitted = self.omitted.as_ref();
        json!({
            "complete": omitted.is_none() && diagnostic_bytes_omitted == 0,
            "observations_omitted": omitted.map_or(0, |scope| scope.count),
            "first_omitted_sequence": omitted.map(|scope| scope.first_sequence),
            "first_attempt": omitted.map(|scope| attempt_name(scope.first_attempt)),
            "last_attempt": omitted.map(|scope| attempt_name(scope.last_attempt)),
            "diagnostic_bytes_omitted": diagnostic_bytes_omitted,
        })
    }
}
