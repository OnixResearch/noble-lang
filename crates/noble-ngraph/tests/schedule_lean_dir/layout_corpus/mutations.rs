use noble_ngraph::layout::{PackedLayout, Refusal};

pub(super) type Case = (&'static str, Refusal, PackedLayout);

pub(super) fn all(layout: &PackedLayout) -> Vec<Case> {
    let mut cases = registers(layout);
    cases.extend(tables(layout));
    cases.extend(layers(layout));
    cases
}

fn registers(layout: &PackedLayout) -> Vec<Case> {
    let mut cases = Vec::new();

    let mut stale_revision = layout.clone();
    stale_revision.revision = String::from("ngraph-layout/stale");
    cases.push(("stale_revision", Refusal::StaleRevision, stale_revision));

    let mut short_register_map = layout.clone();
    short_register_map.reg.pop();
    cases.push((
        "register_map_shape",
        Refusal::RegisterMapShape,
        short_register_map,
    ));

    let mut out_of_range = layout.clone();
    out_of_range.reg[0] = layout.regs;
    cases.push((
        "register_out_of_range",
        Refusal::RegisterOutOfRange,
        out_of_range,
    ));

    let mut aliased = layout.clone();
    aliased.reg[1] = layout.reg[0];
    cases.push(("aliased_registers", Refusal::AliasedRegisters, aliased));

    let mut wrong_count = layout.clone();
    wrong_count.regs += 1;
    cases.push(("register_count", Refusal::RegisterCount, wrong_count));

    let mut reserved_one_held = layout.clone();
    let last_node = reserved_one_held.reg.len() - 1;
    reserved_one_held.reg[last_node] =
        u32::try_from(reserved_one_held.reg.len()).expect("reserved register fits");
    cases.push((
        "one_register_held",
        Refusal::OneRegisterHeld,
        reserved_one_held,
    ));

    cases
}

fn tables(layout: &PackedLayout) -> Vec<Case> {
    let mut cases = Vec::new();

    let mut wrong_constant = layout.clone();
    wrong_constant.consts[0].1 = wrong_constant.consts[0].1.wrapping_add(1);
    cases.push(("constant_entry", Refusal::ConstantTable, wrong_constant));

    let mut missing_constant = layout.clone();
    missing_constant.consts.pop();
    cases.push(("constant_count", Refusal::ConstantTable, missing_constant));

    let mut repeated_constant = layout.clone();
    repeated_constant.consts[1] = repeated_constant.consts[0];
    cases.push((
        "constant_duplicate",
        Refusal::ConstantTable,
        repeated_constant,
    ));

    let mut wrong_slot = layout.clone();
    wrong_slot.slot_regs[0].1 = 99;
    cases.push(("slot_entry", Refusal::SlotTable, wrong_slot));

    let mut missing_slot = layout.clone();
    missing_slot.slot_regs.pop();
    cases.push(("slot_count", Refusal::SlotTable, missing_slot));

    let mut repeated_slot = layout.clone();
    repeated_slot.slot_regs[1] = repeated_slot.slot_regs[0];
    cases.push(("slot_duplicate", Refusal::SlotTable, repeated_slot));

    cases
}

fn layers(layout: &PackedLayout) -> Vec<Case> {
    let mut cases = Vec::new();

    let mut extra_layer = layout.clone();
    let last = *extra_layer.layers.last().expect("the graph has layers");
    extra_layer.layers.push((last.1, last.1));
    cases.push(("layer_shape", Refusal::LayerShape, extra_layer));

    let mut unordered_layer = layout.clone();
    unordered_layer.layers[0] = (1, 0);
    cases.push(("layer_order", Refusal::LayerOrder, unordered_layer));

    let mut wrong_suffix = layout.clone();
    wrong_suffix.fail_suffix_min[0] = 0;
    cases.push(("failure_suffix", Refusal::FailSuffixMismatch, wrong_suffix));

    let mut wrong_coverage = layout.clone();
    wrong_coverage.layers[1].1 += 1;
    wrong_coverage.layers[2].0 += 1;
    cases.push(("layer_coverage", Refusal::LayerCoverage, wrong_coverage));

    cases
}
