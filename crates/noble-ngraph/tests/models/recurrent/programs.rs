use noble_ngraph::ir::{
    Fact, Graph, GraphBuilder, NodeId, Op, Provenance, ScalarType, SubProfile, Subject,
};

#[derive(Clone, Copy, Debug)]
pub enum Program {
    Accumulator,
    StackSwap,
}

fn bit_params(builder: &mut GraphBuilder) -> (NodeId, NodeId) {
    let zero = builder.param();
    let one = builder.param();
    builder.fact(zero, Fact::Bit, Provenance::InputValidated);
    builder.fact(one, Fact::Bit, Provenance::InputValidated);
    (zero, one)
}

fn select(builder: &mut GraphBuilder, bit: NodeId, off: NodeId, on: NodeId) -> NodeId {
    builder.op(Op::F32Select { bit, off, on })
}

fn add_step(
    builder: &mut GraphBuilder,
    left: &[NodeId],
    right: &[NodeId],
    zero: NodeId,
    one: NodeId,
) -> Vec<NodeId> {
    let mut carry = zero;
    let mut sum = Vec::with_capacity(64);
    for (&a, &b) in left.iter().zip(right) {
        let not_b = select(builder, b, one, zero);
        let a_xor_b = select(builder, a, b, not_b);
        let not_carry = select(builder, carry, one, zero);
        sum.push(select(builder, a_xor_b, carry, not_carry));
        let b_or_carry = select(builder, b, carry, one);
        let b_and_carry = select(builder, b, zero, carry);
        carry = select(builder, a, b_and_carry, b_or_carry);
    }
    builder.guard(Fact::Bit, carry);
    sum
}

pub fn one_step_adder() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&crate::common::IDS, "recurrent-weights-wrapping-adder"),
    );
    let mut left = Vec::with_capacity(64);
    let mut right = Vec::with_capacity(64);
    for _ in 0..64 {
        let a = builder.input(ScalarType::F32);
        let b = builder.input(ScalarType::F32);
        builder.guard(Fact::Bit, a);
        builder.guard(Fact::Bit, b);
        left.push(a);
        right.push(b);
    }
    let (zero, one) = bit_params(&mut builder);
    for bit in add_step(&mut builder, &left, &right, zero, one) {
        builder.output(bit);
    }
    builder.finish()
}

pub fn unrolled_adder(steps: usize) -> Graph {
    assert!((1..=4).contains(&steps), "unrolled cap is four transitions");
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&crate::common::IDS, "recurrent-weights-unrolled-adder"),
    );
    let mut state = Vec::with_capacity(64);
    for _ in 0..64 {
        let bit = builder.input(ScalarType::F32);
        builder.guard(Fact::Bit, bit);
        state.push(bit);
    }
    let (zero, one) = bit_params(&mut builder);
    for _ in 0..steps {
        let mut input = Vec::with_capacity(64);
        for _ in 0..64 {
            let bit = builder.input(ScalarType::F32);
            builder.guard(Fact::Bit, bit);
            input.push(bit);
        }
        state = add_step(&mut builder, &state, &input, zero, one);
    }
    for bit in state {
        builder.output(bit);
    }
    builder.finish()
}

pub fn unrolled_swap(steps: usize) -> Graph {
    assert!((1..=4).contains(&steps), "unrolled cap is four transitions");
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&crate::common::IDS, "recurrent-weights-unrolled-stack-swap"),
    );
    let mut words = [Vec::with_capacity(64), Vec::with_capacity(64)];
    for word in &mut words {
        for _ in 0..64 {
            let bit = builder.input(ScalarType::F32);
            builder.guard(Fact::Bit, bit);
            word.push(bit);
        }
    }
    for _ in 0..steps {
        words.swap(0, 1);
    }
    for word in &words {
        for bit in word {
            builder.output(*bit);
        }
    }
    builder.finish()
}

pub fn swap_graph() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&crate::common::IDS, "recurrent-weights-stack-swap"),
    );
    let mut words = [Vec::with_capacity(64), Vec::with_capacity(64)];
    for word in &mut words {
        for _ in 0..64 {
            let bit = builder.input(ScalarType::F32);
            builder.guard(Fact::Bit, bit);
            word.push(bit);
        }
    }
    for bit in words[1].iter().chain(&words[0]) {
        builder.output(*bit);
    }
    builder.finish()
}

pub fn source_transition(program: Program, state: &[u64], input: &[u64]) -> Option<Vec<u64>> {
    match program {
        Program::Accumulator if state.len() == 1 && input.len() == 1 => {
            Some(vec![state[0].wrapping_add(input[0])])
        }
        Program::StackSwap if state.len() == 2 && input.is_empty() => {
            Some(vec![state[1], state[0]])
        }
        _ => None,
    }
}

pub fn inputs_for(program: Program, state: &[u64], input: &[u64]) -> Option<Vec<u64>> {
    match program {
        Program::Accumulator if state.len() == 1 && input.len() == 1 => {
            Some(vec![state[0], input[0]])
        }
        Program::StackSwap if state.len() == 2 && input.is_empty() => Some(state.to_vec()),
        _ => None,
    }
}
