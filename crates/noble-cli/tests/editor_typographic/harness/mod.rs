//! The measurement harness: real-CLI legs over the frozen corpus.
//!
//! Each leg drives `noble editor analyze|admit` and compares verdicts rather
//! than asserting a reimplementation of the CLI.

use noble_contracts::source::editor::{Candidate, Node};
use serde_json::Value;

use super::measure::Verdict;
use super::surface::{Control, EffectFacts, HoleFacts, Widget};

mod coverage;
mod evidence;
mod identity;
mod receipt;
mod refusals;

/// The analyze report's hole facts, in source order, plus the program effect.
pub(super) fn analyze_facts(verdict: &Verdict) -> Result<(Vec<HoleFacts>, EffectFacts), String> {
    let value: Value =
        serde_json::from_str(&verdict.report).map_err(|error| format!("parse report: {error}"))?;
    let holes = value
        .get("holes")
        .and_then(Value::as_array)
        .ok_or("the analyze report carries no holes array")?;
    let mut facts = Vec::with_capacity(holes.len());
    for hole in holes {
        facts.push(HoleFacts {
            input_stack: text_field(hole, "input_stack")?,
            output_stack: text_field(hole, "required_output_stack")?,
            required_output_suffix: strings_field(hole, "required_output_suffix")?,
            effect: effect_facts(Some(&hole["effect"]))?,
        });
    }
    Ok((facts, effect_facts(value.get("effect"))?))
}

fn effect_facts(value: Option<&Value>) -> Result<EffectFacts, String> {
    let value = value.ok_or("the report carries no effect object")?;
    let known = value
        .get("known")
        .and_then(Value::as_array)
        .ok_or("effect known must be an array")?;
    let mut ids = Vec::with_capacity(known.len());
    for id in known {
        let id = id.as_u64().ok_or("effect ids must be unsigned")?;
        ids.push(u32::try_from(id).map_err(|_| "effect id outside u32")?);
    }
    Ok(EffectFacts {
        known: ids,
        unresolved: value
            .get("unresolved")
            .and_then(Value::as_bool)
            .ok_or("effect unresolved must be a boolean")?,
    })
}

fn text_field(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("report field {key} must be text"))
}

fn strings_field(value: &Value, key: &str) -> Result<Vec<String>, String> {
    let Some(items) = value.get(key).and_then(Value::as_array) else {
        return Err(format!("report field {key} must be an array"));
    };
    let mut strings = Vec::with_capacity(items.len());
    for item in items {
        strings.push(
            item.as_str()
                .ok_or_else(|| format!("report field {key} entries must be text"))?
                .to_owned(),
        );
    }
    Ok(strings)
}

/// The rendered widget kinds in tree order, walked with an explicit stack.
pub(super) fn widget_kinds(controls: &[Control]) -> Vec<String> {
    let mut kinds = Vec::new();
    let mut stack: Vec<&Control> = controls.iter().rev().collect();
    while let Some(control) = stack.pop() {
        kinds.push(
            match &control.widget {
                Widget::Number { .. } => "integer",
                Widget::Checkbox { .. } => "boolean",
                Widget::Word { .. } => "word",
                Widget::Group { .. } => "quotation",
                Widget::Hole { .. } => "hole",
            }
            .to_owned(),
        );
        if let Widget::Group { children } = &control.widget {
            stack.extend(children.iter().rev());
        }
    }
    kinds
}

pub(super) fn assert_hole_refusal(name: &str, admit: &Verdict) -> Result<(), String> {
    assert_eq!(admit.exit, 2, "{name}: {}", admit.summary());
    assert_eq!(admit.stage, "admission", "{name}: {}", admit.summary());
    assert_eq!(admit.outcome, "reject", "{name}");
    assert_eq!(admit.category.as_deref(), Some("invalid"), "{name}");
    assert_eq!(admit.accepted_program_created, Some(false), "{name}");
    assert_eq!(admit.guest_requests, 0, "{name}");
    assert_eq!(admit.protected_operations, 0, "{name}");
    Ok(())
}

pub(super) fn assert_documented_edits(
    corpus: &super::frozen::Snapshot,
    exercised: &[String],
) -> Result<(), String> {
    assert_eq!(corpus.documented_edits.len(), 6);
    for documented in &corpus.documented_edits {
        assert!(!documented.edits.is_empty(), "{}", documented.name);
        for edit in &documented.edits {
            assert!(
                exercised.iter().any(|name| name == edit),
                "documented edit {} names unexercised edit {edit}",
                documented.name
            );
        }
    }
    Ok(())
}

pub(super) fn contains_marker(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// The renderer adds no CLI action and is not compiled into the binary.
pub(super) fn assert_no_new_action(corpus: &super::frozen::Snapshot) -> Result<(), String> {
    let fixture = corpus.fixtures.first().ok_or("the corpus is empty")?;
    let directory = super::scratch("actions");
    std::fs::create_dir_all(&directory).map_err(|error| format!("create actions dir: {error}"))?;
    let path = directory.join("document.json");
    std::fs::write(&path, fixture.document()?).map_err(|error| format!("write probe: {error}"))?;
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["editor", "typographic"])
        .arg(&path)
        .output()
        .map_err(|error| format!("launch noble editor typographic: {error}"))?;
    super::measure::remove_probe(&path)?;
    let report = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    assert_eq!(output.status.code(), Some(2), "{report}");
    assert!(
        report.contains("\"stage\":\"transport\""),
        "an unknown action is a transport refusal: {report}"
    );
    assert!(
        !report.contains("typographic") && !report.contains("widget"),
        "no renderer action exists: {report}"
    );
    let binary = std::fs::read(env!("CARGO_BIN_EXE_noble"))
        .map_err(|error| format!("read the noble binary: {error}"))?;
    assert!(
        !contains_marker(&binary, b"hole{unbound}"),
        "the renderer must not be compiled into the noble binary"
    );
    Ok(())
}

/// `admitted_source` equality for hole-free trees, or an equal refusal.
pub(super) fn admitted_source(name: &str, left: &[Node], right: &[Node]) -> Result<Value, String> {
    let limits = noble_contracts::Limits::default();
    let left = candidate(left).admitted_source(limits);
    let right = candidate(right).admitted_source(limits);
    match (left, right) {
        (Ok(a), Ok(b)) => {
            assert_eq!(a, b, "{name}: admitted_source bytes differ");
            Ok(serde_json::json!({
                "result": "equal",
                "bytes": a.len(),
                "blake3": blake3::hash(&a).to_hex().to_string(),
            }))
        }
        (Err(a), Err(b)) => {
            assert_eq!(a.stage(), b.stage(), "{name}: refusal stages differ");
            assert_eq!(
                a.diagnostic().kind.category(),
                b.diagnostic().kind.category(),
                "{name}: refusal categories differ"
            );
            Ok(serde_json::json!({
                "result": "refused",
                "stage": format!("{:?}", a.stage()),
                "category": a.diagnostic().kind.category(),
            }))
        }
        _ => Err(format!(
            "{name}: one tree produced source and the other was refused"
        )),
    }
}

fn candidate(nodes: &[Node]) -> Candidate {
    Candidate {
        format: 1,
        nodes: nodes.to_vec(),
    }
}
