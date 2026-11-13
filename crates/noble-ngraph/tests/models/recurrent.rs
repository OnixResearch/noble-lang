use crate::common;
#[path = "recurrent/checker.rs"]
mod checker;
#[path = "recurrent/measurement.rs"]
mod measurement;
#[path = "recurrent/programs.rs"]
mod programs;
use noble_ngraph::ir::{Fact, GraphBuilder, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;
use programs::Program;

#[test]
fn wrapping_adder_matches_reference_and_source_boundaries() {
    let checker = checker::Candidate::new(
        Program::Accumulator,
        programs::one_step_adder(),
        &[0.0, 1.0],
    )
    .expect("checker accepts emitted graph");
    for a in 0..4 {
        for b in 0..4 {
            assert_eq!(
                checker
                    .one_step(&[a], &[b])
                    .expect("accumulator transition"),
                [a.wrapping_add(b)]
            );
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
        assert_eq!(
            checker.one_step(&[a], &[b]).expect("boundary transition"),
            [a.wrapping_add(b)]
        );
    }
}

#[test]
fn stack_routing_is_exhaustive_over_the_tiny_domain() {
    let checker = checker::Candidate::new(Program::StackSwap, programs::swap_graph(), &[])
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
    let adder_a = checker::Candidate::new(
        Program::Accumulator,
        programs::one_step_adder(),
        &[0.0, 1.0],
    )
    .expect("adder checker");
    let adder_b = checker::Candidate::new(
        Program::Accumulator,
        programs::one_step_adder(),
        &[0.0, 1.0],
    )
    .expect("same adder checker");
    let swap_a = checker::Candidate::new(Program::StackSwap, programs::swap_graph(), &[])
        .expect("swap checker");
    let swap_b = checker::Candidate::new(Program::StackSwap, programs::swap_graph(), &[])
        .expect("same swap checker");
    assert_eq!(adder_a.identities(), adder_b.identities());
    assert_eq!(swap_a.identities(), swap_b.identities());
    assert_eq!(adder_a.graph_bytes(), adder_b.graph_bytes());
    assert_eq!(swap_a.graph_bytes(), swap_b.graph_bytes());
    for (name, checker) in [("accumulator", adder_a), ("stack-swap", swap_a)] {
        let (graph, snapshot, bytes) = checker.identities();
        let graph_bytes = checker.graph_bytes();
        println!(
            "RECURRENT_MANIFEST\t{name}\t{}\t{}\t{}\t{}",
            hex(&graph),
            hex(&snapshot),
            hex(&bytes),
            hex(&graph_bytes)
        );
    }
}

#[test]
fn host_loop_trajectory_reports_teacher_forced_and_free_running_separately() {
    let checker = checker::Candidate::new(
        Program::Accumulator,
        programs::one_step_adder(),
        &[0.0, 1.0],
    )
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
    println!("RECURRENT_TRAJECTORY\taccumulator\tteacher_forced\t{teacher_steps}");
    println!("RECURRENT_TRAJECTORY\taccumulator\tfree_running\t{free_steps}");
}

#[test]
fn stack_swap_trajectory_reports_teacher_forced_and_free_running_separately() {
    let candidate = checker::Candidate::new(Program::StackSwap, programs::swap_graph(), &[])
        .expect("stack-swap checker");
    let (mut teacher_left, mut teacher_right) = (1u64, 2u64);
    for _ in 0..1_000 {
        let expected =
            programs::source_transition(Program::StackSwap, &[teacher_left, teacher_right], &[])
                .expect("source domain");
        assert_eq!(
            candidate
                .one_step(&[teacher_left, teacher_right], &[])
                .expect("teacher-forced swap"),
            expected
        );
        [teacher_left, teacher_right] = [expected[0], expected[1]];
    }
    let (mut free_left, mut free_right) = (1u64, 2u64);
    for step in 0..1_000 {
        let target = candidate
            .one_step(&[free_left, free_right], &[])
            .expect("free-running swap");
        let expected =
            programs::source_transition(Program::StackSwap, &[free_left, free_right], &[])
                .expect("source domain");
        assert_eq!(target, expected, "free-running swap step {step}");
        [free_left, free_right] = [target[0], target[1]];
    }
    println!("RECURRENT_TRAJECTORY\tstack-swap\tteacher_forced\t1000");
    println!("RECURRENT_TRAJECTORY\tstack-swap\tfree_running\t1000");
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn malformed_state_input_and_decoder_values_are_refused() {
    let candidate = checker::Candidate::new(
        Program::Accumulator,
        programs::one_step_adder(),
        &[0.0, 1.0],
    )
    .expect("accumulator checker");
    assert!(candidate.one_step(&[], &[0]).is_err());
    assert!(candidate.one_step(&[0], &[]).is_err());
    assert!(candidate.one_step(&[0, 1], &[0]).is_err());
    let mut malformed = vec![Value::f32(0.0); 64];
    malformed[7] = Value::f32(f32::NAN);
    assert!(checker::decode_words(&malformed).is_err());
    malformed[7] = Value::f32(-0.0);
    assert!(checker::decode_words(&malformed).is_err());
    assert!(checker::decode_words(&malformed[..63]).is_err());
}

#[test]
fn corrupted_weights_and_seeded_wrong_transition_are_detected() {
    let corrupted = checker::Candidate::new(
        Program::Accumulator,
        programs::one_step_adder(),
        &[1.0, 0.0],
    )
    .expect("bit-valued but corrupted snapshot");
    assert!(corrupted.one_step(&[0], &[1]).is_err());

    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&common::IDS, "recurrent-weights-seeded-zero-transition"),
    );
    for _ in 0..128 {
        let input = builder.input(ScalarType::F32);
        builder.guard(Fact::Bit, input);
    }
    let zero = builder.f32_const(0.0);
    for _ in 0..64 {
        builder.output(zero);
    }
    let incorrect = checker::Candidate::new(Program::Accumulator, builder.finish(), &[])
        .expect("seeded wrong candidate passes structural admission");
    assert!(incorrect.one_step(&[0], &[1]).is_err());
}

#[test]
fn wrong_program_configuration_and_little_endian_decoder_are_detected() {
    let wrong_program =
        checker::Candidate::new(Program::StackSwap, programs::one_step_adder(), &[0.0, 1.0])
            .expect("shape-compatible graph with wrong declared program");
    assert!(wrong_program.one_step(&[1, 2], &[]).is_err());
    let candidate = checker::Candidate::new(Program::StackSwap, programs::swap_graph(), &[])
        .expect("stack-swap checker");
    let reversed = [1u64, 2]
        .into_iter()
        .flat_map(|word| {
            (0..64)
                .rev()
                .map(move |bit| Value::f32(((word >> bit) & 1) as f32))
        })
        .collect::<Vec<_>>();
    let wrong_decode = checker::decode_words(&reversed).expect("valid but wrong byte order");
    let correct = candidate.one_step(&[1, 2], &[]).expect("source transition");
    assert!(candidate
        .verify_decoded(&[1, 2], &[], &wrong_decode)
        .is_err());
    assert_eq!(correct, [2, 1]);
}

#[test]
fn insufficient_budget_cancellation_and_resume_are_explicit() {
    let candidate = checker::Candidate::new(
        Program::Accumulator,
        programs::one_step_adder(),
        &[0.0, 1.0],
    )
    .expect("accumulator checker");
    assert!(candidate
        .one_step_with_control(&[1], &[2], Some(0), None)
        .is_err());
    let cancel_at_layer = |_layer: u32| true;
    assert!(candidate
        .one_step_with_control(&[1], &[2], None, Some(&cancel_at_layer))
        .is_err());
    assert_eq!(
        candidate.one_step(&[1], &[2]).expect("resume at boundary"),
        [3]
    );
}
