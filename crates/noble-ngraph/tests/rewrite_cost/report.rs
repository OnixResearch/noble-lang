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
            "\"check_threshold_met\": {}, ",
            "\"pipeline\": {}",
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
        row.pipeline.json,
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
            "  \"schema\": \"noble-rewrite-cost-report/v2\",\n",
            "  \"corpus\": \"{}\",\n",
            "  \"samples_per_row\": {},\n",
            "  \"identity_scheme\": \"test-siphash13x4\",\n",
            "  \"threshold\": {{\"residual\": \"{}\", \"check_per_step_us\": {:.2}}},\n",
            "  \"certificate_coverage\": {{\"attempts\": {}, \"accepted\": {}, \"refused\": {}, ",
            "\"target_reached\": {}}},\n",
            "  \"pipeline_coverage\": {{\"attempts\": {}, \"input_refused\": {}, ",
            "\"certificate_refused\": {}, \"candidate_refused\": {}, \"layout_refused\": {}, ",
            "\"execution_refused\": {}, \"executed\": {}}},\n",
            "  \"stages\": [{}],\n",
            "  \"rows\": [\n{}\n  ],\n",
            "  \"break_even\": \"per-row linear estimate only for an executed quality-eligible ",
            "candidate with positive measured warm saving; never inferred for refusals\",\n",
            "  \"cold_warm\": \"fresh paired baseline/certified attempts reuse no admitted graph, ",
            "compiled artifact or workspace; warm execution reuses the admitted artifact and workspace. ",
            "Median and IQR over {} samples after 64 priming iterations; not OS-cache-cold\",\n",
            "  \"comparison\": \"raw-graph certificate component coverage is separate from ",
            "full pipeline admission. Refused cases retain their measured prefix cost and ",
            "never execute a candidate. Baseline/certified paths use identical fixture inputs. ",
            "No BLAKE3 deployment, Lean correspondence or allocator/RSS claim\"\n",
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
        run.rows.len(),
        run.pipeline.input_refused,
        run.pipeline.certificate_refused,
        run.pipeline.candidate_refused,
        run.pipeline.layout_refused,
        run.pipeline.execution_refused,
        run.pipeline.executed,
        stages,
        run.rows.join(",\n"),
        samples,
    )
}
