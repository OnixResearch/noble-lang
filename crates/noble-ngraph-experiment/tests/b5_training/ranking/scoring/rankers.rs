use super::*;

#[derive(Clone, Debug)]
pub(crate) struct SeedSummary {
    pub(crate) seed: u64,
    pub(crate) validation_loss: f32,
    pub(crate) training_elapsed_ns: u64,
    pub(crate) steps: u64,
    pub(crate) optimizer_state_digest: [u8; chunks::REVISION_BYTES],
    pub(crate) optimizer_state_bytes: u64,
    pub(crate) parameter_bytes: u64,
    pub(crate) provenance_data: [u8; chunks::REVISION_BYTES],
}

pub(crate) struct ModelFit {
    pub(crate) graph: CheckedGraph,
    pub(crate) loss_graph: CheckedGraph,
    pub(crate) snapshot: ParamSnapshot,
    pub(crate) provenance: train::Provenance,
    pub(crate) state: train::AdamState,
    pub(crate) selected_seed: u64,
    pub(crate) validation_loss: f32,
    pub(crate) seeds: Vec<SeedSummary>,
    pub(crate) training_rows: Dataset,
    pub(crate) validation_rows: Dataset,
    pub(crate) training_elapsed_ns: u64,
    pub(crate) model_selection_elapsed_ns: u64,
    pub(crate) train_scratch_peak_bytes: u64,
    pub(crate) train_evaluated_nodes: u64,
}

pub(crate) struct ModelRanker<'a> {
    pub(crate) model_identity: [u8; chunks::REVISION_BYTES],
    pub(crate) graph: &'a CheckedGraph,
    pub(crate) snapshot: &'a ParamSnapshot,
    pub(crate) contexts: &'a [RankingContext],
    pub(crate) active_identity: std::cell::RefCell<Option<chunks::RankingIdentity>>,
    pub(crate) score_calls: u32,
    pub(crate) scorer_elapsed_ns: u64,
    pub(crate) evaluated_nodes: u64,
    pub(crate) scratch: noble_ngraph::eval::ScratchCounters,
    pub(crate) scored: Vec<ScoredCandidate>,
}

#[derive(Clone, Debug)]
pub(crate) struct ScoredCandidate {
    pub(crate) candidate: chunks::CandidateId,
    pub(crate) features: chunks::CandidateFeatureVector,
    pub(crate) score: f32,
}

impl ModelRanker<'_> {
    pub(crate) fn context_matches(&self, identity: &chunks::RankingIdentity) -> bool {
        identity.model_identity == Some(self.model_identity)
            && identity.feature_revision == chunks::FEATURE_REVISION
            && identity.semantic_revision == 0
            && self.contexts.iter().any(|context| {
                identity.library_revision == context.library_revision
                    && identity.candidate_set == context.candidate_set
                    && identity.policy == context.policy
                    && identity.editor_request == context.editor_request
                    && context.task_interfaces.contains(&identity.task_interface)
            })
    }
}

impl chunks::CandidateRanker for ModelRanker<'_> {
    fn model_identity(&self) -> [u8; chunks::REVISION_BYTES] {
        self.model_identity
    }

    fn provenance_matches(&self, identity: &chunks::RankingIdentity) -> bool {
        let matched = self.context_matches(identity);
        *self.active_identity.borrow_mut() = matched.then(|| identity.clone());
        matched
    }

    fn score(
        &mut self,
        request: &chunks::CandidateScoreRequest<'_>,
    ) -> Result<chunks::CandidateScoreResponse, chunks::ScorerFailure> {
        let active_identity = self.active_identity.borrow();
        let Some(identity) = active_identity.as_ref() else {
            return Err(chunks::ScorerFailure::StaleProvenance);
        };
        if request.request_identity != identity.request
            || request.model_identity != self.model_identity
        {
            return Err(chunks::ScorerFailure::StaleProvenance);
        }
        let Some(context) = self.contexts.iter().find(|context| {
            identity.library_revision == context.library_revision
                && identity.candidate_set == context.candidate_set
                && identity.policy == context.policy
                && identity.editor_request == context.editor_request
                && context.task_interfaces.contains(&identity.task_interface)
        }) else {
            return Err(chunks::ScorerFailure::StaleProvenance);
        };
        if !context.allowed_candidates.contains(&request.candidate) {
            return Err(chunks::ScorerFailure::InvalidResponse);
        }
        if !request.features.iter().all(|feature| feature.is_finite()) {
            return Err(chunks::ScorerFailure::InvalidFeature);
        }
        let started = std::time::Instant::now();
        let score = numeric_output_traced(
            self.graph,
            self.snapshot,
            request.features,
            &mut self.scratch,
            &mut self.evaluated_nodes,
        )
        .map_err(|_| chunks::ScorerFailure::Evaluation)?;
        self.scorer_elapsed_ns = self.scorer_elapsed_ns.saturating_add(elapsed_ns(started));
        self.score_calls = self.score_calls.saturating_add(1);
        self.scored.push(ScoredCandidate {
            candidate: request.candidate,
            features: *request.features,
            score,
        });
        if !score.is_finite() {
            return Err(chunks::ScorerFailure::Evaluation);
        }
        Ok(chunks::CandidateScoreResponse {
            request_identity: request.request_identity,
            model_identity: self.model_identity,
            candidate: request.candidate,
            score,
        })
    }
}

pub(crate) struct ShuffledRanker {
    pub(crate) model_identity: [u8; chunks::REVISION_BYTES],
    pub(crate) contexts: Vec<RankingContext>,
    pub(crate) active_identity: std::cell::RefCell<Option<chunks::RankingIdentity>>,
    pub(crate) scores: std::collections::BTreeMap<u32, f32>,
    pub(crate) score_calls: u32,
    pub(crate) scored: Vec<ScoredCandidate>,
}

impl chunks::CandidateRanker for ShuffledRanker {
    fn model_identity(&self) -> [u8; chunks::REVISION_BYTES] {
        self.model_identity
    }

    fn provenance_matches(&self, identity: &chunks::RankingIdentity) -> bool {
        let matched = identity.model_identity == Some(self.model_identity)
            && identity.feature_revision == chunks::FEATURE_REVISION
            && identity.semantic_revision == 0
            && self.contexts.iter().any(|context| {
                identity.library_revision == context.library_revision
                    && identity.candidate_set == context.candidate_set
                    && identity.policy == context.policy
                    && identity.editor_request == context.editor_request
                    && context.task_interfaces.contains(&identity.task_interface)
            });
        *self.active_identity.borrow_mut() = matched.then(|| identity.clone());
        matched
    }

    fn score(
        &mut self,
        request: &chunks::CandidateScoreRequest<'_>,
    ) -> Result<chunks::CandidateScoreResponse, chunks::ScorerFailure> {
        let active_identity = self.active_identity.borrow();
        let Some(identity) = active_identity.as_ref() else {
            return Err(chunks::ScorerFailure::StaleProvenance);
        };
        if request.request_identity != identity.request
            || request.model_identity != self.model_identity
            || !request.features.iter().all(|feature| feature.is_finite())
        {
            return Err(chunks::ScorerFailure::StaleProvenance);
        }
        let Some(score) = self.scores.get(&request.candidate.ordinal).copied() else {
            return Err(chunks::ScorerFailure::InvalidResponse);
        };
        self.score_calls = self.score_calls.saturating_add(1);
        self.scored.push(ScoredCandidate {
            candidate: request.candidate,
            features: *request.features,
            score,
        });
        Ok(chunks::CandidateScoreResponse {
            request_identity: request.request_identity,
            model_identity: self.model_identity,
            candidate: request.candidate,
            score,
        })
    }
}
