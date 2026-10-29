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
//! the folded endpoint evaluates strictly fewer nodes while producing the same
//! outputs, including for a sum that wraps.

#[path = "cert/differential.rs"]
mod differential;

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
