//! Solver models for the fixed graph pairs, replayed afresh (issue #56, plan item 3).
//! `verification/symbolic/solver.mjs` writes every distinct model its untrusted solvers gave for
//! a corpus pair to a witness file. A line reads `refutes` when the lane's own evaluator replayed
//! the model as a refutation of the original graphs, and `rejects` when it did not, as for a model
//! of a corrupted encoding.
//!
//! Here each line is decided again: both graphs are built, checked and compiled afresh and run
//! through the compiled machine and the reference interpreter, which must agree, and the verdict
//! must be the line's. `SymbolicSolverReplay.lean` is generated from the lines, and there the
//! kernel decides each one on the model. `pairs::write_the_lean_file` regenerates it.

use super::fixtures::{Fixture, FIXTURES};
use super::{values_term, Admitted};

/// The witness file of the solver lane's retained receipt.
const WITNESSES: &str =
    include_str!("../../../../../verification/symbolic/receipts/2026-11-09/witnesses.txt");

/// The generated file the foundation lane kernel-checks.
pub(super) const COMMITTED: &str =
    include_str!("../../../../../proofs/foundation/NobleFoundation/SymbolicSolverReplay.lean");

/// The most slots a line may name; the fixed pairs use at most two.
const MAX_SLOTS: usize = 16;

/// One witness line: the lane's verdict, the pair, and the model's exact slot bits, with every
/// unnamed slot zero, as the lane and `SymbolicShrink.inputsOf` read it.
struct Line {
    is_refuting: bool,
    fixture: &'static Fixture,
    inputs: Vec<i64>,
}

/// One slot word, `sN=0x` followed by exactly sixteen hex digits.
fn slot_word(word: &str) -> (usize, i64) {
    let (name, bits) = word.split_once('=').expect("a slot word is name=bits");
    let slot = name
        .strip_prefix('s')
        .and_then(|digits| digits.parse::<usize>().ok())
        .expect("a slot is named sN");
    let digits = bits.strip_prefix("0x").expect("slot bits are hex");
    assert!(
        slot < MAX_SLOTS,
        "a witness names at most {MAX_SLOTS} slots"
    );
    assert_eq!(digits.len(), 16, "a slot holds exactly 64 bits");
    let value = u64::from_str_radix(digits, 16).expect("slot bits parse as hex");
    (slot, value.cast_signed())
}

fn parse(line: &str) -> Line {
    let mut words = line.split_whitespace();
    let is_refuting = match words.next() {
        Some("refutes") => true,
        Some("rejects") => false,
        other => panic!("unknown witness verdict {other:?}"),
    };
    let name = words.next().expect("a witness names its pair");
    let fixture = FIXTURES
        .iter()
        .find(|fixture| fixture.name == name)
        .expect("a witness names a corpus pair");
    let mut inputs = Vec::new();
    for word in words {
        let (slot, value) = slot_word(word);
        if inputs.len() <= slot {
            inputs.resize(slot + 1, 0);
        }
        inputs[slot] = value;
    }
    assert!(!inputs.is_empty(), "a witness names at least one slot");
    Line {
        is_refuting,
        fixture,
        inputs,
    }
}

/// Every witness line, each decided afresh on both Rust evaluators against the lane's verdict.
fn lines() -> Vec<Line> {
    let parsed: Vec<Line> = WITNESSES
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(parse)
        .collect();
    for line in &parsed {
        assert_eq!(
            Admitted::new(line.fixture).refutes(&line.inputs),
            line.is_refuting,
            "both Rust evaluators give the lane's verdict for a {} model",
            line.fixture.name
        );
    }
    assert!(
        parsed.iter().any(|line| line.is_refuting),
        "the witness file holds refuting models"
    );
    assert!(
        parsed.iter().any(|line| !line.is_refuting),
        "the witness file holds rejected models"
    );
    parsed
}

/// The Lean text for one pair's models of one verdict, if it has any.
fn group_text(fixture: &Fixture, lines: &[Line], is_refuting: bool) -> String {
    let values: Vec<String> = lines
        .iter()
        .filter(|line| line.fixture.name == fixture.name && line.is_refuting == is_refuting)
        .map(|line| values_term(&line.inputs))
        .collect();
    if values.is_empty() {
        return String::new();
    }
    let name = fixture.name;
    let list = values.join(",\n   ");
    if is_refuting {
        format!(
            "/-- Solver models that refute `{name}`, each replayed on both Rust evaluators. -/\n\
             def {name}SolverRefuting : List (List (BitVec 64)) :=\n  [{list}]\n\n\
             /-- The kernel decides that every one refutes the pair on the model. -/\n\
             theorem {name}_solver_models_refute :\n    ∀ values ∈ {name}SolverRefuting, refuted {name} (inputsOf values) = true := by\n  decide\n\n"
        )
    } else {
        format!(
            "/-- Models of corrupted encodings that the lane rejected for `{name}`, each replayed on both Rust evaluators. -/\n\
             def {name}SolverRejected : List (List (BitVec 64)) :=\n  [{list}]\n\n\
             /-- The kernel decides that none of them refutes the pair: the rejection was right. -/\n\
             theorem {name}_rejected_models_agree :\n    ∀ values ∈ {name}SolverRejected, refuted {name} (inputsOf values) = false := by\n  decide\n\n"
        )
    }
}

const HEADER: &str = "import NobleFoundation.SymbolicPairCorpus

/-!
# Solver models for the fixed graph pairs, replayed by the kernel (issue #56)

Generated by `crates/noble-ngraph/tests/schedule_lean_dir/pairs/replay.rs` from
`verification/symbolic/receipts/2026-11-09/witnesses.txt`; do not edit by hand.
`pairs::replay::the_lean_file_matches_the_witnesses` fails when the witness file or a verdict
changes, until this file is regenerated and the foundation lane rerun.

The untrusted solvers of `verification/symbolic/solver.mjs` gave these models. Each refuting model
replayed on the original graphs through the lane's evaluator; each rejected one, a model of a
corrupted encoding, did not. The generator decided every model again on both Rust evaluators,
building, checking and compiling both graphs afresh, and here the kernel decides it on the model.
-/

namespace NobleFoundation.SymbolicSolver.Corpus

open NobleFoundation.SymbolicPair NobleFoundation.SymbolicShrink

";

/// The whole generated replay module, pair by pair in fixture order.
pub(super) fn lean_file() -> String {
    let lines = lines();
    let mut text = String::from(HEADER);
    for fixture in &FIXTURES {
        let groups = format!(
            "{}{}",
            group_text(fixture, &lines, true),
            group_text(fixture, &lines, false)
        );
        if !groups.is_empty() {
            text.push_str(&format!("/-! ## `{}` -/\n\n{groups}", fixture.name));
        }
    }
    text.push_str("end NobleFoundation.SymbolicSolver.Corpus\n");
    text
}

#[test]
fn the_lean_file_matches_the_witnesses() {
    assert!(
        lean_file() == COMMITTED,
        "the witness file or a verdict changed: regenerate SymbolicSolverReplay.lean with `cargo test -p noble-ngraph --test schedule_correspondence -- --ignored pairs::write_the_lean_file` and rerun the foundation lane"
    );
}
