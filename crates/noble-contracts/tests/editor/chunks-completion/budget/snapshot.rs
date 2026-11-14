use super::super::monotonic_now_ns;
use super::support::*;

#[test]
fn retention_and_stack_caps_are_shared() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")])?;
    for resource in [
        Resource::LibraryBytes,
        Resource::Entries,
        Resource::RetainedBytes,
        Resource::StackHeight,
    ] {
        let candidate = with_budget(hole(), vec![0], vec![], vec![Ty::I64], resource);
        expect_exhausted(&session, &entries, &candidate, resource)?;
    }
    Ok(())
}

#[test]
fn editor_context_source_lower_bound_exhausts_before_analysis() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let mut nodes = Vec::with_capacity(2_050);
    nodes.push(EditorNode::Hole);
    for _ in 0..2_049 {
        nodes.push(EditorNode::Integer(1));
    }
    let candidate = request(editor(nodes), vec![0], Vec::new(), Vec::new());
    let response = complete(&session, &library(), &candidate, monotonic_now_ns);
    let Response::Exhausted { refusal, report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(refusal.resource, Some(Resource::SourceBytes));
    assert_eq!(refusal.location.field, "request.editor");
    assert_eq!(refusal.observed, 4_098);
    assert_eq!(refusal.limit, Budget::PROFILE.source_bytes);
    assert_eq!(report.candidates, 0);
    Ok(())
}

#[test]
fn enumeration_and_diagnostic_caps_are_shared() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")])?;
    let candidate = with_budget(hole(), vec![0], vec![], vec![Ty::I64], Resource::Candidates);
    assert_eq!(candidate.budget.candidates, 0);
    expect_exhausted(&session, &entries, &candidate, Resource::Candidates)?;

    let candidate = with_budget(
        hole(),
        vec![0],
        vec![],
        vec![Ty::I64],
        Resource::ComposedChunks,
    );
    assert_eq!(candidate.budget.composed_chunks, 0);
    expect_exhausted(&session, &entries, &candidate, Resource::ComposedChunks)?;

    let candidate = with_budget(
        hole(),
        vec![0],
        vec![],
        vec![Ty::Bool],
        Resource::Diagnostics,
    );
    assert_eq!(candidate.budget.diagnostics, 0);
    expect_exhausted(&session, &entries, &candidate, Resource::Diagnostics)?;
    Ok(())
}
