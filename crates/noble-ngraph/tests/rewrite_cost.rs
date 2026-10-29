#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    tigerstyle::assertion_density,
    tigerstyle::ambiguous_params,
    tigerstyle::fallible_int_fallback,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; this integration harness reads frozen fixtures and prices their measured costs, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! End-to-end cost accounting for certified graph rewrites (issue #64 items 5-7).
//!
//! The quality lane proves that correctness alone cannot satisfy a quality target. This lane prices
//! that work: per fixture and proposer it reports the derivation's step count, its structural
//! in-memory footprint, the warm proposal and certificate-check costs, and every end-to-end stage it
//! cannot observe as `unmeasured` with a reason instead of zero.
//!
//! The continuation thresholds come from the frozen manifest
//! `verification/ngraph/rewrite-cost-v1.txt`, read before any number is taken, so a failing
//! threshold is a regression signal rather than a retro-fitted pass. Nothing here executes a graph,
//! so no per-run saving exists and break-even is reported as unmeasured rather than invented.

mod common;

#[path = "rewrite_cost/fixtures.rs"]
mod fixtures;
#[path = "rewrite_cost/measure.rs"]
mod measure;
#[path = "rewrite_cost/report.rs"]
mod report;

#[test]
fn certified_rewrite_costs_are_reported_apart_from_validity_and_coverage(
) -> Result<(), Box<dyn std::error::Error>> {
    let (manifest, corpus) = fixtures::load()?;
    let threshold = manifest.threshold.as_ref().ok_or("threshold")?;
    let samples = 200;
    let run = measure::collect(&corpus, threshold, samples)?;
    assert!(
        run.reached_target > 0,
        "the corpus must contain at least one reachable target, or the thresholds are vacuous"
    );
    assert!(
        run.refused > 0,
        "refused derivations stay in the denominator rather than being dropped"
    );
    let report = report::render(&manifest, threshold, samples, &run);
    println!("{report}");
    Ok(())
}
