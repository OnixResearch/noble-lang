use super::common::rewrite_corpus;
use super::measure::Measured;

/// One report row as JSON.
pub(super) fn format_row(
    fixture: &str,
    proposer: &str,
    row: &Measured,
    per_step_us: Option<f64>,
    is_threshold_met: bool,
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
        is_threshold_met,
    )
}

pub(super) fn render(
    manifest: &rewrite_corpus::Manifest,
    threshold: &rewrite_corpus::Threshold,
    samples: usize,
    run: &super::measure::Run,
) -> String {
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
    std::format!(
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
        run.accepted + run.refused,
        run.accepted,
        run.refused,
        run.reached_target,
        stages,
        run.rows.join(",\n"),
        samples,
    )
}
