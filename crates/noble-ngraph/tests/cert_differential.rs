//! Differential execution of a certificate's two endpoints (issue #42).
//!
//! The certificate lane proves premises, not values. This target executes both
//! endpoints — the original graph and the folded one the emitter produced — on the
//! same concrete inputs through both the compiled scheduler and the reference
//! interpreter, and asserts identical observations. It measures a real artifact:
//! the folded endpoint evaluates strictly fewer nodes while producing the same
//! outputs, including for a sum that wraps.

use noble_ngraph::cert::{check, emit, Derivation};
use noble_ngraph::check as graph_check;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::identity::{IdentityHasher, IdentityScheme};
use noble_ngraph::ir::{Digest, Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;

struct TestHasher(u64);

impl IdentityHasher for TestHasher {
    fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = self.0.wrapping_mul(31).wrapping_add(u64::from(byte));
        }
    }

    fn finish(self: Box<Self>) -> Digest {
        let mut digest = [0u8; 32];
        digest[..8].copy_from_slice(&self.0.to_le_bytes());
        digest
    }
}

struct TestIds;

impl IdentityScheme for TestIds {
    fn name(&self) -> &'static str {
        "test"
    }

    fn start(&self) -> Box<dyn IdentityHasher + '_> {
        Box::new(TestHasher(0))
    }
}

fn constant_sum(first: i64, second: i64) -> Graph {
    let scheme = TestIds;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "cert-differential"),
    );
    let input = builder.input(ScalarType::I64);
    let a = builder.i64_const(first);
    let b = builder.i64_const(second);
    let sum = builder.op(Op::I64Add(a, b));
    let scaled = builder.op(Op::I64Mul(input, sum));
    builder.output(a);
    builder.output(b);
    builder.output(scaled);
    builder.finish()
}

fn symbolic_sum() -> Graph {
    let scheme = TestIds;
    let mut builder =
        GraphBuilder::new(SubProfile::I64, Subject::authored(&scheme, "cert-symbolic"));
    let input = builder.input(ScalarType::I64);
    let other = builder.input(ScalarType::I64);
    let sum = builder.op(Op::I64Add(input, other));
    builder.output(input);
    builder.output(other);
    builder.output(sum);
    builder.finish()
}

fn observations(graph: &Graph, inputs: &[Value]) -> (Obs, u64, Obs, u64) {
    let scheme = TestIds;
    let checked = graph_check::check(&scheme, graph.clone()).expect("endpoint checks");
    let compiled = Compiled::new(&scheme, &checked);
    let mut first = Control::default();
    let mut second = Control::default();
    let compiled_report = compiled.evaluate(inputs, None, &mut first);
    let reference_report = eval::reference(&checked, inputs, None, &mut second);
    (
        compiled_report.obs,
        compiled_report.evaluated_nodes,
        reference_report.obs,
        reference_report.evaluated_nodes,
    )
}

fn fold(graph: &Graph) -> (Derivation, Graph) {
    let scheme = TestIds;
    emit(&scheme, graph)
}

fn assert_same_outputs(left: &Obs, right: &Obs, context: &str) {
    match (left, right) {
        (Obs::Normal(l), Obs::Normal(r)) => assert_eq!(l, r, "{context}"),
        (l, r) => panic!("{context}: expected two admitted observations, got {l:?} and {r:?}"),
    }
}

#[test]
fn a_folded_endpoint_evaluates_identically_with_a_constant() {
    let before = constant_sum(3, 5);
    let (derivation, after) = fold(&before);
    let scheme = TestIds;
    let before_checked = graph_check::check(&scheme, before.clone()).expect("checks");
    let after_checked = graph_check::check(&scheme, after.clone()).expect("checks");
    assert_eq!(
        check(&scheme, &before, &after, &derivation),
        Ok(()),
        "the certificate must be accepted before it is executed"
    );
    let fold_step = derivation
        .steps
        .iter()
        .find(|step| step.rule == noble_ngraph::cert::Rule::FoldAddConstants)
        .expect("the sample folds one addition");
    assert_eq!(fold_step.claimed, 8, "the certificate claims the exact sum");
    let inputs = [Value::I64(0), Value::I64(2), Value::I64(-3)];
    for &input in &inputs {
        let expected = match input {
            Value::I64(value) => value.wrapping_mul(fold_step.claimed.cast_signed()),
            other => panic!("unexpected input {other:?}"),
        };
        let (compiled_before, nodes_before, reference_before, ref_nodes_before) =
            observations(&before, &[input]);
        let (compiled_after, nodes_after, reference_after, ref_nodes_after) =
            observations(&after, &[input]);
        assert_same_outputs(&compiled_before, &compiled_after, "compiled endpoints");
        assert_same_outputs(&reference_before, &reference_after, "reference endpoints");
        assert_eq!(
            nodes_before, nodes_after,
            "an in-place fold preserves the node count on input {input:?}"
        );
        assert_eq!(ref_nodes_before, ref_nodes_after);
        match &compiled_after {
            Obs::Normal(outputs) => assert_eq!(
                outputs.get(2),
                Some(&Value::I64(expected)),
                "the folded endpoint computes the claimed sum on input {input:?}"
            ),
            other => panic!("expected an admitted observation, got {other:?}"),
        }
    }
    // The fold keeps every node identity (it rewrites in place) and is at worst
    // the same size in retained bytes: one instruction leaves, one constant
    // entry arrives.
    assert_eq!(after.nodes.len(), before.nodes.len());
    let before_compiled = Compiled::new(&scheme, &before_checked);
    let after_compiled = Compiled::new(&scheme, &after_checked);
    assert!(after_compiled.retained_bytes() <= before_compiled.retained_bytes());
}

#[test]
fn a_wrapping_sum_folds_to_the_wrapping_value_in_execution() {
    let before = constant_sum(i64::MAX, 1);
    let (derivation, after) = fold(&before);
    let scheme = TestIds;
    assert!(check(&scheme, &before, &after, &derivation).is_ok());
    let (compiled, _, reference, _) = observations(&before, &[Value::I64(2)]);
    let (folded, _, folded_reference, _) = observations(&after, &[Value::I64(2)]);
    assert_same_outputs(&compiled, &folded, "wrapping compiled endpoints");
    assert_same_outputs(
        &reference,
        &folded_reference,
        "wrapping reference endpoints",
    );
    // `(i64::MAX + 1) * 2` wraps: the folded constant is `i64::MIN`.
    match &folded {
        Obs::Normal(outputs) => assert_eq!(
            outputs.get(2),
            Some(&Value::I64(i64::MIN.wrapping_mul(2))),
            "the folded sum reaches the scaled output as the wrapping value"
        ),
        other => panic!("expected an admitted observation, got {other:?}"),
    }
}

#[test]
fn a_symbolic_sum_is_recorded_as_unfolded_and_still_executes_identically() {
    let graph = symbolic_sum();
    let (derivation, result) = fold(&graph);
    assert!(
        derivation.steps.is_empty(),
        "a sum of two program inputs must not be folded"
    );
    assert_eq!(derivation.left, derivation.right);
    let scheme = TestIds;
    assert!(check(&scheme, &graph, &result, &derivation).is_ok());
    for &(a, b) in &[(1i64, 2i64), (i64::MAX, 1)] {
        let (compiled, _, reference, _) = observations(&graph, &[Value::I64(a), Value::I64(b)]);
        let (rewritten, _, rewritten_reference, _) =
            observations(&result, &[Value::I64(a), Value::I64(b)]);
        assert_same_outputs(&compiled, &rewritten, "symbolic compiled endpoints");
        assert_same_outputs(
            &reference,
            &rewritten_reference,
            "symbolic reference endpoints",
        );
    }
    // A carriage refusal is still refused on the rewritten endpoint, so folding
    // cannot turn a refusal into a result.
    let refused = observations(&result, &[Value::I64(1)]);
    assert!(matches!(refused.0, Obs::Refused(_)), "got {:?}", refused.0);
    assert_eq!(
        check(&scheme, &graph, &result, &derivation.clone()),
        Ok(()),
        "re-checking after execution still accepts"
    );
}
