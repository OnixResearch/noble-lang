use super::super::monotonic_now_ns;
use super::support::*;

#[test]
fn lowered_candidate_limit_exhausts_before_the_next_proposal() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")])?;
    let mut candidate = request(
        editor(vec![
            EditorNode::Hole,
            EditorNode::Word(String::from("swap")),
        ]),
        vec![0],
        vec![],
        vec![Ty::Bool, Ty::I64],
    );
    candidate.budget.candidates = 2;
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Exhausted { refusal, report } = response else {
        return Err(format!("candidate budget: {}", response.outcome()));
    };
    assert_eq!(
        refusal.resource,
        Some(Resource::Candidates),
        "refusal={refusal:?}, candidates={}, checking_work={}, rejected={}",
        report.candidates,
        report.checking_work,
        report.rejected,
    );
    assert_eq!(Budget::PROFILE.candidates, 128);
    assert_eq!(refusal.observed, 3);
    assert_eq!(refusal.limit, 2);
    assert_eq!(report.candidates, 2);
    assert_eq!(report.rejected, 2);
    assert!(!report.diagnostics.is_empty());
    Ok(())
}

#[test]
fn malformed_editor_shapes_are_classified_before_any_proposal() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")])?;
    reject_malformed_editor_shapes(&session, &entries)?;
    reject_unsupported_request_forms(&session, &entries)
}

fn reject_malformed_editor_shapes(
    session: &noble_contracts::source::Session,
    entries: &Library,
) -> Result<(), String> {
    let two_holes = request(
        editor(vec![EditorNode::Hole, EditorNode::Hole]),
        vec![0],
        vec![],
        vec![Ty::I64],
    );
    let response = complete(session, entries, &two_holes, monotonic_now_ns);
    let Response::Invalid { refusal, report } = response else {
        return Err(format!("two holes: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.editor");
    assert_eq!(report.candidates, 0);

    let no_hole = request(
        editor(vec![EditorNode::Integer(1)]),
        vec![0],
        vec![],
        vec![Ty::I64],
    );
    let response = complete(session, entries, &no_hole, monotonic_now_ns);
    let Response::Invalid { refusal, .. } = response else {
        return Err(format!("no hole: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.editor");

    let wrong_path = request(hole(), vec![3], vec![], vec![Ty::I64]);
    let response = complete(session, entries, &wrong_path, monotonic_now_ns);
    let Response::Invalid { refusal, .. } = response else {
        return Err(format!("wrong path: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.hole_path");

    assert_eq!(report.candidates, 0);
    Ok(())
}

fn reject_unsupported_request_forms(
    session: &noble_contracts::source::Session,
    entries: &Library,
) -> Result<(), String> {
    let foreign = request(
        editor(vec![
            EditorNode::Quotation(vec![EditorNode::Hole]),
            EditorNode::Word(String::from("test.emit")),
        ]),
        vec![0, 0],
        vec![],
        vec![Ty::I64],
    );
    let response = complete(session, entries, &foreign, monotonic_now_ns);
    let Response::Unsupported { refusal, .. } = response else {
        return Err(format!("foreign word: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.editor");

    let mut newer = request(hole(), vec![0], vec![], vec![Ty::I64]);
    newer.editor.format = 2;
    let response = complete(session, entries, &newer, monotonic_now_ns);
    assert_eq!(response.outcome(), "unsupported");

    let mut profile = request(hole(), vec![0], vec![], vec![Ty::I64]);
    profile.profile = String::from("pure-i64-bool/2");
    let response = complete(session, entries, &profile, monotonic_now_ns);
    let Response::Unsupported { refusal, .. } = response else {
        return Err(format!("profile: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.profile");

    let text = request(
        editor(vec![EditorNode::Hole]),
        vec![0],
        vec![],
        vec![Ty::Text],
    );
    let response = complete(session, entries, &text, monotonic_now_ns);
    let Response::Unsupported { refusal, .. } = response else {
        return Err(format!("text task: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.task.stack_out");
    Ok(())
}

#[test]
fn an_empty_sequence_is_identity() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")])?;
    let candidate = request(
        editor(vec![EditorNode::Integer(2), EditorNode::Hole]),
        vec![1],
        vec![],
        vec![Ty::I64, Ty::I64],
    );
    // With two literals and a hole, the empty replacement leaves `2` alone and
    // does not reach `I64`; one literal chunk completes it.
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    assert_eq!(response.outcome(), "accepted");
    let Response::Accepted { accepted, .. } = response else {
        return Err(format!(
            "accepted outcome has wrong response variant: {}",
            response.outcome()
        ));
    };
    assert_eq!(accepted.source, b"2 1 ");
    let empty = request(
        editor(vec![EditorNode::Hole]),
        vec![0],
        vec![Ty::Bool],
        vec![Ty::Bool],
    );
    let response = complete(&session, &entries, &empty, monotonic_now_ns);
    let Response::Accepted { accepted, .. } = response else {
        return Err(format!("identity: {}", response.outcome()));
    };
    assert!(accepted.selected.is_empty());
    assert_eq!(accepted.equation.tail, 1);
    Ok(())
}

#[test]
fn operations_outside_the_whitelist_are_not_search_surface() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")])?;
    // `dip`, `quote`, `compose` and `if` exist in the namespace but are outside
    // the profile, so a request cannot route a completion through them.
    for word in ["dip", "quote", "compose", "if", "over", "not", "reflect"] {
        let candidate = request(
            editor(vec![EditorNode::Hole, EditorNode::Word(String::from(word))]),
            vec![0],
            vec![],
            vec![Ty::I64],
        );
        assert_eq!(candidate.editor.nodes.len(), 2);
        assert_eq!(candidate.hole_path, [0]);
        let response = complete(&session, &entries, &candidate, monotonic_now_ns);
        assert_eq!(
            response.outcome(),
            "unsupported",
            "{word}: {}",
            response.outcome()
        );
    }
    Ok(())
}
