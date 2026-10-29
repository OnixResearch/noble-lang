//! Shared data and guards for one fail-closed pipeline attempt.

#[path = "attempt/fresh.rs"]
pub(super) mod fresh;
#[path = "attempt/refusal.rs"]
mod refusal;
#[path = "attempt/warm.rs"]
pub(super) mod warm;

use noble_ngraph::cert;
use noble_ngraph::check;
use noble_ngraph::compiled::{Compiled, Workspace};
use noble_ngraph::eval::{Obs, ScratchCounters};
use noble_ngraph::ir::hex;
use noble_ngraph::{layout, sem};

pub(super) const INPUT_REFUSED: &str = "input-refused";
pub(super) const CERTIFICATE_REFUSED: &str = "certificate-refused";
pub(super) const CANDIDATE_REFUSED: &str = "candidate-refused";
pub(super) const LAYOUT_REFUSED: &str = "layout-refused";
pub(super) const EXECUTION_REFUSED: &str = "execution-refused";
pub(super) const EXECUTED: &str = "executed";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    InputAdmission,
    ProposalSearch,
    CertificateChecking,
    CandidateAdmission,
    BackendPreparation,
    ScheduleValidation,
    FirstExecution,
    FullFreshAttempt,
    WarmExecution,
}

pub(super) const STAGES: [Stage; 9] = [
    Stage::InputAdmission,
    Stage::ProposalSearch,
    Stage::CertificateChecking,
    Stage::CandidateAdmission,
    Stage::BackendPreparation,
    Stage::ScheduleValidation,
    Stage::FirstExecution,
    Stage::FullFreshAttempt,
    Stage::WarmExecution,
];

#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; the closed measurement-stage schema must update both indices and wire labels when a stage is added."
)]
impl Stage {
    pub(super) const fn index(self) -> usize {
        match self {
            Self::InputAdmission => 0,
            Self::ProposalSearch => 1,
            Self::CertificateChecking => 2,
            Self::CandidateAdmission => 3,
            Self::BackendPreparation => 4,
            Self::ScheduleValidation => 5,
            Self::FirstExecution => 6,
            Self::FullFreshAttempt => 7,
            Self::WarmExecution => 8,
        }
    }

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::InputAdmission => "input_admission",
            Self::ProposalSearch => "proposal_search",
            Self::CertificateChecking => "certificate_checking",
            Self::CandidateAdmission => "candidate_admission",
            Self::BackendPreparation => "backend_preparation",
            Self::ScheduleValidation => "schedule_validation",
            Self::FirstExecution => "first_execution",
            Self::FullFreshAttempt => "full_fresh_attempt",
            Self::WarmExecution => "warm_execution",
        }
    }
}

#[derive(Clone)]
pub(super) struct RefusalInfo {
    pub(super) stage: &'static str,
    pub(super) code: String,
    pub(super) detail: String,
}

#[derive(Clone)]
pub(super) struct CertificateInfo {
    pub(super) revision: String,
    pub(super) steps: usize,
    pub(super) left: String,
    pub(super) right: String,
    pub(super) in_memory_bytes: usize,
}

#[derive(Clone, Copy)]
pub(super) struct ScratchObservation {
    pub(super) scope: &'static str,
    pub(super) acquired: u64,
    pub(super) released: u64,
    pub(super) live_bytes: u64,
    pub(super) peak_bytes: u64,
}

impl ScratchObservation {
    pub(super) fn from(scope: &'static str, counters: &ScratchCounters) -> Self {
        Self {
            scope,
            acquired: counters.acquired,
            released: counters.released,
            live_bytes: counters.live_bytes,
            peak_bytes: counters.peak_bytes,
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct PathDetails {
    pub(super) graph_logical_id: Option<String>,
    pub(super) build_compiler_revision: Option<String>,
    pub(super) build_layout_key: Option<String>,
    pub(super) graph_retained_bytes: Option<u64>,
    pub(super) code_retained_bytes: Option<u64>,
    pub(super) workspace_scratch_bytes: Option<u64>,
    pub(super) certificate: Option<CertificateInfo>,
    pub(super) residual: Option<usize>,
    pub(super) fresh_output: Option<Vec<sem::Value>>,
    pub(super) warm_output: Option<Vec<sem::Value>>,
    pub(super) fresh_scratch: Option<ScratchObservation>,
    pub(super) warm_scratch: Option<ScratchObservation>,
}

#[derive(Clone)]
pub(super) struct AcceptedArtifact {
    pub(super) compiled: Compiled,
    pub(super) workspace: Workspace,
    pub(super) details: PathDetails,
}

pub(super) struct FreshRun {
    pub(super) total_us: f64,
    pub(super) stage_durations: [Option<f64>; STAGES.len()],
    pub(super) disposition: &'static str,
    pub(super) refusal: Option<RefusalInfo>,
    pub(super) execution_called: bool,
    pub(super) details: PathDetails,
    pub(super) artifact: Option<AcceptedArtifact>,
}

pub(super) struct WarmRun {
    pub(super) duration_us: Vec<f64>,
    pub(super) executed: bool,
    pub(super) refusal: Option<RefusalInfo>,
    pub(super) calls: usize,
    pub(super) details: PathDetails,
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; elapsed time reads the runtime monotonic clock."
)]
fn elapsed_us(started: std::time::Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1_000_000.0
}

fn source_details(source_id: &str, graph_retained_bytes: u64) -> PathDetails {
    PathDetails {
        graph_logical_id: Some(source_id.to_owned()),
        graph_retained_bytes: Some(graph_retained_bytes),
        ..PathDetails::default()
    }
}

pub(super) fn diagnostic_reason(
    stage: Stage,
    diagnostic: &noble_ngraph::Diagnostic,
) -> RefusalInfo {
    RefusalInfo {
        stage: stage.name(),
        code: diagnostic.code_string(),
        detail: diagnostic.to_string(),
    }
}

pub(super) fn certificate_reason(refusal: &cert::Refusal) -> RefusalInfo {
    RefusalInfo {
        stage: Stage::CertificateChecking.name(),
        code: refusal::certificate(refusal).to_owned(),
        detail: "the exact proposed endpoints or premises failed certificate checking".to_owned(),
    }
}

pub(super) fn layout_reason(refusal: layout::Refusal) -> RefusalInfo {
    RefusalInfo {
        stage: Stage::ScheduleValidation.name(),
        code: refusal::layout(refusal).to_owned(),
        detail: "compiled packed layout was refused by the independent validator".to_owned(),
    }
}

pub(super) fn observation_reason(observation: &Obs) -> RefusalInfo {
    match observation {
        Obs::Normal(_) => RefusalInfo {
            stage: Stage::FirstExecution.name(),
            code: "OUTPUT-MISMATCH".to_owned(),
            detail: "compiled output did not agree with the source reference".to_owned(),
        },
        Obs::Refused(diagnostic) => diagnostic_reason(Stage::FirstExecution, diagnostic),
        Obs::GuardFailed { node, .. } => RefusalInfo {
            stage: Stage::FirstExecution.name(),
            code: "G-FACT-FALSE".to_owned(),
            detail: std::format!("guard or index refusal at node %{}", node.0),
        },
        Obs::Exhausted(kind) => RefusalInfo {
            stage: Stage::FirstExecution.name(),
            code: std::format!("E-BUDGET-{}", kind.name()),
            detail: "evaluation exhausted its host budget".to_owned(),
        },
        Obs::Cancelled => RefusalInfo {
            stage: Stage::FirstExecution.name(),
            code: "C-CANCELLED".to_owned(),
            detail: "evaluation was cancelled".to_owned(),
        },
    }
}

pub(super) fn outputs(observation: &Obs) -> Option<Vec<sem::Value>> {
    match observation {
        Obs::Normal(values) => Some(values.clone()),
        Obs::Refused(_) | Obs::GuardFailed { .. } | Obs::Exhausted(_) | Obs::Cancelled => None,
    }
}

pub(super) fn agreement(observation: &Obs, expected: Option<&Obs>) -> bool {
    observation.outputs().is_some()
        && expected.is_some_and(|reference| observation.obs_eq(reference))
}

fn certificate_info(derivation: &cert::Derivation) -> CertificateInfo {
    let in_memory_bytes = std::mem::size_of::<cert::Derivation>()
        .saturating_add(std::mem::size_of::<cert::Step>().saturating_mul(derivation.steps.len()))
        .saturating_add(derivation.revision.capacity());
    CertificateInfo {
        revision: derivation.revision.clone(),
        steps: derivation.steps.len(),
        left: hex(&derivation.left),
        right: hex(&derivation.right),
        in_memory_bytes,
    }
}

fn accepted_details(
    mut details: PathDetails,
    checked: &noble_ngraph::CheckedGraph,
    compiled: &Compiled,
    output: &Obs,
    scratch: ScratchObservation,
) -> PathDetails {
    details.graph_logical_id = Some(hex(&checked.id()));
    details.graph_retained_bytes = Some(check::retained_bytes(checked.graph()));
    details.build_compiler_revision = Some(noble_ngraph::compiled::COMPILER_REVISION.to_owned());
    details.build_layout_key = Some(hex(&compiled.layout_key()));
    details.code_retained_bytes = Some(compiled.retained_bytes());
    details.workspace_scratch_bytes = Some(compiled.scratch_bytes());
    details.residual = Some(cert::residual_fold_sites(checked.graph()));
    details.fresh_output = outputs(output);
    details.fresh_scratch = Some(scratch);
    details
}

impl FreshRun {
    fn refuse(&mut self, disposition: &'static str, refusal: RefusalInfo) {
        self.disposition = disposition;
        self.refusal = Some(refusal);
    }
}
