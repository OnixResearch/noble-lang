//! Structured pipeline JSON receipt.

use super::super::attempt::{CertificateInfo, RefusalInfo, Stage};
use super::super::{PathAggregate, Reference, PRIMING_PAIRS};
use super::format::{
    optional_string, optional_u64, optional_usize, quoted, scratch, summary, values,
};

fn refusal(value: Option<&RefusalInfo>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |value| {
            std::format!(
                "{{\"stage\":{},\"code\":{},\"detail\":{}}}",
                quoted(value.stage),
                quoted(&value.code),
                quoted(&value.detail)
            )
        },
    )
}

fn certificate(value: Option<&CertificateInfo>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |value| {
            std::format!(
                concat!(
        "{{\"revision\":{},\"steps\":{},\"left\":{},\"right\":{},\"in_memory_bytes\":{},",
        "\"wire_bytes\":null,\"wire_bytes_reason\":\"no Rust certificate encoder exists\"}}"),
                quoted(&value.revision),
                value.steps,
                quoted(&value.left),
                quoted(&value.right),
                value.in_memory_bytes
            )
        },
    )
}

fn path(aggregate: &mut PathAggregate, is_certified: bool) -> String {
    let fresh = aggregate.fresh_summary();
    let warm = aggregate.warm_summary();
    let warm_reason = if warm.is_none() {
        quoted(&aggregate.stage_reason(Stage::WarmExecution))
    } else {
        "null".to_owned()
    };
    let stages = super::stages::render(aggregate, is_certified);
    let details = &aggregate.details;
    let certificate = if is_certified {
        certificate(details.certificate.as_ref())
    } else {
        "null".to_owned()
    };
    std::format!(
        concat!(
            "{{\"disposition\":{},\"refusal\":{},\"proposer_present\":{},",
            "\"graph_logical_id\":{},\"build\":{{\"compiler_revision\":{},\"layout_key\":{}}},",
            "\"graph_retained_bytes_estimate\":{},\"code_retained_bytes_estimate\":{},",
            "\"workspace_scratch_bytes\":{},\"certificate\":{},\"residual_fold_sites\":{},",
            "\"stages\":{},\"fresh_total_us\":{},\"warm_execution_us\":{},",
            "\"fresh_total_not_reached_reason\":null,\"warm_execution_not_reached_reason\":{},",
            "\"fresh_output\":{},\"warm_output\":{},\"scratch\":{{\"fresh\":{},\"warm\":{}}},",
            "\"evaluation_calls\":{{\"priming\":{},\"fresh\":{},\"warm\":{},\"total\":{}}}}}"
        ),
        quoted(aggregate.disposition),
        refusal(aggregate.refusal.as_ref()),
        is_certified,
        optional_string(details.graph_logical_id.as_ref()),
        optional_string(details.build_compiler_revision.as_ref()),
        optional_string(details.build_layout_key.as_ref()),
        optional_u64(details.graph_retained_bytes),
        optional_u64(details.code_retained_bytes),
        optional_u64(details.workspace_scratch_bytes),
        certificate,
        optional_usize(details.residual),
        stages,
        summary(fresh),
        summary(warm),
        warm_reason,
        values(details.fresh_output.as_deref()),
        values(details.warm_output.as_deref()),
        scratch(details.fresh_scratch),
        scratch(details.warm_scratch),
        aggregate.priming_execution_calls,
        aggregate.fresh_execution_calls,
        aggregate.warm_execution_calls,
        aggregate.execution_calls()
    )
}

const fn unmeasured_stages() -> &'static str {
    concat!(
        "[{\"name\":\"certificate_wire_bytes\",\"status\":\"unmeasured\",",
        "\"reason\":\"no Rust certificate encoder exists; structural footprint is not wire size\"},",
        "{\"name\":\"decoding_replay\",\"status\":\"unmeasured\",",
        "\"reason\":\"no Rust decoder exists in this in-memory path\"},",
        "{\"name\":\"retained_memory\",\"status\":\"unmeasured\",",
        "\"reason\":\"no allocator or RSS accounting; graph/code estimates and scratch lease observations are separately labeled\"}]"
    )
}

fn reference(source: &Reference) -> String {
    std::format!(
        concat!(
            "{{\"graph_logical_id\":{},\"graph_retained_bytes_estimate\":{},",
            "\"reference\":{{\"outcome\":{},\"evaluated_nodes\":{},",
            "\"outputs\":{},\"scratch_counters\":{},\"refusal\":{}}}}}"
        ),
        quoted(&source.graph_logical_id),
        source.graph_retained_bytes,
        quoted(source.outcome),
        source.evaluated_nodes,
        values(
            source
                .observation
                .as_ref()
                .and_then(|observation| observation.outputs())
        ),
        scratch(Some(source.scratch)),
        refusal(source.refusal.as_ref())
    )
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the receipt formats runtime timing summaries and observations into owned JSON."
)]
pub(in crate::pipeline) fn render(
    source: &Reference,
    baseline: &mut PathAggregate,
    certified: &mut PathAggregate,
    samples: usize,
) -> String {
    let disposition = if certified.disposition == super::super::attempt::EXECUTED {
        baseline.disposition
    } else {
        certified.disposition
    };
    let baseline_calls = baseline.execution_calls();
    let candidate_calls = certified.execution_calls();
    let comparison = super::comparison::render(baseline, certified);
    let baseline = path(baseline, false);
    let certified_json = path(certified, true);
    std::format!(
        concat!(
            "{{\"schema\":\"noble-rewrite-cost-pipeline/v2\",\"disposition\":{},",
            "\"candidate_execution_calls\":{},\"baseline_execution_calls\":{},",
            "\"samples_per_row\":{},\"priming_pairs\":{},\"source\":{},",
            "\"baseline\":{},\"certified\":{},\"unmeasured_stages\":{},",
            "\"comparison\":{{\"break_even\":{}}}}}"
        ),
        quoted(disposition),
        candidate_calls,
        baseline_calls,
        samples,
        PRIMING_PAIRS,
        reference(source),
        baseline,
        certified_json,
        unmeasured_stages(),
        comparison
    )
}
