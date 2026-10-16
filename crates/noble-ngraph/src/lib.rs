#![no_std]
//! `Numeric-Graph-v1`: the pure library of the opt-in numeric graph profile
//! (`NumericGraph-Design`, design changes `numeric-graph-profile` and
//! `numeric-graph-v1`).
//!
//! `F32` graphs are authored directly in `ngraph-ir/0` and admitted only by
//! [`check`] (option B0(b)); source extraction exists for `I64` only. The
//! crate is `no_std` + `alloc`, executes no filesystem, process, environment,
//! clock, thread or output effect, and depends only on `noble-kernel` and
//! `noble-contracts`. Identity hashing is injected ([`identity`]).
//!
//! Non-claims: finite tests and evaluator agreement do not prove the
//! extractor (NG-PO-A), the checker (NG-PO-B), the evaluators (NG-PO-C), the
//! binary32 model (NG-PO-D) or any other NG-PO obligation. No performance
//! claim is made.

extern crate alloc;

pub mod check;
pub mod compiled;
pub mod diag;
pub mod eval;
pub mod extract;
pub mod facts;
pub mod format;
pub mod grad;
pub mod identity;
pub mod ir;
pub mod prng;
pub mod sem;
pub mod snapshot;
pub mod train;

mod prelude {
    pub(crate) use alloc::borrow::ToOwned;
    pub(crate) use alloc::boxed::Box;
    pub(crate) use alloc::format;
    pub(crate) use alloc::string::{String, ToString};
    pub(crate) use alloc::vec;
    pub(crate) use alloc::vec::Vec;
}

pub use check::{check, check_with, Budgets, CheckedGraph};
pub use diag::{BudgetKind, Code, Diagnostic, Malformed, Outcome, Site};
pub use eval::{reference, Control, Obs, Report};
pub use identity::{IdentityHasher, IdentityScheme};
pub use ir::{Graph, GraphBuilder, NodeId, Op, ScalarType, SubProfile};
pub use sem::Value;
