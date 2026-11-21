//! Test-only model of the `SMERGE/1` four-version relation (issue #30).
//!
//! This is a bounded reference model and the Phase 3 claim lane, not the
//! production checker and not an integration. The model exercises the
//! per-observable merge, wrapping `I64` equality, conflict/counterexample
//! outcomes and exact subject identity on a tiny stack language. The
//! `claim`/`consumer`/`lean` modules add `noble-merge-claim/v1` regeneration,
//! the consumer's refusals of producer-supplied fields, a replay-validated
//! counterexample search and an isolated check by the pinned Lean model
//! (`proofs/mc1/NobleContracts/Merge.lean`).
//!
//! The recorded decision stays no-go for integration: the production
//! sandboxed consumer is not wired, source-model proof and installed Wasm
//! correspondence stay separate, and the integration plan awaits its
//! independent review. Nothing here runs a prover from `prepare`, merges,
//! publishes, admits a live slot or grants release permission.

#[path = "merge/model.rs"]
mod model;

#[path = "merge/identity.rs"]
mod identity;

#[path = "merge/fixtures.rs"]
mod fixtures;

#[path = "merge/claim.rs"]
mod claim;

#[path = "merge/consumer.rs"]
mod consumer;

#[path = "merge/lean.rs"]
mod lean;

#[path = "merge/verification.rs"]
mod verification;

#[path = "merge/tests.rs"]
mod tests;
