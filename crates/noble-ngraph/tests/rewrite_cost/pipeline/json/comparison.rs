//! Conditional amortization estimate; never a measured crossover or retention verdict.

use super::super::{attempt::EXECUTED, PathAggregate};
use super::format::quoted;

fn unavailable(reason: &str) -> String {
    std::format!(
        "{{\"status\":\"unmeasured\",\"reason\":{}}}",
        quoted(reason)
    )
}

const fn runs(additional_us: f64, saving_us: f64) -> Option<(f64, f64)> {
    if !additional_us.is_finite() || !saving_us.is_finite() || saving_us <= 0.0 {
        return None;
    }
    let further = (additional_us.max(0.0) / saving_us).ceil();
    // JSON readers must retain both integer run counts exactly, including the first run.
    if !further.is_finite() || further > 9_007_199_254_740_990.0 {
        return None;
    }
    Some((further, further + 1.0))
}

#[test]
fn amortization_requires_positive_saving_and_counts_the_first_run() {
    assert_eq!(runs(10.1, 2.0), Some((6.0, 7.0)));
    assert_eq!(runs(-1.0, 2.0), Some((0.0, 1.0)));
    assert_eq!(runs(10.0, 0.0), None);
    assert_eq!(runs(10.0, -1.0), None);
    assert_eq!(runs(f64::INFINITY, 1.0), None);
    assert_eq!(runs(9_007_199_254_740_992.0, 1.0), None);
}

pub(super) fn render(baseline: &mut PathAggregate, certified: &mut PathAggregate) -> String {
    if baseline.disposition != EXECUTED || certified.disposition != EXECUTED {
        return unavailable("both baseline and certified paths must execute");
    }
    if certified.details.residual != Some(0) {
        return unavailable("certified residual target was not reached");
    }
    let (Some(base_fresh), Some(cert_fresh), Some(base_warm), Some(cert_warm)) = (
        baseline.fresh_summary(),
        certified.fresh_summary(),
        baseline.warm_summary(),
        certified.warm_summary(),
    ) else {
        return unavailable("both fresh and warm execution medians are required");
    };
    let saving_us = base_warm.median_us - cert_warm.median_us;
    if saving_us <= 0.0 {
        return std::format!(
            concat!(
                "{{\"status\":\"unmeasured\",\"reason\":\"warm saving is not positive\",",
                "\"warm_saving_us\":{:.9}}}"
            ),
            saving_us
        );
    }
    let extra_us = (cert_fresh.median_us - base_fresh.median_us).max(0.0);
    let Some((further, total)) = runs(extra_us, saving_us) else {
        return unavailable("run estimate exceeds exact JSON integer range");
    };
    std::format!(concat!(
        "{{\"status\":\"estimated\",\"label\":\"linear median-based estimate\",",
        "\"warm_saving_us\":{:.9},\"additional_fresh_cost_us\":{:.9},",
        "\"further_runs\":{:.0},\"estimated_total_runs\":{:.0},",
        "\"formula\":\"first run + ceil(max(0, certified_fresh_total - baseline_fresh_total) / warm_saving) further runs\"}}"),
        saving_us, extra_us, further, total)
}
