//! Issue #14 `NumericGraph-Design` Phase B/C experiment crate.
//!
//! Isolated (approval P3): it does not modify, and is not used by, the
//! ordinary Noble compiler path. `F32` graphs are authored directly in
//! `ngraph-ir/0` and checked only by [`check`] (approval P2, option B0(b));
//! source extraction exists for `I64` only.
//!
//! Non-claims: finite tests and evaluator agreement do not prove the
//! compiler, the checker (NG-PO-B), the evaluators (NG-PO-C) or the binary32
//! model (NG-PO-D) correct. NG-PO-A..H stay open.

pub mod check;
pub mod diag;
pub mod eval;
pub mod facts;
pub mod ir;
pub mod sem;
pub mod snapshot;

pub use check::{check, check_with, Budgets, CheckedGraph};
pub use diag::{BudgetKind, Code, Diagnostic, Malformed, Site};
pub use eval::{reference, Control, Obs, Report};
pub use ir::{Graph, GraphBuilder, NodeId, Op, ScalarType, SubProfile};
pub use sem::Value;
