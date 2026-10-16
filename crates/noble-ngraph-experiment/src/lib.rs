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

pub mod b2i64;
pub mod bench;
pub mod check;
pub mod compiled;
pub mod diag;
pub mod differential;
pub mod eval;
pub mod evidence;
pub mod extract;
pub mod facts;
pub mod fixtures;
pub mod grad;
pub mod ir;
pub mod json;
pub mod l1;
pub mod oracle;
pub mod prng;
pub mod random;
pub mod rejections;
pub mod sem;
pub mod snapshot;
pub mod suites;
pub mod text;
pub mod train;
pub mod workloads;

pub use check::{check, check_with, Budgets, CheckedGraph};
pub use diag::{BudgetKind, Code, Diagnostic, Malformed, Site};
pub use eval::{reference, Control, Obs, Report};
pub use ir::{Graph, GraphBuilder, NodeId, Op, ScalarType, SubProfile};
pub use sem::Value;
