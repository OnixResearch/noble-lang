#![feature(register_tool)]
#![register_tool(octet, tigerstyle)]
#![expect(
    tigerstyle::assertion_density,
    reason = "this target also owns the resource-bound controls, whose scenarios add their assertions where the bound is checked"
)]
//! Budget and resource-bound controls for the trusted kernel.
//!
//! The first two controls pin the call budget. The `resource_bounds` modules
//! pin the bounds the kernel promises a caller: the work limit bounds the
//! checking work it actually performs, substitution stops before an oversized
//! value exists, the calculator rows are canonical, a witness chain spends the
//! request's work, and a witness too deep to copy is refused rather than
//! overflow the stack. They share this target because the derived compiler
//! architecture admits only a fixed number of units.
//!
//! Every bound is stated against the environment the kernel builds itself, so
//! the controls name the scenario and the refusal rather than restating rules.

#[path = "resource_bounds/builders.rs"]
mod builders;
#[path = "resource_bounds/calculator_rows.rs"]
mod calculator_rows;
#[path = "resource_bounds/deep_values.rs"]
mod deep_values;
#[path = "resource_bounds/subst_bound.rs"]
mod subst_bound;
#[path = "resource_bounds/type_walk.rs"]
mod type_walk;
#[path = "resource_bounds/type_walk_cost.rs"]
mod type_walk_cost;
#[path = "resource_bounds/witness_fuel.rs"]
mod witness_fuel;

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
