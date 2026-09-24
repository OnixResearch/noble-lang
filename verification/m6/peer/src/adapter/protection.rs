use super::checked;
use anyhow::{Context, Result, bail};
use noble_kernel::{authority as auth, types::EffId};
use serde_json::{Value, json};

const CHECKPOINT: auth::Checkpoint = auth::Checkpoint {
    sequence: 1,
    now: 10,
};
const SOURCE: auth::SourceId = auth::SourceId(1);
const OBSERVER: auth::SourceId = auth::SourceId(2);
const POLICY: auth::PolicyRevision = auth::PolicyRevision(1);
const OPERATIONS: [&str; 3] = [
    "noble-test:async-boundary/host@1.0.0#first",
    "noble-test:async-resources/counters@1.0.0#transfer",
    "noble-test:async-resources/counters@1.0.0#consume-error",
];

struct Grant {
    plan: auth::Plan,
    witness: Option<auth::Witness>,
    claim: auth::WitnessClaim,
}

pub(super) struct Protection {
    pub(super) authority: auth::Authority,
    grants: Vec<Grant>,
    owner: auth::OwnerContext,
    receipts: Vec<Value>,
    observations: Vec<(auth::AttemptKey, auth::Observation)>,
    local_total: i64,
    retired_unconsumed: usize,
    quota: u64,
    revoked: bool,
}

impl Protection {
    pub(super) fn new(_case: &str, namespace: u64) -> Result<Self> {
        let owner = auth::OwnerContext {
            instance: namespace,
            invocation: 1,
            generation: 1,
        };
        let mut authority = checked(auth::Authority::new(auth::Profile {
            owner,
            witness_kind: auth::WITNESS_KIND,
            authority_source: SOURCE,
            observation_source: OBSERVER,
            invocation_source: auth::SourceId(3),
            checkpoint: CHECKPOINT,
            limits: auth::Limits {
                witnesses: 3,
                attempts: 3,
                requests: 64,
            },
        }))?;
        let mut grants = Vec::with_capacity(3);
        for (index, operation) in OPERATIONS.iter().enumerate() {
            let argument = if index == 0 { 40 } else { 1 };
            let plan = plan(owner, operation, argument)?;
            let facts = facts(&plan, 3, false);
            let witness = match authority.authorize_from_trusted_host(plan.clone(), &facts) {
                auth::Authorization::Authorized(witness) => witness,
                auth::Authorization::Denied(rejection)
                | auth::Authorization::Preflight(rejection) => {
                    bail!(
                        "fixed local authority profile rejected: {:?}",
                        rejection.receipt().description()
                    );
                }
            };
            let claim = witness.claim();
            grants.push(Grant {
                plan,
                witness: Some(witness),
                claim,
            });
        }
        Ok(Self {
            authority,
            grants,
            owner,
            receipts: Vec::with_capacity(8),
            observations: Vec::with_capacity(3),
            local_total: 0,
            retired_unconsumed: 0,
            quota: 3,
            revoked: false,
        })
    }

    pub(super) fn admit(
        &mut self,
        argument: i64,
        resource_return: Option<bool>,
    ) -> Result<auth::Execution> {
        let index = match resource_return {
            None => 0,
            Some(true) => 1,
            Some(false) => 2,
        };
        let grant = &mut self.grants[index];
        let witness = grant.witness.take().context("ConsumedWitness")?;
        // The requested argument is untrusted. Current facts use only the fixed
        // host grant retained at initialization, never a clone of that request.
        let requested = plan(self.owner, OPERATIONS[index], argument)?;
        let current = facts(&grant.plan, self.quota, self.revoked);
        let request = auth::AdmissionRequest {
            claim: witness.claim(),
            plan: requested,
        };
        match self.authority.admit(Some(witness), &request, &current) {
            auth::Admission::Committed(execution) => Ok(execution),
            auth::Admission::Denied { witness, rejection }
            | auth::Admission::Preflight { witness, rejection } => {
                grant.witness = witness;
                let error = format!("{:?}", rejection.receipt().description().denial);
                self.receipts.push(receipt_json(rejection.receipt()));
                bail!("authority admission refused: {error}");
            }
        }
    }

    pub(super) fn observe(
        &mut self,
        started: auth::Started,
        argument: i64,
        domain_error: bool,
    ) -> Result<()> {
        let outcome = if domain_error {
            auth::ObservedOutcome::OperationFailure
        } else {
            self.local_total = self
                .local_total
                .checked_add(argument)
                .context("local operation overflow")?;
            auth::ObservedOutcome::OperationSuccess
        };
        let observation = checked(self.authority.observe_from_trusted_host(
            auth::ObservationDescription {
                attempt: started.attempt(),
                plan: started.plan().clone(),
                source: OBSERVER,
                checkpoint: CHECKPOINT,
                outcome,
            },
        ))?;
        let claim = if domain_error {
            auth::ReceiptClaim::OperationFailure
        } else {
            auth::ReceiptClaim::OperationSuccess
        };
        let receipt = checked(self.authority.receipt(
            started.attempt(),
            claim,
            Some(&observation),
        ))?;
        self.receipts.push(receipt_json(&receipt));
        self.observations.push((started.attempt(), observation));
        Ok(())
    }

    pub(super) fn cancel(&mut self) -> Result<()> {
        if self.authority.invocation_outcome() == auth::InvocationOutcome::Pending {
            checked(self.authority.cancel_invocation())?;
            self.discard_retired_witnesses();
        }
        Ok(())
    }

    pub(super) fn fail(&mut self) -> Result<()> {
        if self.authority.invocation_outcome() == auth::InvocationOutcome::Pending {
            checked(self.authority.fail_invocation())?;
            self.discard_retired_witnesses();
        }
        Ok(())
    }

    pub(super) fn finish_invocation(&mut self) -> Result<()> {
        if let Some((attempt, observation)) = self.observations.last() {
            let receipt = checked(self.authority.deliver_success(*attempt, observation))?;
            self.receipts.push(receipt_json(&receipt));
            self.discard_retired_witnesses();
            Ok(())
        } else {
            self.close_unused()
        }
    }

    fn discard_retired_witnesses(&mut self) {
        for grant in &mut self.grants {
            if grant.witness.take().is_some() {
                self.retired_unconsumed += 1;
            }
        }
    }

    pub(super) fn close_unused(&mut self) -> Result<()> {
        for grant in &mut self.grants {
            if let Some(witness) = grant.witness.take() {
                checked(self.authority.retire_witness(witness))?;
                self.retired_unconsumed += 1;
            }
        }
        Ok(())
    }

    pub(super) fn set_quota(&mut self, quota: u64) {
        self.quota = quota;
    }
    pub(super) fn set_revoked(&mut self, revoked: bool) {
        self.revoked = revoked;
    }

    pub(super) fn report(&self) -> Value {
        let counters = self.authority.counters();
        let witnesses: Vec<_> = self.grants.iter().map(|grant| {
            let state = self.authority.witness_snapshot(grant.claim).map(|snapshot| snapshot.state);
            json!({"operation":String::from_utf8_lossy(&grant.plan.description().operation.name),
                "arguments":grant.plan.description().arguments,"token_retained":grant.witness.is_some(),
                "state":state.map(|state|format!("{state:?}")).unwrap_or_else(|error|format!("{error:?}"))})
        }).collect();
        json!({"witness_available":self.grants[0].witness.is_some(),
            "witness_state":witnesses[0]["state"],"witnesses":witnesses,
            "retired_unconsumed":self.retired_unconsumed,
            "invocation":format!("{:?}",self.authority.invocation_outcome()),"local_total":self.local_total,
            "counters":{"authorization_requests":counters.authorization_requests,
                "admission_requests":counters.admission_requests,"witnesses_created":counters.witnesses_created,
                "witness_consumptions":counters.witness_consumptions,"attempts_admitted":counters.attempts_admitted,
                "protected_operations":counters.protected_operations,"approved_observations":counters.approved_observations,
                "successful_deliveries":counters.successful_deliveries}, "receipts":self.receipts})
    }
}

fn plan(owner: auth::OwnerContext, operation: &str, argument: i64) -> Result<auth::Plan> {
    checked(auth::Plan::new(auth::PlanDescription {
        operation: auth::OperationContract {
            name: operation.as_bytes().to_vec(),
            effect: EffId(8),
        },
        arguments: argument.to_le_bytes().to_vec(),
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
    }))
}

fn facts(grant: &auth::Plan, quota: u64, revoked: bool) -> auth::CurrentFacts {
    auth::CurrentFacts {
        source: SOURCE,
        checkpoint: CHECKPOINT,
        grant: Some(auth::Grant {
            plan: grant.clone(),
            kind: auth::WITNESS_KIND,
            rights: auth::EXECUTE_RIGHT,
        }),
        credentials_valid: Some(true),
        revoked: Some(revoked),
        policy: Some(POLICY),
        quota_remaining: Some(quota),
    }
}

fn receipt_json(receipt: &auth::Receipt) -> Value {
    let description = receipt.description();
    json!({"claim":format!("{:?}",description.claim),"scope":format!("{:?}",description.scope),
        "invocation":format!("{:?}",description.invocation),"attempt":description.attempt.map(|attempt|attempt.slot),
        "source":description.source.0,"operation":String::from_utf8_lossy(&description.plan.description().operation.name),
        "arguments":description.plan.description().arguments,
        "denial":description.denial.map(|denial|format!("{denial:?}")),
        "preflight":description.preflight.map(|failure|format!("{failure:?}"))})
}
