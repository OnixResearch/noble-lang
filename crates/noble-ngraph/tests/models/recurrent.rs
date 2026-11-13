use crate::common;
#[path = "recurrent/checker.rs"]
mod checker;
#[path = "recurrent/programs.rs"]
mod programs;
use noble_ngraph::ir::{
    Fact, Graph, GraphBuilder, NodeId, Op, Provenance, ScalarType, SubProfile, Subject,
};
use programs::Program;

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

fn run(graph: &Graph, a: u64, b: u64) -> u64 {
    checker::CandidateChecker::new(Program::Accumulator, graph.clone(), &[0.0, 1.0])
        .expect("checker accepts emitted graph")
        .one_step(&[a], &[b])
        .expect("checker verifies transition")[0]
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

#[test]
fn stack_routing_is_exhaustive_over_the_tiny_domain() {
    let checker = checker::CandidateChecker::new(Program::StackSwap, programs::swap_graph(), &[])
        .expect("checker accepts routing graph");
    for left in 0..4 {
        for right in 0..4 {
            assert_eq!(
                checker
                    .one_step(&[left, right], &[])
                    .expect("swap transition"),
                [right, left]
            );
        }
    }
}

#[test]
fn both_target_programs_emit_stable_artifact_manifest_rows() {
    let adder_a = checker::CandidateChecker::new(Program::Accumulator, exact_adder(), &[0.0, 1.0])
        .expect("adder checker");
    let adder_b = checker::CandidateChecker::new(Program::Accumulator, exact_adder(), &[0.0, 1.0])
        .expect("same adder checker");
    let swap_a = checker::CandidateChecker::new(Program::StackSwap, programs::swap_graph(), &[])
        .expect("swap checker");
    let swap_b = checker::CandidateChecker::new(Program::StackSwap, programs::swap_graph(), &[])
        .expect("same swap checker");
    assert_eq!(adder_a.identities(), adder_b.identities());
    assert_eq!(swap_a.identities(), swap_b.identities());
    for (name, checker) in [("accumulator", adder_a), ("stack-swap", swap_a)] {
        let (graph, snapshot, bytes) = checker.identities();
        println!(
            "RECURRENT_MANIFEST\t{name}\t{}\t{}\t{}",
            hex(&graph),
            hex(&snapshot),
            hex(&bytes)
        );
    }
}

#[test]
fn host_loop_trajectory_reports_teacher_forced_and_free_running_separately() {
    let checker = checker::CandidateChecker::new(Program::Accumulator, exact_adder(), &[0.0, 1.0])
        .expect("adder checker");
    let mut teacher_source = 0u64;
    let mut teacher_steps = 0usize;
    for step in 0..1_000u64 {
        let input = step.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let expected =
            programs::source_transition(Program::Accumulator, &[teacher_source], &[input])
                .expect("source domain")[0];
        assert_eq!(
            checker
                .one_step(&[teacher_source], &[input])
                .expect("teacher-forced step"),
            [expected]
        );
        teacher_source = expected;
        teacher_steps += 1;
    }

    let mut free_state = 0u64;
    let mut free_steps = 0usize;
    for step in 0..1_000u64 {
        let input = step.wrapping_mul(0xd6e8_feb8_6659_fd93);
        let target_next = checker
            .one_step(&[free_state], &[input])
            .expect("free-running step")[0];
        let source_next =
            programs::source_transition(Program::Accumulator, &[free_state], &[input])
                .expect("source domain")[0];
        assert_eq!(target_next, source_next, "free-running step {step}");
        free_state = target_next;
        free_steps += 1;
    }
    println!("RECURRENT_TRAJECTORY\tteacher_forced\t{teacher_steps}");
    println!("RECURRENT_TRAJECTORY\tfree_running\t{free_steps}");
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
