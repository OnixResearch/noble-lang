//! Parameter-slot layout scenarios for the differential layout harness.

use super::{checked, common, i64_guarded};
use noble_ngraph::check::CheckedGraph;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::ir::{GraphBuilder, ScalarType, SubProfile, Subject};
use noble_ngraph::layout::{PackedLayout, Refusal};

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
    assert_eq!(noble_ngraph::layout::check(&checked, &layout), Ok(()));
}

#[test]
fn several_slots_and_input_free_layouts_are_measured() {
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
    assert_eq!(noble_ngraph::layout::check(&checked, &layout), Ok(()));

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
    assert_eq!(noble_ngraph::layout::check(&checked, &layout), Ok(()));

    // The input-only integer control keeps the ordinary behaviour: no parameter needed.
    let integer = super::checked(&i64_guarded());
    let integer_layout = Compiled::new(&scheme, &integer).packed_layout();
    assert_eq!(
        noble_ngraph::layout::check(&integer, &integer_layout),
        Ok(())
    );
}

#[test]
fn the_real_training_fixtures_pass_layout_validation() {
    // Issue #68 asks for the existing F32 training graphs, not only a purpose-built fixture. These
    // are the graphs the trainer and the gradient reference actually use; each carries parameter
    // slots created by `GraphBuilder::param()`, and their compiler-produced tables must validate.
    for guarded in [false, true] {
        let (checked, layout) = compiled_layout_crate(common::linear(guarded, false));
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
            noble_ngraph::layout::check(&checked, &layout),
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
    let (checked, layout) = compiled_layout_crate(common::mse());
    assert_eq!(layout.slot_regs.len(), 2, "two inputs, no parameters");
    assert_eq!(
        noble_ngraph::layout::check(&checked, &layout),
        Ok(()),
        "the loss graph's layout validates"
    );
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
        noble_ngraph::layout::check(&checked, &layout),
        Ok(()),
        "the pristine table passes"
    );
    let parameter = *layout
        .slot_regs
        .iter()
        .find(|(_, slot)| {
            let index = usize::try_from(*slot).expect("fixture slot index fits usize");
            checked.graph().slots[index].kind == noble_ngraph::ir::SlotKind::Param
        })
        .expect("the fixture has one parameter entry");
    let input_entry = *layout
        .slot_regs
        .iter()
        .find(|(_, slot)| {
            let index = usize::try_from(*slot).expect("fixture slot index fits usize");
            checked.graph().slots[index].kind == noble_ngraph::ir::SlotKind::Input
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
        noble_ngraph::layout::check(&checked, &wrong_register),
        Err(Refusal::SlotTable),
        "a register no parameter node carries is refused"
    );

    // Duplicate parameter entry.
    let mut duplicated = layout.clone();
    duplicated.slot_regs.push(parameter);
    assert_eq!(
        noble_ngraph::layout::check(&checked, &duplicated),
        Err(Refusal::SlotTable),
        "one slot is covered once"
    );

    // Omitted parameter entry.
    let mut omitted = layout.clone();
    omitted.slot_regs.retain(|entry| *entry != parameter);
    assert_eq!(
        noble_ngraph::layout::check(&checked, &omitted),
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
        noble_ngraph::layout::check(&checked, &swapped),
        Err(Refusal::SlotTable),
        "slot identities are not interchangeable across kinds"
    );
}
