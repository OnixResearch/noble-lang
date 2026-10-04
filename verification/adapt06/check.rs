//! ADAPT-06 peer: reports, without judging, the typed test.emit premise and the
//! kernel/compiler outcomes for the authentic, forged-contract and request-only
//! submissions. No engine, guest instance or host is constructed.

use noble_contracts::source::Session;
use noble_contracts::Limits;
use noble_kernel::contracts::{Behavior, Definition, TEST_EMIT};
use noble_kernel::execution::Submission;
use noble_kernel::shapes::EffectSlot;
use noble_kernel::types::{EffId, EffSet};
use noble_kernel::untrusted::{Constraint, Node, Outcome};
use noble_wasm::source::Compiler;
use std::io::Write;

const EMIT: Definition = Definition(22);

fn escape(text: &str) -> String {
    text.chars()
        .flat_map(|character| match character {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            character if character.is_control() => {
                format!("\\u{:04x}", u32::from(character)).chars().collect()
            }
            character => vec![character],
        })
        .collect()
}

fn effect(id: EffId) -> String {
    if id == TEST_EMIT {
        "\"test.emit\"".into()
    } else {
        format!("\"effect:{}\"", id.0)
    }
}

fn effects(set: &EffSet) -> String {
    let names: Vec<String> = set.as_slice().iter().map(|id| effect(*id)).collect();
    format!("[{}]", names.join(","))
}

fn slots(slots: &[EffectSlot]) -> String {
    let names: Vec<String> = slots
        .iter()
        .map(|slot| match slot {
            EffectSlot::Effect(id) => effect(*id),
            other => format!("\"{}\"", escape(&format!("{other:?}"))),
        })
        .collect();
    format!("[{}]", names.join(","))
}

fn constraint(constraint: &Constraint) -> String {
    match constraint {
        Constraint::EffectInclusion(id) if *id == TEST_EMIT => "EffectInclusion(test.emit)".into(),
        other => escape(&format!("{other:?}")),
    }
}

fn kernel(submission: &Submission) -> String {
    match noble_kernel::acceptance::check(
        &submission.environment,
        &submission.request,
        &submission.body.candidate,
    ) {
        Outcome::Accepted(checked) => format!(
            "{{\"outcome\":\"Accepted\",\"effects\":{}}}",
            effects(&checked.interface.effects)
        ),
        Outcome::Invalid(diagnostic) => format!(
            "{{\"outcome\":\"Invalid\",\"constraint\":\"{}\"}}",
            constraint(&diagnostic.constraint)
        ),
        Outcome::Unsupported(kind) => format!(
            "{{\"outcome\":\"Unsupported\",\"kind\":\"{}\"}}",
            escape(&format!("{kind:?}"))
        ),
        Outcome::Exhausted(kind) => format!(
            "{{\"outcome\":\"Exhausted\",\"kind\":\"{}\"}}",
            escape(&format!("{kind:?}"))
        ),
        Outcome::InternalFailure => "{\"outcome\":\"InternalFailure\"}".into(),
    }
}

/// A prepared module is written only for the path's first successful
/// compilation, so a negative observation can never leave WAT behind.
fn compiler(submission: &Submission, wat: Option<&str>) -> Result<String, String> {
    match Compiler::new().prepare(submission) {
        Ok(prepared) => {
            let written = match wat {
                Some(path) => {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(path)
                        .and_then(|mut file| file.write_all(prepared.wat()))
                        .map_err(|error| format!("{path}: {error}"))?;
                    format!("\"{}\"", escape(path))
                }
                None => "null".into(),
            };
            Ok(format!(
                "{{\"outcome\":\"prepared\",\"wat_bytes\":{},\"wat_file\":{written}}}",
                prepared.wat().len()
            ))
        }
        Err(diagnostic) => Ok(format!(
            "{{\"outcome\":\"{}\",\"wat_bytes\":null,\"wat_file\":null}}",
            match diagnostic {
                noble_wasm::Diagnostic::Invalid => "Invalid",
                noble_wasm::Diagnostic::Exhausted => "Exhausted",
                noble_wasm::Diagnostic::Unsupported => "Unsupported",
                noble_wasm::Diagnostic::Defective => "Defective",
            }
        )),
    }
}

fn premise(submission: &Submission) -> String {
    let expected = &submission.request.expected;
    let invocations: Vec<String> = submission
        .body
        .candidate
        .nodes
        .iter()
        .filter_map(|node| match node {
            Node::Invocation { def, .. } => Some(format!(
                "{{\"definition\":{},\"behavior\":\"{}\"}}",
                def.0,
                escape(&format!("{:?}", submission.environment.kind(*def)))
            )),
            _ => None,
        })
        .collect();
    let texts: Vec<String> = submission
        .body
        .texts
        .iter()
        .map(|text| format!("\"{}\"", escape(&String::from_utf8_lossy(&text.bytes))))
        .collect();
    format!(
        "{{\"definitions\":{},\"nodes\":{},\"stack_in\":{},\"stack_out\":{},\"allowed_effects\":{},\"invocations\":[{}],\"texts\":[{}],\"emit_behavior\":\"{}\",\"emit_contract_effects\":{}}}",
        submission.definitions.len(),
        submission.body.candidate.nodes.len(),
        expected.stack_in.len(),
        expected.stack_out.len(),
        effects(&expected.allowed_effects),
        invocations.join(","),
        texts.join(","),
        escape(&format!("{:?}", submission.environment.kind(EMIT))),
        submission
            .environment
            .scheme(EMIT)
            .map_or_else(|| "null".into(), |scheme| slots(&scheme.effects)),
    )
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [source, wat] = args.as_slice() else {
        return Err("usage: adapt06-check SOURCE NEW_WAT_FILE".into());
    };
    let bytes = std::fs::read(source).map_err(|error| format!("{source}: {error}"))?;
    let prepared = Session::new()
        .prepare(&bytes, &[], Limits::default())
        .map_err(|error| format!("source preparation failed: {error:?}"))?;
    let authentic = prepared
        .submission()
        .cloned()
        .ok_or("typed source produced no executable submission")?;
    if authentic.environment.kind(EMIT) != Some(Behavior::TestEmit) {
        return Err("definition 22 is not the fixed test.emit host operation".into());
    }
    let mut forged = authentic.clone();
    forged.environment.defs[22].effects = Vec::new();
    forged.request.expected.allowed_effects = EffSet::empty();
    let mut bounded = authentic.clone();
    bounded.request.expected.allowed_effects = EffSet::empty();
    println!(
        "{{\"id\":\"ADAPT-06\",\"premise\":{},\"authentic\":{{\"kernel\":{},\"compiler\":{}}},\"forged\":{{\"mutations\":[\"environment.defs[22].effects=[]\",\"request.expected.allowed_effects=[]\"],\"emit_contract_effects\":{},\"kernel\":{},\"compiler\":{}}},\"request_only\":{{\"mutations\":[\"request.expected.allowed_effects=[]\"],\"kernel\":{},\"compiler\":{}}}}}",
        premise(&authentic),
        kernel(&authentic),
        compiler(&authentic, Some(wat))?,
        slots(&forged.environment.defs[22].effects),
        kernel(&forged),
        compiler(&forged, None)?,
        kernel(&bounded),
        compiler(&bounded, None)?,
    );
    Ok(())
}
