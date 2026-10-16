//! Issue #14 `NumericGraph-Design` Phase B/C evidence harness.
//!
//! The graph library itself is `noble-ngraph` (`Numeric-Graph-v1`); this
//! crate keeps the measurement and evidence legs that need host effects or
//! third-party crates: the receipt bins, the L1 CLI harness, the `I64` L2
//! Node oracle, the `rustc_apfloat` binary32 oracle, workloads, fixtures,
//! benches and the source-level controls. It injects the BLAKE3 identity
//! scheme ([`scheme`]) into the library.
//!
//! Non-claims: finite tests and evaluator agreement do not prove the
//! compiler, the checker (NG-PO-B), the evaluators (NG-PO-C) or the binary32
//! model (NG-PO-D) correct. NG-PO-A..H stay open.

pub mod b2i64;
pub mod bench;
pub mod controls;
pub mod differential;
pub mod evidence;
pub mod fixtures;
pub mod json;
pub mod l1;
pub mod oracle;
pub mod random;
pub mod rejections;
pub mod scheme;
pub mod suites;
pub mod workloads;
