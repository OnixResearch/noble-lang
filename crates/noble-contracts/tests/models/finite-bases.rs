//! Test-only prototype for the `finite-bases/v0` study (issue #29).
//!
//! The prototype keeps the selected relation deliberately small: data types
//! are reflexive only, nominal identities are exact, and the only
//! non-reflexive relation is Program effect-bound inclusion. It compares an
//! explicit-family checker with a target-directed checker over the same
//! declarative judgments and records the principal-candidate result.
//!
//! It does not change source acceptance, the kernel type encoding or the ABI.

#[path = "finite/relation.rs"]
mod relation;

#[path = "finite/candidates.rs"]
mod candidates;

#[path = "finite/tests.rs"]
mod tests;
