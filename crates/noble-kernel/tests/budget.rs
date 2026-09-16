// r[verify VT-M1-02]
#[test]
fn positive_budget_decrements_once() {
    for (remaining, expected) in [(1, 0), (2, 1), (42, 41), (u32::MAX, 4_294_967_294)] {
        assert!(matches!(
            noble_kernel::consume_budget(remaining),
            noble_kernel::BudgetOutcome::Remaining(next) if next == expected
        ));
    }
}

// r[verify VT-M1-02]
#[test]
fn zero_budget_is_exhausted() {
    assert!(matches!(
        noble_kernel::consume_budget(0),
        noble_kernel::BudgetOutcome::Exhausted
    ));
}
