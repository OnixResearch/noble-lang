#![no_std]
//! Deterministic internal transitions. This crate does not evaluate Noble programs.

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
