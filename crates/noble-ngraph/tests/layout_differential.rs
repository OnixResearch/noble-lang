#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    tigerstyle::assertion_density,
    tigerstyle::ambiguous_params,
    tigerstyle::fallible_int_fallback,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; this integration harness builds checked graphs and asserts their layout and execution outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Differential execution of a compiled, layout-validated endpoint (issue #43).
//!
//! The packed layout is validated against the graph, and the same graph is then
//! executed through both the compiled scheduler and the reference interpreter on
//! the same concrete inputs. Agreement is a measured artifact: it says the
//! scheduler's packed order, register map and constant table produce the reference
//! denotation for these inputs, not that any Lean model corresponds to the bytes.

use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{Graph, GraphBuilder, ScalarType, SubProfile, Subject};
use noble_ngraph::layout::Refusal;
use noble_ngraph::sem::Value;

mod common;
#[path = "layout_differential/parameters.rs"]
mod parameters;

fn f32_chain() -> Graph {
    let scheme = common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&scheme, "layout-differential-f32"),
    );
    let input = builder.input(ScalarType::F32);
    let bias = builder.f32_const(0.5);
    let affine = builder.op(noble_ngraph::ir::Op::F32Add(input, bias));
    let relu = builder.op(noble_ngraph::ir::Op::F32Relu(affine));
    builder.output(affine);
    builder.output(relu);
    builder.finish()
}

fn i64_guarded() -> Graph {
    let scheme = common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "layout-differential-i64"),
    );
    let left = builder.input(ScalarType::I64);
    let right = builder.input(ScalarType::I64);
    let sum = builder.op(noble_ngraph::ir::Op::I64Add(left, right));
    builder.guard(noble_ngraph::ir::Fact::Bit, sum);
    builder.output(left);
    builder.output(right);
    builder.output(sum);
    builder.finish()
}

fn checked(graph: &Graph) -> CheckedGraph {
    let scheme = common::IDS;
    check::check(&scheme, graph.clone()).expect("endpoint checks")
}

fn execute(checked: &CheckedGraph, inputs: &[Value]) -> (Obs, Obs) {
    let scheme = common::IDS;
    let compiled = Compiled::new(&scheme, checked);
    let mut control_a = Control::default();
    let mut control_b = Control::default();
    (
        compiled.evaluate(inputs, None, &mut control_a).obs,
        eval::reference(checked, inputs, None, &mut control_b).obs,
    )
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
fn a_validated_f32_endpoint_agrees_with_the_reference_interpreter() {
    let graph = f32_chain();
    let checked = checked(&graph);
    let scheme = common::IDS;
    let layout = Compiled::new(&scheme, &checked).packed_layout();
    assert_eq!(noble_ngraph::layout::check(&checked, &layout), Ok(()));
    let inputs = [
        [Value::F32(0.0f32.to_bits())],
        [Value::F32((-2.5f32).to_bits())],
        [Value::F32(3.25f32.to_bits())],
    ];
    for case in inputs {
        let (compiled, reference) = execute(&checked, &case);
        assert_agree(&compiled, &reference, "f32 chain");
    }
}

#[test]
fn a_validated_guarded_endpoint_agrees_or_fails_the_same_guard() {
    let graph = i64_guarded();
    let checked = checked(&graph);
    let scheme = common::IDS;
    let layout = Compiled::new(&scheme, &checked).packed_layout();
    assert_eq!(noble_ngraph::layout::check(&checked, &layout), Ok(()));
    // One admitting sum and one sum that is not an exact bit.
    for case in [
        [Value::I64(0), Value::I64(1)],
        [Value::I64(2), Value::I64(3)],
    ] {
        let (compiled, reference) = execute(&checked, &case);
        assert_agree(&compiled, &reference, "guarded i64 sum");
    }
    let (compiled, reference) = execute(&checked, &[Value::I64(2), Value::I64(3)]);
    assert!(
        matches!(compiled, Obs::GuardFailed { .. }),
        "the failing guard is the observation on both endpoints: {compiled:?}"
    );
    assert_agree(&compiled, &reference, "failing guard");
}

#[test]
fn a_layout_that_miscounts_instructions_is_refused_before_execution() {
    let graph = i64_guarded();
    let checked = checked(&graph);
    let scheme = common::IDS;
    let compiled = Compiled::new(&scheme, &checked);
    let layout = compiled.packed_layout();
    let mut truncated = layout.clone();
    if let Some(entry) = truncated.layers.last_mut() {
        entry.1 = entry.1.saturating_sub(1);
    }
    assert_eq!(
        noble_ngraph::layout::check(&checked, &truncated),
        Err(Refusal::LayerCoverage),
        "the declaration must name exactly the graph's instruction nodes"
    );
    // The real layout is still fine, so the refusal is about the mutation.
    assert_eq!(noble_ngraph::layout::check(&checked, &layout), Ok(()));
}
