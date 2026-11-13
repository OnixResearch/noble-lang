fn redigested(
    derivation: &noble_ngraph::cert::Derivation,
    result: &noble_ngraph::ir::Graph,
) -> noble_ngraph::cert::Derivation {
    let mut renamed = derivation.clone();
    renamed.right = result.logical_id(&crate::differential::TestIds);
    renamed
}

#[test]
fn substituted_subtraction_operands_and_input_slots_are_refused() {
    let before = crate::differential::independent_folds();
    let (derivation, after) = crate::differential::fold(&before);
    let scheme = crate::differential::TestIds;
    let subtraction = after
        .nodes
        .iter()
        .position(|node| matches!(node.op, noble_ngraph::ir::Op::I64Sub(_, _)));
    assert!(
        subtraction.is_some(),
        "the fixture has a subtraction consumer"
    );
    let Some(subtraction) = subtraction else {
        return;
    };
    let noble_ngraph::ir::Op::I64Sub(first, second) = after.nodes[subtraction].op else {
        return;
    };
    let mut swapped_subtraction = after.clone();
    swapped_subtraction.nodes[subtraction].op = noble_ngraph::ir::Op::I64Sub(second, first);
    let mut exchanged_slots = after.clone();
    for node in &mut exchanged_slots.nodes {
        if let noble_ngraph::ir::Op::Input(slot) = node.op {
            node.op = noble_ngraph::ir::Op::Input(match slot {
                0 => 1,
                1 => 0,
                other => other,
            });
        }
    }
    assert_ne!(swapped_subtraction, after);
    assert_ne!(exchanged_slots, after);
    assert_substitution_is_refused(&scheme, &before, &after, &derivation, &swapped_subtraction);
    assert_substitution_is_refused(&scheme, &before, &after, &derivation, &exchanged_slots);
}

fn assert_substitution_is_refused(
    scheme: &crate::differential::TestIds,
    before: &noble_ngraph::ir::Graph,
    after: &noble_ngraph::ir::Graph,
    derivation: &noble_ngraph::cert::Derivation,
    substituted: &noble_ngraph::ir::Graph,
) {
    assert_ne!(substituted, after);
    assert!(noble_ngraph::check::check(scheme, substituted.clone()).is_ok());
    assert_eq!(
        noble_ngraph::cert::check(
            scheme,
            before,
            substituted,
            &redigested(derivation, substituted)
        ),
        Err(noble_ngraph::cert::Refusal::ResultMismatch)
    );
    assert_eq!(
        noble_ngraph::cert::check(scheme, before, after, &redigested(derivation, after)),
        Ok(())
    );
}

#[test]
fn malformed_endpoints_are_rejected() {
    let scheme = crate::differential::TestIds;
    assert_forward_reference_refused(&scheme);
    assert_cycle_refused(&scheme);
}

fn assert_forward_reference_refused(scheme: &crate::differential::TestIds) {
    let mut builder = crate::differential::i64_builder("cert-forward-reference");
    let sum = builder.op(noble_ngraph::ir::Op::I64Add(
        noble_ngraph::ir::NodeId(1),
        noble_ngraph::ir::NodeId(2),
    ));
    let three = builder.i64_const(3);
    let five = builder.i64_const(5);
    builder.output(sum);
    builder.output(three);
    builder.output(five);
    let forward = builder.finish();
    assert_eq!(forward.nodes.len(), 3);
    assert_eq!(
        noble_ngraph::check::check(scheme, forward)
            .err()
            .map(|diagnostic| diagnostic.code),
        Some(noble_ngraph::diag::Code::GraphMalformed(
            noble_ngraph::diag::Malformed::ForwardReference
        ))
    );
}

fn assert_cycle_refused(scheme: &crate::differential::TestIds) {
    let mut builder = crate::differential::i64_builder("cert-cycle");
    let _first = builder.op(noble_ngraph::ir::Op::I64Add(
        noble_ngraph::ir::NodeId(1),
        noble_ngraph::ir::NodeId(1),
    ));
    let second = builder.op(noble_ngraph::ir::Op::I64Add(
        noble_ngraph::ir::NodeId(0),
        noble_ngraph::ir::NodeId(0),
    ));
    builder.output(second);
    let cycle = builder.finish();
    assert_eq!(
        noble_ngraph::check::check(scheme, cycle.clone())
            .err()
            .map(|diagnostic| diagnostic.code),
        Some(noble_ngraph::diag::Code::GraphMalformed(
            noble_ngraph::diag::Malformed::Cycle
        ))
    );
    let derivation = cyclic_derivation(&cycle, second, scheme);
    assert_eq!(
        noble_ngraph::cert::check(scheme, &cycle, &cycle, &derivation),
        Err(noble_ngraph::cert::Refusal::Malformed)
    );
}

fn cyclic_derivation(
    cycle: &noble_ngraph::ir::Graph,
    second: noble_ngraph::ir::NodeId,
    scheme: &crate::differential::TestIds,
) -> noble_ngraph::cert::Derivation {
    let step = noble_ngraph::cert::Step {
        rule: noble_ngraph::cert::Rule::FoldAddConstants,
        node: second,
        operands: [noble_ngraph::ir::NodeId(0), noble_ngraph::ir::NodeId(0)],
        first_bits: 0,
        second_bits: 0,
        claimed: 0,
    };
    noble_ngraph::cert::Derivation {
        revision: String::from(noble_ngraph::cert::CERT_REVISION),
        model: String::from(noble_ngraph::cert::MODEL_REVISION),
        steps: vec![step],
        left: cycle.logical_id(scheme),
        right: cycle.logical_id(scheme),
    }
}
