use noble_ngraph::ir::{Fact, Graph, GraphBuilder, ScalarType, SubProfile, Subject};

#[derive(Clone, Copy, Debug)]
pub enum Program {
    Accumulator,
    StackSwap,
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
