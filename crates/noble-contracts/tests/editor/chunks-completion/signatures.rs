use super::super::monotonic_now_ns;
use super::support::*;

#[test]
fn equal_interfaces_do_not_transfer_behavior() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let empty_inputs = Vec::new();
    let agree = {
        let body = Recipe {
            nodes: vec![
                Node::I64(1),
                Node::I64(1),
                Node::Operation {
                    op: Op::Eq,
                    instantiation: 0,
                },
            ],
            entry: vec![0, 1, 2],
        };
        envelope(&[], &[Ty::Bool], body, b"1 1 = ")
    };
    let differ = {
        let body = Recipe {
            nodes: vec![
                Node::I64(1),
                Node::I64(2),
                Node::Operation {
                    op: Op::Eq,
                    instantiation: 0,
                },
            ],
            entry: vec![0, 1, 2],
        };
        envelope(&[], &[Ty::Bool], body, b"1 2 = ")
    };
    for (chunk, expected, agreement) in [
        (agree.clone(), Scalar::Bool(true), true),
        (differ.clone(), Scalar::Bool(true), false),
    ] {
        let entries = admitted(&session, &[chunk])?;
        let mut candidate = request(hole(), vec![0], vec![], vec![Ty::Bool]);
        candidate.fixtures = Some(fixtures(6, vec![(empty_inputs.clone(), vec![expected])]));
        let response = complete(&session, &entries, &candidate, monotonic_now_ns);
        let Response::Accepted { accepted, report } = response else {
            return Err(format!("outcome: {}", response.outcome()));
        };
        // The same ordered interface is accepted in both libraries; the host
        // fixture separately reports that one of them behaves differently.
        assert_eq!(accepted.interface.stack_out, vec![Ty::Bool]);
        let reported = report
            .fixtures
            .as_ref()
            .ok_or_else(|| String::from("fixture report"))?;
        assert_eq!(reported.agreement, agreement, "{reported:?}");
    }
    // Both bodies remain distinct entries when a library holds both.
    let entries = admitted(&session, &[agree, differ])?;
    assert_eq!(entries.entries().len(), 2);
    Ok(())
}

#[test]
fn canonical_order_selects_the_first_equal_interface_chunk() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let one = literal(1, &[Ty::I64], b"1 ");
    let two = literal(2, &[Ty::I64], b"2 ");
    let entries = admitted(&session, &[one, two])?;
    let first = entries
        .entries()
        .iter()
        .min_by(|left, right| left.envelope().chunk_id.cmp(&right.envelope().chunk_id))
        .ok_or_else(|| String::from("entries"))?
        .envelope()
        .chunk_id;
    let value = if entries
        .entries()
        .first()
        .is_some_and(|entry| entry.envelope().chunk_id == first)
    {
        Scalar::I64(1)
    } else {
        Scalar::I64(2)
    };
    let mut candidate = request(hole(), vec![0], vec![], vec![Ty::I64]);
    candidate.fixtures = Some(fixtures(8, vec![(Vec::new(), vec![value])]));
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { accepted, report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(accepted.selected, vec![first]);
    let reported = report
        .fixtures
        .as_ref()
        .ok_or_else(|| String::from("fixture report"))?;
    assert!(reported.agreement, "{reported:?}");
    Ok(())
}

#[test]
fn no_result_is_not_a_completeness_claim() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")])?;
    let candidate = request(
        editor(vec![
            EditorNode::Hole,
            EditorNode::Word(String::from("drop")),
            EditorNode::Word(String::from("drop")),
            EditorNode::Word(String::from("=")),
            EditorNode::Word(String::from("swap")),
        ]),
        vec![0],
        vec![],
        vec![Ty::Bool, Ty::I64],
    );
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::NoResult { report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(report.candidates, 5);
    assert_eq!(report.rejected, 5);
    assert!(!report.diagnostics.is_empty());
    let longer_source = b"1 1 1 1 1 drop drop = swap ";
    let prepared = session
        .prepare(longer_source, &[], limits())
        .map_err(|error| format!("five-chunk completion: {error:?}"))?;
    assert_eq!(prepared.output(), [Ty::Bool, Ty::I64]);
    Ok(())
}
