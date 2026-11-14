#![allow(
    tigerstyle::ambiguous_params,
    tigerstyle::assertion_density,
    tigerstyle::clone_then_borrow,
    tigerstyle::explicit_defaults,
    tigerstyle::fallible_int_fallback,
    tigerstyle::missing_const_fn,
    tigerstyle::no_unwrap,
    tigerstyle::no_panic,
    tigerstyle::non_trait_imports,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness builds an interleaved checked I64 graph, validates its actual packed stream and then mutates one obligation at a time, so terse fixture names, unwraps and assertion panics are the readable form."
)]
//! The actual emitted I64/Bool stream validated against its checked graph, one mutation per
//! obligation the issue names (issue #43).
use super::common;
use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{Control, Obs};
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::layout::{self, PackedLayout, Refusal};
use noble_ngraph::schedule::{self, I64Instruction, StreamRefusal};

/// Two branches sharing both inputs, a constant and a Bool equality: four instruction-emitting
/// nodes, so a within-layer reorder and an operand swap are both expressible.
pub(super) fn interleaved() -> Graph {
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

/// The real declaration and the real emitted stream of one checked graph, with the assertion
/// that this pair is the accepted one.
pub(super) fn packed(graph: Graph) -> (CheckedGraph, PackedLayout, Vec<I64Instruction>) {
    let scheme = common::IDS;
    let accepted = check::check(&scheme, graph).expect("endpoint checks");
    let compiled = Compiled::new(&scheme, &accepted);
    let layout = compiled.packed_layout();
    let ins = compiled.i64_instructions().expect("I64 stream").to_vec();
    let pair = schedule::check(&accepted, &layout, &ins);
    assert_eq!(pair, Ok(()), "real pair");
    assert_eq!(layout::check(&accepted, &layout), Ok(()), "real declare");
    (accepted, layout, ins)
}

pub(super) fn declared(layout: &PackedLayout) -> u32 {
    layout.layers.iter().map(|(start, end)| end - start).sum()
}

#[test]
fn the_real_packed_stream_is_validated() {
    let (accepted, layout, ins) = packed(interleaved());
    assert_eq!(ins.len(), 5, "four arithmetic nodes plus one equality");
    assert_eq!(declared(&layout), u32::try_from(ins.len()).expect("fits"));
    let equal = |instruction: &I64Instruction| matches!(instruction, I64Instruction::Eq(..));
    assert!(ins.iter().any(equal), "the Bool equality is in the stream");
    assert!(accepted.graph().outputs.len() == 2, "both outputs retained");
}

#[test]
fn a_wrong_operation_is_refused() {
    let (accepted, layout, mut ins) = packed(interleaved());
    let at = ins
        .iter()
        .position(|instruction| matches!(instruction, I64Instruction::Add(..)))
        .expect("a wrapping addition");
    if let I64Instruction::Add(destination, left, right) = ins[at] {
        ins[at] = I64Instruction::Sub(destination, left, right);
    }
    let refused = schedule::check(&accepted, &layout, &ins);
    assert_eq!(refused, Err(StreamRefusal::BodyMismatch), "Add for Sub");
}

#[test]
fn swapped_operands_are_refused() {
    let (accepted, layout, mut ins) = packed(interleaved());
    let distinct = |instruction: &I64Instruction| match *instruction {
        I64Instruction::Add(_, left, right) => left != right,
        I64Instruction::Sub(..)
        | I64Instruction::Mul(..)
        | I64Instruction::Eq(..)
        | I64Instruction::Guard { .. } => false,
    };
    let at = ins.iter().position(distinct).expect("distinct operands");
    if let I64Instruction::Add(destination, left, right) = ins[at] {
        ins[at] = I64Instruction::Add(destination, right, left);
    }
    let refused = schedule::check(&accepted, &layout, &ins);
    assert_eq!(refused, Err(StreamRefusal::BodyMismatch), "operand order");
}

#[test]
fn a_wrong_destination_is_refused() {
    let (accepted, layout, mut ins) = packed(interleaved());
    let taken: Vec<u32> = ins
        .iter()
        .filter_map(|instruction| match *instruction {
            I64Instruction::Add(d, ..)
            | I64Instruction::Sub(d, ..)
            | I64Instruction::Mul(d, ..)
            | I64Instruction::Eq(d, ..) => Some(d),
            I64Instruction::Guard { .. } => None,
        })
        .collect();
    let free = (0..layout.regs)
        .find(|register| !taken.contains(register))
        .expect("a register the stream does not write");
    let at = ins
        .iter()
        .position(|instruction| matches!(instruction, I64Instruction::Mul(..)))
        .expect("a wrapping multiplication");
    if let I64Instruction::Mul(destination, left, right) = ins[at] {
        assert!(taken.contains(&destination));
        ins[at] = I64Instruction::Mul(free, left, right);
    }
    let refused = schedule::check(&accepted, &layout, &ins);
    assert_eq!(refused, Err(StreamRefusal::BodyMismatch), "destination");
}

#[test]
fn a_within_layer_reorder_is_refused() {
    let (accepted, layout, mut ins) = packed(interleaved());
    let (start, end) = layout
        .layers
        .iter()
        .copied()
        .find(|(start, end)| end - start >= 2)
        .expect("a layer with two instructions");
    let first = usize::try_from(start).expect("fits");
    ins.swap(first, first + 1);
    let refused = schedule::check(&accepted, &layout, &ins);
    assert_eq!(refused, Err(StreamRefusal::BodyMismatch), "reordered pair");
    assert_eq!(end - start, 2, "the layer holds exactly the pair");
}

#[test]
fn a_missing_or_duplicated_computation_is_refused() {
    let (accepted, layout, ins) = packed(interleaved());
    let mut missing = ins.clone();
    missing.pop();
    let dropped = schedule::check(&accepted, &layout, &missing);
    assert_eq!(dropped, Err(StreamRefusal::StreamShape), "one dropped");
    let mut duplicated = ins.clone();
    duplicated.push(ins[0]);
    let duplicated = schedule::check(&accepted, &layout, &duplicated);
    assert_eq!(duplicated, Err(StreamRefusal::StreamShape), "duplicated");
}

#[test]
fn a_bad_layer_boundary_keeps_the_declaration_class() {
    let (accepted, layout, ins) = packed(interleaved());
    let mut mutated = layout.clone();
    // Shorten the last range: ordering still holds, so the refusal is about coverage.
    let last = mutated.layers.len().checked_sub(1).expect("one layer");
    let (start, end) = mutated.layers[last];
    assert!(end > start, "the last layer holds an instruction");
    mutated.layers[last].1 = end.saturating_sub(1);
    let refused = schedule::check(&accepted, &mutated, &ins);
    let coverage = Err(StreamRefusal::Declarations(Refusal::LayerCoverage));
    assert_eq!(refused, coverage);
}

#[test]
fn a_stale_layout_revision_is_refused_before_any_body() {
    let (accepted, layout, ins) = packed(interleaved());
    let mut mutated = layout.clone();
    mutated.revision = String::from("ngraph-layout/1");
    let refused = schedule::check(&accepted, &mutated, &ins);
    let expected = Err(StreamRefusal::Declarations(Refusal::StaleRevision));
    assert_eq!(refused, expected, "a layout from another revision");
}

#[test]
fn swapped_output_registers_are_refused() {
    let (accepted, layout, ins) = packed(interleaved());
    let outputs = accepted.graph().outputs.clone();
    let mut mutated = layout.clone();
    let first = usize::try_from(outputs[0].0).expect("fits");
    let second = usize::try_from(outputs[1].0).expect("fits");
    mutated.reg.swap(first, second);
    let declaration = layout::check(&accepted, &mutated);
    assert_eq!(declaration, Ok(()), "still injective and in range");
    let refused = schedule::check(&accepted, &mutated, &ins);
    assert_eq!(refused, Err(StreamRefusal::BodyMismatch), "swapped regs");
}

#[test]
fn a_post_check_substitution_is_refused() {
    let (accepted, layout, _) = packed(interleaved());
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
    let other = check::check(&scheme, builder.finish()).expect("checks");
    let compiled = Compiled::new(&scheme, &other);
    let other_layout = compiled.packed_layout();
    let other_ins = compiled.i64_instructions().expect("I64").to_vec();
    let stream_len = u32::try_from(other_ins.len()).expect("fits");
    assert_ne!(stream_len, declared(&layout), "not this stream");
    let substituted = schedule::check(&accepted, &other_layout, &other_ins);
    assert!(matches!(
        substituted,
        Err(StreamRefusal::Declarations(_))
            | Err(StreamRefusal::StreamShape)
            | Err(StreamRefusal::BodyMismatch)
    ));
    let own = schedule::check(&other, &other_layout, &other_ins);
    assert_eq!(own, Ok(()), "the substituted graph's own pair validates");
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
    let accepted = check::check(&scheme, builder.finish()).expect("checks");
    let compiled = Compiled::new(&scheme, &accepted);
    assert!(compiled.i64_instructions().is_none());
    let refused = schedule::check(&accepted, &compiled.packed_layout(), &[]);
    assert_eq!(refused, Err(StreamRefusal::UnsupportedProfile));
}

#[test]
fn an_empty_graph_compiles_to_a_layout_and_stream_both_validators_accept() {
    let scheme = common::IDS;
    let empty = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "schedule-correspondence-empty"),
    )
    .finish();
    let accepted = check::check(&scheme, empty).expect("the checker admits an empty graph");
    assert_eq!(accepted.layer_count(), 0);
    let compiled = Compiled::new(&scheme, &accepted);
    let layout = compiled.packed_layout();
    assert!(layout.layers.is_empty(), "no layers to tile");
    assert_eq!(layout.fail_suffix_min, vec![u32::MAX], "one trailing entry");
    assert_eq!(layout::check(&accepted, &layout), Ok(()));
    let stream = compiled.i64_instructions().expect("I64");
    assert!(stream.is_empty());
    assert_eq!(schedule::check(&accepted, &layout, stream), Ok(()));
    let report = compiled.evaluate(&[], None, &mut Control::default());
    assert_eq!(report.obs, Obs::Normal(Vec::new()));
    assert_eq!(report.evaluated_nodes, 0);
}
