#![expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; test-only helpers invoke dynamic graph evaluators and assertions, which are not const-evaluable."
)]

#[derive(Clone, Copy)]
struct InputPair {
    first: i64,
    second: i64,
}

fn assert_fixture_shape(before: &noble_ngraph::ir::Graph) {
    let output_ids: Vec<u32> = before.outputs.iter().map(|output| output.0).collect();
    assert_eq!(output_ids, [2, 3, 5, 10, 11, 11, 9]);
    assert!(before
        .nodes
        .iter()
        .any(|node| matches!(node.op, noble_ngraph::ir::Op::I64Sub(_, _))));
}

fn assert_derivation_shape(derivation: &noble_ngraph::cert::Derivation) {
    let rules: Vec<noble_ngraph::cert::Rule> =
        derivation.steps.iter().map(|step| step.rule).collect();
    assert_eq!(
        rules,
        [
            noble_ngraph::cert::Rule::FoldAddConstants,
            noble_ngraph::cert::Rule::FoldAddConstants,
            noble_ngraph::cert::Rule::NeutralMulRight,
        ]
    );
    let nodes: Vec<u32> = derivation.steps.iter().map(|step| step.node.0).collect();
    assert_eq!(nodes, [4, 7, 9]);
    let claims: Vec<u64> = derivation
        .steps
        .iter()
        .filter(|step| step.rule == noble_ngraph::cert::Rule::FoldAddConstants)
        .map(|step| step.claimed)
        .collect();
    assert_eq!(claims, [8, i64::MIN.cast_unsigned()]);
}

fn expected(input: InputPair) -> noble_ngraph::eval::Obs {
    let shifted = input.first.wrapping_mul(8).wrapping_add(i64::MIN);
    let difference = shifted.wrapping_sub(input.second);
    noble_ngraph::eval::Obs::Normal(vec![
        noble_ngraph::sem::Value::I64(3),
        noble_ngraph::sem::Value::I64(5),
        noble_ngraph::sem::Value::I64(i64::MAX),
        noble_ngraph::sem::Value::I64(shifted),
        noble_ngraph::sem::Value::I64(difference),
        noble_ngraph::sem::Value::I64(difference),
        noble_ngraph::sem::Value::I64(input.second),
    ])
}

fn assert_evaluated_pair(
    before: &noble_ngraph::ir::Graph,
    after: &noble_ngraph::ir::Graph,
    input_pair: InputPair,
) -> Result<(), noble_ngraph::Diagnostic> {
    let input = [
        noble_ngraph::sem::Value::I64(input_pair.first),
        noble_ngraph::sem::Value::I64(input_pair.second),
    ];
    let (compiled_before, compiled_nodes, reference_before, reference_nodes) =
        crate::differential::observations(before, &input)?;
    let (compiled_after, folded_nodes, reference_after, folded_reference_nodes) =
        crate::differential::observations(after, &input)?;
    let expected = expected(input_pair);
    for (observed, endpoint) in [
        (&compiled_before, "compiled original"),
        (&reference_before, "reference original"),
        (&compiled_after, "compiled folded"),
        (&reference_after, "reference folded"),
    ] {
        assert_eq!(
            observed, &expected,
            "{endpoint} on inputs {}, {}",
            input_pair.first, input_pair.second
        );
    }
    assert_eq!(
        (compiled_nodes, reference_nodes),
        (folded_nodes, folded_reference_nodes),
        "in-place folds preserve evaluated node counts"
    );
    Ok(())
}

#[test]
fn an_accepted_derivation_executes_both_endpoints_identically(
) -> Result<(), noble_ngraph::Diagnostic> {
    let before = crate::differential::independent_folds();
    let (derivation, after) = crate::differential::fold(&before);
    let scheme = crate::differential::TestIds;
    assert_eq!(
        noble_ngraph::cert::check(&scheme, &before, &after, &derivation),
        Ok(())
    );
    assert!(noble_ngraph::check::check(&scheme, before.clone()).is_ok());
    assert!(noble_ngraph::check::check(&scheme, after.clone()).is_ok());
    assert_fixture_shape(&before);
    assert_derivation_shape(&derivation);
    for input in [
        (0, 0),
        (2, 1),
        (-3, 7),
        (i64::MAX, i64::MIN),
        (i64::MIN, -1),
    ] {
        assert_evaluated_pair(
            &before,
            &after,
            InputPair {
                first: input.0,
                second: input.1,
            },
        )?;
    }
    Ok(())
}
