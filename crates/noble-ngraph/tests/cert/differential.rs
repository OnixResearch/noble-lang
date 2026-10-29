pub(super) struct TestHasher(u64);

impl noble_ngraph::identity::IdentityHasher for TestHasher {
    fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = self.0.wrapping_mul(31).wrapping_add(u64::from(byte));
        }
    }

    fn finish(self: Box<Self>) -> noble_ngraph::ir::Digest {
        let mut digest = [0u8; 32];
        digest[..8].copy_from_slice(&self.0.to_le_bytes());
        digest
    }
}

pub(super) struct TestIds;

impl noble_ngraph::identity::IdentityScheme for TestIds {
    fn name(&self) -> &'static str {
        "test"
    }

    fn start(&self) -> Box<dyn noble_ngraph::identity::IdentityHasher + '_> {
        Box::new(TestHasher(0))
    }
}

pub(super) struct SumOperands {
    pub(super) first: i64,
    pub(super) second: i64,
}

pub(super) fn constant_sum(operands: SumOperands) -> noble_ngraph::ir::Graph {
    let scheme = TestIds;
    let mut builder = noble_ngraph::ir::GraphBuilder::new(
        noble_ngraph::ir::SubProfile::I64,
        noble_ngraph::ir::Subject::authored(&scheme, "cert-differential"),
    );
    let input = builder.input(noble_ngraph::ir::ScalarType::I64);
    let a = builder.i64_const(operands.first);
    let b = builder.i64_const(operands.second);
    let sum = builder.op(noble_ngraph::ir::Op::I64Add(a, b));
    let scaled = builder.op(noble_ngraph::ir::Op::I64Mul(input, sum));
    builder.output(a);
    builder.output(b);
    builder.output(scaled);
    builder.finish()
}

pub(super) fn symbolic_sum() -> noble_ngraph::ir::Graph {
    let scheme = TestIds;
    let mut builder = noble_ngraph::ir::GraphBuilder::new(
        noble_ngraph::ir::SubProfile::I64,
        noble_ngraph::ir::Subject::authored(&scheme, "cert-symbolic"),
    );
    let input = builder.input(noble_ngraph::ir::ScalarType::I64);
    let other = builder.input(noble_ngraph::ir::ScalarType::I64);
    let sum = builder.op(noble_ngraph::ir::Op::I64Add(input, other));
    builder.output(input);
    builder.output(other);
    builder.output(sum);
    builder.finish()
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; this helper returns both real evaluator observations for the caller's independent comparison, including deliberate wrong-arity refusals; asserting success here would destroy that negative control."
)]
pub(super) fn observations(
    graph: &noble_ngraph::ir::Graph,
    inputs: &[noble_ngraph::sem::Value],
) -> Result<(noble_ngraph::eval::Obs, u64, noble_ngraph::eval::Obs, u64), noble_ngraph::Diagnostic>
{
    let scheme = TestIds;
    let checked = noble_ngraph::check::check(&scheme, graph.clone())?;
    let compiled = noble_ngraph::compiled::Compiled::new(&scheme, &checked);
    let mut first = noble_ngraph::eval::Control {
        fuel: None,
        cancel: None,
        scratch: None,
        forward_scratch: None,
        evaluated_nodes: None,
    };
    let mut second = noble_ngraph::eval::Control {
        fuel: None,
        cancel: None,
        scratch: None,
        forward_scratch: None,
        evaluated_nodes: None,
    };
    let compiled_report = compiled.evaluate(inputs, None, &mut first);
    let reference_report = noble_ngraph::eval::reference(&checked, inputs, None, &mut second);
    Ok((
        compiled_report.obs,
        compiled_report.evaluated_nodes,
        reference_report.obs,
        reference_report.evaluated_nodes,
    ))
}

pub(super) fn fold(
    graph: &noble_ngraph::ir::Graph,
) -> (noble_ngraph::cert::Derivation, noble_ngraph::ir::Graph) {
    let scheme = TestIds;
    noble_ngraph::cert::emit(&scheme, graph)
}

pub(super) fn assert_same_outputs(
    left: &noble_ngraph::eval::Obs,
    right: &noble_ngraph::eval::Obs,
    context: &str,
) {
    match (left, right) {
        (noble_ngraph::eval::Obs::Normal(left), noble_ngraph::eval::Obs::Normal(right)) => {
            assert_eq!(left, right, "{context}");
        }
        _ => panic!("{context}: expected two admitted observations, got {left:?} and {right:?}"),
    }
}
