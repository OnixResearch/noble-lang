//! The retained receipt: produced only from a clean committed tree.

use serde_json::{json, Value};

use super::super::frozen::{Snapshot, CORPUS_PATH, CRITERIA_PATH};
use super::super::measure::Runner;
use super::identity::measurement;
use super::refusals::negative_measurement;
use super::{assert_documented_edits, assert_no_new_action};

/// The receipt leg is inert unless the runner names an output path and the
/// clean source commit: an unaudited local test run never writes evidence.
#[test]
fn receipt_records_the_measurement() -> Result<(), Box<dyn std::error::Error>> {
    let Some(request) = super::receipt::request()? else {
        return Ok(());
    };
    let corpus = Snapshot::load()?;
    let mut runner = Runner::new(&super::super::scratch("receipt"))?;
    let (measured, exercised) = measurement(&corpus, &mut runner)?;
    assert_documented_edits(&corpus, &exercised)?;
    let hostile = negative_measurement(&corpus, &mut runner)?;
    assert_no_new_action(&corpus)?;
    let receipt = json!({
        "schema": "noble-editor-typographic-receipt/v1",
        "issue": 141,
        "source_commit": request.source_commit(),
        "corpus_blake3": super::super::blake3_file(CORPUS_PATH)?,
        "criteria_blake3": super::super::blake3_file(CRITERIA_PATH)?,
        "renderer": {
            "compiled_into_binary": false,
            "max_quotation_depth": super::super::apply::MAX_QUOTATION_DEPTH,
            "max_envelope_bytes": super::super::document::MAX_ENVELOPE_BYTES,
            "ingress": "noble editor analyze|admit JSON_FILE",
        },
        "fixtures": Value::Array(measured),
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
