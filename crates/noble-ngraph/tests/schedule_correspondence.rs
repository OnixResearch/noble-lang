#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![expect(
    tigerstyle::underscore_in_module_filename,
    reason = "test target: the filename identifies the packed-schedule correspondence scenario"
)]
#![expect(
    tigerstyle::no_panic,
    reason = "test target: assertion panic reports a refused or wrongly admitted packed stream"
)]
#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::fallible_int_fallback,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::assertion_density,
    tigerstyle::ambiguous_params,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; this harness builds an interleaved checked I64 graph, validates its actual packed stream and then mutates one obligation at a time, so terse fixture names and unwraps are the readable form."
)]
//! The actual emitted I64/Bool instruction stream, validated against its checked graph, and one
//! mutation per obligation the issue names (issue #43).
use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::layout::{self, PackedLayout, Refusal};
use noble_ngraph::schedule::{self, I64Instruction, StreamRefusal};

mod common;

/// Two independent branches sharing both inputs, a constant, and a Bool equality: four
/// instruction-emitting nodes, so a within-layer reorder and an operand swap are both expressible.
fn interleaved() -> Graph {
    let scheme = common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "schedule-correspondence-i64"),
    );
    let left = builder.input(ScalarType::I64);
    let right = builder.input(ScalarType::I64);
    let scale = builder.i64_const(-3);
    let sum = builder.op(Op::I64Add(left, right));
    let difference = builder.op(Op::I64Sub(right, left));
    let scaled = builder.op(Op::I64Mul(sum, scale));
    let product = builder.op(Op::I64Mul(difference, scale));
    let equal = builder.op(Op::I64Eq(scaled, product));
    builder.output(equal);
    builder.output(sum);
    builder.finish()
}

fn checked(graph: &Graph) -> CheckedGraph {
    let scheme = common::IDS;
    check::check(&scheme, graph.clone()).expect("endpoint checks")
}

/// The real packed layout and the real emitted stream of one checked graph.
fn packed(graph: &Graph) -> (CheckedGraph, PackedLayout, Vec<I64Instruction>) {
    let scheme = common::IDS;
    let accepted = checked(graph);
    let compiled = Compiled::new(&scheme, &accepted);
    let layout = compiled.packed_layout();
    let ins = compiled
        .i64_instructions()
        .expect("the I64 sub-profile has a stream")
        .to_vec();
    (accepted, layout, ins)
}

fn validated(graph: &Graph) -> (CheckedGraph, PackedLayout, Vec<I64Instruction>) {
    let (accepted, layout, ins) = packed(graph);
    assert_eq!(
        schedule::check(&accepted, &layout, &ins),
        Ok(()),
        "the real emitted stream is validated"
    );
    assert_eq!(
        layout::check(&accepted, &layout),
        Ok(()),
        "the real declaration is admitted"
    );
    (accepted, layout, ins)
}

#[test]
fn the_real_packed_stream_is_validated() {
    let (accepted, layout, ins) = validated(&interleaved());
    assert_eq!(ins.len(), 5, "four arithmetic nodes plus one equality");
    assert_eq!(
        layout.layers.iter().map(|(start, end)| end - start).sum::<u32>() as usize,
        ins.len(),
        "the declared ranges cover exactly the stream"
    );
    assert!(
        ins.iter()
            .any(|instruction| matches!(instruction, I64Instruction::Eq(_, _, _))),
        "the Bool equality is part of the validated stream"
    );
    assert!(
        accepted.graph().outputs.len() == 2,
        "both outputs are retained"
    );
}

#[test]
fn a_wrong_operation_is_refused() {
    let (accepted, layout, mut ins) = validated(&interleaved());
    let at = ins
        .iter()
        .position(|instruction| matches!(instruction, I64Instruction::Add(_, _, _)))
        .expect("a wrapping addition");
    if let I64Instruction::Add(destination, left, right) = ins[at] {
        ins[at] = I64Instruction::Sub(destination, left, right);
    }
    assert_eq!(
        schedule::check(&accepted, &layout, &ins),
        Err(StreamRefusal::BodyMismatch),
        "addition replaced by subtraction"
    );
}

#[test]
fn swapped_operands_are_refused() {
    let (accepted, layout, mut ins) = validated(&interleaved());
    let at = ins
        .iter()
        .position(|instruction| match *instruction {
            I64Instruction::Add(_, left, right) => left != right,
            _ => false,
        })
        .expect("an addition with two distinct operands");
    if let I64Instruction::Add(destination, left, right) = ins[at] {
        ins[at] = I64Instruction::Add(destination, right, left);
    }
    assert_eq!(
        schedule::check(&accepted, &layout, &ins),
        Err(StreamRefusal::BodyMismatch),
        "ordered source registers are part of the body"
    );
}

#[test]
fn a_wrong_destination_is_refused() {
    let (accepted, layout, mut ins) = validated(&interleaved());
    let taken: Vec<u32> = ins
        .iter()
        .filter_map(|instruction| match *instruction {
            I64Instruction::Add(d, _, _)
            | I64Instruction::Sub(d, _, _)
            | I64Instruction::Mul(d, _, _)
            | I64Instruction::Eq(d, _, _) => Some(d),
            I64Instruction::Guard { .. } => None,
        })
        .collect();
    let free = (0..layout.regs)
        .find(|register| !taken.contains(register))
        .expect("a register the stream does not write");
    let at = ins
        .iter()
        .position(|instruction| matches!(instruction, I64Instruction::Mul(_, _, _)))
        .expect("a wrapping multiplication");
    if let I64Instruction::Mul(destination, left, right) = ins[at] {
        assert!(taken.contains(&destination));
        ins[at] = I64Instruction::Mul(free, left, right);
    }
    assert_eq!(
        schedule::check(&accepted, &layout, &ins),
        Err(StreamRefusal::BodyMismatch),
        "a destination that is not the node's declared register"
    );
}

#[test]
fn a_within_layer_reorder_is_refused() {
    let (accepted, layout, mut ins) = validated(&interleaved());
    let (start, end) = layout
        .layers
        .iter()
        .copied()
        .find(|(start, end)| end - start >= 2)
        .expect("a layer with two independent instructions");
    ins.swap(start as usize, start as usize + 1);
    assert_eq!(
        schedule::check(&accepted, &layout, &ins),
        Err(StreamRefusal::BodyMismatch),
        "each position is compared against its own node, not against the layer as a set"
    );
    assert_eq!(end - start, 2);
}

#[test]
fn a_missing_or_duplicated_computation_is_refused() {
    let (accepted, layout, ins) = validated(&interleaved());
    let mut missing = ins.clone();
    missing.pop();
    assert_eq!(
        schedule::check(&accepted, &layout, &missing),
        Err(StreamRefusal::StreamShape),
        "one instruction dropped"
    );
    let mut duplicated = ins.clone();
    duplicated.push(ins[0]);
    assert_eq!(
        schedule::check(&accepted, &layout, &duplicated),
        Err(StreamRefusal::StreamShape),
        "one instruction duplicated"
    );
}

#[test]
fn a_bad_layer_boundary_keeps_the_declaration_class() {
    let (accepted, layout, ins) = validated(&interleaved());
    let mut mutated = layout.clone();
    // Shorten the last range: the ordering rule still holds, so the refusal is about coverage.
    let last = mutated.layers.len().checked_sub(1).expect("at least one layer");
    let (start, end) = mutated.layers[last];
    assert!(end > start, "the last layer holds at least one instruction");
    mutated.layers[last].1 = end.saturating_sub(1);
    assert_eq!(
        schedule::check(&accepted, &mutated, &ins),
        Err(StreamRefusal::Declarations(Refusal::LayerCoverage)),
        "a range that no longer covers exactly one layer's instructions"
    );
}

#[test]
fn a_stale_layout_revision_is_refused_before_any_body() {
    let (accepted, layout, ins) = validated(&interleaved());
    let mut mutated = layout.clone();
    mutated.revision = String::from("ngraph-layout/1");
    assert_eq!(
        schedule::check(&accepted, &mutated, &ins),
        Err(StreamRefusal::Declarations(Refusal::StaleRevision)),
        "a layout declaration from another revision"
    );
}

#[test]
fn swapped_output_registers_are_refused() {
    let scheme = common::IDS;
    let graph = interleaved();
    let (accepted, layout, ins) = validated(&graph);
    let outputs = accepted.graph().outputs.clone();
    let mut mutated = layout.clone();
    let left = outputs[0].0 as usize;
    let right = outputs[1].0 as usize;
    mutated.reg.swap(left, right);
    assert_eq!(
        layout::check(&accepted, &mutated),
        Ok(()),
        "the swapped declaration is still injective and in range"
    );
    assert_eq!(
        schedule::check(&accepted, &mutated, &ins),
        Err(StreamRefusal::BodyMismatch),
        "the stream still writes the original registers, so it cannot be the swapped graph's body"
    );
    assert_eq!(Compiled::new(&scheme, &accepted).layout_key(), Compiled::new(&scheme, &accepted).layout_key());
}

#[test]
fn a_post_check_substitution_is_refused() {
    let (accepted, layout, _) = validated(&interleaved());
    let scheme = common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "schedule-correspondence-i64-substituted"),
    );
    let left = builder.input(ScalarType::I64);
    let right = builder.input(ScalarType::I64);
    let sum = builder.op(Op::I64Add(left, right));
    let doubled = builder.op(Op::I64Add(sum, sum));
    builder.output(doubled);
    let other = checked(&builder.finish());
    let other_layout = Compiled::new(&scheme, &other).packed_layout();
    let other_ins = Compiled::new(&scheme, &other)
        .i64_instructions()
        .expect("I64 stream")
        .to_vec();
    assert_ne!(
        other_ins.len(),
        layout.layers.iter().map(|(start, end)| end - start).sum::<u32>() as usize,
        "the substituted stream is not this declaration's"
    );
    assert!(matches!(
        schedule::check(&accepted, &other_layout, &other_ins.clone()),
        Err(StreamRefusal::Declarations(_)) | Err(StreamRefusal::StreamShape) | Err(StreamRefusal::BodyMismatch)
    ));
    assert!(
        schedule::check(&other, &other_layout, &other_ins).is_ok(),
        "and the substituted graph's own pair validates, so the refusal above is about the pairing"
    );
}

#[test]
fn an_f32_profile_is_outside_this_validator() {
    let scheme = common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&scheme, "schedule-correspondence-f32"),
    );
    let constant = builder.f32_const(0.5);
    builder.output(constant);
    let accepted = checked(&builder.finish());
    let compiled = Compiled::new(&scheme, &accepted);
    assert!(compiled.i64_instructions().is_none());
    assert_eq!(
        schedule::check(&accepted, &compiled.packed_layout(), &[]),
        Err(StreamRefusal::UnsupportedProfile),
        "the F32 slice is not covered"
    );
}
