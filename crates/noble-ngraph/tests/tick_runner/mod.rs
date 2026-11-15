//! Sequential tick protocol fixtures, independently expected recurrences and export.

mod export;
mod faults;
mod protocol;

use crate::common::IDS;
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::tick::{Limits, Partition, Runner};

pub(super) fn add_graph() -> Graph {
    let mut b = GraphBuilder::new(SubProfile::I64, Subject::authored(&IDS, "tick/add/v1"));
    let s = b.input(ScalarType::I64);
    let x = b.input(ScalarType::I64);
    let c = b.input(ScalarType::I64);
    let product = b.op(Op::I64Mul(c, x));
    let next = b.op(Op::I64Add(s, product));
    b.output(next);
    b.output(next);
    b.finish()
}

pub(super) fn sub_graph() -> Graph {
    let mut b = GraphBuilder::new(SubProfile::I64, Subject::authored(&IDS, "tick/sub/v1"));
    let s = b.input(ScalarType::I64);
    let x = b.input(ScalarType::I64);
    let c = b.input(ScalarType::I64);
    let product = b.op(Op::I64Mul(c, x));
    let next = b.op(Op::I64Sub(s, product));
    b.output(next);
    b.output(next);
    b.finish()
}

pub(super) fn two_graph() -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&IDS, "tick/two-state/v1"),
    );
    let s0 = b.input(ScalarType::I64);
    let s1 = b.input(ScalarType::I64);
    let x0 = b.input(ScalarType::I64);
    let x1 = b.input(ScalarType::I64);
    let c0 = b.input(ScalarType::I64);
    let c1 = b.input(ScalarType::I64);
    let product0 = b.op(Op::I64Mul(c0, x0));
    let next0 = b.op(Op::I64Sub(s0, product0));
    let product1 = b.op(Op::I64Mul(s1, c1));
    let sum = b.op(Op::I64Add(s0, product1));
    let next1 = b.op(Op::I64Sub(sum, x1));
    b.output(next0);
    b.output(next1);
    b.output(next1);
    b.output(next0);
    b.finish()
}

pub(super) const ONE: Partition = Partition {
    state_words: 1,
    input_words: 1,
    control_words: 1,
    output_words: 1,
};
pub(super) const TWO: Partition = Partition {
    state_words: 2,
    input_words: 2,
    control_words: 2,
    output_words: 2,
};

pub(super) fn runner(
    graph: Graph,
    partition: Partition,
    state: &[i64],
    controls: &[i64],
) -> Runner {
    Runner::prepare(&IDS, graph, partition, state, controls, Limits::default())
        .expect("fixture admitted")
}

// The independent oracle is straight host arithmetic, not the graph evaluator.
pub(super) fn expected_one(s: i64, x: i64, c: i64, subtract: bool) -> i64 {
    if subtract {
        s.wrapping_sub(c.wrapping_mul(x))
    } else {
        s.wrapping_add(c.wrapping_mul(x))
    }
}

pub(super) fn expected_two(
    state: [i64; 2],
    input: [i64; 2],
    control: [i64; 2],
) -> ([i64; 2], [i64; 2]) {
    let next0 = state[0].wrapping_sub(control[0].wrapping_mul(input[0]));
    let next1 = state[0]
        .wrapping_add(state[1].wrapping_mul(control[1]))
        .wrapping_sub(input[1]);
    ([next0, next1], [next1, next0])
}
