//! Measured, shared and unreached pipeline stages.

use super::super::{attempt::Stage, PathAggregate};
use super::format::{quoted, summary};

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; runtime samples are sorted and serialized into owned JSON."
)]
fn observed(path: &mut PathAggregate, stage: Stage) -> String {
    match path.stage_summary(stage) {
        Some(value) => std::format!(
            "{{\"status\":\"measured\",\"duration_us\":{},\"not_reached_reason\":null}}",
            summary(Some(value))
        ),
        None => std::format!(
            "{{\"status\":\"not_reached\",\"duration_us\":null,\"not_reached_reason\":{}}}",
            quoted(&path.stage_reason(stage))
        ),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; runtime proposal samples are serialized without adding construction cost twice."
)]
fn construction(path: &mut PathAggregate) -> String {
    match path.stage_summary(Stage::ProposalSearch) {
        Some(value) => std::format!(
            concat!(
            "{{\"status\":\"shared\",\"duration_us\":{},",
            "\"not_reached_reason\":null,\"additive\":false,\"shared_with\":\"proposal_search\"}}"),
            summary(Some(value))
        ),
        None => std::format!(
            concat!(
                "{{\"status\":\"not_reached\",\"duration_us\":null,\"not_reached_reason\":{},",
                "\"additive\":false,\"shared_with\":\"proposal_search\"}}"
            ),
            quoted(&path.stage_reason(Stage::ProposalSearch))
        ),
    }
}

fn not_applicable(reason: &str) -> String {
    std::format!(
        "{{\"status\":\"not_applicable\",\"duration_us\":null,\"not_reached_reason\":{}}}",
        quoted(reason)
    )
}

pub(super) fn render(path: &mut PathAggregate, is_certified: bool) -> String {
    let proposal = if is_certified {
        observed(path, Stage::ProposalSearch)
    } else {
        not_applicable("baseline has no proposal stage")
    };
    let construction = if is_certified {
        construction(path)
    } else {
        not_applicable("baseline has no certificate construction stage")
    };
    let certificate = if is_certified {
        observed(path, Stage::CertificateChecking)
    } else {
        not_applicable("baseline has no certificate checking stage")
    };
    let candidate = if is_certified {
        observed(path, Stage::CandidateAdmission)
    } else {
        not_applicable("baseline has no candidate admission stage")
    };
    let warm = match path.warm_summary() {
        Some(value) => std::format!(
            "{{\"status\":\"measured\",\"duration_us\":{},\"not_reached_reason\":null}}",
            summary(Some(value))
        ),
        None => std::format!(
            "{{\"status\":\"not_reached\",\"duration_us\":null,\"not_reached_reason\":{}}}",
            quoted(&path.stage_reason(Stage::WarmExecution))
        ),
    };
    std::format!(concat!(
        "{{\"input_admission\":{},\"proposal_search\":{},\"certificate_construction\":{},",
        "\"certificate_checking\":{},\"candidate_admission\":{},\"backend_preparation\":{},",
        "\"schedule_validation\":{},\"first_execution\":{},\"full_fresh_attempt\":{},\"warm_execution\":{}}}"),
        observed(path, Stage::InputAdmission), proposal, construction, certificate, candidate,
        observed(path, Stage::BackendPreparation), observed(path, Stage::ScheduleValidation),
        observed(path, Stage::FirstExecution), observed(path, Stage::FullFreshAttempt), warm)
}
