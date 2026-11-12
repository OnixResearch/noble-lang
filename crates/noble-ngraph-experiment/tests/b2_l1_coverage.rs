#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs the L1 record/precondition boundary and asserts its outcomes, so terse names are the readable form."
)]
//! B2 L1 coverage: a per-program mode record is honest only when every vector is
//! either an ordinary agreement or a recorded ordinary quota outcome, and at
//! least one ordinary comparison agreed. The committed phase-B-extract receipt
//! shows `random-1` agreeing on zero vectors, which this rule refuses.

use noble_ngraph_experiment::b2i64::l1_coverage;

#[test]
fn a_program_with_only_quota_outcomes_has_no_agreement() {
    // The observed defect: 20 vectors, all refused at the acceptance stage.
    assert_eq!(l1_coverage(0, 20, 20), Err("no ordinary-path agreement"));
    // The same program with a single ordinary comparison is honest.
    assert_eq!(l1_coverage(1, 19, 20), Ok(()));
}

#[test]
fn every_vector_must_be_accounted_for_by_agreement_or_quota() {
    assert_eq!(l1_coverage(20, 0, 20), Ok(()));
    // A disagreeing vector is in neither class: the mode's record must fail.
    assert_eq!(
        l1_coverage(19, 0, 20),
        Err("vectors outside agreement plus recorded ordinary quota")
    );
    assert_eq!(l1_coverage(0, 19, 20), Err("no ordinary-path agreement"));
    // An empty mode cannot agree on anything.
    assert_eq!(l1_coverage(0, 0, 0), Err("no ordinary-path agreement"));
    // Counts that overflow cannot be read as full coverage.
    assert_eq!(
        l1_coverage(usize::MAX, 1, 0),
        Err("vectors outside agreement plus recorded ordinary quota")
    );
}
