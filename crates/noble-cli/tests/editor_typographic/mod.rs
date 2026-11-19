//! Issue #141: a default-off typographic view over the five editor node kinds.
//!
//! The view is compiled only into this experiment's integration target, never
//! into the `noble` binary, and it is not an admission route: every measured
//! round trip hands the re-encoded document to the existing
//! `noble editor analyze|admit` ingress and compares the verdicts with the
//! equivalent JSON-tree edit. `verification/editor-typographic/` holds the
//! frozen corpus, criteria and the retained receipt.
//!
//! Proof boundary: these are measurements of the real CLI on a frozen corpus,
//! not a theorem about all editor trees. The renderer's own refusals are checked
//! exhaustively over the frozen negative controls only.

// Justification: this test-only lane is a table-driven measurement harness that
// builds frozen boundary corpora and asserts real CLI verdicts; terse fixture
// names, unwraps inside assertions and long legs are the readable form, matching
// the neighbouring `tests/editor.rs` harness.
#![allow(
    tigerstyle::ambiguous_params,
    tigerstyle::ambient_env,
    tigerstyle::assertion_density,
    tigerstyle::bool_naming,
    tigerstyle::borrowed_argument_types,
    tigerstyle::compound_assertion,
    tigerstyle::excessive_file_length,
    tigerstyle::explicit_defaults,
    tigerstyle::function_length,
    tigerstyle::missing_const_fn,
    tigerstyle::nested_conditionals,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this experiment lane is a test-only measurement harness that constructs frozen boundary corpora and asserts their CLI verdicts, so terse fixture names, table-driven legs and unwraps in assertions are the readable form."
)]

use std::path::PathBuf;
use std::process::Command;

use noble_contracts::source::editor::{Candidate, Node};
use serde_json::{json, Value};

mod corpus;
mod document;
mod measure;
mod receipt;
mod view;

use corpus::{Corpus, Fixture};
use measure::{Runner, Verdict};
use view::{Control, EffectFacts, HoleFacts, View, Widget};

/// One renderer refusal: a stable machine code plus a readable detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub code: &'static str,
    pub detail: String,
}

impl Refusal {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Refusal {
            code,
            detail: detail.into(),
        }
    }
}

/// The frozen criteria ids, in the committed order. A criterion may not be
/// dropped or reordered after the measurement it governs has run.
const CRITERIA: [&str; 10] = [
    "default-off",
    "five-kinds",
    "hole-coverage",
    "identity-tree",
    "identity-verdict",
    "identity-source",
    "edit-tree",
    "edit-verdict",
    "negative-refusal",
    "hole-admission",
];

/// The five editor node kinds this experiment must cover, exactly.
const FIVE_KINDS: [&str; 5] = ["boolean", "hole", "integer", "quotation", "word"];

fn lane(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "noble-editor-typographic-{name}-{}",
        std::process::id()
    ))
}

fn read(relative: &str) -> Result<String, String> {
    let path = lane(relative);
    std::fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))
}

fn blake3_file(relative: &str) -> Result<String, String> {
    let bytes =
        std::fs::read(lane(relative)).map_err(|error| format!("read {relative}: {error}"))?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

#[test]
fn frozen_criteria_are_committed_before_the_measurement() -> Result<(), Box<dyn std::error::Error>>
{
    let text = read(corpus::CRITERIA)?;
    let value: Value = serde_json::from_str(&text)?;
    let frozen: Vec<String> = value["criteria"]
        .as_array()
        .ok_or("criteria must be an array")?
        .iter()
        .map(|criterion| {
            criterion["id"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "criterion id must be text".into())
        })
        .collect::<Result<Vec<String>, String>>()?;
    assert_eq!(frozen, CRITERIA, "the frozen criteria changed");
    Ok(())
}

#[test]
fn typographic_view_covers_every_kind_and_hole_state_through_the_cli(
) -> Result<(), Box<dyn std::error::Error>> {
    let corpus = Corpus::load()?;
    let mut runner = Runner::new(&scratch("coverage"))?;
    let mut measured = Vec::new();
    for fixture in &corpus.fixtures {
        measured.push(coverage_fixture(&mut runner, fixture)?);
    }
    let mut kinds: Vec<String> = measured
        .iter()
        .flat_map(|measurement| measurement["kinds"].as_array().cloned().unwrap_or_default())
        .filter_map(|kind| kind.as_str().map(str::to_owned))
        .collect();
    kinds.sort();
    kinds.dedup();
    assert_eq!(kinds, FIVE_KINDS, "the corpus must cover all five kinds");

    let mut states = Vec::new();
    for state in &corpus.hole_states {
        let fixture = corpus
            .fixture(&state.fixture)
            .ok_or_else(|| format!("hole state {} names an unknown fixture", state.state))?;
        let measured = measured
            .iter()
            .find(|measurement| measurement["fixture"] == fixture.name.as_str())
            .ok_or_else(|| format!("hole state {} was not measured", state.state))?;
        let holes = measured["holes"].as_u64().unwrap_or(0);
        assert!(
            holes >= 1,
            "hole state {} must render at least one hole",
            state.state
        );
        states.push(json!({
            "state": state.state,
            "fixture": fixture.name,
            "holes": holes,
            "program_effect": measured["program_effect"].clone(),
        }));
    }
    // The unresolved-effect and known-effect states must really differ in the
    // report, not only in the corpus prose.
    let unresolved = states
        .iter()
        .find(|state| state["state"] == "unresolved-effect")
        .ok_or("missing unresolved-effect state")?;
    let known = states
        .iter()
        .find(|state| state["state"] == "known-effect")
        .ok_or("missing known-effect state")?;
    assert_eq!(unresolved["program_effect"]["known"], json!([]));
    assert_eq!(unresolved["program_effect"]["unresolved"], json!(true));
    assert_eq!(known["program_effect"]["known"], json!([0]));
    assert_eq!(known["program_effect"]["unresolved"], json!(true));
    Ok(())
}

#[test]
fn typographic_identity_and_edits_match_the_json_path() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = Corpus::load()?;
    let mut runner = Runner::new(&scratch("measure"))?;
    let (measured, exercised) = measurement(&corpus, &mut runner)?;
    assert_documented_edits(&corpus, &exercised)?;
    assert_eq!(measured.len(), corpus.fixtures.len());
    Ok(())
}

#[test]
fn typographic_negative_controls_refuse_before_admission() -> Result<(), Box<dyn std::error::Error>>
{
    let corpus = Corpus::load()?;
    let mut runner = Runner::new(&scratch("controls"))?;
    let _ = negative_measurement(&corpus, &mut runner)?;
    assert_no_new_action(&corpus)?;
    Ok(())
}

/// The receipt leg is inert unless the runner names an output path and the
/// clean source commit: an unaudited local test run never writes evidence.
#[test]
fn typographic_receipt_records_the_measurement() -> Result<(), Box<dyn std::error::Error>> {
    let Some(request) = receipt::request()? else {
        return Ok(());
    };
    let corpus = Corpus::load()?;
    let mut runner = Runner::new(&scratch("receipt"))?;
    let (measured, exercised) = measurement(&corpus, &mut runner)?;
    assert_documented_edits(&corpus, &exercised)?;
    let hostile = negative_measurement(&corpus, &mut runner)?;
    assert_no_new_action(&corpus)?;
    let measured = serde_json::Value::Array(measured);
    let receipt = json!({
        "schema": "noble-editor-typographic-receipt/v1",
        "issue": 141,
        "source_commit": request.source_commit(),
        "corpus_blake3": blake3_file(corpus::CORPUS)?,
        "criteria_blake3": blake3_file(corpus::CRITERIA)?,
        "renderer": {
            "compiled_into_binary": false,
            "max_quotation_depth": view::MAX_QUOTATION_DEPTH,
            "max_document_bytes": document::MAX_DOCUMENT_BYTES,
            "ingress": "noble editor analyze|admit JSON_FILE",
        },
        "fixtures": measured,
        "hole_states": corpus.hole_states.iter().map(|state| json!({
            "state": state.state,
            "fixture": state.fixture,
        })).collect::<Vec<Value>>(),
        "hostile_transport": hostile,
        "counts": {
            "fixtures": corpus.fixtures.len(),
            "edits": exercised.len(),
            "hole_states": corpus.hole_states.len(),
            "negative_controls": corpus.negative_controls.len(),
            "cli_invocations": runner.invocations(),
        },
        "limitations": "measured only over the frozen corpus through the real CLI; no theorem about all editor trees, no GUI, no second admission route, and no claim about the posts' demos",
    });
    request.write(&receipt)?;
    Ok(())
}

/// Coverage leg: kinds, frozen expectations, hole binding/rendering and the
/// identity pair for one fixture.
fn coverage_fixture(runner: &mut Runner, fixture: &Fixture) -> Result<Value, String> {
    let view = View::from_nodes(&fixture.nodes).map_err(|refusal| refusal.detail)?;
    assert_eq!(
        widget_kinds(&view.root),
        fixture.kinds,
        "{}: widget kinds",
        fixture.name
    );
    assert!(
        document::nodes_equal(&view.to_nodes(), &fixture.nodes),
        "{}: identity re-encoding changed the node tree",
        fixture.name
    );
    let document = fixture.document()?;
    let analyze = runner.analyze(&fixture.name, &document)?;
    let (holes, effect) = analyze_facts(&analyze)?;
    assert_eq!(
        analyze.exit, fixture.expect.analyze_exit,
        "{}",
        fixture.name
    );
    assert_eq!(holes.len(), fixture.expect.holes, "{}", fixture.name);
    assert_eq!(
        effect.known, fixture.expect.program_effect_known,
        "{}",
        fixture.name
    );
    assert_eq!(
        effect.unresolved, fixture.expect.program_effect_unresolved,
        "{}",
        fixture.name
    );
    let mut bound = view.clone();
    bound.set_program_effect(effect.clone());
    bound
        .bind_report(&holes)
        .map_err(|refusal| refusal.detail)?;
    assert_eq!(bound.hole_facts(), holes, "{}: bound holes", fixture.name);
    let rendered = bound.render();
    assert_eq!(
        rendered.matches("hole{").count(),
        holes.len(),
        "{}: rendered holes",
        fixture.name
    );
    for hole in &holes {
        assert!(
            rendered.contains(&hole.input_stack) && rendered.contains(&hole.output_stack),
            "{}: a rendered hole hides its reported stacks",
            fixture.name
        );
    }
    let admit = runner.admit(&fixture.name, &document)?;
    assert_eq!(admit.exit, fixture.expect.admit_exit, "{}", fixture.name);
    assert_eq!(admit.stage, fixture.expect.admit_stage, "{}", fixture.name);
    assert_eq!(
        admit.outcome, fixture.expect.admit_outcome,
        "{}",
        fixture.name
    );
    assert_eq!(
        admit.category, fixture.expect.admit_category,
        "{}",
        fixture.name
    );
    if document::contains_hole(&fixture.nodes) {
        assert_hole_refusal(&fixture.name, &admit)?;
    }
    Ok(json!({
        "fixture": fixture.name,
        "kinds": fixture.kinds,
        "holes": holes.len(),
        "program_effect": {
            "known": effect.known,
            "unresolved": effect.unresolved,
        },
        "analyze": analyze.json(),
        "admit": admit.json(),
        "rendered": rendered,
    }))
}

/// Measurement leg: identity and every documented edit, compared with the
/// equivalent JSON-tree edit through the same CLI.
fn measurement(corpus: &Corpus, runner: &mut Runner) -> Result<(Vec<Value>, Vec<String>), String> {
    let mut measured = Vec::new();
    let mut exercised = Vec::new();
    for fixture in &corpus.fixtures {
        let mut entry = identity_leg(runner, fixture)?;
        let mut edits = Vec::new();
        for edit in &fixture.edits {
            edits.push(edit_leg(runner, fixture, edit)?);
            exercised.push(edit.name.clone());
        }
        entry["edits"] = Value::Array(edits);
        measured.push(entry);
    }
    Ok((measured, exercised))
}

fn identity_leg(runner: &mut Runner, fixture: &Fixture) -> Result<Value, String> {
    let view = View::from_nodes(&fixture.nodes).map_err(|refusal| refusal.detail)?;
    let original = fixture.document()?;
    let reencoded = document::encode(&view.to_nodes()).map_err(|refusal| refusal.detail)?;
    let analyze_original = runner.analyze(&format!("{}-identity", fixture.name), &original)?;
    let analyze_view = runner.analyze(&format!("{}-view", fixture.name), &reencoded)?;
    measure::equal(
        &fixture.name,
        "identity analyze",
        &analyze_original,
        &analyze_view,
    )?;
    let admit_original = runner.admit(&format!("{}-identity", fixture.name), &original)?;
    let admit_view = runner.admit(&format!("{}-view", fixture.name), &reencoded)?;
    measure::equal(
        &fixture.name,
        "identity admit",
        &admit_original,
        &admit_view,
    )?;
    let source = admitted_source(&fixture.name, &fixture.nodes, &view.to_nodes())?;
    Ok(json!({
        "fixture": fixture.name,
        "analyze": analyze_original.json(),
        "admit": admit_original.json(),
        "admitted_source": source,
    }))
}

fn edit_leg(runner: &mut Runner, fixture: &Fixture, edit: &corpus::Edit) -> Result<Value, String> {
    let view = View::from_nodes(&fixture.nodes).map_err(|refusal| refusal.detail)?;
    let name = format!("{}-{}", fixture.name, edit.name);
    let document = edit
        .gesture
        .apply(&view)
        .map_err(|refusal| format!("{name}: gesture refused: {}", refusal.detail))?;
    assert!(
        document::nodes_equal(&document.nodes, &edit.nodes),
        "{name}: the widget edit and the JSON edit produced different trees"
    );
    let json_document = edit.document()?;
    let analyze_gesture = runner.analyze(&format!("{name}-gesture"), &document.text)?;
    let analyze_json = runner.analyze(&format!("{name}-json"), &json_document)?;
    measure::equal(&name, "edited analyze", &analyze_gesture, &analyze_json)?;
    assert_ne!(
        analyze_gesture.stage, "transport",
        "{name}: the renderer produced a document the ingress rejects"
    );
    let admit_gesture = runner.admit(&format!("{name}-gesture"), &document.text)?;
    let admit_json = runner.admit(&format!("{name}-json"), &json_document)?;
    measure::equal(&name, "edited admit", &admit_gesture, &admit_json)?;
    let source = admitted_source(&name, &document.nodes, &edit.nodes)?;
    if document::contains_hole(&document.nodes) {
        assert_hole_refusal(&name, &admit_gesture)?;
    } else {
        assert_eq!(
            admit_gesture.stage, "wasm",
            "{name}: a hole-free edited tree must reach execution"
        );
    }
    Ok(json!({
        "edit": edit.name,
        "analyze": analyze_gesture.json(),
        "admit": admit_gesture.json(),
        "admitted_source": source,
    }))
}

/// Negative leg: every frozen control is refused by the renderer, no admission
/// runs, and the ingress still refuses the hostile transport documents with no
/// guest request or accepted program.
fn negative_measurement(corpus: &Corpus, runner: &mut Runner) -> Result<Vec<Value>, String> {
    let mut refused = Vec::new();
    for control in &corpus.negative_controls {
        let fixture = corpus
            .fixture(&control.fixture)
            .ok_or_else(|| format!("{}: unknown fixture", control.name))?;
        let view = View::from_nodes(&fixture.nodes).map_err(|refusal| refusal.detail)?;
        let before = runner.invocations();
        let error = control
            .gesture
            .apply(&view)
            .expect_err(&format!("{}: the gesture was not refused", control.name));
        assert_eq!(error.code, control.code, "{}", control.name);
        assert!(!error.detail.is_empty(), "{}", control.name);
        assert_eq!(
            runner.invocations(),
            before,
            "{}: a refused gesture must not reach the CLI",
            control.name
        );
        refused.push(json!({
            "control": control.name,
            "code": error.code,
            "fixture": fixture.name,
            "admit_invocations": runner.invocations() - before,
        }));
    }
    let mut hostile = Vec::new();
    for (index, document) in corpus.hostile_transport.iter().enumerate() {
        let name = format!("hostile-{index}");
        let admit = runner.admit(&name, document)?;
        let analyze = runner.analyze(&name, document)?;
        for verdict in [&analyze, &admit] {
            assert_eq!(verdict.exit, 2, "{name}: {}", verdict.summary());
            assert_eq!(verdict.outcome, "reject", "{name}");
            assert_eq!(
                verdict.accepted_program_created,
                Some(false),
                "{name}: the ingress claimed an accepted program"
            );
            assert_eq!(verdict.guest_requests, 0, "{name}");
            assert_eq!(verdict.protected_operations, 0, "{name}");
        }
        hostile.push(json!({
            "document": document,
            "analyze": analyze.json(),
            "admit": admit.json(),
        }));
    }
    assert_eq!(refused.len(), corpus.negative_controls.len());
    Ok(hostile)
}

/// The renderer adds no CLI action and is not compiled into the binary.
fn assert_no_new_action(corpus: &Corpus) -> Result<(), String> {
    let fixture = corpus.fixtures.first().ok_or("the corpus is empty")?;
    let directory = scratch("actions");
    std::fs::create_dir_all(&directory).map_err(|error| format!("create actions dir: {error}"))?;
    let path = directory.join("document.json");
    std::fs::write(&path, fixture.document()?).map_err(|error| format!("write probe: {error}"))?;
    let output = Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["editor", "typographic"])
        .arg(&path)
        .output()
        .map_err(|error| format!("launch noble editor typographic: {error}"))?;
    let _ = std::fs::remove_file(&path);
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

fn contains_marker(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn assert_hole_refusal(name: &str, admit: &Verdict) -> Result<(), String> {
    assert_eq!(admit.exit, 2, "{name}: {}", admit.summary());
    assert_eq!(admit.stage, "admission", "{name}: {}", admit.summary());
    assert_eq!(admit.outcome, "reject", "{name}");
    assert_eq!(admit.category.as_deref(), Some("invalid"), "{name}");
    assert_eq!(admit.accepted_program_created, Some(false), "{name}");
    assert_eq!(admit.guest_requests, 0, "{name}");
    assert_eq!(admit.protected_operations, 0, "{name}");
    Ok(())
}

fn assert_documented_edits(corpus: &Corpus, exercised: &[String]) -> Result<(), String> {
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

fn widget_kinds(controls: &[Control]) -> Vec<String> {
    let mut kinds = Vec::new();
    for control in controls {
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
            kinds.extend(widget_kinds(children));
        }
    }
    kinds
}

fn analyze_facts(verdict: &Verdict) -> Result<(Vec<HoleFacts>, EffectFacts), String> {
    let value: Value =
        serde_json::from_str(&verdict.report).map_err(|error| format!("parse report: {error}"))?;
    let holes = value
        .get("holes")
        .and_then(Value::as_array)
        .ok_or("the analyze report carries no holes array")?;
    let mut facts = Vec::new();
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
    let mut ids = Vec::new();
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
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("report field {key} entries must be text"))
        })
        .collect()
}

/// `admitted_source` equality for hole-free trees, or an equal refusal.
fn admitted_source(name: &str, left: &[Node], right: &[Node]) -> Result<Value, String> {
    let limits = noble_contracts::Limits::default();
    let left = candidate(left).admitted_source(limits);
    let right = candidate(right).admitted_source(limits);
    match (left, right) {
        (Ok(a), Ok(b)) => {
            assert_eq!(a, b, "{name}: admitted_source bytes differ");
            Ok(json!({
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
            Ok(json!({
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
