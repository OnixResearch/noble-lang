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
pub(crate) struct RankingContext {
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
    pub(crate) contexts: Vec<RankingContext>,
    pub(crate) feature_collection_ns: u64,
    pub(crate) target_label_ns: u64,
}

pub(crate) fn elapsed_ns(start: std::time::Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

pub(crate) fn library_identity(library: &Library) -> [u8; chunks::REVISION_BYTES] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(library.revision());
    for entry in library.entries() {
        hasher.update(&entry.envelope().chunk_id);
        hasher.update(&entry.envelope().source_revision);
    }
    *hasher.finalize().as_bytes()
}

pub(crate) fn label_description(outcome: &chunks::CandidateOutcome, response: &chunks::Response) -> String {
    match (
        outcome.accepted,
        outcome.fixture_agreement,
        outcome.refusal_class,
    ) {
        (true, Some(true), _) => String::from("finite-fixture-success"),
        (true, Some(false), _) => String::from("accepted-fixture-mismatch"),
        (true, None, _) => String::from("accepted-without-fixture"),
        (false, _, Some(class)) => String::from(class.name()),
        (false, _, None) => String::from(response.outcome()),
    }
}

pub(crate) fn collect_split(
    session: &noble_contracts::source::Session,
    libraries: &[Library],
    tasks: &[TaskSpec],
    split: &str,
) -> Result<Dataset, String> {
    let mut data = Dataset::default();
    let mut known_contexts = std::collections::BTreeSet::new();
    for spec in tasks.iter().filter(|task| task.split() == split) {
        let library = libraries
            .get(spec.family)
            .ok_or_else(|| format!("library missing for {}", spec.id()))?;
        let feature_started = std::time::Instant::now();
        let capture = collect_features(session, library, spec)?;
        data.feature_collection_ns = data
            .feature_collection_ns
            .saturating_add(elapsed_ns(feature_started));
        let context_key = (
            spec.family,
            capture.identity.library_revision,
            capture.identity.candidate_set,
            capture.identity.policy,
            capture.identity.editor_request,
        );
        known_contexts.insert(context_key);
        let queue = chunks::candidate_queue(
            library,
            chunks::CandidateLimit::from(request(spec, library).budget.candidates),
        );
        if queue.candidates().len() != capture.features.len() {
            return Err(format!("queue/features diverged for {}", spec.id()));
        }
        for candidate in queue.candidates() {
            let captured = capture
                .features
                .get(usize::try_from(candidate.ordinal).map_err(|error| error.to_string())?)
                .ok_or_else(|| {
                    format!(
                        "feature missing for {} ordinal {}",
                        spec.id(),
                        candidate.ordinal
                    )
                })?;
            if captured.candidate.ordinal != candidate.ordinal {
                return Err(format!("candidate order drifted for {}", spec.id()));
            }
            if !captured.features.iter().all(|feature| feature.is_finite()) {
                return Err(format!(
                    "nonfinite feature in {} ordinal {}",
                    spec.id(),
                    candidate.ordinal
                ));
            }
            let (target, attempt, label_request_identity, outcome_description) = if candidate
                .sequence
                .is_empty()
            {
                let outcome = candidate_attempt_for(&capture.outcomes, &[])
                    .cloned()
                    .ok_or_else(|| format!("empty sequence refusal missing for {}", spec.id()))?;
                let description = label_description(&outcome, &capture.response);
                let request_identity = outcome.request_identity;
                (0.0, outcome, request_identity, description)
            } else {
                if candidate.sequence.len() != 1 {
                    return Err(format!(
                        "training queue exceeded one-chunk scope for {}",
                        spec.id()
                    ));
                }
                let index = usize::from(candidate.sequence[0]);
                let envelope = library
                    .entries()
                    .get(index)
                    .ok_or_else(|| format!("candidate chunk index missing for {}", spec.id()))?
                    .envelope();
                let label_started = std::time::Instant::now();
                let (target, outcome, response) =
                    candidate_label(session, library, spec, envelope)?;
                data.target_label_ns = data
                    .target_label_ns
                    .saturating_add(elapsed_ns(label_started));
                let description = label_description(&outcome, &response);
                let request_identity = outcome.request_identity;
                (target, outcome, request_identity, description)
            };
            let row = TrainingRow {
                task_id: spec.id(),
                family: FAMILY_NAMES[spec.family].to_string(),
                split: split.to_string(),
                ordinal: candidate.ordinal,
                sequence: candidate.sequence.clone(),
                candidate: captured.candidate,
                label_candidate: attempt.candidate,
                features: captured.features,
                target,
                accepted: attempt.accepted,
                fixture_agreement: attempt.fixture_agreement,
                refusal_class: attempt.refusal_class,
                outcome: outcome_description,
                request_identity: capture.identity.request,
                label_request_identity,
                candidate_set_identity: capture.identity.candidate_set,
                independent_goal_identity: capture.identity.independent_goal,
                task_interface_identity: capture.identity.task_interface,
                policy_identity: capture.identity.policy,
                library_revision: capture.identity.library_revision,
                library_identity: library_identity(library),
                model_identity: capture
                    .identity
                    .model_identity
                    .unwrap_or([0; chunks::REVISION_BYTES]),
                cost: attempt.cost,
            };
            data.rows.push(row);
        }
    }
    for (family, revision, candidate_set, policy, editor_request) in known_contexts {
        let library = libraries
            .get(family)
            .ok_or_else(|| String::from("ranking context library missing"))?;
        let mut task_interfaces = Vec::new();
        let mut allowed_candidates = Vec::new();
        for row in data
            .rows
            .iter()
            .filter(|row| row.family == FAMILY_NAMES[family])
        {
            if !task_interfaces.contains(&row.task_interface_identity) {
                task_interfaces.push(row.task_interface_identity);
            }
            if !allowed_candidates.contains(&row.candidate) {
                allowed_candidates.push(row.candidate);
            }
        }
        allowed_candidates.sort_by_key(|candidate| candidate.ordinal);
        data.contexts.push(RankingContext {
            family: FAMILY_NAMES[family].to_string(),
            library_identity: library_identity(library),
            library_revision: revision,
            candidate_set,
            policy,
            editor_request,
            task_interfaces,
            allowed_candidates,
        });
    }
    Ok(data)
}
