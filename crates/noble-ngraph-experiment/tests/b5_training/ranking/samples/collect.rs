use super::*;

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

pub(crate) fn label_description(
    outcome: &chunks::CandidateOutcome,
    response: &chunks::Response,
) -> String {
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

struct TaskCapture<'a> {
    session: &'a noble_contracts::source::Session,
    library: &'a Library,
    spec: &'a TaskSpec,
    split: &'a str,
    capture: &'a FeatureCapture,
}

struct CandidateLabel {
    target: f32,
    attempt: chunks::CandidateOutcome,
    request_identity: [u8; chunks::REVISION_BYTES],
    outcome: String,
    elapsed_ns: u64,
}

fn label_for_queue(context: &TaskCapture<'_>, candidate: &chunks::QueueCandidate) -> Result<CandidateLabel, String> {
    if candidate.sequence.is_empty() {
        let attempt = candidate_attempt_for(&context.capture.outcomes, &[])
            .cloned()
            .ok_or_else(|| format!("empty sequence refusal missing for {}", context.spec.id()))?;
        return Ok(CandidateLabel {
            target: 0.0,
            request_identity: attempt.request_identity,
            outcome: label_description(&attempt, &context.capture.response),
            attempt,
            elapsed_ns: 0,
        });
    }
    if candidate.sequence.len() != 1 {
        return Err(format!("training queue exceeded one-chunk scope for {}", context.spec.id()));
    }
    let envelope = context.library.entries()
        .get(usize::from(candidate.sequence[0]))
        .ok_or_else(|| format!("candidate chunk index missing for {}", context.spec.id()))?
        .envelope();
    let started = std::time::Instant::now();
    let (target, attempt, response) = candidate_label(context.session, context.library, context.spec, envelope)?;
    debug_assert_eq!(candidate.sequence.len(), 1);
    debug_assert!(context.library.entries().iter().any(|entry| entry.envelope().chunk_id == envelope.chunk_id));
    Ok(CandidateLabel {
        target,
        request_identity: attempt.request_identity,
        outcome: label_description(&attempt, &response),
        attempt,
        elapsed_ns: elapsed_ns(started),
    })
}

fn row_for_candidate(context: &TaskCapture<'_>, candidate: &chunks::QueueCandidate) -> Result<(TrainingRow, u64), String> {
    let index = usize::try_from(candidate.ordinal).map_err(|error| error.to_string())?;
    let captured = context.capture.features.get(index).ok_or_else(|| {
        format!("feature missing for {} ordinal {}", context.spec.id(), candidate.ordinal)
    })?;
    if captured.candidate.ordinal != candidate.ordinal {
        return Err(format!("candidate order drifted for {}", context.spec.id()));
    }
    if !captured.features.iter().all(|feature| feature.is_finite()) {
        return Err(format!("nonfinite feature in {} ordinal {}", context.spec.id(), candidate.ordinal));
    }
    let label = label_for_queue(context, candidate)?;
    debug_assert_eq!(context.capture.identity.library_revision, *context.library.revision());
    debug_assert!(context.spec.family < FAMILY_NAMES.len());
    let row = TrainingRow {
        task_id: context.spec.id(),
        family: FAMILY_NAMES[context.spec.family].to_string(),
        split: context.split.to_string(),
        ordinal: candidate.ordinal,
        sequence: candidate.sequence.clone(),
        candidate: captured.candidate,
        label_candidate: label.attempt.candidate,
        features: captured.features,
        target: label.target,
        accepted: label.attempt.accepted,
        fixture_agreement: label.attempt.fixture_agreement,
        refusal_class: label.attempt.refusal_class,
        outcome: label.outcome,
        request_identity: context.capture.identity.request,
        label_request_identity: label.request_identity,
        candidate_set_identity: context.capture.identity.candidate_set,
        independent_goal_identity: context.capture.identity.independent_goal,
        task_interface_identity: context.capture.identity.task_interface,
        policy_identity: context.capture.identity.policy,
        library_revision: context.capture.identity.library_revision,
        library_identity: library_identity(context.library),
        model_identity: context.capture.identity.model_identity.unwrap_or([0; chunks::REVISION_BYTES]),
        cost: label.attempt.cost,
    };
    Ok((row, label.elapsed_ns))
}

fn collect_task_rows(
    session: &noble_contracts::source::Session,
    library: &Library,
    spec: &TaskSpec,
    split: &str,
    data: &mut Dataset,
) -> Result<FeatureCapture, String> {
    let feature_started = std::time::Instant::now();
    let capture = collect_features(session, library, spec)?;
    data.feature_collection_ns = data.feature_collection_ns.saturating_add(elapsed_ns(feature_started));
    let queue = chunks::candidate_queue(library, chunks::CandidateLimit::from(request(spec, library).budget.candidates));
    if queue.candidates().len() != capture.features.len() {
        return Err(format!("queue/features diverged for {}", spec.id()));
    }
    debug_assert_eq!(queue.candidates().len(), capture.features.len());
    debug_assert_eq!(capture.identity.library_revision, *library.revision());
    let context = TaskCapture { session, library, spec, split, capture: &capture };
    for candidate in queue.candidates() {
        let (row, elapsed) = row_for_candidate(&context, candidate)?;
        data.target_label_ns = data.target_label_ns.saturating_add(elapsed);
        data.rows.push(row);
    }
    Ok(capture)
}

fn add_contexts(libraries: &[Library], data: &mut Dataset,
    keys: std::collections::BTreeSet<(usize, [u8; chunks::REVISION_BYTES],
        [u8; chunks::REVISION_BYTES], [u8; chunks::REVISION_BYTES], [u8; chunks::REVISION_BYTES])>
) -> Result<(), String> {
    for (family, revision, candidate_set, policy, editor_request) in keys {
        let library = libraries.get(family)
            .ok_or_else(|| String::from("ranking context library missing"))?;
        let mut task_interfaces = Vec::new();
        let mut allowed_candidates = Vec::new();
        for row in data.rows.iter().filter(|row| row.family == FAMILY_NAMES[family]) {
            if !task_interfaces.contains(&row.task_interface_identity) {
                task_interfaces.push(row.task_interface_identity);
            }
            if !allowed_candidates.contains(&row.candidate) {
                allowed_candidates.push(row.candidate);
            }
        }
        allowed_candidates.sort_by_key(|candidate| candidate.ordinal);
        debug_assert_eq!(revision, *library.revision());
        debug_assert!(allowed_candidates.len() <= usize::try_from(chunks::MAX_CANDIDATES).expect("fixed profile"));
        data.contexts.push(RankingContext {
            family: FAMILY_NAMES[family].to_string(),
            library_identity: library_identity(library),
            library_revision: revision,
            candidate_set, policy, editor_request, task_interfaces, allowed_candidates,
        });
    }
    Ok(())
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
        let library = libraries.get(spec.family)
            .ok_or_else(|| format!("library missing for {}", spec.id()))?;
        let capture = collect_task_rows(session, library, spec, split, &mut data)?;
        known_contexts.insert((spec.family, capture.identity.library_revision,
            capture.identity.candidate_set, capture.identity.policy, capture.identity.editor_request));
    }
    add_contexts(libraries, &mut data, known_contexts)?;
    Ok(data)
}
