#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::fallible_int_fallback,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::assertion_density,
    tigerstyle::ambiguous_params,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; this integration harness builds an interleaved checked graph and asserts its validated layout and execution outcomes, so terse fixture names and unwraps are the readable form."
)]
use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::layout::{check as layout_check, PackedLayout, Refusal};
use noble_ngraph::sem::Value;

mod common;

/// Two independent I64 branches that share both inputs, plus an exact constant, a Bool
/// equality over the two products, and every intermediate as an output. The dataflow fixes only
/// the two roots before their consumers, which is the reordering case rather than a chain.
fn reordered_i64() -> Graph {
    let scheme = common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "layout-differential-i64-reordered"),
    );
    let left = builder.input(ScalarType::I64);
    let right = builder.input(ScalarType::I64);
    let scale = builder.i64_const(-3);
    let sum = builder.op(Op::I64Add(left, right));
    let difference = builder.op(Op::I64Sub(right, left));
    let scaled = builder.op(Op::I64Mul(sum, scale));
    let product = builder.op(Op::I64Mul(difference, scale));
    let equal = builder.op(Op::I64Eq(scaled, product));
    builder.output(scaled);
    builder.output(product);
    builder.output(equal);
    builder.output(sum);
    builder.finish()
}

fn checked(graph: &Graph) -> CheckedGraph {
    let scheme = common::IDS;
    check::check(&scheme, graph.clone()).expect("endpoint checks")
}

fn compiled_layout(graph: &Graph) -> (CheckedGraph, PackedLayout) {
    let scheme = common::IDS;
    let accepted = checked(graph);
    let layout = Compiled::new(&scheme, &accepted).packed_layout();
    (accepted, layout)
}

fn assert_agree(left: &Obs, right: &Obs, context: &str) {
    match (left, right) {
        (Obs::Normal(l), Obs::Normal(r)) => assert_eq!(l, r, "{context}"),
        (Obs::GuardFailed { node: ln, fact: lf }, Obs::GuardFailed { node: rn, fact: rf }) => {
            assert_eq!((ln, lf), (rn, rf), "{context}");
        }
        (l, r) => panic!("{context}: observations differ in kind: {l:?} vs {r:?}"),
    }
}

#[test]
fn an_interleaved_i64_schedule_with_shared_inputs_and_equality_is_validated() {
    let scheme = common::IDS;
    let graph = reordered_i64();
    let (accepted, layout) = compiled_layout(&graph);
    assert_eq!(layout_check(&accepted, &layout), Ok(()), "declared layout");
    assert_eq!(
        layout.slot_regs.len(),
        2,
        "two declared slot registers (both shared inputs)"
    );
    assert_eq!(layout.consts.len(), 1, "one declared exact constant");
    assert_eq!(
        layout.layers.len(),
        layout.fail_suffix_min.len() - 1,
        "one range and one minimum per layer"
    );

    let compiled = Compiled::new(&scheme, &accepted);
    let cases: [[Value; 2]; 5] = [
        [Value::I64(3), Value::I64(4)],
        [Value::I64(-7), Value::I64(9)],
        [Value::I64(0), Value::I64(0)],
        [Value::I64(i64::MAX), Value::I64(1)],
        [Value::I64(i64::MIN), Value::I64(-1)],
    ];
    for inputs in cases {
        let mut control_a = Control::default();
        let mut control_b = Control::default();
        let packed = compiled.evaluate(&inputs, None, &mut control_a).obs;
        let reference = eval::reference(&accepted, &inputs, None, &mut control_b).obs;
        assert_agree(&packed, &reference, "interleaved i64 graph");
    }
}

#[test]
fn a_schedule_that_breaks_the_declaration_is_refused() {
    let graph = reordered_i64();
    let (accepted, layout) = compiled_layout(&graph);

    let mut short = layout.clone();
    short.reg.pop();
    assert_eq!(
        layout_check(&accepted, &short),
        Err(Refusal::RegisterMapShape),
        "missing register"
    );

    let mut wrong_bits = layout.clone();
    if let Some(entry) = wrong_bits.consts.first_mut() {
        entry.1 ^= 1;
    }
    assert_eq!(
        layout_check(&accepted, &wrong_bits),
        Err(Refusal::ConstantTable),
        "wrong constant bits"
    );

    // Swapping whole `(register, ordinal)` pairs is a permutation of a valid table and is accepted;
    // the declaration is wrong only when a register keeps its place while the ordinals swap.
    let mut swapped = layout.clone();
    if swapped.slot_regs.len() == 2 {
        let first = swapped.slot_regs[0].1;
        swapped.slot_regs[0].1 = swapped.slot_regs[1].1;
        swapped.slot_regs[1].1 = first;
    }
    assert_eq!(
        layout_check(&accepted, &swapped),
        Err(Refusal::SlotTable),
        "swapped slot ordinals"
    );

    let mut permuted = layout.clone();
    if permuted.slot_regs.len() == 2 {
        permuted.slot_regs.swap(0, 1);
    }
    assert_eq!(
        layout_check(&accepted, &permuted),
        Ok(()),
        "a pair permutation is still a valid table"
    );

    let mut aliased = layout.clone();
    if aliased.reg.len() >= 2 {
        aliased.reg[1] = aliased.reg[0];
    }
    let aliased_refusal = layout_check(&accepted, &aliased);
    assert!(
        matches!(
            aliased_refusal,
            Err(Refusal::AliasedRegisters) | Err(Refusal::SlotTable)
        ),
        "aliased registers: {aliased_refusal:?}"
    );

    assert_eq!(
        layout_check(&accepted, &layout),
        Ok(()),
        "unmutated declaration"
    );
}

#[test]
fn the_declared_constant_bits_are_the_authored_literal() {
    let graph = reordered_i64();
    let (_, layout) = compiled_layout(&graph);
    assert_eq!(
        layout
            .consts
            .iter()
            .map(|entry| entry.1)
            .collect::<Vec<_>>(),
        vec![(-3i64) as u64],
        "declared constant bits"
    );
}
