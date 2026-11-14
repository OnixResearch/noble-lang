//! Completion request, deterministic search and fixture controls
//! (TCHUNK-02, TCHUNK-04).
//!
//! Fixture expectations are written independently of chunk bodies, and fixture
//! agreement is asserted separately from acceptance.

#[path = "chunks-completion/support/mod.rs"]
mod support;

#[path = "chunks-completion/budget/expansion.rs"]
mod budget_expansion;
#[path = "chunks-completion/budget/snapshot.rs"]
mod budget_snapshot;
#[path = "chunks-completion/core.rs"]
mod core;
#[path = "chunks-completion/fixture/limits.rs"]
mod fixture_limits;
#[path = "chunks-completion/fixtures.rs"]
mod fixtures;
#[path = "chunks-completion/instantiation.rs"]
mod instantiation;
#[path = "chunks-completion/requests.rs"]
mod requests;
#[path = "chunks-completion/signatures.rs"]
mod signatures;
