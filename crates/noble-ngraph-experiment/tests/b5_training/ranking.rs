#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the experiment harness serializes a fixed deterministic corpus and checks its independent rows."
)]

use noble_contracts::source::chunks;
use noble_contracts::source::chunks::{
    Binding, Budget, Envelope, Fixture, FixtureSet, Instantiation, Interface, Library, Node, Op,
    Recipe, Request, Scalar, Task,
};
use noble_contracts::source::editor;
use noble_kernel::types::{EffSet, Ty};
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{GraphBuilder, ScalarType, SubProfile, Subject};
use noble_ngraph::snapshot::{layout_key, ParamSnapshot};
use noble_ngraph::train::{self, Adam, Contract, Point, TrainControl, Trained};
use noble_ngraph::{check, CheckedGraph};
use noble_ngraph_experiment::scheme::BLAKE3;
use serde_json::{json, Value};

const FAMILY_NAMES: [&str; 5] = [
    "arithmetic",
    "ordered-stack-routing",
    "boolean",
    "nested-quotation",
    "equal-signature-contrast",
];
const FIXTURE_CASES: usize = 4;
const FROZEN_CORPUS: &str =
    include_str!("../../../../verification/typed-chunk-ranking/corpus-v1.json");
const FROZEN_CRITERIA: &str =
    include_str!("../../../../verification/typed-chunk-ranking/criteria-v1.json");

#[path = "ranking/fixtures.rs"]
mod fixtures;
use fixtures::*;

#[path = "ranking/labels.rs"]
mod labels;
use labels::*;

#[path = "ranking/samples.rs"]
mod samples;
use samples::*;

#[path = "ranking/scoring.rs"]
mod scoring;
use scoring::*;

#[path = "ranking/execution.rs"]
mod execution;
use execution::*;

#[path = "ranking/measure.rs"]
mod measure;
use measure::*;
