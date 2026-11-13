//! The opt-in graph-to-LLVM lowering, its independent validator and its controls (issue #107).
//!
//! The nested modules keep graph fixtures, parser-derived Lean exports, retained bindings and
//! assertions separate while remaining one existing Cargo test target.

use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::eval::{self, Obs};
use noble_ngraph::identity::IdentityScheme;
use noble_ngraph::ir::{
    Fact, Graph, GraphBuilder, Literal, Op, Provenance, ScalarType, ShapeTree, SlotKind,
    SubProfile, Subject,
};
use noble_ngraph::llvm::{self, Instruction, Module, Operand, Refusal};
use noble_ngraph::sem::Value;

use super::differential::TestIds;

#[path = "llvm/artifact_tests.rs"]
mod artifact_tests;
#[path = "llvm/bundle.rs"]
mod bundle;
#[path = "llvm/controls.rs"]
mod controls;
#[path = "llvm/corpus.rs"]
mod corpus;
#[path = "llvm/fixtures.rs"]
mod fixtures;
#[path = "llvm/model.rs"]
mod model;
#[path = "llvm/profile_tests.rs"]
mod profile_tests;

use bundle::{request_text, retained_files, retained_json};
use controls::{
    control_path, control_records, json_list, json_string, retained_identity, ControlRecord,
};
use corpus::{certificate_text, lean_source};
use fixtures::{artifact_named, artifacts, simple_graph, Artifact};
use model::{
    executed, hex_bits, lean_instruction, lean_name, lean_node, lean_operand, operand_text,
    reference_value, term_list,
};

const LANE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../verification/llvm-lowering"
);

const EXECUTIONS: [[i64; 2]; 2] = [[0, 0], [i64::MIN, i64::MAX]];

const SOURCE_PATHS: &[&str] = &[
    "crates/noble-ngraph/src/llvm.rs",
    "crates/noble-ngraph/src/ir.rs",
    "crates/noble-ngraph/src/check.rs",
    "crates/noble-ngraph/src/format/graph.rs",
    "crates/noble-ngraph/src/sem.rs",
    "crates/noble-ngraph/src/eval.rs",
    "crates/noble-ngraph/src/cert.rs",
    "crates/noble-ngraph/tests/cert/differential.rs",
    "crates/noble-ngraph/tests/cert/llvm.rs",
    "crates/noble-ngraph/tests/cert/llvm/artifact_tests.rs",
    "crates/noble-ngraph/tests/cert/llvm/bundle.rs",
    "crates/noble-ngraph/tests/cert/llvm/controls.rs",
    "crates/noble-ngraph/tests/cert/llvm/corpus.rs",
    "crates/noble-ngraph/tests/cert/llvm/fixtures.rs",
    "crates/noble-ngraph/tests/cert/llvm/model.rs",
    "crates/noble-ngraph/tests/cert/llvm/profile_tests.rs",
    "proofs/foundation/NobleFoundation/LlvmLowering.lean",
    "verification/llvm-lowering/replay.mjs",
];

fn read_repo_file(path: &str) -> String {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let full = repo.join(path);
    std::fs::read_to_string(&full)
        .unwrap_or_else(|error| panic!("repository file {path} is readable: {error}"))
}

fn source_bindings() -> Vec<(String, String)> {
    SOURCE_PATHS
        .iter()
        .map(|path| ((*path).to_owned(), read_repo_file(path)))
        .collect()
}
