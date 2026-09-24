//! Executable acceptance controls over the same retained host/core as the ABI.
//! Results contain observed decisions and storage, never expectation labels.
mod schema;

use crate::adapter::{CONTEXT, Callback, Host, checked, default_limits};
use anyhow::{Result, bail, ensure};
use noble_kernel::{async_tasks as task, resources as owner};
use serde_json::{Value, json};

struct Control {
    case: String,
    snapshots: Vec<Value>,
    results: Vec<Value>,
}

impl Control {
    fn new(case: &str) -> Self {
        Self {
            case: case.to_owned(),
            snapshots: Vec::new(),
            results: Vec::new(),
        }
    }

    fn capture(&mut self, label: &str, host: &Host) {
        self.snapshots
            .push(json!({"label":label,"host":host.report()}));
    }

    fn rejected<T>(&mut self, operation: &str, result: Result<T>, expected: &str) -> Result<()> {
        match result {
            Ok(_) => bail!("{operation}: unexpectedly accepted"),
            Err(error) => {
                let error = format!("{error:#}");
                ensure!(
                    error.contains(expected),
                    "{operation}: wrong refusal {error}"
                );
                self.results
                    .push(json!({"operation":operation,"status":"rejected","error":error}));
                Ok(())
            }
        }
    }

    fn accepted(&mut self, operation: &str, result: Value) {
        self.results
            .push(json!({"operation":operation,"status":"accepted","value":result}));
    }

    fn finish(self, host: &Host) -> Value {
        json!({"schema":"noble-m6-peer/v1","case":self.case,
            "observation":{"outcome":"normal","results":[]},
            "control":{"case":self.case,"snapshots":self.snapshots,"results":self.results},
            "host":host.report()})
    }
}

pub fn run(case: &str) -> Result<Value> {
    if case.starts_with("lifecycle-") {
        return schema::run(case);
    }
    let mut control = Control::new(case);
    match case {
        "delivery-before-cancel" => delivery_before_cancel(&mut control),
        "cancel-before-delivery" => cancel_ready(&mut control, false),
        "ready-resource-cancel" | "complete-cancel-before-delivery" | "result-owner-cancel" => {
            cancel_ready(&mut control, true)
        }
        "cancel-pending-late-success" | "cancel-before-completion" | "late-operation-receipt" => {
            cancel_pending(&mut control)
        }
        "resource-delivery" => resource_delivery(&mut control, false, true),
        "domain-error-returns-owner" => resource_delivery(&mut control, true, true),
        "domain-error-resource-cleanup" => resource_delivery(&mut control, true, false),
        "wrong-native-id" | "invalid-handle" | "foreign-context" | "wrong-context-callback" => {
            invalid_callback(&mut control)
        }
        "foreign-owner" => foreign_owner(&mut control),
        "stale-generation" | "stale-generation-completion" => stale_generation(&mut control),
        "duplicate-callback" | "duplicate-completion-after-retirement" => {
            duplicate_callback(&mut control)
        }
        "repeated-cancel" | "repeated-cancellation-during-retirement" => {
            repeated_cancel(&mut control)
        }
        "pending-quota"
        | "terminal-quota"
        | "byte-quota"
        | "pin-quota"
        | "parked-quota"
        | "wakeup-quota"
        | "retirement-quota"
        | "task-quota-preflight-failure"
        | "buffer-quota-preflight-failure" => quota(&mut control),
        "generation-exhaustion" => generation_exhaustion(&mut control),
        "wakeup-bound" => wakeup_bound(&mut control),
        "premature-pin-settlement" => premature_settlement(&mut control),
        "invalid-input-disposition" => invalid_disposition(&mut control),
        "authority-preflight" | "authority-owner-preflight" => authority_preflight(&mut control),
        "authority-changed-plan" | "authority-revoked" | "authority-quota" => {
            authority_denial(&mut control)
        }
        "oversized-buffered-result" => oversized_result(&mut control),
        "trap-with-native-pin" => trap(&mut control),
        "delivery-before-ready" => premature_delivery(&mut control),
        _ => bail!("unknown lifecycle control {case}"),
    }
}

fn scalar(host: &mut Host, protected: bool) -> Result<Callback> {
    host.admit("first", 8, 8, protected.then_some(40))
}

fn successful(host: &mut Host, callback: Callback) -> Result<()> {
    host.complete(callback, false, 8)?;
    host.settle(callback)?;
    host.deliver(callback)
}

fn delivery_before_cancel(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let rep = host.create_resource(41)?;
    let callback = host.admit_resource(rep, 1, true)?;
    host.complete(callback, false, 8)?;
    control.capture("ready", &host);
    let returned = host.deliver_resource(callback)?;
    ensure!(
        host.resource_value(returned)? == 42,
        "delivered wrong native payload"
    );
    control.capture("delivered", &host);
    host.cancel(callback)?;
    ensure!(
        host.state(callback)? == task::State::Delivered,
        "late cancel revoked delivery"
    );
    ensure!(
        host.resource_value(returned)? == 42,
        "late cancel revoked resource"
    );
    control.capture("late-cancel", &host);
    host.settle(callback)?;
    host.finish_invocation()?;
    host.drop_resource(returned)?;
    ensure!(
        host.report()["resources"]["native_releases"] == 1,
        "resource not released once"
    );
    control.accepted("late-cancel", host.snapshot(callback)?);
    Ok(control_value(control, &host))
}

fn cancel_ready(control: &mut Control, resource: bool) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let callback = if resource {
        host.admit_result_owner(42)?
    } else {
        scalar(&mut host, false)?
    };
    host.complete(callback, false, 8)?;
    control.capture("ready", &host);
    host.cancel(callback)?;
    ensure!(
        host.state(callback)? == task::State::Retiring,
        "ready cancel did not retire"
    );
    ensure!(
        host.report()["tasks"]["outstanding_pins"] == 1,
        "cancel settled native pin"
    );
    if resource {
        ensure!(
            host.report()["resources"]["native_releases"] == 0,
            "cancel destroyed pinned result"
        );
    }
    control.capture("cancelled", &host);
    let delivery = if resource {
        host.deliver_resource(callback).map(|_| ())
    } else {
        host.deliver(callback)
    };
    control.rejected("deliver-cancelled", delivery, "Retiring")?;
    host.settle(callback)?;
    ensure!(
        host.state(callback)? == task::State::Retired,
        "cancel result not retired"
    );
    if resource {
        ensure!(
            host.report()["resources"]["native_releases"] == 1,
            "undelivered resource not released once"
        );
        ensure!(
            host.report()["accounting"]["cleaned_results"] == 1,
            "result owner cleanup missing"
        );
    }
    control.capture("retired", &host);
    Ok(control_value(control, &host))
}

fn cancel_pending(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let callback = scalar(&mut host, true)?;
    control.capture("admitted", &host);
    host.cancel(callback)?;
    let report = host.report();
    ensure!(
        report["tasks"]["outstanding_pins"] == 1,
        "cancellation is not native completion"
    );
    ensure!(
        report["authority"]["counters"]["protected_operations"] == 1,
        "protected work did not commit"
    );
    ensure!(
        report["authority"]["local_total"] == 0,
        "operation ran before native callback"
    );
    control.capture("cancelled", &host);
    host.complete(callback, false, 8)?;
    let report = host.report();
    ensure!(
        report["authority"]["local_total"] == 40,
        "late native operation did not run"
    );
    ensure!(
        report["authority"]["receipts"][0]["claim"] == "OperationSuccess",
        "missing authentic operation receipt"
    );
    ensure!(
        report["authority"]["receipts"][0]["invocation"] == "Cancelled",
        "late receipt changed cancellation"
    );
    ensure!(
        report["authority"]["counters"]["witness_consumptions"] == 1,
        "witness was restored"
    );
    control.capture("late-complete", &host);
    control.rejected("deliver-cancelled", host.deliver(callback), "Retiring")?;
    host.settle(callback)?;
    host.complete(callback, false, 8)?;
    control.rejected(
        "replay-consumed-witness",
        scalar(&mut host, true),
        "ConsumedWitness",
    )?;
    ensure!(
        host.report()["authority"]["local_total"] == 40,
        "duplicate callback retried operation"
    );
    ensure!(
        host.report()["accounting"]["pins_released"] == 1,
        "native pin not settled once"
    );
    control.capture("retired", &host);
    Ok(control_value(control, &host))
}

fn resource_delivery(
    control: &mut Control,
    domain_error: bool,
    return_owner: bool,
) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let rep = host.create_resource(41)?;
    let before = host.resource_snapshot(rep)?;
    let callback = host.admit_resource(rep, 1, return_owner)?;
    control.rejected("sender-reuse", host.resource_value(rep), "unknown resource")?;
    control.capture("admitted", &host);
    host.complete(callback, domain_error, 8)?;
    ensure!(
        host.terminal_outcome(callback)?
            == if domain_error {
                task::Outcome::DomainError
            } else {
                task::Outcome::Success
            },
        "wrong native terminal classification"
    );
    control.capture("ready", &host);
    host.settle(callback)?;
    control.capture("native-settled-ready", &host);
    if return_owner {
        let rep = host.deliver_resource(callback)?;
        let value = host.resource_value(rep)?;
        ensure!(
            value == if domain_error { 41 } else { 42 },
            "resource outcome changed payload"
        );
        let after = host.resource_snapshot(rep)?;
        ensure!(
            before["generation"] != after["generation"],
            "resource transfer reused generation"
        );
        control.accepted("owner-return", json!({"before":before,"after":after}));
        control.capture("delivered", &host);
        host.drop_resource(rep)?;
    } else {
        host.deliver(callback)?;
        ensure!(
            host.report()["resources"]["guest_owners"] == 0,
            "non-returning domain error restored owner"
        );
    }
    host.finish_invocation()?;
    ensure!(
        host.report()["resources"]["native_releases"] == 1,
        "resource cleanup not exactly once"
    );
    ensure!(
        host.report()["tasks"]["reserved"]["tasks"] == 0,
        "task reservation leaked"
    );
    Ok(control_value(control, &host))
}

fn invalid_callback(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let callback = scalar(&mut host, false)?;
    let mut hostile = callback;
    let expected = match control.case.as_str() {
        "wrong-native-id" => {
            hostile.native.0 += 1;
            "WrongNative"
        }
        "invalid-handle" => {
            hostile.task.slot = task::MAX_SLOTS;
            "InvalidHandle"
        }
        _ => {
            hostile.task.context = owner::Context(999);
            "WrongContext"
        }
    };
    let before = host.snapshot(callback)?;
    control.rejected(
        "hostile-completion",
        checked(
            host.tasks
                .complete(hostile, task::Outcome::Success, scalar_completion(8)),
        ),
        expected,
    )?;
    ensure!(
        host.snapshot(callback)? == before,
        "rejected callback changed retained record"
    );
    control.capture("rejected", &host);
    successful(&mut host, callback)?;
    host.finish_invocation()?;
    Ok(control_value(control, &host))
}

fn foreign_owner(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let mut foreign = Host::new("foreign-resource-context")?;
    let rep = host.create_resource(41)?;
    let foreign_rep = foreign.create_resource(90)?;
    let handle = host.resource_handle(rep)?;
    let foreign_handle = foreign.resource_handle(foreign_rep)?;
    control.rejected(
        "foreign-owner",
        host.validate_resource(foreign_handle, CONTEXT),
        "InvalidHandle",
    )?;
    control.rejected(
        "foreign-owner-context",
        host.validate_resource(handle, owner::Context(999)),
        "WrongContext",
    )?;
    let mut stale = handle;
    stale.generation += 1;
    control.rejected(
        "stale-owner-generation",
        host.validate_resource(stale, CONTEXT),
        "WrongGeneration",
    )?;
    let mut kind = handle;
    kind.kind = noble_kernel::types::ResourceKind(999);
    control.rejected(
        "wrong-owner-kind",
        host.validate_resource(kind, CONTEXT),
        "WrongKind",
    )?;
    ensure!(
        host.resource_value(rep)? == 41,
        "foreign claim changed native value"
    );
    ensure!(
        host.report()["authority"]["counters"]["protected_operations"] == 0,
        "foreign claim started protected work"
    );
    control.capture("rejected", &host);
    host.drop_resource(rep)?;
    foreign.drop_resource(foreign_rep)?;
    host.finish_invocation()?;
    foreign.finish_invocation()?;
    control.accepted("foreign-host-cleanup", foreign.report());
    Ok(control_value(control, &host))
}

fn stale_generation(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let old = scalar(&mut host, false)?;
    successful(&mut host, old)?;
    let current = scalar(&mut host, false)?;
    ensure!(
        current.task.slot == old.task.slot && current.task.generation != old.task.generation,
        "task slot did not advance generation"
    );
    let before = host.snapshot(current)?;
    control.rejected(
        "old-generation-completion",
        checked(
            host.tasks
                .complete(old, task::Outcome::Success, scalar_completion(8)),
        ),
        "WrongGeneration",
    )?;
    ensure!(
        host.snapshot(current)? == before,
        "old callback changed current generation"
    );
    control.capture("rejected", &host);
    successful(&mut host, current)?;
    host.finish_invocation()?;
    Ok(control_value(control, &host))
}

fn duplicate_callback(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let callback = host.admit_result_owner(42)?;
    host.cancel(callback)?;
    host.complete(callback, false, 8)?;
    host.settle(callback)?;
    let before = host.report();
    host.complete(callback, false, 8)?;
    host.complete(callback, true, 8)?;
    host.settle(callback)?;
    let after = host.report();
    ensure!(
        before["resources"] == after["resources"],
        "duplicate completed resource twice"
    );
    ensure!(
        after["accounting"]["completions"] == 1 && after["accounting"]["pins_released"] == 1,
        "duplicate callback changed terminal accounting"
    );
    control.capture("duplicate-rejected-as-noop", &host);
    Ok(control_value(control, &host))
}

fn repeated_cancel(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let callback = scalar(&mut host, false)?;
    host.cancel(callback)?;
    let before = host.snapshot(callback)?;
    host.cancel(callback)?;
    host.cancel(callback)?;
    ensure!(
        before == host.snapshot(callback)?,
        "repeated cancel restarted cleanup"
    );
    ensure!(
        host.report()["accounting"]["cancellations"] == 1,
        "cancel acknowledged more than once"
    );
    control.capture("repeated-cancel", &host);
    host.complete(callback, false, 8)?;
    host.settle(callback)?;
    Ok(control_value(control, &host))
}

fn quota(control: &mut Control) -> Result<Value> {
    let mut limits = default_limits();
    let expected = match control.case.as_str() {
        "pending-quota" | "task-quota-preflight-failure" => {
            limits.tasks = 1;
            "TaskCapacity"
        }
        "terminal-quota" => {
            limits.terminal_results = 1;
            "TerminalCapacity"
        }
        "byte-quota" | "buffer-quota-preflight-failure" => {
            limits.bytes = 24;
            "ByteCapacity"
        }
        "pin-quota" => {
            limits.pins = 1;
            "PinCapacity"
        }
        "parked-quota" => {
            limits.parked_payloads = 1;
            "ParkedCapacity"
        }
        "wakeup-quota" => {
            limits.wakeups = 4;
            "WakeCapacity"
        }
        "retirement-quota" => {
            limits.retirement_work = 3;
            "RetirementCapacity"
        }
        _ => unreachable!(),
    };
    let mut host = Host::with_limits(&control.case, limits)?;
    let mut request = scalar_request(1);
    if control.case == "parked-quota" {
        request.parked_bytes = 1;
    }
    let callback = host.admit_request("quota-filler", request, None)?;
    let before = host.report();
    request.native = task::NativeId(2);
    control.rejected(
        "quota-admission",
        host.admit_request("first", request, Some(40)),
        expected,
    )?;
    let after = host.report();
    ensure!(
        before["tasks"] == after["tasks"],
        "quota refusal changed reservation"
    );
    ensure!(
        after["authority"]["witness_available"] == true,
        "quota refusal consumed witness"
    );
    ensure!(
        after["authority"]["counters"]["protected_operations"] == 0,
        "quota refusal started work"
    );
    control.capture("rejected", &host);
    successful(&mut host, callback)?;
    host.finish_invocation()?;
    Ok(control_value(control, &host))
}

fn generation_exhaustion(control: &mut Control) -> Result<Value> {
    let mut limits = default_limits();
    limits.generations = 1;
    let mut host = Host::with_limits(&control.case, limits)?;
    let callback = scalar(&mut host, false)?;
    successful(&mut host, callback)?;
    control.rejected(
        "generation-exhaustion",
        scalar(&mut host, true),
        "GenerationExhausted",
    )?;
    ensure!(
        host.report()["authority"]["witness_available"] == true,
        "identity exhaustion spent witness"
    );
    control.capture("rejected", &host);
    host.finish_invocation()?;
    Ok(control_value(control, &host))
}

fn wakeup_bound(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let mut request = scalar_request(1);
    request.wakeups = 2;
    let callback = host.admit_request("wakeup-bounded", request, None)?;
    let decision = checked(host.tasks.wake(callback))?;
    host.record("wake", decision);
    let decision = checked(host.tasks.wake(callback))?;
    ensure!(
        decision.action == task::Action::WakeCoalesced,
        "wake storm did not coalesce"
    );
    host.record("wake-coalesced", decision);
    control.capture("coalesced", &host);
    let decision = checked(host.tasks.take_wake(callback.task))?;
    host.record("take-wake", decision);
    let decision = checked(host.tasks.wake(callback))?;
    host.record("wake", decision);
    let decision = checked(host.tasks.take_wake(callback.task))?;
    host.record("take-wake", decision);
    let decision = checked(host.tasks.wake(callback))?;
    ensure!(
        decision.record.failure == Some(task::Failure::Budget),
        "unbounded wakeup accepted"
    );
    host.record("wake-exhausted", decision);
    control.capture("exhausted", &host);
    host.fail_all(task::Failure::Budget)?;
    host.complete(callback, false, 8)?;
    host.settle(callback)?;
    ensure!(
        host.report()["accounting"]["wake_queued"] == 2,
        "wake bound failed"
    );
    Ok(control_value(control, &host))
}

fn premature_settlement(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let callback = scalar(&mut host, false)?;
    control.rejected(
        "shell-settle-running",
        host.settle(callback),
        "NativeStillRunning",
    )?;
    control.rejected(
        "pin-settle-running",
        checked(host.tasks.settle_pins(callback, 1)),
        "NativeStillRunning",
    )?;
    host.complete(callback, false, 8)?;
    control.rejected(
        "completion-is-not-stop",
        checked(host.tasks.settle_pins(callback, 1)),
        "NativeStillRunning",
    )?;
    control.capture("completed-pinned", &host);
    host.settle(callback)?;
    control.rejected(
        "duplicate-pin-settlement",
        checked(host.tasks.settle_pins(callback, 1)),
        "InvalidPins",
    )?;
    control.rejected(
        "result-cleanup-before-delivery",
        checked(host.tasks.cleanup(
            callback.task,
            task::Obligations {
                inputs: 0,
                results: 0,
                buffers: 2,
            },
        )),
        "InvalidCleanup",
    )?;
    control.rejected(
        "finish-ready",
        checked(host.tasks.finish(callback.task)),
        "Ready",
    )?;
    host.deliver(callback)?;
    host.finish_invocation()?;
    Ok(control_value(control, &host))
}

fn invalid_disposition(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let rep = host.create_resource(41)?;
    let callback = host.admit_resource(rep, 1, true)?;
    let before = host.snapshot(callback)?;
    for (name, returned, consumed, retired, produced, error) in [
        ("overlapping-disposition", 1, 1, 0, 0, "InvalidDisposition"),
        ("missing-disposition", 0, 0, 0, 0, "InvalidDisposition"),
        ("invented-input", 2, 0, 0, 0, "InvalidDisposition"),
        ("unreserved-result-owner", 1, 0, 0, 1, "ResultCapacity"),
    ] {
        let completion = task::Completion {
            inputs: task::Disposition {
                returned,
                consumed,
                retired,
            },
            produced,
            bytes: 8,
        };
        control.rejected(
            name,
            checked(
                host.tasks
                    .complete(callback, task::Outcome::Success, completion),
            ),
            error,
        )?;
        ensure!(
            before == host.snapshot(callback)?,
            "invalid disposition changed owner custody"
        );
    }
    control.capture("rejected", &host);
    host.complete(callback, false, 8)?;
    host.settle(callback)?;
    let returned = host.deliver_resource(callback)?;
    ensure!(
        host.resource_value(returned)? == 42,
        "rejection destroyed native owner"
    );
    host.drop_resource(returned)?;
    host.finish_invocation()?;
    Ok(control_value(control, &host))
}

fn authority_preflight(control: &mut Control) -> Result<Value> {
    let mut limits = default_limits();
    limits.tasks = 1;
    let mut host = Host::with_limits(&control.case, limits)?;
    let filler = scalar(&mut host, false)?;
    let resource = if control.case == "authority-owner-preflight" {
        Some(host.create_resource(41)?)
    } else {
        None
    };
    let owner_before = resource
        .map(|rep| host.resource_snapshot(rep))
        .transpose()?;
    let result = if let Some(rep) = resource {
        host.admit_resource(rep, 1, true)
    } else {
        scalar(&mut host, true)
    };
    control.rejected("reservation-before-authority", result, "TaskCapacity")?;
    ensure!(
        host.report()["authority"]["counters"]["witness_consumptions"] == 0,
        "preflight spent witness"
    );
    if let Some(rep) = resource {
        ensure!(
            Some(host.resource_snapshot(rep)?) == owner_before,
            "preflight moved caller owner"
        );
    }
    control.capture("preflight-preserved", &host);
    successful(&mut host, filler)?;
    let callback = if let Some(rep) = resource {
        host.admit_resource(rep, 1, true)?
    } else {
        scalar(&mut host, true)?
    };
    host.complete(callback, false, 8)?;
    host.settle(callback)?;
    if resource.is_some() {
        let returned = host.deliver_resource(callback)?;
        host.drop_resource(returned)?;
    } else {
        host.deliver(callback)?;
    }
    host.finish_invocation()?;
    ensure!(
        host.report()["authority"]["counters"]["witness_consumptions"] == 1,
        "retry did not consume exactly original witness"
    );
    ensure!(
        host.report()["authority"]["counters"]["protected_operations"] == 1,
        "retry duplicated protected work"
    );
    control.capture("admitted-after-capacity-returned", &host);
    Ok(control_value(control, &host))
}

fn authority_denial(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let (argument, error) = match control.case.as_str() {
        "authority-changed-plan" => (41, "ChangedPlan"),
        "authority-revoked" => {
            host.set_authority_revoked(true);
            (40, "Revoked")
        }
        "authority-quota" => {
            host.set_authority_quota(0);
            (40, "QuotaExhausted")
        }
        _ => unreachable!(),
    };
    control.rejected(
        "authority-denial",
        host.admit("first", 8, 8, Some(argument)),
        error,
    )?;
    let report = host.report();
    ensure!(
        report["authority"]["counters"]["protected_operations"] == 0,
        "denial started protected work"
    );
    ensure!(
        report["authority"]["counters"]["witness_consumptions"] == 0,
        "denial consumed witness"
    );
    ensure!(
        report["tasks"]["reserved"]["tasks"] == 0,
        "denial committed task"
    );
    control.capture("denied", &host);
    host.finish_invocation()?;
    Ok(control_value(control, &host))
}

fn oversized_result(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let callback = host.admit_result_owner(42)?;
    host.complete(callback, false, 9)?;
    ensure!(
        host.authenticated(callback)?.failure == Some(task::Failure::Budget),
        "oversized payload did not exhaust budget"
    );
    ensure!(
        host.report()["accounting"]["rejected_result_bytes"] == 9,
        "oversized payload was retained"
    );
    control.capture("oversized", &host);
    host.settle(callback)?;
    ensure!(
        host.report()["resources"]["native_releases"] == 1,
        "oversized result lost resource cleanup"
    );
    ensure!(
        host.report()["storage_capacity_bytes"] == 0,
        "retired result retained payload backing"
    );
    host.fail_all(task::Failure::Budget)?;
    Ok(control_value(control, &host))
}

fn trap(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let callback = scalar(&mut host, true)?;
    host.fail_all(task::Failure::Trap)?;
    ensure!(
        host.report()["tasks"]["outstanding_pins"] == 1,
        "trap released live native pin"
    );
    control.capture("trapped", &host);
    host.complete(callback, false, 8)?;
    host.settle(callback)?;
    ensure!(
        host.authenticated(callback)?.failure == Some(task::Failure::Trap),
        "late completion replaced primary trap"
    );
    ensure!(
        host.report()["authority"]["receipts"][0]["invocation"] == "Failed",
        "operation receipt forged invocation success"
    );
    Ok(control_value(control, &host))
}

fn premature_delivery(control: &mut Control) -> Result<Value> {
    let mut host = Host::new(&control.case)?;
    let callback = scalar(&mut host, false)?;
    let before = host.snapshot(callback)?;
    control.rejected("deliver-pending", host.deliver(callback), "Pending")?;
    ensure!(
        before == host.snapshot(callback)?,
        "premature delivery consumed task"
    );
    control.capture("pending", &host);
    successful(&mut host, callback)?;
    host.finish_invocation()?;
    Ok(control_value(control, &host))
}

fn scalar_request(native: u64) -> task::Request {
    task::Request {
        context: CONTEXT,
        native: task::NativeId(native),
        inputs: 0,
        results: 0,
        input_bytes: 8,
        result_bytes: 8,
        parked_bytes: 0,
        pins: 1,
        wakeups: 4,
    }
}

fn scalar_completion(bytes: usize) -> task::Completion {
    task::Completion {
        inputs: task::Disposition {
            returned: 0,
            consumed: 0,
            retired: 0,
        },
        produced: 0,
        bytes,
    }
}

fn control_value(control: &mut Control, host: &Host) -> Value {
    let owned = std::mem::replace(control, Control::new("finished"));
    owned.finish(host)
}
