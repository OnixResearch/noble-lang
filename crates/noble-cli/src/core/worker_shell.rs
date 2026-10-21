//! Local synthetic send boundary for a checked, pure worker transition.
//! The guest returns inert Action data. Only an independently selected host
//! grant can authorize the exact action; native completion and cancellation
//! are settled through M6's retained task and resource tables, never through
//! a guest token. This local datagram path is not the Worker Send WIT ABI.

use super::output::{ErrorContext, Failure, Report};
use noble_kernel::{
    async_tasks as task, authority as auth, resources as owner,
    types::{EffId, ResourceKind},
};
use serde_json::{json, Value};
use std::os::unix::net::UnixDatagram;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::time::Duration;

const USAGE: &str = "usage: noble worker shell PACKAGE --schema SCHEMA --state STATE --submit EVENT --cancel EVENT --authority HOST_GRANT";
const OPERATION: &str = "noble-test:worker-shell/send@1.0.0#send";
const SOURCE: auth::SourceId = auth::SourceId(1);
const POLICY: auth::PolicyRevision = auth::PolicyRevision(1);
const CHECKPOINT: auth::Checkpoint = auth::Checkpoint {
    sequence: 1,
    now: 10,
};
static NAMESPACE: AtomicU64 = AtomicU64::new(1);

fn refusal(stage: &'static str, outcome: &'static str, message: impl Into<String>) -> Failure {
    Failure::new(ErrorContext { stage, outcome }, message)
}

fn internal(message: impl Into<String>) -> Failure {
    refusal("shell", "internal-failure", message)
}

fn invalid(message: impl Into<String>) -> Failure {
    refusal("shell", "invalid-input", message)
}

fn checked<T, E: std::fmt::Debug>(value: Result<T, E>) -> Result<T, Failure> {
    value.map_err(|error| internal(format!("host-owned worker transition: {error:?}")))
}

fn preserve(mut primary: Failure, cleanup: Result<(), Failure>) -> Failure {
    if let Err(error) = cleanup {
        primary.message.push_str("; retirement: ");
        primary.message.push_str(&error.message);
    }
    primary
}

fn note<T>(failure: &mut Option<Failure>, result: Result<T, Failure>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            if let Some(primary) = failure {
                primary.message.push_str("; retirement: ");
                primary.message.push_str(&error.message);
            } else {
                *failure = Some(error);
            }
            None
        }
    }
}

fn require(failure: &mut Option<Failure>, satisfied: bool, message: &'static str) {
    if !satisfied {
        let _ = note(failure, Err::<(), _>(internal(message)));
    }
}

struct Grant {
    attempt: i64,
    text: String,
    allow: bool,
}

fn decimal(value: &Value) -> Result<i64, Failure> {
    let text = value
        .as_str()
        .ok_or_else(|| invalid("attempt identity must be signed decimal I64"))?;
    let number = text
        .parse::<i64>()
        .map_err(|_| invalid("attempt identity exceeds I64"))?;
    if number.to_string() != text || number <= 0 {
        return Err(invalid("attempt identity must be a positive canonical I64"));
    }
    Ok(number)
}

fn grant(bytes: &[u8]) -> Result<Grant, Failure> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| invalid("invalid host grant"))?;
    if serde_json::to_vec(&value).map_err(|_| invalid("invalid grant encoding"))? != bytes {
        return Err(invalid(
            "host grant must be canonical JSON without duplicate keys",
        ));
    }
    let object = value
        .as_object()
        .ok_or_else(|| invalid("host grant must be an object"))?;
    if object.len() != 5
        || value["format"] != "noble-worker-shell-grant/v1"
        || value["operation"] != OPERATION
    {
        return Err(invalid("unsupported or incomplete independent host grant"));
    }
    let attempt = decimal(&value["attempt"])?;
    let text = value["text"]
        .as_str()
        .filter(|text| text.len() <= auth::MAX_ARGUMENT_BYTES - 8)
        .ok_or_else(|| invalid("host grant has no bounded exact Text"))?
        .to_owned();
    let allow = value["allow"]
        .as_bool()
        .ok_or_else(|| invalid("host grant allow must be bool"))?;
    Ok(Grant {
        attempt,
        text,
        allow,
    })
}

fn plan(owner: auth::OwnerContext, attempt: i64, text: &str) -> Result<auth::Plan, Failure> {
    if text.len() > auth::MAX_ARGUMENT_BYTES - 8 {
        return Err(invalid("action text exceeds protected operation budget"));
    }
    // Bound and unambiguously encode the complete arguments; neither a digest
    // nor an action label can substitute for the host's exact grant.
    let mut arguments = Vec::with_capacity(8 + text.len());
    arguments.extend_from_slice(&attempt.to_le_bytes());
    arguments.extend_from_slice(text.as_bytes());
    checked(auth::Plan::new(auth::PlanDescription {
        operation: auth::OperationContract {
            name: OPERATION.as_bytes().to_vec(),
            effect: EffId(8),
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
    }))
}

fn facts(plan: &auth::Plan, allow: bool) -> auth::CurrentFacts {
    auth::CurrentFacts {
        source: SOURCE,
        checkpoint: CHECKPOINT,
        grant: allow.then(|| auth::Grant {
            plan: plan.clone(),
            kind: auth::WITNESS_KIND,
            rights: auth::EXECUTE_RIGHT,
        }),
        credentials_valid: Some(allow),
        revoked: Some(false),
        policy: Some(POLICY),
        quota_remaining: Some(1),
    }
}

fn action(guest: &Value) -> Result<(i64, &str, &Value), Failure> {
    if guest["outcome"] != "normal"
        || guest["guest_requests"] != 0
        || guest["protected_operations"] != 0
    {
        return Err(invalid(
            "pure worker did not produce a checked normal transition",
        ));
    }
    let output = guest["typed_output"]
        .as_object()
        .ok_or_else(|| invalid("worker returned no typed output"))?;
    let actions = output["actions"]["items"]
        .as_array()
        .filter(|items| items.len() == 1)
        .ok_or_else(|| invalid("submit must produce exactly one inert action"))?;
    let send = &actions[0];
    if send["tag"] != "variant" || send["arm"] != "left" || send["value"]["tag"] != "pair" {
        return Err(invalid(
            "only checked Send actions can cross the shell boundary",
        ));
    }
    let attempt = decimal(&send["value"]["left"]["value"]["value"])?;
    let text = send["value"]["right"]["value"]
        .as_str()
        .ok_or_else(|| invalid("Send action has no Text argument"))?;
    let state = &output["state"];
    if state["value"]["left"]["value"] != "1"
        || state["value"]["right"]["arm"] != "right"
        || decimal(&state["value"]["right"]["value"]["value"]["value"])? != attempt
    {
        return Err(invalid(
            "worker action does not match its checked pending state",
        ));
    }
    Ok((attempt, text, state))
}

fn frame(state: &Value) -> Result<Vec<u8>, Failure> {
    if !state["schema"].is_object() {
        return Err(invalid("missing checked returned State schema"));
    }
    serde_json::to_vec(&json!({"format":"noble-worker-value/v1",
        "expected_schema":state["schema"],"value":state}))
    .map_err(|_| internal("cannot encode checked State frame"))
}

fn cancellation(event: &[u8], attempt: i64) -> Result<(), Failure> {
    let event: Value =
        serde_json::from_slice(event).map_err(|_| invalid("invalid cancellation frame"))?;
    if event["value"]["tag"] != "variant"
        || event["value"]["arm"] != "right"
        || decimal(&event["value"]["value"]["value"]["value"])? != attempt
    {
        return Err(invalid(
            "shell cancellation must target the admitted action",
        ));
    }
    Ok(())
}

#[derive(Default)]
struct ShellEvidence {
    counters: Option<auth::Counters>,
    tasks: Option<task::Observation>,
    resources: Option<owner::Observation>,
    native_bytes: Option<usize>,
}

impl ShellEvidence {
    fn observe(
        &mut self,
        tasks: &task::Table,
        resources: &owner::Table,
        authority: &auth::Authority,
    ) {
        self.counters = Some(authority.counters());
        self.tasks = Some(tasks.observation());
        self.resources = Some(resources.observation());
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-maintainers; the report constructor maps each optional observation to one fixed JSON field."
)]
fn report(
    outcome: &'static str,
    stage: &'static str,
    first: Option<&Value>,
    second: Option<&Value>,
    tasks: Option<task::Observation>,
    resource: Option<owner::Observation>,
    counters: Option<auth::Counters>,
    native_bytes: Option<usize>,
    message: Option<&str>,
) -> Report {
    let value = json!({
        "schema":"noble-worker-shell-local/v1", "profile":"Worker-Synthetic-Send-Local",
        "stage":stage,"outcome":outcome,"interface":"State Event -- State List<Action> ! {}",
        "source_sha256":first.map(|value| &value["source_sha256"]),
        "first_guest":first, "cancel_guest":second,
        "worker_host_requests":first.map(|value| &value["guest_requests"]),
        "native_send_bytes":native_bytes,
        "protected_operation_attempts":counters.map(|value| value.protected_operations),
        "successful_result_deliveries":counters.map(|value| value.successful_deliveries),
        "task_state":tasks.map(|value| if value.retired == 1 {"Retired"} else {"not-retired"}),
        "outstanding_native_pins":tasks.map(|value| value.outstanding_pins),
        "resource_state":resource.map(|value| if value.retired == 1 {"Retired"} else {"not-retired"}),
        "resource_native_pins":resource.map(|value| value.native_pins),
        "external_send_outcome":"not-attempted",
        "external_rollback_claimed":false,
        "diagnostic":message.map(|value| value.chars().take(512).collect::<String>()),
    });
    Report {
        outcome: outcome.to_owned(),
        json: value.to_string(),
    }
}

fn failure_report(error: &Failure, evidence: &ShellEvidence) -> Report {
    report(
        error.context.outcome,
        error.context.stage,
        None,
        None,
        evidence.tasks,
        evidence.resources,
        evidence.counters,
        evidence.native_bytes,
        Some(&error.message),
    )
}

struct Native {
    unblock: Option<mpsc::SyncSender<()>>,
    joined: Option<std::thread::JoinHandle<()>>,
    receiver: UnixDatagram,
    receive_buffer: Vec<u8>,
}

impl Native {
    fn start(
        started: auth::Started,
        bytes: Vec<u8>,
        sender: UnixDatagram,
        receiver: UnixDatagram,
        receive_buffer: Vec<u8>,
    ) -> Result<(Self, mpsc::Receiver<std::io::Result<usize>>), Failure> {
        if started.plan().description().arguments.as_slice() != bytes.as_slice() {
            return Err(internal(
                "native request differs from consumed exact authority",
            ));
        }
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let joined = std::thread::Builder::new()
            .name("noble-worker-native-send".into())
            .spawn(move || {
                let _permit = started;
                let sent = sender.send(&bytes);
                let _ = started_tx.send(sent);
                let _ = release_rx.recv();
            })
            .map_err(|error| internal(error.to_string()))?;
        Ok((
            Self {
                unblock: Some(release_tx),
                joined: Some(joined),
                receiver,
                receive_buffer,
            },
            started_rx,
        ))
    }

    fn finish(&mut self, attempt: i64, text: &str) -> Result<(), Failure> {
        self.stop()?;
        let size = self
            .receiver
            .recv(&mut self.receive_buffer)
            .map_err(|error| internal(error.to_string()))?;
        if size != self.receive_buffer.len()
            || &self.receive_buffer[..8] != attempt.to_le_bytes().as_slice()
            || &self.receive_buffer[8..] != text.as_bytes()
        {
            return Err(internal(
                "native send did not carry the exact admitted action",
            ));
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Failure> {
        self.release();
        if self
            .joined
            .take()
            .ok_or_else(|| internal("native owner was already joined"))?
            .join()
            .is_err()
        {
            return Err(internal("native sender panicked"));
        }
        Ok(())
    }

    fn release(&mut self) {
        if let Some(release) = self.unblock.take() {
            let _ = release.send(());
        }
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        self.release();
        if let Some(joined) = self.joined.take() {
            let _ = joined.join();
        }
    }
}

fn cleanup_retiring(
    tasks: &mut task::Table,
    callback: task::Callback,
    context: task::Context,
) -> Result<(), Failure> {
    let snapshot = checked(tasks.inspect(callback.task, context))?;
    if snapshot.state != task::State::Retiring || !snapshot.native_stopped || snapshot.pins != 0 {
        return Err(internal(
            "native task was not stopped before physical cleanup",
        ));
    }
    let obligations = task::Obligations {
        inputs: snapshot.retiring_inputs,
        results: snapshot.results,
        buffers: snapshot.buffers,
    };
    if obligations != task::Obligations::empty() {
        checked(tasks.cleanup(callback.task, obligations))?;
    }
    Ok(())
}

// A rejected cancellation returns the original move-only Task. Hold it until
// trusted host retirement succeeds; dropping it is not a cleanup operation.
fn cancel_owned(
    tasks: &mut task::Table,
    admitted: task::Admitted<()>,
    context: task::Context,
    bytes: Option<usize>,
    failure: &mut Option<Failure>,
) -> (task::Callback, Option<task::Task>) {
    let callback = admitted.callback;
    let token = match tasks.cancel(admitted.task, context) {
        Ok(cancelled) => {
            require(
                failure,
                cancelled.action == task::Action::CancellationAcknowledged
                    && cancelled.record.state == task::State::Retiring
                    && cancelled.record.pins == 1
                    && cancelled.record.retiring_inputs == 1
                    && bytes
                        .is_none_or(|bytes| tasks.observation().reserved.bytes == 3 * bytes + 256),
                "cancellation did not preserve native ownership",
            );
            None
        }
        Err(rejected) => {
            let _ = note(
                failure,
                Err::<(), _>(internal(format!(
                    "host-owned worker transition: {:?}",
                    rejected.error
                ))),
            );
            Some(rejected.input)
        }
    };
    (callback, token)
}

fn retire_owned(
    tasks: &mut task::Table,
    callback: task::Callback,
    token: &mut Option<task::Task>,
    failure: &mut Option<Failure>,
) -> bool {
    let state = tasks
        .snapshot(callback.task.slot)
        .filter(|snapshot| snapshot.handle == callback.task)
        .map(|snapshot| snapshot.state);
    if matches!(state, Some(task::State::Retiring | task::State::Retired)) {
        token.take();
        return true;
    }
    if let Some(retired) = note(
        failure,
        checked(tasks.retire(callback.task, task::Failure::Internal)),
    ) {
        let settled = retired.record.state == task::State::Retiring;
        require(
            failure,
            settled,
            "host task retirement did not retain native debt",
        );
        if settled {
            token.take();
        }
        return settled;
    }
    false
}

fn revoke_owned(
    resources: &mut owner::Table,
    scope: owner::Scope,
    reason: owner::Retirement,
    failure: &mut Option<Failure>,
) -> bool {
    if let Some(revoked) = note(failure, checked(resources.revoke(scope, reason))) {
        let settled = revoked.record.state == owner::State::Retiring(scope.serial);
        require(
            failure,
            settled && resources.observation().native_pins == 1,
            "resource cancellation did not retain the native pin",
        );
        if settled {
            return true;
        }
    }
    retire_resource_if_needed(resources, scope, failure)
}

fn retire_resource_if_needed(
    resources: &mut owner::Table,
    scope: owner::Scope,
    failure: &mut Option<Failure>,
) -> bool {
    let state = resources
        .snapshot(scope.handle.slot)
        .filter(|snapshot| snapshot.handle == scope.handle)
        .map(|snapshot| snapshot.state);
    if matches!(
        state,
        Some(owner::State::Retiring(_) | owner::State::Retired)
    ) {
        return true;
    }
    if let Some(retired) = note(
        failure,
        checked(resources.retire(scope.handle, owner::Retirement::HostCleanup)),
    ) {
        let settled = retired.record.state == owner::State::Retiring(scope.serial);
        require(
            failure,
            settled,
            "host resource retirement did not retain native debt",
        );
        return settled;
    }
    false
}

// Called only after the producer has joined (or before it was spawned). Each
// independent obligation is attempted even if a preceding settlement failed.
fn settle_native(
    tasks: &mut task::Table,
    resources: &mut owner::Table,
    callback: task::Callback,
    scope: Option<owner::Scope>,
    completion: owner::Completion,
    failure: &mut Option<Failure>,
) {
    let _ = note(failure, checked(tasks.native_stopped(callback)));
    if let Some(scope) = scope {
        let state = resources
            .snapshot(scope.handle.slot)
            .filter(|snapshot| snapshot.handle == scope.handle)
            .map(|snapshot| snapshot.state);
        if state == Some(owner::State::Retiring(scope.serial)) {
            if let Some(released) = note(failure, checked(resources.complete(scope, completion))) {
                require(
                    failure,
                    released.owner.is_none()
                        && released.decision.accounting().pins_released == 1
                        && released.decision.accounting().local_releases == 1,
                    "late native completion resurrected the protected resource owner",
                );
            }
        } else {
            require(
                failure,
                state == Some(owner::State::Retired),
                "resource scope remained busy after native stop",
            );
        }
    }
    let snapshot = tasks
        .snapshot(callback.task.slot)
        .filter(|snapshot| snapshot.handle == callback.task);
    if let Some(snapshot) = snapshot {
        if snapshot.native_stopped && snapshot.pins == 1 {
            if let Some(settled) = note(failure, checked(tasks.settle_pins(callback, 1))) {
                require(
                    failure,
                    settled.accounting.pins_released == 1,
                    "native pin release was not accounted",
                );
            }
        } else {
            require(
                failure,
                snapshot.native_stopped && snapshot.pins == 0,
                "task pin remained unsettled after native stop",
            );
        }
    } else {
        require(
            failure,
            false,
            "native task disappeared before pin settlement",
        );
    }
    if note(
        failure,
        cleanup_retiring(tasks, callback, callback.task.context),
    )
    .is_some()
    {
        if let Some(finalization) = note(failure, checked(tasks.finish(callback.task))) {
            require(
                failure,
                finalization.action == task::Action::RetirementCompleted,
                "native task reservation was not finalized",
            );
        }
    }
    let tasks = tasks.observation();
    let resources = resources.observation();
    require(
        failure,
        tasks.retired == 1
            && tasks.outstanding_pins == 0
            && tasks.reserved.bytes == 0
            && resources.retired == 1
            && resources.native_pins == 0,
        "native retirement retained a task, resource, reservation or pin",
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-maintainers; one started-send retirement names the task, authority and resource tables, the borrowed scope, the admitted task, the native handle, the cancel context, its attempt and text, and the reserved byte count explicitly."
)]
fn retire_started(
    tasks: &mut task::Table,
    authority: &mut auth::Authority,
    resources: &mut owner::Table,
    borrowed: &owner::Borrow,
    admitted: task::Admitted<()>,
    mut native: Native,
    cancel_context: task::Context,
    attempt: i64,
    text: &str,
    bytes: usize,
) -> Result<(task::Observation, owner::Observation), Failure> {
    let mut failure = None;
    let (callback, mut token) =
        cancel_owned(tasks, admitted, cancel_context, Some(bytes), &mut failure);
    let guest_cancelled = token.is_none();
    let task_retired = retire_owned(tasks, callback, &mut token, &mut failure);
    if note(&mut failure, checked(authority.cancel_invocation())).is_none() {
        let _ = note(&mut failure, checked(authority.fail_invocation()));
    }
    let scope = borrowed.scope();
    let reason = if guest_cancelled {
        owner::Retirement::Cancelled
    } else {
        owner::Retirement::HostCleanup
    };
    let resource_retired = revoke_owned(resources, scope, reason, &mut failure);
    require(
        &mut failure,
        resources.native_access(borrowed).is_err(),
        "resource access remained live after cancellation",
    );

    // finish joins the producer before inspecting its datagram. An error from
    // join (including a panic) still means the native thread has stopped.
    // Drop the socket and receive buffer before refunding their task reservation.
    let native_result = native.finish(attempt, text);
    drop(native);
    let native_ok = native_result.is_ok();
    let _ = note(&mut failure, native_result);
    if !task_retired {
        let _ = retire_owned(tasks, callback, &mut token, &mut failure);
    }
    if !resource_retired {
        let _ = retire_resource_if_needed(resources, scope, &mut failure);
    }
    if native_ok {
        let state = tasks
            .snapshot(callback.task.slot)
            .filter(|snapshot| snapshot.handle == callback.task)
            .map(|snapshot| snapshot.state);
        if state == Some(task::State::Retiring) {
            if let Some(late) = note(
                &mut failure,
                checked(tasks.complete(
                    callback,
                    task::Outcome::Success,
                    task::Completion {
                        inputs: task::Disposition {
                            returned: 0,
                            consumed: 0,
                            retired: 1,
                        },
                        produced: 0,
                        bytes: 0,
                    },
                )),
            ) {
                require(
                    &mut failure,
                    late.action == task::Action::CompletionRetired(task::Outcome::Success),
                    "late native result was delivered after cancellation",
                );
            }
        } else {
            require(
                &mut failure,
                false,
                "native result could not be retired without delivering to the guest",
            );
        }
    }
    let completion = if native_ok {
        owner::Completion::Success
    } else {
        owner::Completion::DomainError
    };
    settle_native(
        tasks,
        resources,
        callback,
        Some(scope),
        completion,
        &mut failure,
    );
    failure.map_or_else(|| Ok((tasks.observation(), resources.observation())), Err)
}

fn release_unbegun(
    resources: &mut owner::Table,
    capability: owner::Owner,
    required: owner::Requirement,
) -> Result<(), Failure> {
    match resources.release(capability, required) {
        Ok(released) if released.record.state == owner::State::Retired => Ok(()),
        Ok(_) => Err(internal("unbegun resource remained live after release")),
        Err(rejected) => {
            let primary = internal(format!("unbegun resource release: {:?}", rejected.error));
            let retired =
                checked(resources.retire(rejected.input.handle(), owner::Retirement::HostCleanup))
                    .map(|_| ());
            Err(preserve(primary, retired))
        }
    }
}

fn retire_unstarted(
    tasks: &mut task::Table,
    authority: &mut auth::Authority,
    resources: &mut owner::Table,
    scope: Option<owner::Scope>,
    admitted: task::Admitted<()>,
) -> Result<(), Failure> {
    let mut failure = None;
    let context = admitted.callback.task.context;
    let (callback, mut token) = cancel_owned(tasks, admitted, context, None, &mut failure);
    let task_retired = retire_owned(tasks, callback, &mut token, &mut failure);
    let _ = note(&mut failure, checked(authority.fail_invocation()));
    if let Some(scope) = scope {
        let _ = revoke_owned(
            resources,
            scope,
            owner::Retirement::HostCleanup,
            &mut failure,
        );
    }
    if !task_retired {
        let _ = retire_owned(tasks, callback, &mut token, &mut failure);
    }
    settle_native(
        tasks,
        resources,
        callback,
        scope,
        owner::Completion::DomainError,
        &mut failure,
    );
    failure.map_or(Ok(()), Err)
}

#[cfg(test)]
pub(super) fn execute(
    package: &[u8],
    schema: &[u8],
    initial: &[u8],
    submit: &[u8],
    cancel: &[u8],
    grant_bytes: &[u8],
) -> Result<Report, Failure> {
    execute_recording(
        package,
        schema,
        initial,
        submit,
        cancel,
        grant_bytes,
        &mut ShellEvidence::default(),
    )
}

fn execute_recording(
    package: &[u8],
    schema: &[u8],
    initial: &[u8],
    submit: &[u8],
    cancel: &[u8],
    grant_bytes: &[u8],
    evidence: &mut ShellEvidence,
) -> Result<Report, Failure> {
    let grant = grant(grant_bytes)?;
    let initial_frame: Value =
        serde_json::from_slice(initial).map_err(|_| invalid("invalid initial State frame"))?;
    if initial_frame["value"]["value"]["left"]["value"] != "0"
        || initial_frame["value"]["value"]["right"]["arm"] != "left"
    {
        return Err(invalid(
            "worker shell starts with count zero and no pending attempt",
        ));
    }
    let first = super::worker_typed::execute(package, schema, initial, submit)?;
    let first: Value = serde_json::from_str(&first.json)
        .map_err(|_| internal("malformed checked guest report"))?;
    let (attempt, text, state) = action(&first)?;
    cancellation(cancel, attempt)?;
    let next_state = frame(state)?;
    super::worker_typed::validate_secondary(package, schema, &next_state, cancel)?;

    let namespace = NAMESPACE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| internal("worker shell namespace exhausted"))?;
    let context = task::Context(namespace);
    let owner = auth::OwnerContext {
        instance: namespace,
        invocation: 1,
        generation: 1,
    };
    let mut authority = checked(auth::Authority::new(auth::Profile {
        owner,
        witness_kind: auth::WITNESS_KIND,
        authority_source: SOURCE,
        observation_source: auth::SourceId(2),
        invocation_source: auth::SourceId(3),
        checkpoint: CHECKPOINT,
        limits: auth::Limits {
            witnesses: 1,
            attempts: 1,
            requests: 2,
        },
    }))?;
    evidence.counters = Some(authority.counters());
    let granted = plan(owner, grant.attempt, &grant.text)?;
    let proposed = plan(owner, attempt, text)?;
    let bytes = 8 + text.len();
    let mut tasks = checked(task::Table::new(
        task::TableId(namespace),
        task::Limits {
            tasks: 1,
            terminal_results: 1,
            bytes: 3 * auth::MAX_ARGUMENT_BYTES + 256,
            parked_payloads: 1,
            pins: 1,
            wakeups: 0,
            retirement_work: 5,
            generations: 1,
        },
    ))?;
    let request = task::Request {
        context,
        native: task::NativeId(namespace),
        inputs: 1,
        results: 0,
        input_bytes: bytes,
        result_bytes: bytes,
        parked_bytes: bytes + 256,
        pins: 1,
        wakeups: 0,
    };
    let prepared = checked(tasks.prepare(request))?;
    // Reserve the concrete buffers that remain reachable from the native
    // producer and task callback before consuming the host's grant.
    let mut wire = Vec::with_capacity(bytes);
    wire.extend_from_slice(&attempt.to_le_bytes());
    wire.extend_from_slice(text.as_bytes());
    let receive_buffer = vec![0; bytes];
    let (sender, receiver) = UnixDatagram::pair().map_err(|error| internal(error.to_string()))?;
    sender
        .set_nonblocking(true)
        .map_err(|error| internal(error.to_string()))?;
    receiver
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| internal(error.to_string()))?;
    let required = owner::Requirement {
        context,
        kind: ResourceKind(1),
        rights: owner::Rights(1),
    };
    let mut resources = checked(owner::Table::new(
        owner::TableId(namespace),
        owner::Limits {
            slots: 1,
            pins: 1,
            owners_per_context: 1,
            generations: 1,
            scopes: 1,
        },
    ))?;
    evidence.resources = Some(resources.observation());
    let capability = checked(resources.register(required))?;
    let facts = facts(&granted, grant.allow);
    let witness = match authority.authorize_from_trusted_host(proposed.clone(), &facts) {
        auth::Authorization::Authorized(witness) => witness,
        auth::Authorization::Denied(_) | auth::Authorization::Preflight(_) => {
            if let Err(error) = release_unbegun(&mut resources, capability, required) {
                evidence.counters = Some(authority.counters());
                evidence.resources = Some(resources.observation());
                return Err(error);
            }
            return Ok(report(
                "denied",
                "authorization",
                Some(&first),
                None,
                None,
                Some(resources.observation()),
                Some(authority.counters()),
                None,
                Some("host grant denied exact worker action"),
            ));
        }
    };
    let admission = auth::AdmissionRequest {
        claim: witness.claim(),
        plan: proposed,
    };
    let execution = match authority.admit(Some(witness), &admission, &facts) {
        auth::Admission::Committed(execution) => execution,
        auth::Admission::Denied { witness, .. } | auth::Admission::Preflight { witness, .. } => {
            let mut failure = release_unbegun(&mut resources, capability, required).err();
            if let Some(witness) = witness {
                let _ = note(&mut failure, checked(authority.retire_witness(witness)));
            }
            if let Some(failure) = failure {
                evidence.counters = Some(authority.counters());
                evidence.resources = Some(resources.observation());
                return Err(failure);
            }
            return Ok(report(
                "denied",
                "authorization",
                Some(&first),
                None,
                None,
                Some(resources.observation()),
                Some(authority.counters()),
                None,
                Some("exact action admission refused"),
            ));
        }
    };
    let admitted = prepared.commit(());
    let result = (|| -> Result<Report, Failure> {
        let borrowed = match resources.begin(capability, required) {
            Ok(borrowed) => borrowed,
            Err(rejected) => {
                let primary = internal(format!("host send resource refused: {:?}", rejected.error));
                let released = release_unbegun(&mut resources, rejected.input, required);
                drop((wire, receive_buffer, sender, receiver));
                let retired =
                    retire_unstarted(&mut tasks, &mut authority, &mut resources, None, admitted);
                return Err(preserve(preserve(primary, released), retired));
            }
        };
        let scope = borrowed.borrow.scope();
        if let Err(error) = resources.native_access(&borrowed.borrow) {
            drop((wire, receive_buffer, sender, receiver));
            let retired = retire_unstarted(
                &mut tasks,
                &mut authority,
                &mut resources,
                Some(scope),
                admitted,
            );
            return Err(preserve(
                internal(format!("host send resource lost native access: {error:?}")),
                retired,
            ));
        }
        let started = match authority.start_execution(execution) {
            Ok(started) => started,
            Err(error) => {
                drop((wire, receive_buffer, sender, receiver));
                let retired = retire_unstarted(
                    &mut tasks,
                    &mut authority,
                    &mut resources,
                    Some(scope),
                    admitted,
                );
                return Err(preserve(
                    internal(format!(
                        "host authority refused committed invocation: {error:?}"
                    )),
                    retired,
                ));
            }
        };
        // This is an in-process datagram, not an external send. It exercises a
        // real native operation and a live retained pin without claiming network
        // delivery, a portable capability, or a WORKER-01 receipt.
        let (mut native, started_rx) =
            match Native::start(started, wire, sender, receiver, receive_buffer) {
                Ok(value) => value,
                Err(error) => {
                    let retired = retire_unstarted(
                        &mut tasks,
                        &mut authority,
                        &mut resources,
                        Some(scope),
                        admitted,
                    );
                    return Err(preserve(error, retired));
                }
            };
        let sent = started_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| internal("native operation did not start before its deadline"))
            .and_then(|sent| sent.map_err(|error| internal(error.to_string())));
        if let Ok(size) = &sent {
            evidence.native_bytes = Some(*size);
        }
        if !matches!(&sent, Ok(size) if *size == bytes) {
            let stopped = native.stop();
            drop(native);
            let retired = retire_unstarted(
                &mut tasks,
                &mut authority,
                &mut resources,
                Some(scope),
                admitted,
            );
            let primary = sent
                .err()
                .unwrap_or_else(|| internal("native operation sent incomplete action"));
            return Err(preserve(preserve(primary, stopped), retired));
        }

        let cancel_guest = super::worker_typed::execute(package, schema, &next_state, cancel);
        let retirement = retire_started(
            &mut tasks,
            &mut authority,
            &mut resources,
            &borrowed.borrow,
            admitted,
            native,
            context,
            attempt,
            text,
            bytes,
        );
        let (second, (observation, resource_observation)) = match (cancel_guest, retirement) {
            (Ok(second), Ok(retirement)) => (second, retirement),
            (Err(guest), Err(retirement)) => return Err(preserve(guest, Err(retirement))),
            (Err(guest), Ok(_)) => return Err(guest),
            (Ok(_), Err(retirement)) => return Err(retirement),
        };
        let second: Value = serde_json::from_str(&second.json)
            .map_err(|_| internal("malformed cancellation guest report"))?;
        if second["outcome"] != "normal"
            || second["typed_output"]["state"]["value"]["left"]["value"] != "1"
            || second["typed_output"]["state"]["value"]["right"]["arm"] != "left"
            || second["typed_output"]["actions"]["items"]
                .as_array()
                .is_none_or(|rows| !rows.is_empty())
            || second["source_sha256"] != first["source_sha256"]
        {
            return Err(invalid(
                "cancellation guest did not clear exact pending attempt",
            ));
        }
        Ok(report(
            "normal",
            "retirement",
            Some(&first),
            Some(&second),
            Some(observation),
            Some(resource_observation),
            Some(authority.counters()),
            Some(bytes),
            None,
        ))
    })();
    evidence.observe(&tasks, &resources, &authority);
    result
}

pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let mut evidence = ShellEvidence::default();
    let result = if arguments.len() != 13
        || arguments[1] != "shell"
        || arguments[3] != "--schema"
        || arguments[5] != "--state"
        || arguments[7] != "--submit"
        || arguments[9] != "--cancel"
        || arguments[11] != "--authority"
    {
        Err(invalid(USAGE))
    } else {
        let files = [
            (2, 262_144),
            (4, 65_536),
            (6, 65_536),
            (8, 65_536),
            (10, 65_536),
            (12, 4096),
        ]
        .map(|(index, limit)| {
            super::worker_typed::read_bounded_regular_file(
                std::path::Path::new(&arguments[index]),
                limit,
            )
        });
        match files {
            [Ok(package), Ok(schema), Ok(state), Ok(submit), Ok(cancel), Ok(grant)] => {
                execute_recording(
                    &package,
                    &schema,
                    &state,
                    &submit,
                    &cancel,
                    &grant,
                    &mut evidence,
                )
            }
            files => Err(files
                .into_iter()
                .find_map(Result::err)
                .expect("at least one bounded input failed")),
        }
    };
    let report = match result {
        Ok(report) => report,
        Err(error) => failure_report(&error, &evidence),
    };
    if let Err(error) = super::output::print(&report) {
        eprintln!("noble: {}", error.message);
        return std::process::ExitCode::from(error.exit());
    }
    std::process::ExitCode::from(report.exit())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PendingSend {
        authority: auth::Authority,
        tasks: task::Table,
        resources: owner::Table,
        admitted: task::Admitted<()>,
        borrowed: owner::Admitted,
        execution: auth::Execution,
        context: task::Context,
    }

    fn pending_send() -> Result<PendingSend, String> {
        let namespace = NAMESPACE.fetch_add(1, Ordering::Relaxed);
        let context = task::Context(namespace);
        let owner = auth::OwnerContext {
            instance: namespace,
            invocation: 1,
            generation: 1,
        };
        let mut authority = auth::Authority::new(auth::Profile {
            owner,
            witness_kind: auth::WITNESS_KIND,
            authority_source: SOURCE,
            observation_source: auth::SourceId(2),
            invocation_source: auth::SourceId(3),
            checkpoint: CHECKPOINT,
            limits: auth::Limits {
                witnesses: 1,
                attempts: 1,
                requests: 2,
            },
        })
        .map_err(|error| format!("{error:?}"))?;
        let proposed = plan(owner, 1, "ping").map_err(|error| error.message)?;
        let facts = facts(&proposed, true);
        let witness = match authority.authorize_from_trusted_host(proposed.clone(), &facts) {
            auth::Authorization::Authorized(witness) => witness,
            _ => return Err("real exact worker authority refused".into()),
        };
        let admission = auth::AdmissionRequest {
            claim: witness.claim(),
            plan: proposed,
        };
        let execution = match authority.admit(Some(witness), &admission, &facts) {
            auth::Admission::Committed(execution) => execution,
            _ => return Err("real worker admission refused".into()),
        };
        let mut tasks = task::Table::new(
            task::TableId(namespace),
            task::Limits {
                tasks: 1,
                terminal_results: 1,
                bytes: 3 * auth::MAX_ARGUMENT_BYTES + 256,
                parked_payloads: 1,
                pins: 1,
                wakeups: 0,
                retirement_work: 5,
                generations: 1,
            },
        )
        .map_err(|error| format!("{error:?}"))?;
        let admitted = tasks
            .prepare(task::Request {
                context,
                native: task::NativeId(namespace),
                inputs: 1,
                results: 0,
                input_bytes: 12,
                result_bytes: 12,
                parked_bytes: 268,
                pins: 1,
                wakeups: 0,
            })
            .map_err(|error| format!("{error:?}"))?
            .commit(());
        let mut resources = owner::Table::new(
            owner::TableId(namespace),
            owner::Limits {
                slots: 1,
                pins: 1,
                owners_per_context: 1,
                generations: 1,
                scopes: 1,
            },
        )
        .map_err(|error| format!("{error:?}"))?;
        let required = owner::Requirement {
            context,
            kind: ResourceKind(1),
            rights: owner::Rights(1),
        };
        let capability = resources
            .register(required)
            .map_err(|error| format!("{error:?}"))?;
        let borrowed = resources
            .begin(capability, required)
            .map_err(|rejected| format!("{:?}", rejected.error))?;
        Ok(PendingSend {
            authority,
            tasks,
            resources,
            admitted,
            borrowed,
            execution,
            context,
        })
    }

    fn retired(
        tasks: &task::Table,
        resources: &owner::Table,
        callback: task::Callback,
        failure: task::Failure,
    ) -> Result<(), String> {
        let snapshot = tasks
            .snapshot(callback.task.slot)
            .ok_or("retired task record disappeared")?;
        if snapshot.handle != callback.task
            || snapshot.state != task::State::Retired
            || snapshot.failure != Some(failure)
            || !snapshot.native_stopped
            || !snapshot.completion_closed
            || snapshot.pins != 0
            || !snapshot.finalized
        {
            return Err(format!(
                "native task was not terminal and settled: {snapshot:?}"
            ));
        }
        let task_counts = tasks.observation();
        let resource_counts = resources.observation();
        if task_counts.retired != 1
            || task_counts.outstanding_pins != 0
            || task_counts.reserved.bytes != 0
            || resource_counts.retired != 1
            || resource_counts.native_pins != 0
            || resource_counts.busy != 0
            || resource_counts.retiring != 0
        {
            return Err(format!(
                "retirement leaked task or resource: \
                {task_counts:?}, {resource_counts:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn rejected_move_only_cancellation_still_joins_and_settles_native_send() -> Result<(), String> {
        let mut send = pending_send()?;
        let callback = send.admitted.callback;
        let started = send
            .authority
            .start_execution(send.execution)
            .map_err(|error| format!("{error:?}"))?;
        let mut wire = Vec::with_capacity(12);
        wire.extend_from_slice(&1_i64.to_le_bytes());
        wire.extend_from_slice(b"ping");
        let (sender, receiver) = UnixDatagram::pair().map_err(|error| error.to_string())?;
        sender
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        receiver
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|error| error.to_string())?;
        let (native, sent) = Native::start(started, wire, sender, receiver, vec![0; 12])
            .map_err(|error| error.message)?;
        let size = sent
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        assert_eq!(size, 12);
        let wrong_context = task::Context(send.context.0 + 1);
        let rejected = retire_started(
            &mut send.tasks,
            &mut send.authority,
            &mut send.resources,
            &send.borrowed.borrow,
            send.admitted,
            native,
            wrong_context,
            1,
            "ping",
            12,
        )
        .err()
        .ok_or("wrong-context Task cancellation was accepted")?;
        assert_eq!(rejected.context.outcome, "internal-failure");
        assert_eq!(send.authority.counters().successful_deliveries, 0);
        assert_eq!(
            send.resources
                .snapshot(send.borrowed.borrow.scope().handle.slot)
                .ok_or("resource record disappeared")?
                .retirement,
            Some(owner::Retirement::HostCleanup)
        );
        let mut evidence = ShellEvidence {
            native_bytes: Some(size),
            ..ShellEvidence::default()
        };
        evidence.observe(&send.tasks, &send.resources, &send.authority);
        let reported: Value = serde_json::from_str(&failure_report(&rejected, &evidence).json)
            .map_err(|error| error.to_string())?;
        assert_eq!(reported["outcome"], "internal-failure");
        assert_eq!(reported["protected_operation_attempts"], 1);
        assert_eq!(reported["native_send_bytes"], 12);
        assert_eq!(reported["successful_result_deliveries"], 0);
        assert_eq!(reported["task_state"], "Retired");
        assert_eq!(reported["resource_state"], "Retired");
        assert_eq!(reported["outstanding_native_pins"], 0);
        assert_eq!(reported["resource_native_pins"], 0);
        retired(
            &send.tasks,
            &send.resources,
            callback,
            task::Failure::Internal,
        )
    }

    #[test]
    fn malformed_host_grant_refuses_before_authority_without_fake_attempts() -> Result<(), String> {
        let mut evidence = ShellEvidence::default();
        let rejected = execute_recording(&[], &[], &[], &[], &[], b"{", &mut evidence)
            .err()
            .ok_or("malformed host grant crossed authority boundary")?;
        assert_eq!(rejected.context.outcome, "invalid-input");
        let report = failure_report(&rejected, &evidence);
        let reported: Value =
            serde_json::from_str(&report.json).map_err(|error| error.to_string())?;
        assert_eq!(reported["outcome"], "invalid-input");
        assert_eq!(reported["protected_operation_attempts"], Value::Null);
        assert_eq!(reported["successful_result_deliveries"], Value::Null);
        assert_eq!(reported["native_send_bytes"], Value::Null);
        Ok(())
    }

    #[test]
    fn failed_unstarted_authority_settles_task_and_resource_independently() -> Result<(), String> {
        let mut send = pending_send()?;
        let callback = send.admitted.callback;
        drop(send.execution);
        send.authority
            .cancel_invocation()
            .map_err(|error| format!("{error:?}"))?;
        let rejected = retire_unstarted(
            &mut send.tasks,
            &mut send.authority,
            &mut send.resources,
            Some(send.borrowed.borrow.scope()),
            send.admitted,
        )
        .err()
        .ok_or("already-cancelled invocation did not reject failure settlement")?;
        assert_eq!(rejected.context.outcome, "internal-failure");
        retired(
            &send.tasks,
            &send.resources,
            callback,
            task::Failure::Cancelled,
        )
    }
}
