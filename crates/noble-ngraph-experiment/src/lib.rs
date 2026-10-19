#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the evidence harness imports its own modules and domain types directly for readability"
)]
#![expect(
    tigerstyle::platform_dependent_cast,
    reason = "Owner: noble-maintainers; supported targets have usize >= 32 bits and the flagged casts are host-side u32 indices"
)]
#![expect(
    tigerstyle::explicit_defaults,
    reason = "Owner: noble-maintainers; harness receipts intentionally spell defaulted fields at their construction sites"
)]
#![expect(
    tigerstyle::usize_in_public_api,
    reason = "Owner: noble-maintainers; host-side harness rows and workload sizes use usize and are not guest ABI types"
)]
#![expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; harness vectors are bounded by workload, corpus or receipt row counts"
)]
#![expect(
    tigerstyle::ignored_result,
    reason = "Owner: noble-maintainers; receipt and JSON writes target caller-owned String buffers whose fmt::Write is infallible"
)]
#![expect(
    tigerstyle::function_length,
    reason = "Owner: noble-maintainers; harness legs remain linear so their measurement order is reviewable"
)]
#![expect(
    tigerstyle::excessive_file_length,
    reason = "Owner: noble-maintainers; each harness file is one evidence/review unit"
)]
#![expect(
    tigerstyle::no_panic,
    reason = "Owner: noble-maintainers; harness invariants are checked before the diagnostic panic at the measurement boundary"
)]
#![expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; module paths name the harness leg and item names retain their report spelling"
)]
#![expect(
    tigerstyle::bool_naming,
    reason = "Owner: noble-maintainers; existing harness flags retain their source names until the workload APIs are renamed"
)]
#![expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-maintainers; harness signatures mirror their public workload and evidence inputs"
)]
#![expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; flagged parameters are growable harness buffers that require owned capacity"
)]
#![expect(
    tigerstyle::ambient_clock,
    reason = "Owner: noble-maintainers; the harness measures elapsed time for its receipts"
)]
#![expect(
    tigerstyle::ambient_env,
    reason = "Owner: noble-maintainers; the harness reads its explicit host-selected input paths and budgets"
)]
#![expect(
    tigerstyle::unchecked_narrowing,
    reason = "Owner: noble-maintainers; the flagged conversions are bounded counters/bit patterns with local masks"
)]
#![expect(
    tigerstyle::no_unwrap,
    reason = "Owner: noble-maintainers; harness lookups are guarded by prior validation and retain explicit panic diagnostics"
)]
#![expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-maintainers; harness walks are bounded by validated workload or schema depth"
)]
#![expect(
    tigerstyle::fallible_int_fallback,
    reason = "Owner: noble-maintainers; receipt counters are clamped to the selected report width"
)]
#![expect(
    tigerstyle::sentinel_fallback,
    reason = "Owner: noble-maintainers; harness markers use explicit absent sentinels outside the guest ABI"
)]
#![expect(
    tigerstyle::module_file_count,
    reason = "Owner: noble-maintainers; the harness module layout follows its measurement legs"
)]
#![expect(
    tigerstyle::catch_all_on_enum,
    reason = "Owner: noble-maintainers; closed harness dispatch matches retain their documented default diagnostic"
)]

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
