use super::common::rewrite_corpus;
use super::report;
use noble_ngraph::cert::{self, Derivation, Proposer, Refusal, Step};
use noble_ngraph::ir::Graph;

/// Median and interquartile range of warm samples, in nanoseconds.
#[expect(
    tigerstyle::ambient_clock,
    reason = "Owner: noble-maintainers; this test-only timing helper reads the monotonic clock for real warm samples."
)]
fn warm_stats<T>(
    samples: usize,
    mut f: impl FnMut() -> T,
    mut consume: impl FnMut(T),
) -> (f64, f64) {
    for _ in 0..64 {
        consume(std::hint::black_box(f()));
    }
    let mut values = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = std::time::Instant::now();
        let result = std::hint::black_box(f());
        let elapsed = started.elapsed();
        consume(result);
        values.push(elapsed.as_secs_f64() * 1e9);
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

const fn to_us(nanos: f64) -> f64 {
    (nanos / 1000.0 * 100.0).round() / 100.0
}

/// One measured row. `refusal` is `None` for an accepted derivation, so coverage keeps refused
/// attempts in its denominator rather than dropping them.
pub(super) struct Measured {
    pub(super) steps: usize,
    pub(super) footprint_bytes: usize,
    pub(super) propose_us: f64,
    pub(super) propose_iqr_us: f64,
    pub(super) check_us: f64,
    pub(super) check_iqr_us: f64,
    pub(super) refusal: Option<Refusal>,
    pub(super) residual: usize,
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this runtime helper invokes graph proposal and certificate checking."
)]
fn sample(graph: &Graph, proposer: Proposer, samples: usize) -> Measured {
    let (derivation, result): (Derivation, Graph) =
        cert::propose(&super::common::IDS, proposer, graph);
    let expected_refusal = cert::check(&super::common::IDS, graph, &result, &derivation).err();
    let (propose_median, propose_iqr) = warm_stats(
        samples,
        || {
            // Preserve proposal-plus-artifact-drop timing from the original lane.
            std::hint::black_box(cert::propose(&super::common::IDS, proposer, graph));
        },
        |()| {},
    );
    let (check_median, check_iqr) = warm_stats(
        samples,
        || cert::check(&super::common::IDS, graph, &result, &derivation),
        |outcome| {
            // Handle every sampled Result without pricing the assertion as checker work.
            assert_eq!(outcome.as_ref().err(), expected_refusal.as_ref());
        },
    );
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
        refusal: expected_refusal,
        residual: cert::residual_fold_sites(&result),
    }
}

const PROPOSERS: [(Proposer, &str); 4] = [
    (Proposer::Identity, "identity"),
    (Proposer::FirstEligible, "first-eligible"),
    (Proposer::SinglePass, "single-pass"),
    (Proposer::FixedPoint, "fixed-point"),
];

pub(super) struct Run {
    pub(super) rows: Vec<String>,
    pub(super) accepted: usize,
    pub(super) refused: usize,
    pub(super) reached_target: usize,
}

struct FixtureRun {
    rows: Vec<String>,
    accepted: usize,
    refused: usize,
    reached_target: usize,
}

pub(super) struct RecordedRow {
    text: String,
    accepted: usize,
    refused: usize,
    reached_target: usize,
}

pub(super) fn collect(
    corpus: &rewrite_corpus::Manifest,
    threshold: &rewrite_corpus::Threshold,
    samples: usize,
) -> Result<Run, Box<dyn std::error::Error>> {
    let mut run = Run {
        rows: Vec::with_capacity(corpus.fixtures.len().saturating_mul(PROPOSERS.len())),
        accepted: 0,
        refused: 0,
        reached_target: 0,
    };
    for fixture in &corpus.fixtures {
        let fixture_run = sample_fixture(fixture, threshold, samples)?;
        run.rows.extend(fixture_run.rows);
        run.accepted += fixture_run.accepted;
        run.refused += fixture_run.refused;
        run.reached_target += fixture_run.reached_target;
    }
    Ok(run)
}

fn sample_fixture(
    fixture: &rewrite_corpus::Fixture,
    threshold: &rewrite_corpus::Threshold,
    samples: usize,
) -> Result<FixtureRun, Box<dyn std::error::Error>> {
    assert_eq!(fixture.target, threshold.residual);
    let graph = rewrite_corpus::build(&super::common::IDS, fixture)?;
    let sites = cert::residual_fold_sites(&graph);
    assert!(sites > 0, "{} declares nothing to optimize", fixture.name);
    let mut run = FixtureRun {
        rows: Vec::with_capacity(PROPOSERS.len()),
        accepted: 0,
        refused: 0,
        reached_target: 0,
    };
    for (proposer, label) in PROPOSERS {
        let row = sample(&graph, proposer, samples);
        let recorded = record_row(&fixture.name, proposer, label, &row, threshold);
        run.rows.push(recorded.text);
        run.accepted += recorded.accepted;
        run.refused += recorded.refused;
        run.reached_target += recorded.reached_target;
    }
    Ok(run)
}

const fn per_step_cost(check_us: f64, steps: usize) -> Option<f64> {
    if steps == 0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    Some((check_us / steps as f64 * 100.0).round() / 100.0)
}

#[test]
fn per_step_microseconds_preserve_the_unit_and_zero_step_boundary() {
    assert_eq!(per_step_cost(10.02, 2), Some(5.01));
    assert_eq!(per_step_cost(10.02, 0), None);
}

pub(super) fn record_row(
    fixture: &str,
    proposer: Proposer,
    label: &str,
    row: &Measured,
    threshold: &rewrite_corpus::Threshold,
) -> RecordedRow {
    let (accepted, refused, reached_target) = if row.refusal.is_some() {
        (0, 1, 0)
    } else if row.residual == 0 {
        (1, 0, 1)
    } else {
        (1, 0, 0)
    };
    // The identity proposer costs nothing to check yet reaches no target: validity and
    // quality stay separate facts in the same report.
    if proposer == Proposer::Identity {
        assert!(
            row.refusal.is_none(),
            "{fixture}: identity certificate is valid"
        );
        assert_eq!(row.steps, 0, "{fixture}: identity derivation has no steps");
        assert!(row.residual > 0, "{fixture}: identity reaches no target");
    }
    assert!(row.check_us >= 0.0, "{fixture}: check cost is not negative");
    assert!(
        row.propose_us >= 0.0,
        "{fixture}: proposal cost is not negative"
    );
    // A zero-step derivation has no per-step figure rather than a fabricated one.
    let per_step_us = per_step_cost(row.check_us, row.steps);
    let is_threshold_met = match per_step_us {
        Some(value) => value <= threshold.check_per_step_us,
        None => false,
    };
    RecordedRow {
        text: report::format_row(fixture, label, row, per_step_us, is_threshold_met),
        accepted,
        refused,
        reached_target,
    }
}
