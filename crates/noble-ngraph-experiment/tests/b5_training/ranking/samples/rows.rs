use super::*;

#[derive(Clone, Debug)]
pub(crate) struct TrainingRow {
    pub(crate) task_id: String,
    pub(crate) family: String,
    pub(crate) split: String,
    pub(crate) ordinal: u32,
    pub(crate) sequence: Vec<u16>,
    pub(crate) candidate: chunks::CandidateId,
    pub(crate) label_candidate: chunks::CandidateId,
    pub(crate) features: chunks::CandidateFeatureVector,
    pub(crate) target: f32,
    pub(crate) accepted: bool,
    pub(crate) fixture_agreement: Option<bool>,
    pub(crate) refusal_class: Option<chunks::Class>,
    pub(crate) outcome: String,
    pub(crate) request_identity: [u8; chunks::REVISION_BYTES],
    pub(crate) label_request_identity: [u8; chunks::REVISION_BYTES],
    pub(crate) candidate_set_identity: [u8; chunks::REVISION_BYTES],
    pub(crate) independent_goal_identity: [u8; chunks::REVISION_BYTES],
    pub(crate) task_interface_identity: [u8; chunks::REVISION_BYTES],
    pub(crate) policy_identity: [u8; chunks::REVISION_BYTES],
    pub(crate) library_revision: [u8; chunks::REVISION_BYTES],
    pub(crate) library_identity: [u8; chunks::REVISION_BYTES],
    pub(crate) model_identity: [u8; chunks::REVISION_BYTES],
    pub(crate) cost: chunks::CandidateAttemptCost,
}

impl TrainingRow {
    pub(crate) fn point(&self) -> Point {
        Point {
            inputs: self.features.to_vec(),
            targets: vec![self.target],
        }
    }

    pub(crate) fn identity_json(&self) -> Value {
        json!({
            "request": hex(&self.request_identity),
            "label_request": hex(&self.label_request_identity),
            "candidate_set": hex(&self.candidate_set_identity),
            "independent_goal": hex(&self.independent_goal_identity),
            "task_interface": hex(&self.task_interface_identity),
            "policy": hex(&self.policy_identity),
            "library_revision": hex(&self.library_revision),
            "library_identity": hex(&self.library_identity),
            "feature_revision": chunks::FEATURE_REVISION,
            "semantic_revision": 0,
            "feature_capture_identity": hex(&self.model_identity)
        })
    }

    pub(crate) fn json(&self) -> Value {
        let feature_bits: Vec<u32> = self.features.iter().map(|value| value.to_bits()).collect();
        json!({
            "task_id": self.task_id,
            "family": self.family,
            "split": self.split,
            "ordinal": self.ordinal,
            "sequence": self.sequence,
            "candidate_id": {"ordinal": self.candidate.ordinal, "digest": hex(&self.candidate.digest)},
            "label_candidate_id": {"ordinal": self.label_candidate.ordinal, "digest": hex(&self.label_candidate.digest)},
            "features_f32_bits": feature_bits,
            "target_f32_bits": self.target.to_bits(),
            "target": self.target,
            "accepted": self.accepted,
            "fixture_agreement": self.fixture_agreement,
            "refusal_class": self.refusal_class.map(chunks::Class::name),
            "outcome": self.outcome,
            "identity": self.identity_json(),
            "attempt_cost_ns": {
                "feature_extraction": self.cost.feature_extraction_ns,
                "scorer": self.cost.scorer_elapsed_ns,
                "re_admission": self.cost.re_admission_ns,
                "resource_deltas": self.cost.resource_deltas
            },
            "proposer_provenance": {
                "source": if self.sequence.is_empty() { "empty-shortlex-candidate/no-library-entry" } else { "immutable-library-entry/exact-body-and-dependency-identity" },
                "candidate_id": {"ordinal": self.label_candidate.ordinal, "digest": hex(&self.label_candidate.digest)},
                "re_admission_ns": self.cost.re_admission_ns
            }
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CandidateContext {
    pub(crate) family: String,
    pub(crate) library_identity: [u8; chunks::REVISION_BYTES],
    pub(crate) library_revision: [u8; chunks::REVISION_BYTES],
    pub(crate) candidate_set: [u8; chunks::REVISION_BYTES],
    pub(crate) policy: [u8; chunks::REVISION_BYTES],
    pub(crate) editor_request: [u8; chunks::REVISION_BYTES],
    pub(crate) task_interfaces: Vec<[u8; chunks::REVISION_BYTES]>,
    pub(crate) allowed_candidates: Vec<chunks::CandidateId>,
}

#[derive(Default)]
pub(crate) struct Dataset {
    pub(crate) rows: Vec<TrainingRow>,
    pub(crate) contexts: Vec<CandidateContext>,
    pub(crate) feature_collection_ns: u64,
    pub(crate) target_label_ns: u64,
}
