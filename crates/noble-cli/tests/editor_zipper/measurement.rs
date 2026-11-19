//! Frozen-corpus measurement for the opt-in zipper cursor (issue #142).
//!
//! Every measurement here goes through the real editor transport: the
//! documents are handed to `noble editor analyze` / `noble editor admit` as a
//! child process, and the verdicts compared are the ones those commands print.
//! The cursor itself is the same source files the `noble-cli` test build
//! compiles, included with `#[path]` so the measured code is byte-identical to
//! the reviewed module.
//!
//! What this lane is not: it is not a theorem, a refinement, or a
//! correspondence between a model and this code. The cited paper's cost
//! bounds, completeness results and timings are the authors' claims and are
//! never thresholds here (see `verification/editor-zipper/criteria.json`).
//!
//! Usage (from the repository root, receipt produced from a clean commit):
//!   NOBLE_EDITOR_ZIPPER_RECEIPT=<absolute path outside the repository> \
//!     cargo test -p noble-cli --test editor -- cursor_corpus_cells
//! Without that variable the same assertions run and nothing is written.

// The module path words are the lane's own vocabulary: the included cursor is
// the navigation the corpus exercises, and each helper module names a distinct
// part of the measurement.
#[path = "../../src/core/editor/zipper.rs"]
mod navigation;

#[path = "measurement/corpus.rs"]
mod corpus;
#[path = "measurement/paired.rs"]
mod paired;
#[path = "measurement/receipt.rs"]
mod receipt;
#[path = "measurement/transport.rs"]
mod transport;
#[path = "measurement/walk.rs"]
mod walk;

use navigation::{Cursor, Refusal};
use noble_contracts::source::editor::Node;
use std::vec::Vec;

type Error = Box<dyn std::error::Error>;
type Document = serde_json::Value;

const CORPUS_PATH: &str = "verification/editor-zipper/corpus.json";
const CRITERIA_PATH: &str = "verification/editor-zipper/criteria.json";
const IMPLEMENTATION: [&str; 6] = [
    "crates/noble-cli/src/core/editor/zipper.rs",
    "crates/noble-cli/src/core/editor/zipper/document.rs",
    "crates/noble-cli/src/core/editor/zipper/tests.rs",
    "crates/noble-cli/tests/editor_zipper/measurement.rs",
    "crates/noble-cli/tests/editor_zipper/measurement/paired.rs",
    "crates/noble-cli/tests/editor.rs",
];
const RECEIPT_VARIABLE: &str = "NOBLE_EDITOR_ZIPPER_RECEIPT";

// The refusal-class literals exist only in the test-only cursor module; the
// shipped binary must contain none of them, which is how the default-off
// criterion is measured on the actual artifact rather than asserted from the
// source layout. A schema literal that the shipped CLI does contain is the
// positive control, so an empty search cannot pass for an absent cursor.
const CURSOR_MARKERS: [&str; 2] = ["empty-quotation", "no-such-node"];
const SHIPPED_CONTROL: &str = "noble-editor-report/v1";

#[derive(serde::Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    origin: String,
    tree: Document,
    sequences: Vec<Sequence>,
    replacements: Vec<Replacement>,
    refusals: Vec<RefusalCell>,
}

#[derive(serde::Deserialize)]
struct Sequence {
    name: String,
    at: Vec<u32>,
    steps: Vec<Step>,
    addresses: Vec<Vec<u32>>,
    tree: Option<Document>,
}

#[derive(serde::Deserialize)]
struct Step {
    op: String,
}

#[derive(serde::Deserialize)]
struct Replacement {
    name: String,
    at: Vec<u32>,
    node: Document,
    tree: Document,
}

#[derive(serde::Deserialize)]
struct RefusalCell {
    name: String,
    at: Vec<u32>,
    op: String,
    class: String,
}

/// One cursor step the corpus can document.
#[derive(Clone, Copy)]
enum Operation {
    Focus,
    Enter,
    Leave,
    Next,
    Previous,
}

fn operation(name: &str) -> Result<Operation, Error> {
    match name {
        "focus" => Ok(Operation::Focus),
        "enter" => Ok(Operation::Enter),
        "leave" => Ok(Operation::Leave),
        "next" => Ok(Operation::Next),
        "previous" => Ok(Operation::Previous),
        other => Err(std::format!("unknown corpus operation {other}").into()),
    }
}

fn refused(refusal: Refusal) -> Error {
    std::format!("cursor refused with {}", refusal.class()).into()
}

/// Address a node and, unless the step only addresses it, apply one move.
fn attempt(tree: &[Node], address: &[u32], step: Operation) -> Result<Cursor, Refusal> {
    let cursor = Cursor::at(tree, address)?;
    match step {
        Operation::Focus => Ok(cursor),
        Operation::Enter => cursor.enter(),
        Operation::Leave => cursor.leave(),
        Operation::Next => cursor.next(),
        Operation::Previous => cursor.previous(),
    }
}

fn focus(tree: &[Node], address: &[u32]) -> Result<Cursor, Error> {
    Cursor::at(tree, address).map_err(refused)
}
