#![no_std]
//! Deterministic internal transitions. This crate does not evaluate Noble programs.

extern crate alloc;

/// Evaluate one fallible step, returning its failure from the enclosing function.
///
/// This stands in for the `?` operator: the Octet architecture collector marks
/// `Desugaring(QuestionMark)` as an unsupported expansion, while a crate-local
/// macro expansion resolves to this definition.
macro_rules! attempt {
    ($step:expr) => {
        match $step {
            Ok(value) => value,
            Err(failure) => return Err(failure),
        }
    };
}

pub mod acceptance;
pub mod contracts;
pub mod shapes;
pub mod types;
pub mod untrusted;
pub mod words;

/// Outcome of consuming one unit from an explicitly supplied budget.
pub enum BudgetOutcome {
    Remaining(u32),
    Exhausted,
}

/// Consume one unit without underflow or an external effect.
// r[impl VT-M1-02]
pub fn consume_budget(remaining: u32) -> BudgetOutcome {
    match remaining.checked_sub(1) {
        Some(next) => BudgetOutcome::Remaining(next),
        None => BudgetOutcome::Exhausted,
    }
}
