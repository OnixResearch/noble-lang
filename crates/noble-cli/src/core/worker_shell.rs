//! Local synthetic send boundary for a checked, pure worker transition.
//! The guest returns inert Action data. Only an independently selected host
//! grant can authorize the exact action; native completion and cancellation
//! are settled through M6's retained task and resource tables, never through
//! a guest token. This local datagram path is not the Worker Send WIT ABI.

use super::output::{ErrorContext, Failure, Report};
use noble_kernel::{async_tasks as task, authority as auth, resources as owner,
    types::{EffId, ResourceKind}};
use serde_json::{Value, json};
use std::os::unix::net::UnixDatagram;
use std::sync::{atomic::{AtomicU64, Ordering}, mpsc};
use std::time::Duration;

const USAGE: &str = "usage: noble worker shell PACKAGE --schema SCHEMA --state STATE --submit EVENT --cancel EVENT --authority HOST_GRANT";
const OPERATION: &str = "noble-test:worker-shell/send@1.0.0#send";
const SOURCE: auth::SourceId = auth::SourceId(1);
const POLICY: auth::PolicyRevision = auth::PolicyRevision(1);
const CHECKPOINT: auth::Checkpoint = auth::Checkpoint { sequence: 1, now: 10 };
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

struct Grant {
    attempt: i64,
    text: String,
    allow: bool,
}

fn decimal(value: &Value) -> Result<i64, Failure> {
    let text = value.as_str().ok_or_else(|| invalid("attempt identity must be signed decimal I64"))?;
    let number = text.parse::<i64>().map_err(|_| invalid("attempt identity exceeds I64"))?;
    if number.to_string() != text || number <= 0 {
        return Err(invalid("attempt identity must be a positive canonical I64"));
    }
    Ok(number)
}

fn grant(bytes: &[u8]) -> Result<Grant, Failure> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| invalid("invalid host grant"))?;
    if serde_json::to_vec(&value).map_err(|_| invalid("invalid grant encoding"))? != bytes {
        return Err(invalid("host grant must be canonical JSON without duplicate keys"));
    }
    let object = value.as_object().ok_or_else(|| invalid("host grant must be an object"))?;
    if object.len() != 5 || value["format"] != "noble-worker-shell-grant/v1"
        || value["operation"] != OPERATION {
        return Err(invalid("unsupported or incomplete independent host grant"));
    }
    let attempt = decimal(&value["attempt"])?;
    let text = value["text"].as_str()
        .filter(|text| text.len() <= auth::MAX_ARGUMENT_BYTES - 8)
        .ok_or_else(|| invalid("host grant has no bounded exact Text"))?.to_owned();
    let allow = value["allow"].as_bool().ok_or_else(|| invalid("host grant allow must be bool"))?;
    Ok(Grant { attempt, text, allow })
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
        operation: auth::OperationContract { name: OPERATION.as_bytes().to_vec(), effect: EffId(8) },
        arguments, actor: auth::ActorId(1), owner, policy: POLICY,
        constraints: auth::Constraints { not_before: 1, expires_at: 20,
            quota: auth::QuotaId(1), units: 1, scope: 1 },
    }))
}

fn facts(plan: &auth::Plan, allow: bool) -> auth::CurrentFacts {
    auth::CurrentFacts {
        source: SOURCE, checkpoint: CHECKPOINT,
        grant: allow.then(|| auth::Grant { plan: plan.clone(), kind: auth::WITNESS_KIND,
            rights: auth::EXECUTE_RIGHT }),
        credentials_valid: Some(allow), revoked: Some(false), policy: Some(POLICY),
        quota_remaining: Some(1),
    }
}

fn action(guest: &Value) -> Result<(i64, &str, &Value), Failure> {
    if guest["outcome"] != "normal" || guest["guest_requests"] != 0
        || guest["protected_operations"] != 0 {
        return Err(invalid("pure worker did not produce a checked normal transition"));
    }
    let output = guest["typed_output"].as_object().ok_or_else(|| invalid("worker returned no typed output"))?;
    let actions = output["actions"]["items"].as_array().filter(|items| items.len() == 1)
        .ok_or_else(|| invalid("submit must produce exactly one inert action"))?;
    let send = &actions[0];
    if send["tag"] != "variant" || send["arm"] != "left" || send["value"]["tag"] != "pair" {
        return Err(invalid("only checked Send actions can cross the shell boundary"));
    }
    let attempt = decimal(&send["value"]["left"]["value"]["value"])?;
    let text = send["value"]["right"]["value"].as_str()
        .ok_or_else(|| invalid("Send action has no Text argument"))?;
    let state = &output["state"];
    if state["value"]["left"]["value"] != "1"
        || state["value"]["right"]["arm"] != "right"
        || decimal(&state["value"]["right"]["value"]["value"]["value"])? != attempt {
        return Err(invalid("worker action does not match its checked pending state"));
    }
    Ok((attempt, text, state))
}

fn frame(state: &Value) -> Result<Vec<u8>, Failure> {
    if !state["schema"].is_object() { return Err(invalid("missing checked returned State schema")); }
    serde_json::to_vec(&json!({"format":"noble-worker-value/v1",
        "expected_schema":state["schema"],"value":state}))
        .map_err(|_| internal("cannot encode checked State frame"))
}

fn cancellation(event: &[u8], attempt: i64) -> Result<(), Failure> {
    let event: Value = serde_json::from_slice(event).map_err(|_| invalid("invalid cancellation frame"))?;
    if event["value"]["tag"] != "variant" || event["value"]["arm"] != "right"
        || decimal(&event["value"]["value"]["value"]["value"])? != attempt {
        return Err(invalid("shell cancellation must target the admitted action"));
    }
    Ok(())
}

fn report(outcome: &'static str, stage: &'static str, first: Option<&Value>,
          second: Option<&Value>, tasks: Option<task::Observation>,
          resource: Option<owner::Observation>, counters: Option<auth::Counters>,
          native_bytes: Option<usize>,
          message: Option<&str>) -> Report {
    let value = json!({
        "schema":"noble-worker-shell-local/v1", "profile":"Worker-Synthetic-Send-Local",
        "stage":stage,"outcome":outcome,"interface":"State Event -- State List<Action> ! {}",
        "source_sha256":first.map(|value| &value["source_sha256"]),
        "first_guest":first, "cancel_guest":second,
        "worker_host_requests":first.map(|value| &value["guest_requests"]),
        "native_send_bytes":native_bytes,
        "protected_operation_attempts":counters.map(|value| value.protected_operations).unwrap_or(0),
        "successful_result_deliveries":counters.map(|value| value.successful_deliveries).unwrap_or(0),
        "task_state":tasks.map(|value| if value.retired == 1 {"Retired"} else {"not-retired"}),
        "outstanding_native_pins":tasks.map(|value| value.outstanding_pins),
        "resource_state":resource.map(|value| if value.retired == 1 {"Retired"} else {"not-retired"}),
        "resource_native_pins":resource.map(|value| value.native_pins),
        "external_send_outcome":"not-attempted",
        "external_rollback_claimed":false,
        "diagnostic":message.map(|value| value.chars().take(512).collect::<String>()),
    });
    Report { outcome:outcome.to_owned(), json:value.to_string() }
}

struct Native {
    unblock: Option<mpsc::SyncSender<()>>,
    joined: Option<std::thread::JoinHandle<()>>,
    receiver: UnixDatagram,
    receive_buffer: Vec<u8>,
}

impl Native {
    fn start(started: auth::Started, bytes: Vec<u8>,
             sender: UnixDatagram, receiver: UnixDatagram, receive_buffer: Vec<u8>)
        -> Result<(Self, mpsc::Receiver<std::io::Result<usize>>), Failure> {
        if started.plan().description().arguments.as_slice() != bytes.as_slice() {
            return Err(internal("native request differs from consumed exact authority"));
        }
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let joined = std::thread::Builder::new().name("noble-worker-native-send".into())
            .spawn(move || {
                let _permit = started;
                let sent = sender.send(&bytes);
                let _ = started_tx.send(sent);
                let _ = release_rx.recv();
            }).map_err(|error| internal(error.to_string()))?;
        Ok((Self { unblock:Some(release_tx), joined:Some(joined),
            receiver, receive_buffer }, started_rx))
    }

    fn finish(&mut self, attempt: i64, text: &str) -> Result<(), Failure> {
        self.stop()?;
        let size = self.receiver.recv(&mut self.receive_buffer)
            .map_err(|error| internal(error.to_string()))?;
        if size != self.receive_buffer.len()
            || &self.receive_buffer[..8] != attempt.to_le_bytes().as_slice()
            || &self.receive_buffer[8..] != text.as_bytes() {
            return Err(internal("native send did not carry the exact admitted action"));
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Failure> {
        self.release();
        if self.joined.take().ok_or_else(|| internal("native owner was already joined"))?
            .join().is_err() { return Err(internal("native sender panicked")); }
        Ok(())
    }

    fn release(&mut self) {
        if let Some(release) = self.unblock.take() { let _ = release.send(()); }
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        self.release();
        if let Some(joined) = self.joined.take() { let _ = joined.join(); }
    }
}

fn cleanup_retiring(tasks: &mut task::Table, callback: task::Callback,
                    context: task::Context) -> Result<(), Failure> {
    let snapshot = checked(tasks.inspect(callback.task, context))?;
    if snapshot.state != task::State::Retiring || !snapshot.native_stopped || snapshot.pins != 0 {
        return Err(internal("native task was not stopped before physical cleanup"));
    }
    let obligations = task::Obligations {
        inputs:snapshot.retiring_inputs, results:snapshot.results, buffers:snapshot.buffers,
    };
    if obligations != task::Obligations::empty() {
        checked(tasks.cleanup(callback.task, obligations))?;
    }
    Ok(())
}

fn retire_unstarted(tasks: &mut task::Table, authority: &mut auth::Authority,
                    resources: &mut owner::Table, scope: Option<owner::Scope>,
                    admitted: task::Admitted<()>) -> Result<(), Failure> {
    let callback = admitted.callback;
    checked(tasks.cancel(admitted.task, callback.task.context))?;
    let authority_result = authority.fail_invocation();
    if let Some(scope) = scope {
        checked(resources.revoke(scope, owner::Retirement::HostCleanup))?;
        let released = checked(resources.complete(scope, owner::Completion::DomainError))?;
        if released.owner.is_some() { return Err(internal("failed native send returned a retired owner")); }
    }
    checked(tasks.native_stopped(callback))?;
    checked(tasks.settle_pins(callback, 1))?;
    cleanup_retiring(tasks, callback, callback.task.context)?;
    checked(tasks.finish(callback.task))?;
    checked(authority_result)?;
    Ok(())
}

pub(super) fn execute(package: &[u8], schema: &[u8], initial: &[u8],
                      submit: &[u8], cancel: &[u8], grant_bytes: &[u8]) -> Result<Report, Failure> {
    let grant = grant(grant_bytes)?;
    let initial_frame: Value = serde_json::from_slice(initial)
        .map_err(|_| invalid("invalid initial State frame"))?;
    if initial_frame["value"]["value"]["left"]["value"] != "0"
        || initial_frame["value"]["value"]["right"]["arm"] != "left" {
        return Err(invalid("worker shell starts with count zero and no pending attempt"));
    }
    let first = super::worker_typed::execute(package, schema, initial, submit)?;
    let first: Value = serde_json::from_str(&first.json)
        .map_err(|_| internal("malformed checked guest report"))?;
    let (attempt, text, state) = action(&first)?;
    cancellation(cancel, attempt)?;
    let next_state = frame(state)?;
    super::worker_typed::validate_secondary(package, schema, &next_state, cancel)?;

    let namespace = NAMESPACE.fetch_update(Ordering::Relaxed, Ordering::Relaxed,
        |current| current.checked_add(1)).map_err(|_| internal("worker shell namespace exhausted"))?;
    let context = task::Context(namespace);
    let owner = auth::OwnerContext { instance:namespace, invocation:1, generation:1 };
    let mut authority = checked(auth::Authority::new(auth::Profile { owner,
        witness_kind:auth::WITNESS_KIND, authority_source:SOURCE,
        observation_source:auth::SourceId(2), invocation_source:auth::SourceId(3),
        checkpoint:CHECKPOINT, limits:auth::Limits { witnesses:1, attempts:1, requests:2 },
    }))?;
    let granted = plan(owner, grant.attempt, &grant.text)?;
    let proposed = plan(owner, attempt, text)?;
    let bytes = 8 + text.len();
    let mut tasks = checked(task::Table::new(task::TableId(namespace), task::Limits {
        tasks:1, terminal_results:1, bytes:3 * auth::MAX_ARGUMENT_BYTES + 256,
        parked_payloads:1,
        pins:1, wakeups:0, retirement_work:5, generations:1,
    }))?;
    let request = task::Request { context, native:task::NativeId(namespace),
        inputs:1, results:0, input_bytes:bytes, result_bytes:bytes,
        parked_bytes:bytes + 256,
        pins:1, wakeups:0 };
    let prepared = checked(tasks.prepare(request))?;
    // Reserve the concrete buffers that remain reachable from the native
    // producer and task callback before consuming the host's grant.
    let mut wire = Vec::with_capacity(bytes);
    wire.extend_from_slice(&attempt.to_le_bytes());
    wire.extend_from_slice(text.as_bytes());
    let receive_buffer = vec![0; bytes];
    let (sender, receiver) = UnixDatagram::pair().map_err(|error| internal(error.to_string()))?;
    sender.set_nonblocking(true).map_err(|error| internal(error.to_string()))?;
    receiver.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| internal(error.to_string()))?;
    let required = owner::Requirement {
        context, kind:ResourceKind(1), rights:owner::Rights(1),
    };
    let mut resources = checked(owner::Table::new(owner::TableId(namespace),
        owner::Limits { slots:1, pins:1, owners_per_context:1, generations:1, scopes:1 }))?;
    let capability = checked(resources.register(required))?;
    let facts = facts(&granted, grant.allow);
    let witness = match authority.authorize_from_trusted_host(proposed.clone(), &facts) {
        auth::Authorization::Authorized(witness) => witness,
        auth::Authorization::Denied(_) | auth::Authorization::Preflight(_) => {
            checked(resources.release(capability, required))?;
            return Ok(report("denied", "authorization", Some(&first), None,
                None, Some(resources.observation()), Some(authority.counters()), None,
                Some("host grant denied exact worker action")));
        }
    };
    let admission = auth::AdmissionRequest { claim:witness.claim(), plan:proposed };
    let execution = match authority.admit(Some(witness), &admission, &facts) {
        auth::Admission::Committed(execution) => execution,
        auth::Admission::Denied { witness, .. }
        | auth::Admission::Preflight { witness, .. } => {
            checked(resources.release(capability, required))?;
            if let Some(witness) = witness {
                checked(authority.retire_witness(witness))?;
            }
            return Ok(report("denied", "authorization", Some(&first), None,
                None, Some(resources.observation()), Some(authority.counters()), None,
                Some("exact action admission refused")));
        }
    };
    let admitted = prepared.commit(());
    let callback = admitted.callback;
    let borrowed = match resources.begin(capability, required) {
        Ok(borrowed) => borrowed,
        Err(rejected) => {
            checked(resources.release(rejected.input, required))?;
            drop((wire, receive_buffer, sender, receiver));
            retire_unstarted(&mut tasks, &mut authority, &mut resources, None, admitted)?;
            return Err(internal(format!("host send resource refused: {:?}", rejected.error)));
        }
    };
    let scope = borrowed.borrow.scope();
    if let Err(error) = resources.native_access(&borrowed.borrow) {
        drop((wire, receive_buffer, sender, receiver));
        retire_unstarted(&mut tasks, &mut authority, &mut resources, Some(scope), admitted)?;
        return Err(internal(format!("host send resource lost native access: {error:?}")));
    }
    let started = match authority.start_execution(execution) {
        Ok(started) => started,
        Err(error) => {
            drop((wire, receive_buffer, sender, receiver));
            retire_unstarted(&mut tasks, &mut authority, &mut resources, Some(scope), admitted)?;
            return Err(internal(format!("host authority refused committed invocation: {error:?}")));
        }
    };
    // This is an in-process datagram, not an external send. It exercises a
    // real native operation and a live retained pin without claiming network
    // delivery, a portable capability, or a WORKER-01 receipt.
    let (mut native, started_rx) = match Native::start(started, wire, sender, receiver, receive_buffer) {
        Ok(value) => value,
        Err(error) => {
            retire_unstarted(&mut tasks, &mut authority, &mut resources, Some(scope), admitted)?;
            return Err(error);
        }
    };
    let sent = started_rx.recv_timeout(Duration::from_secs(5))
        .map_err(|_| internal("native operation did not start before its deadline"))
        .and_then(|sent| sent.map_err(|error| internal(error.to_string())));
    if !matches!(&sent, Ok(size) if *size == bytes) {
        let stopped = native.stop();
        drop(native);
        let retired = retire_unstarted(&mut tasks, &mut authority, &mut resources, Some(scope), admitted);
        stopped?;
        retired?;
        return Err(sent.err().unwrap_or_else(|| internal("native operation sent incomplete action")));
    }

    let cancel_guest = super::worker_typed::execute(package, schema, &next_state, cancel);
    let cancelled = checked(tasks.cancel(admitted.task, context))?;
    if cancelled.action != task::Action::CancellationAcknowledged
        || cancelled.record.state != task::State::Retiring || cancelled.record.pins != 1
        || cancelled.record.retiring_inputs != 1
        || tasks.observation().reserved.bytes != 3 * bytes + 256 {
        return Err(internal("cancellation did not preserve native ownership"));
    }
    checked(authority.cancel_invocation())?;
    let revoked = checked(resources.revoke(scope, owner::Retirement::Cancelled))?;
    if revoked.record.state != owner::State::Retiring(scope.serial)
        || resources.observation().native_pins != 1
        || resources.native_access(&borrowed.borrow).is_ok() {
        return Err(internal("resource access or native pin violated cancellation acknowledgement"));
    }
    let native_result = native.finish(attempt, text);
    drop(native);
    if native_result.is_ok() {
        let late = checked(tasks.complete(callback, task::Outcome::Success, task::Completion {
            inputs:task::Disposition { returned:0, consumed:0, retired:1 },
            produced:0, bytes:0,
        }))?;
        if late.action != task::Action::CompletionRetired(task::Outcome::Success) {
            return Err(internal("late native result was delivered after cancellation"));
        }
    }
    checked(tasks.native_stopped(callback))?;
    let completion = if native_result.is_ok() {
        owner::Completion::Success
    } else {
        owner::Completion::DomainError
    };
    let released = checked(resources.complete(scope, completion))?;
    if released.owner.is_some() || released.decision.accounting().pins_released != 1
        || released.decision.accounting().local_releases != 1 {
        return Err(internal("late native completion resurrected the protected resource owner"));
    }
    let settled = checked(tasks.settle_pins(callback, 1))?;
    if settled.accounting.pins_released != 1 {
        return Err(internal("native pin release was not accounted"));
    }
    cleanup_retiring(&mut tasks, callback, context)?;
    let finalization = checked(tasks.finish(callback.task))?;
    let observation = tasks.observation();
    let resource_observation = resources.observation();
    if finalization.action != task::Action::RetirementCompleted
        || observation.outstanding_pins != 0 || observation.retired != 1
        || observation.reserved.bytes != 0
        || resource_observation.native_pins != 0 || resource_observation.retired != 1 {
        return Err(internal("late result retained a task owner or pin"));
    }
    native_result?;
    let second = cancel_guest?;
    let second: Value = serde_json::from_str(&second.json)
        .map_err(|_| internal("malformed cancellation guest report"))?;
    if second["outcome"] != "normal" || second["typed_output"]["state"]["value"]["left"]["value"] != "1"
        || second["typed_output"]["state"]["value"]["right"]["arm"] != "left"
        || second["typed_output"]["actions"]["items"].as_array().is_none_or(|rows| !rows.is_empty())
        || second["source_sha256"] != first["source_sha256"] {
        return Err(invalid("cancellation guest did not clear exact pending attempt"));
    }
    Ok(report("normal", "retirement", Some(&first), Some(&second),
        Some(observation), Some(resource_observation),
        Some(authority.counters()), Some(bytes), None))
}

pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let result = if arguments.len() != 13 || arguments[1] != "shell"
        || arguments[3] != "--schema" || arguments[5] != "--state"
        || arguments[7] != "--submit" || arguments[9] != "--cancel"
        || arguments[11] != "--authority" {
        Err(invalid(USAGE))
    } else {
        let files = [(2,262_144),(4,65_536),(6,65_536),(8,65_536),(10,65_536),(12,4096)]
            .map(|(index,limit)| super::worker_typed::read_bounded_regular_file(
                std::path::Path::new(&arguments[index]), limit));
        match files {
            [Ok(package),Ok(schema),Ok(state),Ok(submit),Ok(cancel),Ok(grant)] =>
                execute(&package,&schema,&state,&submit,&cancel,&grant),
            files => Err(files.into_iter().find_map(Result::err)
                .expect("at least one bounded input failed")),
        }
    };
    let report = match result {
        Ok(report) => report,
        Err(error) => report(error.context.outcome, error.context.stage,
            None,None,None,None,None,None,Some(&error.message)),
    };
    if let Err(error) = super::output::print(&report) {
        eprintln!("noble: {}",error.message);
        return std::process::ExitCode::from(error.exit());
    }
    std::process::ExitCode::from(report.exit())
}
