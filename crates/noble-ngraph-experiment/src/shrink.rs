//! Deterministic bounded shrinking for a binary32 expectation discrepancy
//! (issue #38).
//!
//! The authority and the candidate are two binary32 evaluators for one
//! operation. Given an operand pair that disagrees, the shrinker walks a fixed,
//! documented sequence of simplifications and keeps a simplification only while
//! the disagreement survives. The result is deterministic, bounded by an
//! attempt budget, and replayable: re-running the candidate and the authority on
//! the witness reproduces the same expected/candidate encodings.
//!
//! A witness shrunk here is *not* itself kernel evidence. A witness promoted to
//! evidence must first become a retained expectation that the Lean reference
//! checks; this lane only produces the minimized, replayable operand pair.

/// Simplifications tried, in this order, for each operand. Every entry is a
/// fixed bit pattern or a fixed mask, so two runs over the same start pair
/// produce the same witness.
const SIMPLIFICATIONS: [u32; 10] = [
    0x0000_0000, // +0
    0x8000_0000, // -0
    0x0000_0001, // smallest subnormal
    0x3f80_0000, // 1.0
    0x8000_0001, // smallest negative subnormal
    0x007f_ffff, // largest subnormal
    0x0080_0000, // smallest normal
    0x7f80_0000, // infinity
    0x7fc0_0000, // canonical NaN
    0x3f00_0000, // 0.5
];

/// The documented simplicity order of one encoding: fewer set bits first, then
/// the smaller pattern. A simplification is accepted only when it is strictly
/// simpler under this order, which makes the walk monotone and terminating.
const fn simplicity(bits: u32) -> (u32, u32) {
    (bits.count_ones(), bits)
}

/// Whether the pair is strictly simpler than `other` under `simplicity`.
const fn simpler_than(pair: (u32, u32), other: (u32, u32)) -> bool {
    let (pa, pb) = (simplicity(pair.0), simplicity(pair.1));
    let (oa, ob) = (simplicity(other.0), simplicity(other.1));
    if pa.0 == oa.0 && pb.0 == ob.0 {
        pa.1 <= oa.1 && pb.1 <= ob.1 && (pa.1 < oa.1 || pb.1 < ob.1)
    } else {
        pa.0 + pb.0 < oa.0 + ob.0
    }
}

/// How many leading simplifications the joint phase combines on both operands.
const JOINT_OPERANDS: usize = 4;

/// One minimized disagreement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Witness {
    /// The selected operation: `0` add, `1` subtract, `2` multiply.
    pub operation: u8,
    /// Operand `left` encoding that still disagrees, after shrinking.
    pub left: u32,
    /// Operand `right` encoding that still disagrees, after shrinking.
    pub right: u32,
    /// The authority's answer for the witness.
    pub expected: u32,
    /// The candidate's answer for the witness.
    pub candidate: u32,
    /// How many candidate simplifications were evaluated, within the budget.
    pub attempts: usize,
    /// Whether the shrinker stopped because the budget ran out with more
    /// simplifications pending.
    pub budget_exhausted: bool,
}

/// The bound on evaluated simplifications.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Budget {
    /// Maximum number of simplifications to evaluate.
    pub attempts: usize,
}

impl Budget {
    /// A documented default: ten simplifications per operand is 20 evaluations
    /// per pass and the joint phase adds at most 16, so 200 evaluations bound a
    /// walk that terminates within ten improving passes.
    #[must_use]
    pub const fn default_attempts() -> Self {
        Self { attempts: 200 }
    }
}

/// Shrink a disagreement between `authority` and `candidate` for `operation`.
///
/// Returns `None` when the starting pair already agrees: a shrinker must never
/// invent a witness. The disagreement predicate is exactly "the two answers
/// differ as encodings", so the disagreement class is preserved by construction.
///
/// The result is a **local** minimum under the documented simplicity order and
/// the documented simplification list: no single-slot or joint simplification in
/// that list still disagrees. It is not a global minimum, and a pair that needs a
/// simplification outside the list is reported unchanged rather than invented.
#[must_use]
pub fn minimize(
    operation: u8,
    authority: &dyn Fn(u8, u32, u32) -> u32,
    candidate: &dyn Fn(u8, u32, u32) -> u32,
    start: (u32, u32),
    budget: Budget,
) -> Option<Witness> {
    let disagrees = |pair: (u32, u32)| {
        authority(operation, pair.0, pair.1) != candidate(operation, pair.0, pair.1)
    };
    if !disagrees(start) {
        return None;
    }
    let mut current = start;
    let mut attempts = 0;
    let mut improved = true;
    let mut budget_exhausted = false;
    while improved && !budget_exhausted {
        improved = false;
        for slot in 0..2u8 {
            for simplification in SIMPLIFICATIONS {
                if attempts >= budget.attempts {
                    budget_exhausted = true;
                    break;
                }
                attempts += 1;
                let candidate_pair = if slot == 0 {
                    (simplification, current.1)
                } else {
                    (current.0, simplification)
                };
                if simpler_than(candidate_pair, current) && disagrees(candidate_pair) {
                    current = candidate_pair;
                    improved = true;
                }
            }
            if budget_exhausted {
                break;
            }
        }
        if !improved {
            // Some disagreements need both operands simplified at once: an
            // operand pair can stay agreeing while either slot alone changes.
            // The joint phase tries the documented cross product, still bounded
            // by the same attempt budget.
            for left in SIMPLIFICATIONS.into_iter().take(JOINT_OPERANDS) {
                for right in SIMPLIFICATIONS.into_iter().take(JOINT_OPERANDS) {
                    if attempts >= budget.attempts {
                        budget_exhausted = true;
                        break;
                    }
                    attempts += 1;
                    if simpler_than((left, right), current) && disagrees((left, right)) {
                        current = (left, right);
                        improved = true;
                    }
                }
                if budget_exhausted {
                    break;
                }
            }
        }
        if !improved {
            break;
        }
    }
    let (left, right) = current;
    Some(Witness {
        operation,
        left,
        right,
        expected: authority(operation, left, right),
        candidate: candidate(operation, left, right),
        attempts,
        budget_exhausted,
    })
}

/// Replay a witness: `true` when the two evaluators still disagree on it with the
/// recorded encodings. A witness that no longer replays is stale evidence and
/// must not be retained.
#[must_use]
pub fn replays(
    witness: &Witness,
    authority: &dyn Fn(u8, u32, u32) -> u32,
    candidate: &dyn Fn(u8, u32, u32) -> u32,
) -> bool {
    let expected = authority(witness.operation, witness.left, witness.right);
    let observed = candidate(witness.operation, witness.left, witness.right);
    expected == witness.expected && observed == witness.candidate && expected != observed
}
