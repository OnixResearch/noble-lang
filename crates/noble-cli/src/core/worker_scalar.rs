//! Narrow source-checked, effect-free scalar worker precursor. The consumer
//! owns the interface and runtime inputs; this is not portable package ingress.

use super::output::{ErrorContext, Failure, Report};
use noble_kernel::types::Ty;
use serde_json::{json, Value};

const USAGE: &str = "usage: noble worker scalar SOURCE --state I64 --delta I64";
const INTERFACE: &str = "I64 I64 -- I64 ! {}";

fn failure(outcome: &'static str, message: impl Into<String>) -> Failure {
    Failure::new(
        ErrorContext {
            stage: "admission",
            outcome,
        },
        message,
    )
}

fn scalar(text: &std::ffi::OsStr) -> Result<i64, Failure> {
    let text = attempt!(text.to_str().ok_or_else(|| failure("invalid-input", USAGE)));
    let value = attempt!(text
        .parse::<i64>()
        .map_err(|_| failure("invalid-input", USAGE)));
    if value.to_string() != text {
        return Err(failure(
            "invalid-input",
            "scalar input must be canonical signed decimal I64",
        ));
    }
    Ok(value)
}

#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-maintainers; the report constructor maps each optional observation to one fixed JSON field."
)]
fn report(
    outcome: &str,
    stage: &str,
    source: Option<&[u8]>,
    state: Option<i64>,
    delta: Option<i64>,
    module: Option<&Value>,
    result: Option<i64>,
    guest_requests: u64,
    protected_operations: u64,
    diagnostic: Option<&str>,
) -> Report {
    let json = json!({
        "schema": "noble-worker-scalar/v1", "profile": "Worker-Scalar-Precursor",
        "interface": INTERFACE, "accounting_revision": "core-managed-linear-memory/step-limit-4096",
        "step_ceiling": 4096, "stage": stage, "outcome": outcome,
        "invocation": if stage == "execution" { Some("scalar-1") } else { None },
        "ownership_disposition": if stage == "execution" { "no-guest-resource" } else { "not-admitted" },
        "source_sha256": source.map(crate::workflow::intrinsic::sha256),
        "module": module, "state": state.map(|value| value.to_string()),
        "delta": delta.map(|value| value.to_string()),
        "typed_output": result.map(|value| json!({"type": "I64", "value": value.to_string()})),
        "guest_requests": guest_requests, "protected_operations": protected_operations,
        "diagnostic": diagnostic,
    });
    Report {
        outcome: outcome.to_owned(),
        json: json.to_string(),
    }
}

fn checked_source(
    source: &[u8],
) -> Result<
    (
        noble_contracts::source::Prepared,
        noble_wasm::source::Prepared,
    ),
    Failure,
> {
    let frontend = noble_contracts::source::Session::new();
    let prepared = attempt!(frontend
        .prepare(source, &[Ty::I64, Ty::I64], super::SOURCE_LIMITS)
        .map_err(Failure::source));
    let checked = attempt!(prepared
        .checked()
        .ok_or_else(|| failure("reject", "candidate must be an executable expression")));
    if checked.interface.stack_in.as_slice() != [Ty::I64, Ty::I64]
        || checked.interface.stack_out.as_slice() != [Ty::I64]
        || prepared.output() != [Ty::I64]
        || !checked.interface.effects.is_empty()
    {
        return Err(failure(
            "interface-reject",
            "candidate differs from consumer-owned pure scalar interface",
        ));
    }
    let submission = attempt!(prepared
        .submission()
        .ok_or_else(|| failure("reject", "candidate has no executable root")));
    let compiled = attempt!(noble_wasm::source::Compiler::new()
        .prepare(submission)
        .map_err(Failure::backend));
    Ok((prepared, compiled))
}

fn observed(report: &Report) -> Option<Value> {
    serde_json::from_str::<Value>(&report.json).ok()
}

fn decide(arguments: &[std::ffi::OsString]) -> Report {
    if arguments.len() != 7
        || arguments[1] != "scalar"
        || arguments[3] != "--state"
        || arguments[5] != "--delta"
    {
        return report(
            "invalid-input",
            "admission",
            None,
            None,
            None,
            None,
            None,
            0,
            0,
            Some(USAGE),
        );
    }
    let state = match scalar(&arguments[4]) {
        Ok(value) => value,
        Err(error) => {
            return report(
                error.context.outcome,
                "admission",
                None,
                None,
                None,
                None,
                None,
                0,
                0,
                Some(&error.message),
            )
        }
    };
    let delta = match scalar(&arguments[6]) {
        Ok(value) => value,
        Err(error) => {
            return report(
                error.context.outcome,
                "admission",
                None,
                None,
                None,
                None,
                None,
                0,
                0,
                Some(&error.message),
            )
        }
    };
    let path = std::path::Path::new(&arguments[2]);
    let source = match super::framing::read_file(path, super::SOURCE_LIMITS.bytes) {
        Ok(value) => value,
        Err(error) => {
            return report(
                error.context.outcome,
                "admission",
                None,
                Some(state),
                Some(delta),
                None,
                None,
                0,
                0,
                Some(&error.message),
            )
        }
    };
    let (_prepared, compiled) = match checked_source(&source) {
        Ok(value) => value,
        Err(error) => {
            return report(
                error.context.outcome,
                "admission",
                Some(&source),
                Some(state),
                Some(delta),
                None,
                None,
                0,
                0,
                Some(&error.message),
            )
        }
    };
    let mut engine = match super::worker::Engine::start_admission(false) {
        Ok(value) => value,
        Err(error) => {
            return report(
                error.context.outcome,
                "execution",
                Some(&source),
                Some(state),
                Some(delta),
                None,
                None,
                0,
                0,
                Some(&error.message),
            )
        }
    };
    let ready = match engine.prepare(compiled.wat(), &source, 1) {
        Ok(value) => value,
        Err(error) => {
            return report(
                error.context.outcome,
                "execution",
                Some(&source),
                Some(state),
                Some(delta),
                None,
                None,
                0,
                0,
                Some(&error.message),
            )
        }
    };
    let Some(prepared_report) = observed(&ready) else {
        return report(
            "internal-failure",
            "execution",
            Some(&source),
            Some(state),
            Some(delta),
            None,
            None,
            0,
            0,
            Some("malformed worker preparation report"),
        );
    };
    let module = prepared_report.get("module");
    if ready.outcome != "ready" {
        return report(
            &ready.outcome,
            "execution",
            Some(&source),
            Some(state),
            Some(delta),
            module,
            None,
            0,
            0,
            None,
        );
    }
    let digest = crate::workflow::intrinsic::sha256(&source);
    if module
        .and_then(|value| value.get("source_sha256"))
        .and_then(Value::as_str)
        != Some(digest.as_str())
        || module
            .and_then(|value| value.get("wasm_sha256"))
            .and_then(Value::as_str)
            .is_none()
    {
        return report(
            "internal-failure",
            "execution",
            Some(&source),
            Some(state),
            Some(delta),
            None,
            None,
            0,
            0,
            Some("worker preparation is not source-bound"),
        );
    }
    let inputs = json!([{"type": "I64", "value": state.to_string()},
                        {"type": "I64", "value": delta.to_string()}])
    .to_string();
    let execution = match engine.execute_scalar(&inputs) {
        Ok(value) => value,
        Err(error) => {
            return report(
                error.context.outcome,
                "execution",
                Some(&source),
                Some(state),
                Some(delta),
                module,
                None,
                0,
                0,
                Some(&error.message),
            )
        }
    };
    let Some(observation) = observed(&execution) else {
        return report(
            "internal-failure",
            "execution",
            Some(&source),
            Some(state),
            Some(delta),
            module,
            None,
            0,
            0,
            Some("malformed worker outcome"),
        );
    };
    let requests = observation.get("guest_requests").and_then(Value::as_u64);
    let protected = observation
        .get("protected_operations")
        .and_then(Value::as_u64);
    if observation
        .get("module")
        .and_then(|value| value.get("source_sha256"))
        .and_then(Value::as_str)
        != Some(digest.as_str())
        || observation
            .get("module")
            .and_then(|value| value.get("wasm_sha256"))
            != module.and_then(|value| value.get("wasm_sha256"))
        || requests.is_none()
        || protected.is_none()
        || requests != Some(0)
        || protected != Some(0)
    {
        return report(
            "internal-failure",
            "execution",
            Some(&source),
            Some(state),
            Some(delta),
            module,
            None,
            requests.unwrap_or(0),
            protected.unwrap_or(0),
            Some("worker outcome violates source/effect boundary"),
        );
    }
    if execution.outcome == "normal" {
        let values = observation.get("stack").and_then(Value::as_array);
        let result = values
            .filter(|values| values.len() == 1)
            .and_then(|values| {
                values[0]
                    .get("type")
                    .and_then(Value::as_str)
                    .filter(|kind| *kind == "I64")
                    .and_then(|_| values[0].get("value").and_then(Value::as_str))
            })
            .and_then(|value| {
                value
                    .parse::<i64>()
                    .ok()
                    .filter(|parsed| parsed.to_string() == value)
            });
        return match result {
            Some(result) => report(
                "normal",
                "execution",
                Some(&source),
                Some(state),
                Some(delta),
                module,
                Some(result),
                0,
                0,
                None,
            ),
            None => report(
                "internal-failure",
                "execution",
                Some(&source),
                Some(state),
                Some(delta),
                module,
                None,
                0,
                0,
                Some("normal worker return has wrong typed output"),
            ),
        };
    }
    if !matches!(execution.outcome.as_str(), "runtime-exhausted" | "trap") {
        return report(
            "internal-failure",
            "execution",
            Some(&source),
            Some(state),
            Some(delta),
            module,
            None,
            0,
            0,
            Some("unknown worker execution outcome"),
        );
    }
    report(
        &execution.outcome,
        "execution",
        Some(&source),
        Some(state),
        Some(delta),
        module,
        None,
        0,
        0,
        observation.get("quota_reason").and_then(Value::as_str),
    )
}

pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let result = decide(arguments);
    if let Err(error) = super::output::print(&result) {
        eprintln!("noble: {}", error.message);
        return std::process::ExitCode::from(error.exit());
    }
    std::process::ExitCode::from(result.exit())
}
