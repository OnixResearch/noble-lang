#![allow(
    tigerstyle::assertion_density,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-contracts-maintainers; this test-only consumer harness builds finite selections and replays bounded corpora, so long setup legs and fixed-width generator arithmetic are the readable form."
)]
#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the test-only consumer modules import their sibling claim and model types directly so the bounded relation stays readable"
)]
//! Consumer-side checking for the test-only `SMERGE/1` lane: a bounded,
//! replay-validated counterexample search and the outcome discipline that
//! keeps every non-verified state visibly nonverified.
//!
//! The search is not a second semantics and can never certify a merge.  A
//! passing corpus stays `unknown`; a refutation becomes
//! `validated-counterexample` only after the consumer replays the witness and
//! confirms the four exact subjects.  Only Lean acceptance of the regenerated
//! claim yields `verified` (see `claim::checked`).

use super::claim::{admit, checked, Evidence, Observable, Outcome, Refusal, Selection};
use super::model::{combined_value, run, Precondition};

/// Declared work bound of one search: at most this many input vectors.
pub(super) const MAX_VECTORS: usize = 25;

/// A validated input/position where the candidate differs from the merge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Counterexample {
    pub(super) input: Vec<u64>,
    pub(super) position: usize,
    pub(super) expected: u64,
    pub(super) actual: u64,
    pub(super) seed: u64,
}

/// One vector's replay result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Witness {
    /// The four subjects returned the declared stack and the candidate equals
    /// the merge at every position.
    Agrees,
    /// A validated input/position where the candidate differs from the merge.
    Differs(Counterexample),
    /// The merge itself is undefined at this input.
    Conflict { input: Vec<u64>, position: usize },
    /// The input or one subject is outside the declared interface/fragment, so
    /// this vector is not a witness.
    Refused(Refusal),
}

/// The bounded search result.  `truncated` means the bound stopped the search
/// before the corpus was exhausted, and it must never be reported as verified.
#[derive(Clone, Debug)]
pub(super) struct Search {
    pub(super) outcome: Outcome,
    pub(super) counterexample: Option<Counterexample>,
    pub(super) examined: usize,
    pub(super) truncated: bool,
    pub(super) seed: u64,
}

/// Deterministic 64-bit generator for the bounded corpus.  The seed and the
/// revision are retained so the search is reproducible.
fn next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}

/// The declared input types of the test model are `I64`; a vector of another
/// width is outside the selected interface and a non-`I64` observable is
/// outside what this lane's corpus can witness.
fn input_admissible(selection: &Selection, input: &[u64]) -> Result<(), Refusal> {
    if input.len() != selection.input.len() {
        return Err(Refusal::IncompatibleInterface);
    }
    let mut at = 0usize;
    while at < selection.input.len() {
        if selection.input[at] != Observable::I64 {
            return Err(Refusal::UnsupportedObservable);
        }
        at += 1;
    }
    Ok(())
}

/// The consumer's independent replay of one input vector: it re-checks the
/// input types, re-identifies the four subjects against its own selection and
/// executes each subject on a fresh stack.
pub(super) fn replay(selection: &Selection, input: &[u64], seed: u64) -> Witness {
    if let Err(refusal) = input_admissible(selection, input) {
        return Witness::Refused(refusal);
    }
    let mut observations = Vec::with_capacity(4);
    let mut order = 0usize;
    for preparation in [
        &selection.base,
        &selection.first,
        &selection.second,
        &selection.candidate,
    ] {
        let Some(output) = run(&preparation.program, input) else {
            return Witness::Refused(Refusal::UnsupportedFragment);
        };
        if output.len() != selection.output.len() {
            return Witness::Refused(Refusal::IncompatibleInterface);
        }
        observations.push(output);
        order += 1;
    }
    debug_assert!(order == 4);
    let base = &observations[0];
    let first = &observations[1];
    let second = &observations[2];
    let candidate = &observations[3];
    let mut position = 0usize;
    while position < selection.output.len() {
        let Some(expected) = combined_value(base[position], first[position], second[position])
        else {
            return Witness::Conflict {
                input: input.to_vec(),
                position,
            };
        };
        if candidate[position] != expected {
            return Witness::Differs(Counterexample {
                input: input.to_vec(),
                position,
                expected,
                actual: candidate[position],
                seed,
            });
        }
        position += 1;
    }
    Witness::Agrees
}

/// Bounded search over a reproducible corpus.  Only a validated mismatch
/// becomes a counterexample; a conflict is reported as `conflict`, and an
/// exhausted or passing corpus stays nonverified.
pub(super) fn search(selection: &Selection, seed: u64, vectors: usize) -> Search {
    let bound = core::cmp::min(vectors, MAX_VECTORS);
    let mut state = seed;
    let mut examined = 0usize;
    while examined < bound {
        let mut input = Vec::with_capacity(selection.input.len());
        let mut at = 0usize;
        while at < selection.input.len() {
            input.push(next(&mut state));
            at += 1;
        }
        examined += 1;
        match replay(selection, &input, seed) {
            Witness::Differs(counterexample) => {
                // Independent replay: the consumer re-executes the witness
                // before it retains the refutation.
                if replay(selection, &counterexample.input, seed)
                    != Witness::Differs(counterexample.clone())
                {
                    return Search {
                        outcome: Outcome::InternalFailure,
                        counterexample: None,
                        examined,
                        truncated: false,
                        seed,
                    };
                }
                return Search {
                    outcome: Outcome::ValidatedCounterexample,
                    counterexample: Some(counterexample),
                    examined,
                    truncated: false,
                    seed,
                };
            }
            Witness::Conflict { .. } => {
                return Search {
                    outcome: Outcome::Conflict,
                    counterexample: None,
                    examined,
                    truncated: false,
                    seed,
                };
            }
            Witness::Refused(Refusal::IncompatibleInterface) => {
                return Search {
                    outcome: Outcome::Unsupported,
                    counterexample: None,
                    examined,
                    truncated: false,
                    seed,
                };
            }
            Witness::Refused(_) => {
                return Search {
                    outcome: Outcome::Unsupported,
                    counterexample: None,
                    examined,
                    truncated: false,
                    seed,
                };
            }
            Witness::Agrees => {}
        }
    }
    Search {
        outcome: Outcome::Unknown,
        counterexample: None,
        examined,
        truncated: vectors > MAX_VECTORS,
        seed,
    }
}

/// The complete consumer path for one submission:
/// 1. admit (regenerate + refuse differing producer fields),
/// 2. let a validated refutation or an unsupported subject dominate,
/// 3. otherwise ask the isolated pinned Lean consumer about the exact claim.
///
/// `verify` is the isolated Lean check; it receives only the regenerated claim
/// text and the admitted proof bytes.
pub(super) fn evaluate(
    selection: &Selection,
    evidence: &Evidence,
    search: &Search,
    verify: impl Fn(&str, &str) -> (bool, Vec<String>),
) -> Outcome {
    let claim = match admit(selection, evidence) {
        Ok(claim) => claim,
        Err(refusal) => return refusal.outcome(),
    };
    if evidence.claimed_outcome == Outcome::Verified {
        // A producer must never pre-mark a submission as verified.
        return Refusal::WrongOutcome.outcome();
    }
    if search.outcome != Outcome::Unknown {
        return search.outcome;
    }
    let (accepted, axioms) = verify(&claim.claim_text, &evidence.proof);
    checked(&claim, accepted, &axioms)
}

/// A claim whose precondition is `true` and whose input stack has at least one
/// observable has a witness; a conditional or empty-domain claim is refused
/// before any proof is considered.
pub(super) fn has_witness(selection: &Selection) -> bool {
    selection.precondition == Precondition::True && !selection.input.is_empty()
}
