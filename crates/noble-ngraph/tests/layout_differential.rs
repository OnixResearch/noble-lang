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
use noble_ngraph::layout::{check as layout_check, PackedLayout, Refusal};
use noble_ngraph::sem::Value;

mod common;

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
    assert_eq!(layout_check(&checked, &layout), Ok(()));
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
    assert_eq!(layout_check(&checked, &layout), Ok(()));
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
        layout_check(&checked, &truncated),
        Err(Refusal::LayerCoverage),
        "the declaration must name exactly the graph's instruction nodes"
    );
    // The real layout is still fine, so the refusal is about the mutation.
    assert_eq!(layout_check(&checked, &layout), Ok(()));
}

/// A checked graph and the compiler's own packed layout for it (issue #68).
fn compiled_layout(builder: GraphBuilder) -> (CheckedGraph, PackedLayout) {
    let scheme = common::IDS;
    let accepted = checked(&builder.finish());
    let layout = Compiled::new(&scheme, &accepted).packed_layout();
    (accepted, layout)
}

/// The compiler's own packed layout for an already-built graph, under the test scheme.
fn compiled_layout_crate(graph: noble_ngraph::ir::Graph) -> (CheckedGraph, PackedLayout) {
    let scheme = common::IDS;
    let accepted = checked(&graph);
    let layout = Compiled::new(&scheme, &accepted).packed_layout();
    (accepted, layout)
}

#[test]
fn a_compiled_parameter_layout_is_measured_by_the_validator() {
    // Issue #68: the compiler puts both slot kinds in the same `slot_regs` table, so the validator
    // must recognise an entry for either kind. This fixture is the compiler's own layout, not a
    // handcrafted one.
    let scheme = common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&scheme, "compiled-parameter-layout"),
    );
    let input = builder.input(ScalarType::F32);
    let weight = builder.param();
    let product = builder.op(noble_ngraph::ir::Op::F32Mul(input, weight));
    builder.output(product);
    let (checked, layout) = compiled_layout(builder);
    assert_eq!(
        layout.slot_regs.len(),
        2,
        "input and parameter share the table"
    );
    assert_eq!(layout_check(&checked, &layout), Ok(()));
}

#[test]
fn several_parameters_and_parameter_only_layouts_are_measured() {
    let scheme = common::IDS;
    let mut two = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&scheme, "compiled-two-parameters"),
    );
    let input = two.input(ScalarType::F32);
    let weight = two.param();
    let bias = two.param();
    let scaled = two.op(noble_ngraph::ir::Op::F32Mul(input, weight));
    let shifted = two.op(noble_ngraph::ir::Op::F32Add(scaled, bias));
    two.output(shifted);
    let (checked, layout) = compiled_layout(two);
    assert_eq!(layout.slot_regs.len(), 3);
    assert_eq!(layout_check(&checked, &layout), Ok(()));

    let mut only = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&scheme, "compiled-parameter-only"),
    );
    let left = only.param();
    let right = only.param();
    let product = only.op(noble_ngraph::ir::Op::F32Mul(left, right));
    only.output(product);
    let (checked, layout) = compiled_layout(only);
    assert_eq!(layout.slot_regs.len(), 2);
    assert_eq!(layout_check(&checked, &layout), Ok(()));

    // The input-only integer control keeps the ordinary behaviour: no parameter needed.
    let integer = crate::checked(&i64_guarded());
    let integer_layout = Compiled::new(&scheme, &integer).packed_layout();
    assert_eq!(layout_check(&integer, &integer_layout), Ok(()));
}

#[test]
fn the_real_training_fixtures_pass_layout_validation() {
    // Issue #68 asks for the existing F32 training graphs, not only a purpose-built fixture. These
    // are the graphs the trainer and the gradient reference actually use; each carries parameter
    // slots created by `GraphBuilder::param()`, and their compiler-produced tables must validate.
    let scheme = common::IDS;
    for guarded in [false, true] {
        let (checked, layout) = compiled_layout_crate(crate::common::linear(guarded, false));
        assert_eq!(
            checked.graph().slots.len(),
            3,
            "one input and two parameter slots"
        );
        assert_eq!(
            layout.slot_regs.len(),
            3,
            "the compiler declares one register per slot of either kind"
        );
        assert_eq!(
            layout_check(&checked, &layout),
            Ok(()),
            "the training model's own packed layout validates (guarded = {guarded})"
        );
        assert!(
            checked
                .graph()
                .slots
                .iter()
                .filter(|slot| slot.kind == noble_ngraph::ir::SlotKind::Param)
                .count()
                == 2,
            "the fixture really carries two parameter slots"
        );
    }
    let (checked, layout) = compiled_layout_crate(crate::common::mse());
    assert_eq!(layout.slot_regs.len(), 2, "two inputs, no parameters");
    assert_eq!(
        layout_check(&checked, &layout),
        Ok(()),
        "the loss graph's layout validates"
    );
    let _ = scheme;
}

/// Mutations of a real parameter table stay refused, so recognising `Param` did not weaken the
/// exact register-to-slot identity or the one-entry-per-slot rule.
#[test]
fn malformed_parameter_layouts_are_refused() {
    let scheme = common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&scheme, "compiled-parameter-mutations"),
    );
    let input = builder.input(ScalarType::F32);
    let weight = builder.param();
    let product = builder.op(noble_ngraph::ir::Op::F32Mul(input, weight));
    builder.output(product);
    let (checked, layout) = compiled_layout(builder);
    assert_eq!(
        layout_check(&checked, &layout),
        Ok(()),
        "the pristine table passes"
    );
    let parameter = *layout
        .slot_regs
        .iter()
        .find(|(_, slot)| {
            checked.graph().slots[*slot as usize].kind == noble_ngraph::ir::SlotKind::Param
        })
        .expect("the fixture has one parameter entry");
    let input_entry = *layout
        .slot_regs
        .iter()
        .find(|(_, slot)| {
            checked.graph().slots[*slot as usize].kind == noble_ngraph::ir::SlotKind::Input
        })
        .expect("the fixture has one input entry");

    // Wrong register for the parameter.
    let mut wrong_register = layout.clone();
    for entry in &mut wrong_register.slot_regs {
        if *entry == parameter {
            entry.0 = u32::MAX;
        }
    }
    assert_eq!(
        layout_check(&checked, &wrong_register),
        Err(Refusal::SlotTable),
        "a register no parameter node carries is refused"
    );

    // Duplicate parameter entry.
    let mut duplicated = layout.clone();
    duplicated.slot_regs.push(parameter);
    assert_eq!(
        layout_check(&checked, &duplicated),
        Err(Refusal::SlotTable),
        "one slot is covered once"
    );

    // Omitted parameter entry.
    let mut omitted = layout.clone();
    omitted.slot_regs.retain(|entry| *entry != parameter);
    assert_eq!(
        layout_check(&checked, &omitted),
        Err(Refusal::SlotTable),
        "every slot node is declared"
    );

    // The parameter entry claiming the input's slot identity.
    let mut swapped = layout.clone();
    for entry in &mut swapped.slot_regs {
        if *entry == parameter {
            entry.1 = input_entry.1;
        }
    }
    assert_eq!(
        layout_check(&checked, &swapped),
        Err(Refusal::SlotTable),
        "slot identities are not interchangeable across kinds"
    );
}
