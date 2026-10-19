//! Test-only model of the `SMERGE/1` four-version relation (issue #30).
//!
//! This is a bounded reference model, not the Lean checker and not an
//! integration. It exercises the per-observable merge, wrapping `I64`
//! equality, conflict/counterexample outcomes, exact subject identity and the
//! claim/evidence refusals on a tiny stack language. The Lean obligation and
//! claim generator remain open; the recorded decision is no-go for integration.

#[path = "merge/model.rs"]
mod model;

#[path = "merge/identity.rs"]
mod identity;

#[path = "merge/fixtures.rs"]
mod fixtures;

#[path = "merge/tests.rs"]
mod tests;
