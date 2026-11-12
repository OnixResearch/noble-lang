#![feature(register_tool)]
#![register_tool(octet, tigerstyle)]
#![expect(
    tigerstyle::assertion_density,
    reason = "resource-bound crate: assertions are added where each bound is checked"
)]
#![expect(
    tigerstyle::path_segment_repetition,
    reason = "resource-bound module names mirror the kernel stages they control"
)]
//! Resource-bound controls for the trusted checker.
//!
//! Each control pins one bound the kernel promises a caller: the work limit
//! bounds the checking work it actually performs, substitution stops before an
//! oversized value exists, the calculator rows are canonical, a witness chain
//! spends the request's work, and a witness too deep to copy is refused rather
//! than overflow the stack.

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
#[path = "resource_bounds/witness_fuel.rs"]
mod witness_fuel;
