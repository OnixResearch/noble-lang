use crate::common;
use noble_ngraph::check;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{
    Fact, Graph, GraphBuilder, NodeId, Op, Provenance, ScalarType, SubProfile, Subject,
};
use noble_ngraph::sem::Value;
use noble_ngraph::snapshot::ParamSnapshot;

const WIDTH: usize = 64;

fn select(builder: &mut GraphBuilder, bit: NodeId, off: NodeId, on: NodeId) -> NodeId {
    builder.op(Op::F32Select { bit, off, on })
}

fn exact_adder() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&common::IDS, "recurrent-weights-wrapping-adder"),
    );
    let mut left = Vec::with_capacity(WIDTH);
    let mut right = Vec::with_capacity(WIDTH);
    for _ in 0..WIDTH {
        let a = builder.input(ScalarType::F32);
        let b = builder.input(ScalarType::F32);
        builder.guard(Fact::Bit, a);
        builder.guard(Fact::Bit, b);
        left.push(a);
        right.push(b);
    }
    let zero = builder.param();
    let one = builder.param();
    builder.fact(zero, Fact::Bit, Provenance::InputValidated);
    builder.fact(one, Fact::Bit, Provenance::InputValidated);
    let mut carry = zero;
    let mut sum = Vec::with_capacity(WIDTH);
    for (&a, &b) in left.iter().zip(&right) {
        let not_b = select(&mut builder, b, one, zero);
        let a_xor_b = select(&mut builder, a, b, not_b);
        let not_carry = select(&mut builder, carry, one, zero);
        let bit_sum = select(&mut builder, a_xor_b, carry, not_carry);
        let b_or_carry = select(&mut builder, b, carry, one);
        let b_and_carry = select(&mut builder, b, zero, carry);
        carry = select(&mut builder, a, b_and_carry, b_or_carry);
        sum.push(bit_sum);
    }
    builder.guard(Fact::Bit, carry);
    for bit in sum {
        builder.output(bit);
    }
    builder.finish()
}

fn encode_word(word: u64) -> Vec<Value> {
    (0..WIDTH)
        .map(|bit| Value::f32(((word >> bit) & 1) as f32))
        .collect()
}

fn decode_word(values: &[Value]) -> u64 {
    assert_eq!(values.len(), WIDTH);
    values.iter().enumerate().fold(0, |word, (bit, value)| {
        let Value::F32(bits) = value else {
            panic!("non-F32 output: {value:?}")
        };
        let cell = f32::from_bits(*bits);
        match cell {
            0.0 | 1.0 => {}
            _ => panic!("non-bit output: {cell:?}"),
        }
        word | ((cell as u64) << bit)
    })
}

fn run(graph: &Graph, a: u64, b: u64) -> u64 {
    let checked = check::check(&common::IDS, graph.clone()).expect("adder graph checks");
    let weights = ParamSnapshot::new(&common::IDS, &checked, &[0.0, 1.0]).expect("weight snapshot");
    let left = encode_word(a);
    let right = encode_word(b);
    let inputs = left
        .into_iter()
        .zip(right)
        .flat_map(|(a, b)| [a, b])
        .collect::<Vec<_>>();
    let compiled = Compiled::new(&common::IDS, &checked);
    let mut compiled_control = Control::default();
    let mut reference_control = Control::default();
    let compiled_result = compiled.evaluate(&inputs, Some(&weights), &mut compiled_control);
    let reference_result =
        eval::reference(&checked, &inputs, Some(&weights), &mut reference_control);
    let (Obs::Normal(compiled_values), Obs::Normal(reference_values)) =
        (compiled_result.obs, reference_result.obs)
    else {
        panic!("bit-adder evaluator refused a valid word")
    };
    assert_eq!(
        compiled_values, reference_values,
        "compiled/reference disagreement"
    );
    decode_word(&compiled_values)
}

#[test]
fn wrapping_adder_matches_reference_and_source_boundaries() {
    let graph = exact_adder();
    for a in 0..4 {
        for b in 0..4 {
            assert_eq!(run(&graph, a, b), a.wrapping_add(b));
        }
    }
    for (a, b) in [
        (0, 0),
        (1, 1),
        (u64::MAX, 1),
        (1u64 << 63, 1u64 << 63),
        (0x5555_5555_5555_5555, 0xaaaa_aaaa_aaaa_aaaa),
        (0x0000_0000_ffff_ffff, 1),
    ] {
        assert_eq!(run(&graph, a, b), a.wrapping_add(b));
    }
}
