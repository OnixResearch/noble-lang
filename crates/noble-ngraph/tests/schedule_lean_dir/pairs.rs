#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this generator writes small fixed graph pairs as Lean text and must stop at anything outside the exported slice, so expect/assert on the fixed pairs, owner-path imports and string building are the readable form."
)]
//! Fixed graph pairs for the symbolic-equivalence model `NobleFoundation.SymbolicSolver` (issue #56).
//! Each pair (`fixtures`) is built, checked and compiled by `noble-ngraph`. Both graphs run on
//! every candidate of a declared universe, through the compiled machine and the reference
//! interpreter, which must agree. The first candidate whose ordered outputs differ is the Rust
//! verdict. `SymbolicPairCorpus.lean` is generated from these pairs, and there the kernel decides
//! the model's `firstIndex` on the same checked graphs and universe. `shrink` shrinks each refuted
//! pair's reported candidate and generates `SymbolicShrinkCorpus.lean`. Regenerate both after a
//! deliberate change with `cargo test -p noble-ngraph --test schedule_correspondence -- --ignored
//! pairs::write_the_lean_file`, then rerun the foundation lane.

#[path = "pairs/fixtures.rs"]
mod fixtures;
#[path = "pairs/shrink.rs"]
mod shrink;

use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::sem::Value;

use super::emit::{hex, node_term};
use fixtures::{build, Fixture, FIXTURES};

/// The generated file the foundation lane kernel-checks.
const COMMITTED: &str =
    include_str!("../../../../proofs/foundation/NobleFoundation/SymbolicPairCorpus.lean");

/// Both graphs of a fixture, freshly built, checked and compiled.
struct Admitted {
    left: CheckedGraph,
    left_compiled: Compiled,
    right: CheckedGraph,
    right_compiled: Compiled,
}

impl Admitted {
    /// Builds, checks and compiles both graphs of a fixture anew.
    fn new(fixture: &Fixture) -> Self {
        let scheme = super::common::IDS;
        let left = check::check(&scheme, build(fixture.name, fixture.left, fixture.outputs))
            .expect("left checks");
        let right = check::check(&scheme, build(fixture.name, fixture.right, fixture.outputs))
            .expect("right checks");
        let (left_compiled, right_compiled) = (
            Compiled::new(&scheme, &left),
            Compiled::new(&scheme, &right),
        );
        Self {
            left,
            left_compiled,
            right,
            right_compiled,
        }
    }

    /// Whether the two graphs' ordered outputs differ at these inputs.
    fn refutes(&self, inputs: &[i64]) -> bool {
        outputs_at(&self.left, &self.left_compiled, inputs)
            != outputs_at(&self.right, &self.right_compiled, inputs)
    }
}

/// The ordered outputs of one checked graph at one input, through the compiled machine, which
/// must agree with the reference interpreter.
fn outputs_at(checked: &CheckedGraph, compiled: &Compiled, inputs: &[i64]) -> Vec<Value> {
    let values: Vec<Value> = inputs.iter().map(|&value| Value::I64(value)).collect();
    let packed = compiled
        .evaluate(&values, None, &mut Control::default())
        .obs;
    let reference = eval::reference(checked, &values, None, &mut Control::default()).obs;
    assert_eq!(packed, reference, "compiled and reference agree");
    let Obs::Normal(outputs) = packed else {
        panic!("a normal observation")
    };
    outputs
}

/// One checked graph in the model's vocabulary, after checking its outputs sit where the fixture
/// says.
fn graph_term(checked: &CheckedGraph, outputs: &[usize]) -> String {
    let graph = checked.graph();
    let positions: Vec<usize> = graph.outputs.iter().map(|output| output.index()).collect();
    assert_eq!(
        positions, outputs,
        "the checked outputs sit at the fixture's positions"
    );
    let nodes: Vec<String> = graph
        .nodes
        .iter()
        .map(|node| node_term(graph, &node.op))
        .collect();
    format!(
        "FirstProjection.ofList [{}] {}",
        nodes.join(", "),
        outputs[0]
    )
}

/// One witness's exact slot bits as a Lean list.
fn values_term(inputs: &[i64]) -> String {
    let bits: Vec<String> = inputs
        .iter()
        .map(|value| hex(value.cast_unsigned()))
        .collect();
    format!("[{}]", bits.join(", "))
}

/// One fixture as Lean text, with the verdict the Rust graphs give.
fn fixture_text(fixture: &Fixture) -> String {
    let admitted = Admitted::new(fixture);
    let first = fixture
        .universe
        .iter()
        .position(|inputs| admitted.refutes(inputs));
    let candidates: Vec<String> = fixture
        .universe
        .iter()
        .map(|inputs| values_term(inputs))
        .collect();
    let positions: Vec<String> = fixture.outputs.iter().map(usize::to_string).collect();
    let name = fixture.name;
    let mut text = format!(
        "/-! ## `{name}`: {doc} -/\n\n\
         def {name} : Pair :=\n  {{ left := {left},\n    right := {right},\n    outputs := [{positions}] }}\n\n\
         def {name}Values : List (List (BitVec 64)) :=\n  [{candidates}]\n\n\
         def {name}Universe : List (Nat → BitVec 64) :=\n  {name}Values.map inputsOf\n\n\
         theorem {name}_first : firstIndex {name} {name}Universe = {verdict} := by\n  decide\n",
        doc = fixture.doc,
        left = graph_term(&admitted.left, fixture.outputs),
        right = graph_term(&admitted.right, fixture.outputs),
        positions = positions.join(", "),
        candidates = candidates.join(",\n   "),
        verdict = first.map_or_else(|| String::from("none"), |index| format!("some {index}")),
    );
    text.push_str(&match first {
        None => format!(
            "\n/-- The pair agrees on every candidate of the declared universe, and nothing beyond it. -/\n\
             theorem {name}_agrees : ∀ inputs ∈ {name}Universe, agreesOn {name} inputs :=\n  \
             firstIndex_none_agrees {name} {name}Universe {name}_first\n"
        ),
        Some(index) => format!(
            "\n/-- The reported candidate really refutes the pair. -/\n\
             theorem {name}_witness_refutes : refuted {name} ({name}Universe[{index}]'(by decide)) = true := by\n  decide\n"
        ),
    });
    text
}

const HEADER: &str = "import NobleFoundation.SymbolicSolver
import NobleFoundation.SymbolicShrink
import NobleFoundation.FirstProjection

/-!
# Fixed graph pairs from `noble-ngraph`, decided by the kernel (issue #56)

Generated by `crates/noble-ngraph/tests/schedule_lean_dir/pairs.rs`; do not edit by hand.
`pairs::the_lean_file_is_the_real_verdict` fails when the checked graphs or the Rust verdicts change,
until this file is regenerated and the foundation lane rerun.

Each pair's two graphs are built, checked and compiled by `noble-ngraph`. The generator ran both on
every candidate of the declared universe, through the compiled machine and the reference
interpreter, which agree. It took the first candidate whose ordered outputs differ as the Rust
verdict. Here the kernel decides that the model's `firstIndex` over the same checked graphs and
universe gives the same verdict. A pair that agrees does so on the declared universe only, and
nothing beyond it is claimed. Reading checked nodes as model nodes is the one trusted translation.
The shrink of each refuted pair's reported candidate is in `SymbolicShrinkCorpus`.
-/

namespace NobleFoundation.SymbolicSolver.Corpus

open NobleFoundation.PackedSchedule NobleFoundation.SymbolicPair NobleFoundation.SymbolicSolver
open NobleFoundation.SymbolicShrink

";

/// The whole generated Lean module.
fn lean_file() -> String {
    let mut text = String::from(HEADER);
    for fixture in &FIXTURES {
        text.push_str(&fixture_text(fixture));
        text.push('\n');
    }
    text.push_str("end NobleFoundation.SymbolicSolver.Corpus\n");
    text
}

#[test]
fn the_lean_file_is_the_real_verdict() {
    assert!(
        lean_file() == COMMITTED,
        "the checked graphs or verdicts changed: regenerate SymbolicPairCorpus.lean with `cargo test -p noble-ngraph --test schedule_correspondence -- --ignored pairs::write_the_lean_file` and rerun the foundation lane"
    );
}

#[test]
#[ignore = "writes SymbolicPairCorpus.lean and SymbolicShrinkCorpus.lean under proofs/foundation/NobleFoundation; run after a deliberate change"]
fn write_the_lean_file() {
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proofs/foundation/NobleFoundation"
    );
    std::fs::write(format!("{directory}/SymbolicPairCorpus.lean"), lean_file())
        .expect("pair corpus written");
    std::fs::write(
        format!("{directory}/SymbolicShrinkCorpus.lean"),
        shrink::lean_file(),
    )
    .expect("shrink corpus written");
}
