//! Identity and edited-equality legs against the real CLI.

use serde_json::{json, Value};

use super::super::cases::{Edit, Fixture};
use super::super::document;
use super::super::frozen::Snapshot;
use super::super::measure::{equal, Runner};
use super::super::surface::View;
use super::{admitted_source, assert_documented_edits};

#[test]
fn reencoding_and_edits_match_the_json_path() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = Snapshot::load()?;
    let mut runner = Runner::new(&super::super::scratch("measure"))?;
    let (measured, exercised) = measurement(&corpus, &mut runner)?;
    assert_documented_edits(&corpus, &exercised)?;
    assert_eq!(measured.len(), corpus.fixtures.len());
    Ok(())
}

/// Identity and every documented edit, compared with the equivalent JSON edit
/// through the same CLI.
pub(super) fn measurement(
    corpus: &Snapshot,
    runner: &mut Runner,
) -> Result<(Vec<Value>, Vec<String>), String> {
    let mut measured = Vec::with_capacity(corpus.fixtures.len());
    let mut exercised = Vec::new();
    for fixture in &corpus.fixtures {
        let mut entry = reencode_leg(runner, fixture)?;
        let mut edits = Vec::with_capacity(fixture.edits.len());
        for edit in &fixture.edits {
            edits.push(gesture_leg(runner, fixture, edit)?);
            exercised.push(edit.name.clone());
        }
        entry["edits"] = Value::Array(edits);
        measured.push(entry);
    }
    Ok((measured, exercised))
}

fn reencode_leg(runner: &mut Runner, fixture: &Fixture) -> Result<Value, String> {
    let view = View::from_nodes(&fixture.nodes).map_err(|refusal| refusal.detail)?;
    let original = fixture.document()?;
    let reencoded = document::encode(&view.to_nodes()).map_err(|refusal| refusal.detail)?;
    let analyze_original = runner.analyze(&format!("{}-identity", fixture.name), &original)?;
    let analyze_view = runner.analyze(&format!("{}-view", fixture.name), &reencoded)?;
    equal(
        &fixture.name,
        "identity analyze",
        &analyze_original,
        &analyze_view,
    )?;
    let admit_original = runner.admit(&format!("{}-identity", fixture.name), &original)?;
    let admit_view = runner.admit(&format!("{}-view", fixture.name), &reencoded)?;
    equal(
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

fn gesture_leg(runner: &mut Runner, fixture: &Fixture, edit: &Edit) -> Result<Value, String> {
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
    equal(&name, "edited analyze", &analyze_gesture, &analyze_json)?;
    assert_ne!(
        analyze_gesture.stage, "transport",
        "{name}: the renderer produced a document the ingress rejects"
    );
    let admit_gesture = runner.admit(&format!("{name}-gesture"), &document.text)?;
    let admit_json = runner.admit(&format!("{name}-json"), &json_document)?;
    equal(&name, "edited admit", &admit_gesture, &admit_json)?;
    let source = admitted_source(&name, &document.nodes, &edit.nodes)?;
    if document::contains_hole(&document.nodes) {
        super::assert_hole_refusal(&name, &admit_gesture)?;
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
