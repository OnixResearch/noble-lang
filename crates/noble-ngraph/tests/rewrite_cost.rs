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

use std::path::Path;
use std::time::Instant;

use common::rewrite_corpus;
use common::IDS;
use noble_ngraph::cert::{self, Derivation, Proposer, Refusal, Step};
use noble_ngraph::ir::Graph;

/// Median and interquartile range of warm samples, in nanoseconds.
fn warm_stats(samples: usize, mut f: impl FnMut()) -> (f64, f64) {
    for _ in 0..64 {
        f();
    }
    let mut values = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        f();
        values.push(started.elapsed().as_secs_f64() * 1e9);
    }
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    #[allow(clippy::cast_precision_loss)]
    let median = values[values.len() / 2];
    #[allow(clippy::cast_precision_loss)]
    let low = values[values.len() / 4];
    #[allow(clippy::cast_precision_loss)]
    let high = values[values.len() * 3 / 4];
    (median, high - low)
}

fn to_us(nanos: f64) -> f64 {
    (nanos / 1000.0 * 100.0).round() / 100.0
}

/// One measured row. `refusal` is `None` for an accepted derivation, so coverage keeps refused
/// attempts in its denominator rather than dropping them.
struct Measured {
    steps: usize,
    footprint_bytes: usize,
    propose_us: f64,
    propose_iqr_us: f64,
    check_us: f64,
    check_iqr_us: f64,
    refusal: Option<Refusal>,
    residual: usize,
}

fn measure(graph: &Graph, proposer: Proposer, samples: usize) -> Measured {
    let (derivation, result): (Derivation, Graph) = cert::propose(&IDS, proposer, graph);
    let (propose_median, propose_iqr) =
        warm_stats(samples, || drop(cert::propose(&IDS, proposer, graph)));
    let (check_median, check_iqr) = warm_stats(samples, || {
        drop(cert::check(&IDS, graph, &result, &derivation))
    });
    // Structural in-memory estimate: one `Step` per step plus the derivation header. This is not the
    // wire size, and no Rust encoder exists to produce that.
    let footprint_bytes = std::mem::size_of::<Derivation>()
        + std::mem::size_of::<Step>() * derivation.steps.len()
        + derivation.revision.capacity();
    Measured {
        steps: derivation.steps.len(),
        footprint_bytes,
        propose_us: to_us(propose_median),
        propose_iqr_us: to_us(propose_iqr),
        check_us: to_us(check_median),
        check_iqr_us: to_us(check_iqr),
        refusal: cert::check(&IDS, graph, &result, &derivation).err(),
        residual: cert::residual_fold_sites(&result),
    }
}

/// One report row as JSON.
fn format_row(
    fixture: &str,
    proposer: &str,
    row: &Measured,
    per_step_us: Option<f64>,
    threshold_met: bool,
) -> String {
    let per_step = match per_step_us {
        Some(value) => std::format!("{value:.2}"),
        None => "null".to_string(),
    };
    std::format!(
        concat!(
            "    {{",
            "\"fixture\": \"{}\", ",
            "\"proposer\": \"{}\", ",
            "\"steps\": {}, ",
            "\"refused\": {}, ",
            "\"residual\": {}, ",
            "\"propose_us\": {:.2}, ",
            "\"propose_iqr_us\": {:.2}, ",
            "\"check_us\": {:.2}, ",
            "\"check_iqr_us\": {:.2}, ",
            "\"check_per_step_us\": {}, ",
            "\"in_memory_footprint_bytes\": {}, ",
            "\"check_threshold_met\": {}",
            "}}"
        ),
        fixture,
        proposer,
        row.steps,
        row.refusal.is_some(),
        row.residual,
        row.propose_us,
        row.propose_iqr_us,
        row.check_us,
        row.check_iqr_us,
        per_step,
        row.footprint_bytes,
        threshold_met,
    )
}

const PROPOSERS: [(Proposer, &str); 4] = [
    (Proposer::Identity, "identity"),
    (Proposer::FirstEligible, "first-eligible"),
    (Proposer::SinglePass, "single-pass"),
    (Proposer::FixedPoint, "fixed-point"),
];

#[test]
fn certified_rewrite_costs_are_reported_apart_from_validity_and_coverage(
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = rewrite_corpus::parse(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/ngraph/rewrite-cost-v1.txt"
    )))?;
    assert_eq!(manifest.schema, "noble-rewrite-cost/v1");
    assert_eq!(manifest.corpus, "rewrite-quality-v1.txt");
    let threshold = manifest.threshold.as_ref().ok_or("threshold")?;
    assert_eq!(threshold.residual, "no-foldable-sites");
    assert!(
        threshold.check_per_step_us > 0.0,
        "the continuation threshold is pre-registered, not discovered"
    );
    assert!(
        manifest
            .stages
            .iter()
            .any(|stage| stage.status == "measured"),
        "some stage is observed"
    );
    assert!(
        manifest
            .stages
            .iter()
            .filter(|stage| stage.status == "unmeasured")
            .all(|stage| !stage.reason.is_empty()),
        "an unmeasured stage always states why"
    );
    assert!(
        manifest
            .stages
            .iter()
            .all(|stage| stage.status == "measured" || stage.status == "unmeasured"),
        "a stage is measured or unmeasured, never assumed zero"
    );

    let corpus = rewrite_corpus::parse(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/ngraph/rewrite-quality-v1.txt"
    )))?;
    assert!(
        !corpus.fixtures.is_empty(),
        "the corpus named by the cost manifest is not empty"
    );
    let samples = 200;

    let mut rows = Vec::new();
    let mut accepted = 0_usize;
    let mut refused = 0_usize;
    let mut reached_target = 0_usize;
    for fixture in &corpus.fixtures {
        assert_eq!(fixture.target, threshold.residual);
        let graph = rewrite_corpus::build(&IDS, fixture)?;
        let sites = cert::residual_fold_sites(&graph);
        assert!(sites > 0, "{} declares nothing to optimize", fixture.name);
        for (proposer, label) in PROPOSERS {
            let row = measure(&graph, proposer, samples);
            match row.refusal {
                None => {
                    accepted += 1;
                    if row.residual == 0 {
                        reached_target += 1;
                    }
                }
                Some(_) => refused += 1,
            }
            // The identity proposer costs nothing to check yet reaches no target: validity and
            // quality stay separate facts in the same report.
            if proposer == Proposer::Identity {
                assert_eq!(
                    row.refusal, None,
                    "{}: identity certificate is valid",
                    fixture.name
                );
                assert_eq!(
                    row.steps, 0,
                    "{}: identity derivation has no steps",
                    fixture.name
                );
                assert!(
                    row.residual > 0,
                    "{}: identity reaches no target",
                    fixture.name
                );
            }
            assert!(
                row.check_us >= 0.0 && row.propose_us >= 0.0,
                "{}: a measured cost is not negative",
                fixture.name
            );
            // A zero-step derivation has no per-step figure rather than a fabricated one.
            let per_step_us = if row.steps == 0 {
                None
            } else {
                #[allow(clippy::cast_precision_loss)]
                Some(to_us(row.check_us / row.steps as f64))
            };
            let threshold_met = match per_step_us {
                Some(value) => value <= threshold.check_per_step_us,
                None => false,
            };
            rows.push(format_row(
                fixture.name.as_str(),
                label,
                &row,
                per_step_us,
                threshold_met,
            ));
        }
    }
    assert!(
        reached_target > 0,
        "the corpus must contain at least one reachable target, or the thresholds are vacuous"
    );
    assert!(
        refused > 0,
        "refused derivations stay in the denominator rather than being dropped"
    );

    let stages = manifest
        .stages
        .iter()
        .map(|stage| {
            std::format!(
                "{{\"name\": \"{}\", \"status\": \"{}\", \"reason\": \"{}\"}}",
                stage.name,
                stage.status,
                stage.reason
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let report = std::format!(
        concat!(
            "{{\n",
            "  \"schema\": \"noble-rewrite-cost-report/v1\",\n",
            "  \"corpus\": \"{}\",\n",
            "  \"samples_per_row\": {},\n",
            "  \"threshold\": {{\"residual\": \"{}\", \"check_per_step_us\": {:.2}}},\n",
            "  \"coverage\": {{\"attempts\": {}, \"accepted\": {}, \"refused\": {}, ",
            "\"target_reached\": {}}},\n",
            "  \"stages\": [{}],\n",
            "  \"rows\": [\n{}\n  ],\n",
            "  \"break_even\": \"unmeasured: this lane never executes a program, so no per-run ",
            "saving exists to amortize the additional certificate-check cost\",\n",
            "  \"cold_warm\": \"warm medians with interquartile ranges over {} samples per row; ",
            "the library caches nothing across calls, so a cold figure would price page faults ",
            "rather than work\",\n",
            "  \"comparison\": \"unchanged and certified paths are compared on identical fixture ",
            "inputs: the identity row carries no steps, so any certificate-bearing row pays check ",
            "cost without a measured saving\"\n",
            "}}"
        ),
        manifest.corpus,
        samples,
        threshold.residual,
        threshold.check_per_step_us,
        accepted + refused,
        accepted,
        refused,
        reached_target,
        stages,
        rows.join(",\n"),
        samples,
    );
    println!("{report}");
    Ok(())
}
