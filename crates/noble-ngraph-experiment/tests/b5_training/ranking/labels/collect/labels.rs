use super::*;

pub(crate) fn report_of(response: &chunks::Response) -> &chunks::Report {
    match response {
        chunks::Response::Accepted { report, .. }
        | chunks::Response::Invalid { report, .. }
        | chunks::Response::Unsupported { report, .. }
        | chunks::Response::Exhausted { report, .. }
        | chunks::Response::NoResult { report }
        | chunks::Response::InternalFailure { report, .. } => report,
    }
}

pub(crate) fn candidate_attempt_for<'a>(
    outcomes: &'a [chunks::CandidateOutcome],
    sequence: &[u16],
) -> Option<&'a chunks::CandidateOutcome> {
    outcomes.iter().find(|outcome| outcome.sequence == sequence)
}

fn isolated_candidate_response(
    session: &noble_contracts::source::Session,
    library: &Library,
    spec: &TaskSpec,
    envelope: &Envelope,
) -> Result<(chunks::CandidateOutcome, chunks::Response), String> {
    let mut isolated = Library::new(*library.revision());
    isolated
        .admit(session, envelope.clone())
        .map_err(|refusal| format!("isolated target envelope refused: {refusal:?}"))?;
    let (outcomes, response) = observed_isolated_response(session, &isolated, spec);
    let candidate_outcome = candidate_attempt_for(&outcomes, &[0])
        .cloned()
        .ok_or_else(|| format!("isolated candidate was not attempted for {}", spec.id()))?;
    Ok((candidate_outcome, response))
}

fn observed_isolated_response(
    session: &noble_contracts::source::Session,
    isolated: &Library,
    spec: &TaskSpec,
) -> (Vec<chunks::CandidateOutcome>, chunks::Response) {
    let request = request(spec, isolated);
    let mut outcomes = Vec::new();
    let mut observer = |outcome: &chunks::CandidateOutcome| outcomes.push(outcome.clone());
    let response = chunks::complete_observed(
        session,
        isolated,
        &request,
        &mut observer,
        monotonic_now_ns,
    );
    (outcomes, response)
}

fn is_expected_chunk_selected(response: &chunks::Response, envelope: &Envelope) -> bool {
    matches!(response, chunks::Response::Accepted { accepted, .. }
        if accepted.selected.as_slice() == [envelope.chunk_id])
}

fn checked_target_label(
    spec: &TaskSpec,
    envelope: &Envelope,
    candidate_outcome: &chunks::CandidateOutcome,
    response: &chunks::Response,
) -> Result<f32, String> {
    check_target_selection(spec, envelope, candidate_outcome, response)?;
    Ok(if candidate_outcome.accepted && candidate_outcome.fixture_agreement.unwrap_or(false) {
        1.0
    } else {
        0.0
    })
}

fn check_target_selection(
    spec: &TaskSpec,
    envelope: &Envelope,
    candidate_outcome: &chunks::CandidateOutcome,
    response: &chunks::Response,
) -> Result<(), String> {
    if candidate_outcome.candidate.ordinal != 1 {
        return Err(format!(
            "isolated candidate ordinal changed for {}",
            spec.id()
        ));
    }
    if candidate_outcome.accepted && !is_expected_chunk_selected(response, envelope) {
        return Err(format!(
            "isolated admission selected a different body for {}",
            spec.id()
        ));
    }
    Ok(())
}

pub(crate) fn candidate_label(
    session: &noble_contracts::source::Session,
    library: &Library,
    spec: &TaskSpec,
    envelope: &Envelope,
) -> Result<(f32, chunks::CandidateOutcome, chunks::Response), String> {
    let (outcome, response) = isolated_candidate_response(session, library, spec, envelope)?;
    let target = checked_target_label(spec, envelope, &outcome, &response)?;
    Ok((target, outcome, response))
}
