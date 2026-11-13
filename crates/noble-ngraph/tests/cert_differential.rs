#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![expect(
    tigerstyle::no_panic,
    reason = "Owner: noble-maintainers; malformed test fixtures or unexpected evaluator outcomes must fail this integration test immediately with the observed value, not return a successful test result."
)]
#![allow(
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this Cargo integration target keeps its stable underscore filename for the certificate differential executable."
)]

//! Differential execution of a certificate's two endpoints (issue #42).
//!
//! The certificate lane proves premises, not values. This target executes both
//! endpoints — the original graph and the folded one the emitter produced — on the
//! same concrete inputs through both the compiled scheduler and the reference
//! interpreter, and asserts identical observations. It measures a real artifact:
//! the fold rewrites in place, so the folded endpoint evaluates the same number of
//! nodes and retains no more bytes while producing the same outputs, including for a
//! sum that wraps. A three-step certificate (two independent folds and a recorded
//! identity) over a graph with two inputs, a subtraction consumer of both folds and a
//! repeated output is executed the same way, and corrupting the recorded identity step's
//! operands, bits or claim is refused like corrupting a fold. Substituted results that
//! re-digest themselves are refused for a swapped subtraction and for exchanged input
//! slots, admission refuses the forward-reference and cyclic endpoints the checker
//! assumes away, and the wire envelope is kernel-checked against this exact fixture by
//! the generated `NobleFoundation/CertificateCorpus.lean`.

#[path = "cert/differential.rs"]
mod differential;

#[path = "cert/corpus.rs"]
mod corpus;

#[path = "cert/tamper.rs"]
mod tamper;

#[test]
fn a_folded_endpoint_evaluates_identically_with_a_constant() -> Result<(), noble_ngraph::Diagnostic>
{
    let before = differential::constant_sum(differential::SumOperands {
        first: 3,
        second: 5,
    });
    let (derivation, after) = differential::fold(&before);
    let scheme = differential::TestIds;
    let before_checked = noble_ngraph::check::check(&scheme, before.clone())?;
    let after_checked = noble_ngraph::check::check(&scheme, after.clone())?;
    assert_eq!(
        noble_ngraph::cert::check(&scheme, &before, &after, &derivation),
        Ok(()),
        "the certificate must be accepted before it is executed"
    );
    let Some(fold_step) = derivation
        .steps
        .iter()
        .find(|step| step.rule == noble_ngraph::cert::Rule::FoldAddConstants)
    else {
        panic!("the sample folds one addition");
    };
    assert_eq!(fold_step.claimed, 8, "the certificate claims the exact sum");
    let inputs = [
        noble_ngraph::sem::Value::I64(0),
        noble_ngraph::sem::Value::I64(2),
        noble_ngraph::sem::Value::I64(-3),
    ];
    for &input in &inputs {
        let expected = match input {
            noble_ngraph::sem::Value::I64(value) => {
                value.wrapping_mul(fold_step.claimed.cast_signed())
            }
            other => {
                panic!("unexpected input {other:?}");
            }
        };
        let (compiled_before, nodes_before, reference_before, ref_nodes_before) =
            differential::observations(&before, &[input])?;
        let (compiled_after, nodes_after, reference_after, ref_nodes_after) =
            differential::observations(&after, &[input])?;
        differential::assert_same_outputs(&compiled_before, &compiled_after, "compiled endpoints");
        differential::assert_same_outputs(
            &reference_before,
            &reference_after,
            "reference endpoints",
        );
        assert_eq!(
            nodes_before, nodes_after,
            "an in-place fold preserves the node count on input {input:?}"
        );
        assert_eq!(ref_nodes_before, ref_nodes_after);
        let Some(outputs) = compiled_after.outputs() else {
            panic!("expected an admitted observation, got {compiled_after:?}");
        };
        assert_eq!(
            outputs.get(2),
            Some(&noble_ngraph::sem::Value::I64(expected)),
            "the folded endpoint computes the claimed sum on input {input:?}"
        );
    }
    // The fold keeps every node identity (it rewrites in place) and is at worst
    // the same size in retained bytes: one instruction leaves, one constant
    // entry arrives.
    assert_eq!(after.nodes.len(), before.nodes.len());
    let before_compiled = noble_ngraph::compiled::Compiled::new(&scheme, &before_checked);
    let after_compiled = noble_ngraph::compiled::Compiled::new(&scheme, &after_checked);
    assert!(after_compiled.retained_bytes() <= before_compiled.retained_bytes());
    Ok(())
}

#[test]
fn a_wrapping_sum_folds_to_the_wrapping_value_in_execution() -> Result<(), noble_ngraph::Diagnostic>
{
    let before = differential::constant_sum(differential::SumOperands {
        first: i64::MAX,
        second: 1,
    });
    let (derivation, after) = differential::fold(&before);
    let scheme = differential::TestIds;
    assert!(noble_ngraph::cert::check(&scheme, &before, &after, &derivation).is_ok());
    let (compiled, _, reference, _) =
        differential::observations(&before, &[noble_ngraph::sem::Value::I64(2)])?;
    let (folded, _, folded_reference, _) =
        differential::observations(&after, &[noble_ngraph::sem::Value::I64(2)])?;
    differential::assert_same_outputs(&compiled, &folded, "wrapping compiled endpoints");
    differential::assert_same_outputs(
        &reference,
        &folded_reference,
        "wrapping reference endpoints",
    );
    // `(i64::MAX + 1) * 2` wraps: the folded constant is `i64::MIN`.
    let Some(outputs) = folded.outputs() else {
        panic!("expected an admitted observation, got {folded:?}");
    };
    assert_eq!(
        outputs.get(2),
        Some(&noble_ngraph::sem::Value::I64(i64::MIN.wrapping_mul(2))),
        "the folded sum reaches the scaled output as the wrapping value"
    );
    Ok(())
}

#[test]
fn a_symbolic_sum_is_recorded_as_unfolded_and_still_executes_identically(
) -> Result<(), noble_ngraph::Diagnostic> {
    let graph = differential::symbolic_sum();
    let (derivation, result) = differential::fold(&graph);
    assert!(
        derivation.steps.is_empty(),
        "a sum of two program inputs must not be folded"
    );
    assert_eq!(derivation.left, derivation.right);
    let scheme = differential::TestIds;
    assert!(noble_ngraph::cert::check(&scheme, &graph, &result, &derivation).is_ok());
    for &(a, b) in &[(1i64, 2i64), (i64::MAX, 1)] {
        let (compiled, _, reference, _) = differential::observations(
            &graph,
            &[
                noble_ngraph::sem::Value::I64(a),
                noble_ngraph::sem::Value::I64(b),
            ],
        )?;
        let (rewritten, _, rewritten_reference, _) = differential::observations(
            &result,
            &[
                noble_ngraph::sem::Value::I64(a),
                noble_ngraph::sem::Value::I64(b),
            ],
        )?;
        differential::assert_same_outputs(&compiled, &rewritten, "symbolic compiled endpoints");
        differential::assert_same_outputs(
            &reference,
            &rewritten_reference,
            "symbolic reference endpoints",
        );
    }
    // A carriage refusal is still refused on the rewritten endpoint, so folding
    // cannot turn a refusal into a result.
    let refused = differential::observations(&result, &[noble_ngraph::sem::Value::I64(1)])?;
    assert!(
        matches!(refused.0, noble_ngraph::eval::Obs::Refused(_)),
        "got {:?}",
        refused.0
    );
    assert_eq!(
        noble_ngraph::cert::check(&scheme, &graph, &result, &derivation),
        Ok(()),
        "re-checking after execution still accepts"
    );
    Ok(())
}

#[test]
fn a_multi_step_certificate_executes_both_endpoints_identically(
) -> Result<(), noble_ngraph::Diagnostic> {
    let before = differential::independent_folds();
    let (derivation, after) = differential::fold(&before);
    let scheme = differential::TestIds;
    assert_eq!(
        noble_ngraph::cert::check(&scheme, &before, &after, &derivation),
        Ok(()),
        "the three-step certificate must be accepted before it is executed"
    );
    assert!(noble_ngraph::check::check(&scheme, before.clone()).is_ok());
    assert!(noble_ngraph::check::check(&scheme, after.clone()).is_ok());
    assert_eq!(
        before
            .outputs
            .iter()
            .map(|output| output.0)
            .collect::<Vec<u32>>(),
        [2, 3, 5, 10, 11, 11, 9],
        "the seventh output slot repeats a subtraction's node, and the folded-away \
         constants stay live outputs"
    );
    assert!(
        before
            .nodes
            .iter()
            .any(|node| matches!(node.op, noble_ngraph::ir::Op::I64Sub(_, _))),
        "the fixture consumes both folds through a subtraction"
    );
    let rules: Vec<noble_ngraph::cert::Rule> =
        derivation.steps.iter().map(|step| step.rule).collect();
    let nodes: Vec<u32> = derivation.steps.iter().map(|step| step.node.0).collect();
    assert_eq!(
        rules,
        [
            noble_ngraph::cert::Rule::FoldAddConstants,
            noble_ngraph::cert::Rule::FoldAddConstants,
            noble_ngraph::cert::Rule::NeutralMulRight,
        ],
        "two independent folds and one recorded identity"
    );
    assert_eq!(nodes, [4, 7, 9], "steps stay in ascending node order");
    let claims: Vec<u64> = derivation
        .steps
        .iter()
        .filter(|step| step.rule == noble_ngraph::cert::Rule::FoldAddConstants)
        .map(|step| step.claimed)
        .collect();
    assert_eq!(
        claims,
        [8, i64::MIN.cast_unsigned()],
        "the second fold claims the wrapped sum"
    );
    for (first, second) in [
        (0, 0),
        (2, 1),
        (-3, 7),
        (i64::MAX, i64::MIN),
        (i64::MIN, -1),
    ] {
        let input = [
            noble_ngraph::sem::Value::I64(first),
            noble_ngraph::sem::Value::I64(second),
        ];
        let shifted = first.wrapping_mul(8).wrapping_add(i64::MIN);
        let difference = shifted.wrapping_sub(second);
        let expected = noble_ngraph::eval::Obs::Normal(vec![
            noble_ngraph::sem::Value::I64(3),
            noble_ngraph::sem::Value::I64(5),
            noble_ngraph::sem::Value::I64(i64::MAX),
            noble_ngraph::sem::Value::I64(shifted),
            noble_ngraph::sem::Value::I64(difference),
            noble_ngraph::sem::Value::I64(difference),
            noble_ngraph::sem::Value::I64(second),
        ]);
        let (compiled_before, nodes_before, reference_before, ref_nodes_before) =
            differential::observations(&before, &input)?;
        let (compiled_after, nodes_after, reference_after, ref_nodes_after) =
            differential::observations(&after, &input)?;
        for (observed, endpoint) in [
            (&compiled_before, "compiled original"),
            (&reference_before, "reference original"),
            (&compiled_after, "compiled folded"),
            (&reference_after, "reference folded"),
        ] {
            assert_eq!(
                observed, &expected,
                "{endpoint} endpoint on inputs {first}, {second}"
            );
        }
        assert_eq!(
            (nodes_before, ref_nodes_before),
            (nodes_after, ref_nodes_after),
            "in-place folds preserve the evaluated node counts on inputs {first}, {second}"
        );
    }
    Ok(())
}

#[test]
fn a_corrupted_later_step_of_a_multi_step_certificate_is_refused() {
    tamper::later_steps_are_refused();
}

/// The emitted certificate with its result digest renamed to a substituted endpoint, as a
/// dishonest producer would re-digest it: every step still checks against the original graph.
fn redigested(
    derivation: &noble_ngraph::cert::Derivation,
    result: &noble_ngraph::ir::Graph,
) -> noble_ngraph::cert::Derivation {
    let mut renamed = derivation.clone();
    renamed.right = result.logical_id(&differential::TestIds);
    renamed
}

#[test]
fn substituted_subtraction_operands_and_input_slots_are_refused() {
    use noble_ngraph::ir::Op;

    let before = differential::independent_folds();
    let (derivation, after) = differential::fold(&before);
    let scheme = differential::TestIds;
    let subtraction = after
        .nodes
        .iter()
        .position(|node| matches!(node.op, Op::I64Sub(_, _)))
        .expect("the fixture consumes both folds through a subtraction");
    let Op::I64Sub(first, second) = after.nodes[subtraction].op else {
        panic!("node %{subtraction} is the subtraction");
    };
    let mut swapped_subtraction = after.clone();
    swapped_subtraction.nodes[subtraction].op = Op::I64Sub(second, first);
    let mut exchanged_slots = after.clone();
    for node in &mut exchanged_slots.nodes {
        if let Op::Input(slot) = node.op {
            node.op = Op::Input(1 - slot);
        }
    }
    assert_ne!(swapped_subtraction, after);
    assert_ne!(exchanged_slots, after);
    for (substituted, label) in [
        (&swapped_subtraction, "swapped subtraction operands"),
        (&exchanged_slots, "exchanged input slots"),
    ] {
        // Both controls are admitted, well-typed endpoints: the refusal below is about the
        // certificate, not about malformed input.
        assert!(
            noble_ngraph::check::check(&scheme, substituted.clone()).is_ok(),
            "the substituted endpoint ({label}) is admitted"
        );
        assert_eq!(
            noble_ngraph::cert::check(
                &scheme,
                &before,
                substituted,
                &redigested(&derivation, substituted)
            ),
            Err(noble_ngraph::cert::Refusal::ResultMismatch),
            "a re-digested {label} result is not the replay"
        );
    }
    assert_eq!(
        noble_ngraph::cert::check(&scheme, &before, &after, &redigested(&derivation, &after)),
        Ok(()),
        "the honest result is still accepted when re-digested"
    );
}

#[test]
fn admission_refuses_the_endpoints_the_checker_assumes_away() {
    use noble_ngraph::ir::{NodeId, Op};

    let scheme = differential::TestIds;
    // `%0 = %1 + %2` with both constants defined after it: an acyclic forward reference.
    let mut forward = differential::i64_builder("cert-forward-reference");
    let sum = forward.op(Op::I64Add(NodeId(1), NodeId(2)));
    let three = forward.i64_const(3);
    let five = forward.i64_const(5);
    forward.output(sum);
    forward.output(three);
    forward.output(five);
    let forward = forward.finish();
    assert_eq!(
        noble_ngraph::check::check(&scheme, forward.clone())
            .err()
            .map(|diagnostic| diagnostic.code),
        Some(noble_ngraph::diag::Code::GraphMalformed(
            noble_ngraph::diag::Malformed::ForwardReference
        )),
        "a forward-reference endpoint is refused by admission"
    );
    // `%0 = %1 + %1` and `%1 = %0 + %0`: a dependency cycle.
    let mut cycle = differential::i64_builder("cert-cycle");
    let _first = cycle.op(Op::I64Add(NodeId(1), NodeId(1)));
    let second = cycle.op(Op::I64Add(NodeId(0), NodeId(0)));
    cycle.output(second);
    let cycle = cycle.finish();
    assert_eq!(
        noble_ngraph::check::check(&scheme, cycle.clone())
            .err()
            .map(|diagnostic| diagnostic.code),
        Some(noble_ngraph::diag::Code::GraphMalformed(
            noble_ngraph::diag::Malformed::Cycle
        )),
        "a cyclic endpoint is refused by admission"
    );
    // A certificate over the cyclic graph is refused before any denotation is used: neither
    // node is a constant, so the fold premise has nothing to check against.
    let step = noble_ngraph::cert::Step {
        rule: noble_ngraph::cert::Rule::FoldAddConstants,
        node: second,
        operands: [NodeId(0), NodeId(0)],
        first_bits: 0,
        second_bits: 0,
        claimed: 0,
    };
    let derivation = noble_ngraph::cert::Derivation {
        revision: String::from(noble_ngraph::cert::CERT_REVISION),
        model: String::from(noble_ngraph::cert::MODEL_REVISION),
        steps: vec![step],
        left: cycle.logical_id(&scheme),
        right: cycle.logical_id(&scheme),
    };
    assert_eq!(
        noble_ngraph::cert::check(&scheme, &cycle, &cycle, &derivation),
        Err(noble_ngraph::cert::Refusal::Malformed),
        "a certificate over a cyclic endpoint is refused"
    );
}
