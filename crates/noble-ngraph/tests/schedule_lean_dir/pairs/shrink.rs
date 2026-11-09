//! Shrinking each refuted pair's reported candidate toward zero, one slot at a time, for
//! `NobleFoundation.SymbolicShrink` (issue #56, acceptance item 2).
//! - **Afresh.** Every candidate shrink is decided afresh: both graphs are built, checked and
//!   compiled again, and run through both evaluators.
//! - **Kept.** A shrink is kept only while it still refutes.
//! - **Replayed.** The finished trace is replayed afresh, including every single-slot move of its
//!   last witness, which must agree.
//!
//! `SymbolicShrinkCorpus.lean` is generated from the traces, and there the kernel replays each one
//! with the model's own shrinker. `pairs::write_the_lean_file` regenerates it with the pair corpus.

use super::fixtures::{Fixture, FIXTURES};
use super::{values_term, Admitted};

/// The generated file the foundation lane kernel-checks.
pub(super) const COMMITTED: &str =
    include_str!("../../../../../proofs/foundation/NobleFoundation/SymbolicShrinkCorpus.lean");

/// The most witnesses one shrink trace may hold; the fixed pairs need far fewer.
const MAX_SHRINK_COUNT: usize = 128;

/// Whether the inputs refute the fixture's pair, decided afresh: both graphs are built, checked
/// and compiled again, and run through the compiled machine and the reference interpreter, which
/// must agree.
fn is_refuted_afresh(fixture: &Fixture, inputs: &[i64]) -> bool {
    Admitted::new(fixture).refutes(inputs)
}

/// One slot's moves, nearest zero first: zero, the value halved toward zero, and the value one
/// step toward zero, without the value itself (`SymbolicShrink.moves`).
fn moves(value: i64) -> Vec<i64> {
    let halved = value
        .checked_div(2)
        .expect("halving an i64 never overflows");
    let toward_zero = value
        .checked_sub(value.signum())
        .expect("one step toward zero never overflows");
    [0, halved, toward_zero]
        .into_iter()
        .filter(|&candidate| candidate != value)
        .collect()
}

/// Every single-slot move of a witness, slot by slot in order (`SymbolicShrink.neighbours`).
fn neighbours(values: &[i64]) -> Vec<Vec<i64>> {
    values
        .iter()
        .enumerate()
        .flat_map(|(slot, &value)| {
            moves(value).into_iter().map(move |candidate| {
                let mut next = values.to_vec();
                next[slot] = candidate;
                next
            })
        })
        .collect()
}

/// A refuting witness followed by its shrinks: each step takes the first single-slot move that
/// still refutes the pair, decided afresh, until none does (`SymbolicShrink.shrinkTrace`).
fn trace_from(fixture: &Fixture, witness: &[i64]) -> Vec<Vec<i64>> {
    let mut trace = vec![witness.to_vec()];
    while let Some(next) = neighbours(trace.last().expect("a trace starts at its witness"))
        .into_iter()
        .find(|candidate| is_refuted_afresh(fixture, candidate))
    {
        assert!(
            trace.len() < MAX_SHRINK_COUNT,
            "the shrink of a fixed pair stays within its cap"
        );
        trace.push(next);
    }
    trace
}

/// The Lean text for one refuted pair's shrink trace, which replays in the kernel and ends at a
/// local minimum.
fn trace_text(fixture: &Fixture, index: usize) -> String {
    let trace = trace_from(fixture, fixture.universe[index]);
    assert!(
        trace
            .iter()
            .all(|inputs| is_refuted_afresh(fixture, inputs)),
        "every kept shrink replays as a refutation"
    );
    let (last, kept) = trace.split_last().expect("a trace starts at its witness");
    assert!(
        neighbours(last)
            .iter()
            .all(|inputs| !is_refuted_afresh(fixture, inputs)),
        "the shrink ends at a local minimum"
    );
    let steps: String = kept
        .iter()
        .map(|inputs| format!("{},\n   ", values_term(inputs)))
        .collect();
    let name = fixture.name;
    format!(
        "/-! ## `{name}` -/\n\n\
         /-- The generator's shrink of the reported candidate ends here. -/\n\
         def {name}Shrunk : List (BitVec 64) :=\n  {last}\n\n\
         /-- The reported candidate and every shrink the generator kept, in order. -/\n\
         def {name}Shrinks : List (List (BitVec 64)) :=\n  [{steps}{name}Shrunk]\n\n\
         /-- The model's shrinker, started at the reported candidate, replays the generator's trace exactly and stops at its end. -/\n\
         theorem {name}_shrinks_replay :\n    shrinkTrace {name} ({name}Values[{index}]'(by decide)) {count} = {name}Shrinks := by\n  decide\n\n\
         /-- Every shrink of the reported candidate still refutes the pair. -/\n\
         theorem {name}_every_shrink_refutes :\n    ∀ values ∈ {name}Shrinks, refuted {name} (inputsOf values) = true := by\n  decide\n\n\
         /-- Every single-slot move of the last shrink agrees with the pair: a local minimum. -/\n\
         theorem {name}_shrunk_is_a_local_minimum :\n    ∀ values ∈ neighbours {name}Shrunk, agreesOn {name} (inputsOf values) :=\n  \
         shrinkStep_none_minimal {name} {name}Shrunk (by decide)\n",
        last = values_term(last),
        count = trace.len(),
    )
}

const HEADER: &str = "import NobleFoundation.SymbolicPairCorpus
import NobleFoundation.SymbolicShrink

/-!
# Shrunk witnesses for the fixed graph pairs, replayed by the kernel (issue #56)

Generated by `crates/noble-ngraph/tests/schedule_lean_dir/pairs/shrink.rs`; do not edit by hand.
`pairs::shrink::the_lean_file_is_the_real_trace` fails when a shrink trace changes, until this file
is regenerated and the foundation lane rerun.

For each refuted pair of `SymbolicPairCorpus`, the generator shrank the reported candidate toward
zero, one slot at a time. It decided every candidate shrink afresh, building, checking and compiling
both graphs again and running both evaluators, and it kept a shrink only while it still refuted.
The kernel replays that trace with the model's own shrinker (`SymbolicShrink.shrinkTrace`). It then
decides that every kept shrink refutes, and that every single-slot move of the last one agrees: a
local minimum, not a smallest witness over all inputs.
-/

namespace NobleFoundation.SymbolicSolver.Corpus

open NobleFoundation.SymbolicPair NobleFoundation.SymbolicShrink

";

/// The whole generated shrink module: one trace per refuted pair, in fixture order.
pub(super) fn lean_file() -> String {
    let mut text = String::from(HEADER);
    for fixture in &FIXTURES {
        let admitted = Admitted::new(fixture);
        if let Some(index) = fixture
            .universe
            .iter()
            .position(|inputs| admitted.refutes(inputs))
        {
            text.push_str(&trace_text(fixture, index));
            text.push('\n');
        }
    }
    text.push_str("end NobleFoundation.SymbolicSolver.Corpus\n");
    text
}

#[test]
fn the_lean_file_is_the_real_trace() {
    assert!(
        lean_file() == COMMITTED,
        "a shrink trace changed: regenerate SymbolicShrinkCorpus.lean with `cargo test -p noble-ngraph --test schedule_correspondence -- --ignored pairs::write_the_lean_file` and rerun the foundation lane"
    );
}
