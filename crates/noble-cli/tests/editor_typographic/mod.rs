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

mod apply;
mod cases;
mod document;
mod frozen;
mod harness;
mod measure;
mod surface;

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
    let text = read(frozen::CRITERIA_PATH)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
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
