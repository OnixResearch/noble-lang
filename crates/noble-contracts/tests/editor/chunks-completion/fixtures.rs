use super::super::monotonic_now_ns;
use super::support::*;

#[test]
fn nested_quotation_is_reparsed_as_source() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")])?;
    let candidate = request(
        editor(vec![
            EditorNode::Quotation(vec![EditorNode::Hole]),
            EditorNode::Word(String::from("run")),
        ]),
        vec![0, 0],
        vec![],
        vec![Ty::I64],
    );
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { accepted, report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(accepted.source, b"[ 1 ] run ");
    assert_eq!(accepted.selected.len(), 1);
    assert_eq!(report.quotation_depth, 1);
    Ok(())
}

#[test]
fn wrapping_i64_boundaries_match_independent_results() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    type ArithmeticFixture = (Op, &'static [u8], Vec<(i64, i64, i64)>);
    let cases: [ArithmeticFixture; 3] = [
        (
            Op::Add,
            b"+ ",
            vec![
                (i64::MAX, 1, i64::MIN),
                (i64::MIN, -1, i64::MAX),
                (-1, 1, 0),
                (0, 1, 1),
                (1, 0, 1),
            ],
        ),
        (
            Op::Sub,
            b"- ",
            vec![
                (i64::MIN, 1, i64::MAX),
                (i64::MAX, -1, i64::MIN),
                (1, 0, 1),
                (0, 1, -1),
                (1, 3, -2),
            ],
        ),
        (
            Op::Mul,
            b"* ",
            vec![
                (i64::MIN, -1, i64::MIN),
                (i64::MAX, 1, i64::MAX),
                (-1, -1, 1),
                (0, i64::MAX, 0),
                (1, i64::MIN, i64::MIN),
            ],
        ),
    ];
    assert_eq!(cases.len(), 3);
    assert!(cases.iter().all(|(_, _, values)| values.len() >= 5));
    for (op, source, values) in cases {
        check_arithmetic_fixture(&session, op, source, &values)?;
    }
    Ok(())
}

fn check_arithmetic_fixture(
    session: &noble_contracts::source::Session,
    op: Op,
    source: &[u8],
    values: &[(i64, i64, i64)],
) -> Result<(), String> {
    let recipe = Recipe {
        nodes: vec![Node::Operation {
            op,
            instantiation: 0,
        }],
        entry: vec![0],
    };
    let chunk = envelope(&[Ty::I64, Ty::I64], &[Ty::I64], recipe, source);
    let entries = admitted(session, &[chunk])?;
    let mut candidate = request(hole(), vec![0], vec![Ty::I64, Ty::I64], vec![Ty::I64]);
    candidate.fixtures = Some(fixtures(
        4,
        values
            .iter()
            .map(|(left, right, expected)| {
                (
                    vec![Scalar::I64(*left), Scalar::I64(*right)],
                    vec![Scalar::I64(*expected)],
                )
            })
            .collect(),
    ));
    let response = complete(session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { accepted, report } = response else {
        return Err(format!("{source:?}: {}", response.outcome()));
    };
    assert_eq!(accepted.source, source);
    let reported = report
        .fixtures
        .as_ref()
        .ok_or_else(|| String::from("fixture report"))?;
    assert!(reported.agreement, "{reported:?}");
    assert_eq!(reported.cases.len(), values.len());
    for (case, &(left, right, expected)) in reported.cases.iter().zip(values) {
        assert_eq!(case.inputs, [Scalar::I64(left), Scalar::I64(right)]);
        assert_eq!(case.expected, [Scalar::I64(expected)]);
        assert_eq!(case.actual, Some(vec![Scalar::I64(expected)]));
        assert!(case.agrees);
    }
    Ok(())
}

#[test]
fn boolean_results_have_independent_true_and_false_expectations() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let empty_inputs = Vec::new();
    for (left, right, expected, source) in [
        (1, 1, true, b"1 1 = ".as_slice()),
        (1, 2, false, b"1 2 = ".as_slice()),
    ] {
        let recipe = Recipe {
            nodes: vec![
                Node::I64(left),
                Node::I64(right),
                Node::Operation {
                    op: Op::Eq,
                    instantiation: 0,
                },
            ],
            entry: vec![0, 1, 2],
        };
        let entries = admitted(&session, &[envelope(&[], &[Ty::Bool], recipe, source)])?;
        let mut candidate = request(hole(), vec![0], vec![], vec![Ty::Bool]);
        candidate.fixtures = Some(fixtures(
            6,
            vec![(empty_inputs.clone(), vec![Scalar::Bool(expected)])],
        ));
        let response = complete(&session, &entries, &candidate, monotonic_now_ns);
        let Response::Accepted { accepted, report } = response else {
            return Err(format!("{source:?}: {}", response.outcome()));
        };
        assert_eq!(accepted.source, source);
        let fixtures = report
            .fixtures
            .as_ref()
            .ok_or_else(|| String::from("fixture report"))?;
        assert!(fixtures.agreement, "{fixtures:?}");
        assert_eq!(fixtures.cases[0].expected, [Scalar::Bool(expected)]);
        assert_eq!(fixtures.cases[0].actual, Some(vec![Scalar::Bool(expected)]));
    }
    Ok(())
}

#[test]
fn subtraction_operand_order_is_not_guessed() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[arithmetic([10, 3], Op::Sub, b"10 3 - ")])?;
    let mut candidate = request(hole(), vec![0], vec![], vec![Ty::I64]);
    candidate.fixtures = Some(fixtures(5, vec![(Vec::new(), vec![Scalar::I64(7)])]));
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { report, .. } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    let reported = report
        .fixtures
        .as_ref()
        .ok_or_else(|| String::from("fixture report"))?;
    assert!(reported.agreement, "{reported:?}");
    assert_eq!(
        reported.cases.first().and_then(|case| case.actual.clone()),
        Some(vec![Scalar::I64(7)])
    );
    Ok(())
}
