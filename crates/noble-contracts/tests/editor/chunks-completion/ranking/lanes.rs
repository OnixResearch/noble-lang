use super::*;

fn normalized_count(value_count: usize, maximum_count: u32) -> Result<f32, String> {
    let maximum_len = usize::try_from(maximum_count)
        .map_err(|_| String::from("feature maximum does not fit usize"))?;
    let bounded_count = value_count.min(maximum_len);
    let bounded_count = u16::try_from(bounded_count)
        .map_err(|_| String::from("normalized feature count does not fit u16"))?;
    let maximum_count = u16::try_from(maximum_count)
        .map_err(|_| String::from("feature maximum does not fit u16"))?;
    if maximum_count == 0 {
        return Err(String::from("feature maximum must be nonzero"));
    }
    Ok(f32::from(bounded_count) / f32::from(maximum_count))
}

fn first_chunk_features(
    session: &noble_contracts::source::Session,
    library: &Library,
    request: &Request,
) -> Result<CandidateFeatureVector, String> {
    let queue = chunks::candidate_queue(library, CandidateLimit::from(Budget::PROFILE.candidates));
    let candidate = queue
        .candidates()
        .iter()
        .find(|candidate| candidate.sequence.len() == 1)
        .ok_or_else(|| String::from("candidate queue omitted one-chunk proposals"))?;
    let candidate_ordinal = usize::try_from(candidate.ordinal)
        .map_err(|_| String::from("candidate ordinal overflow"))?;
    let mut ranker = test_ranker(library, vec![0.0; 128]);
    let mut scorer = ranker_session(&mut ranker);
    let _ = chunks::complete_ranked(session, library, request, &mut scorer, monotonic_now_ns);
    ranker
        .features_seen
        .get(candidate_ordinal)
        .copied()
        .ok_or_else(|| String::from("one-chunk candidate features were not extracted"))
}

#[test]
fn features_bind_hole_constraints_and_full_chunk_interfaces() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = admitted(&session, &[literal(7, &[Ty::I64], b"7 ")])?;
    let mut request = request(
        editor(vec![
            EditorNode::Hole,
            EditorNode::Word(String::from("drop")),
        ]),
        vec![0],
        Vec::new(),
        Vec::new(),
    );
    request.fixtures = Some(fixtures(75, vec![(Vec::new(), Vec::new())]));
    let analysis = session
        .analyze_editor(
            &request.editor,
            &request.task.stack_in,
            noble_contracts::Limits {
                bytes: 65_536,
                nodes: 16_384,
                depth: 64,
                work: 2_000_000,
            },
        )
        .map_err(|error| format!("editor analysis: {error:?}"))?;
    let hole = analysis
        .holes
        .first()
        .ok_or_else(|| String::from("one-hole analysis omitted its constraint"))?;
    let features = first_chunk_features(&session, &library, &request)?;
    let entry = library
        .entries()
        .first()
        .ok_or_else(|| String::from("fixture chunk was not retained"))?;
    let interface = entry.interface();
    let i64_count = interface
        .stack_in
        .iter()
        .chain(&interface.stack_out)
        .filter(|ty| **ty == Ty::I64)
        .count();
    let bool_count = interface
        .stack_in
        .iter()
        .chain(&interface.stack_out)
        .filter(|ty| **ty == Ty::Bool)
        .count();
    assert_eq!(
        features[9],
        normalized_count(hole.required_output_suffix.len(), chunks::MAX_STACK_HEIGHT)?
    );
    assert_eq!(
        features[10],
        normalized_count(hole.known_effects.len(), chunks::MAX_DIAGNOSTICS)?
    );
    assert_eq!(features[11], if hole.unresolved_effect { 1.0 } else { 0.0 });
    assert_eq!(
        features[34],
        normalized_count(i64_count, chunks::MAX_STACK_HEIGHT)?
    );
    assert_eq!(
        features[35],
        normalized_count(bool_count, chunks::MAX_STACK_HEIGHT)?
    );
    Ok(())
}
