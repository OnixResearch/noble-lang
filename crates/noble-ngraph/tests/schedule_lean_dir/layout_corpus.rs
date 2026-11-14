#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this fixed-fixture exporter stops on graph shapes outside the packed-layout model, checks every Rust refusal before emitting Lean, and writes the generated corpus as text."
)]
//! Real checked layouts and one-obligation mutations exported to the encoded Lean model.

use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::layout::{self, PackedLayout, Refusal};

use super::common;

const HEADER: &str = r#"/-
Generated from accepted Rust CheckedGraph/PackedLayout pairs and refusal-checked single-field
copies by `cargo test -p noble-ngraph --test schedule_correspondence -- --ignored write_packed_layout_lean_file`.
The Lean model checks only its stated finite I64/Bool fragment; this is not a Rust refinement proof.
-/
import NobleFoundation.PackedLayoutEncoded

namespace NobleFoundation.PackedLayoutCorpus
open NobleFoundation.PackedSchedule
open NobleFoundation.PackedLayout
open NobleFoundation.PackedLayoutEncoded

"#;

const CONTROLS: &str = r#"/-! These controls demonstrate why the input encoding and size claim
must retain every field. The metadata extension below is model-only: its extra layer is not
derived by Rust `CheckedGraph` and is deliberately outside the Rust correspondence boundary. -/

def nodesOnlyEncoding (input : Input) : List Bool :=
  encodeList encodeNode input.nodes

theorem nodesOnlyEncoding_is_not_injective :
    ∃ first second, first ≠ second ∧ nodesOnlyEncoding first = nodesOnlyEncoding second := by
  refine ⟨two_constants_real, two_constants_stale_revision, ?_, ?_⟩
  · intro same
    have revisionSame := congrArg (fun input : Input => input.tables.revisionOk) same
    simp [two_constants_real, two_constants_stale_revision] at revisionSame
  · rfl

def finiteFixtureLookup (input : Input) : Bool :=
  [interleaved_real.layerCount, two_constants_real.layerCount].contains input.layerCount

def two_constants_with_trusted_layer_extension : Input :=
  { two_constants_real with
    layerCount := 5,
    tables := { two_constants_real.tables with
      layers := two_constants_real.tables.layers ++ [(5, 5)],
      failSuffixMin := two_constants_real.tables.failSuffixMin ++ [maxRegister] } }

theorem finiteFixtureLookup_misses_model_accepted_extension :
    acceptedInput two_constants_with_trusted_layer_extension = true ∧
      finiteFixtureLookup two_constants_with_trusted_layer_extension = false := by
  decide

theorem node_count_does_not_measure_encoded_input :
    two_constants_real.nodes.length = two_constants_with_trusted_layer_extension.nodes.length ∧
      encodedSize two_constants_real <
        encodedSize two_constants_with_trusted_layer_extension := by
  constructor
  · rfl
  · have hsize :
        encodedSize two_constants_with_trusted_layer_extension =
          encodedSize two_constants_real + 79 := by
      simp [encodedSize, encodeInput, encodeTables, encodeNatPair,
        encodeConstant, encodeRegister, encodeWord, encodeNode, encodeBool, natWidth,
        maxRegister,
        two_constants_with_trusted_layer_extension, two_constants_real]
    omega

"#;

type LayoutMutation = (&'static str, Refusal, PackedLayout);

fn layout_sample() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&common::IDS, "packed-layout-corpus-two-constants"),
    );
    let left = builder.input(ScalarType::I64);
    let right = builder.input(ScalarType::I64);
    let three = builder.i64_const(3);
    let five = builder.i64_const(5);
    let sum = builder.op(Op::I64Add(left, right));
    let difference = builder.op(Op::I64Sub(right, left));
    let scaled = builder.op(Op::I64Mul(sum, three));
    let product = builder.op(Op::I64Mul(difference, five));
    let equal = builder.op(Op::I64Eq(scaled, product));
    builder.output(equal);
    builder.output(sum);
    builder.finish()
}

fn checked_layout(graph: Graph) -> (CheckedGraph, PackedLayout) {
    let checked = check::check(&common::IDS, graph).expect("the corpus graph checks");
    let layout = Compiled::new(&common::IDS, &checked).packed_layout();
    assert_eq!(
        layout::check(&checked, &layout),
        Ok(()),
        "the real layout is accepted"
    );
    (checked, layout)
}

fn mutation_cases(layout: &PackedLayout) -> Vec<LayoutMutation> {
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

fn nat_pairs(items: &[(u32, u32)]) -> String {
    let pairs: Vec<String> = items
        .iter()
        .map(|(left, right)| format!("({left}, {right})"))
        .collect();
    format!("[{}]", pairs.join(", "))
}

fn registers(items: &[u32]) -> String {
    let entries: Vec<String> = items.iter().map(|value| format!("some {value}")).collect();
    format!("[{}]", entries.join(", "))
}

fn constants(items: &[(u32, u64)]) -> String {
    let entries: Vec<String> = items
        .iter()
        .map(|(register, bits)| format!("({register}, BitVec.ofNat 64 {bits})"))
        .collect();
    format!("[{}]", entries.join(", "))
}

fn suffix_minima(items: &[u32]) -> String {
    let entries: Vec<String> = items
        .iter()
        .map(|value| {
            if *value == u32::MAX {
                String::from("NobleFoundation.PackedLayout.maxRegister")
            } else {
                value.to_string()
            }
        })
        .collect();
    format!("[{}]", entries.join(", "))
}

fn input_definition(name: &str, checked: &CheckedGraph, layout: &PackedLayout) -> String {
    let graph = checked.graph();
    let nodes: Vec<String> = graph
        .nodes
        .iter()
        .map(|node| super::emit::node_term(graph, &node.op))
        .collect();
    let layer_of: Vec<u32> = (0..graph.nodes.len())
        .map(|index| {
            checked.layer(noble_ngraph::ir::NodeId(
                u32::try_from(index).expect("node index fits"),
            ))
        })
        .collect();
    let consts: Vec<(u32, u64)> = layout
        .consts
        .iter()
        .map(|(register, bits)| (*register, *bits))
        .collect();
    format!(
        "def {name} : Input := {{\n  nodes := {},\n  layerCount := {},\n  failNodes := [],\n  layerOf := {},\n  tables := {{\n    revisionOk := {},\n    regs := {},\n    registers := {},\n    consts := {},\n    slotRegs := {},\n    layers := {},\n    failSuffixMin := {}\n  }}\n}}\n\n",
        super::emit::term_list(&nodes),
        checked.layer_count(),
        super::emit::nat_list(&layer_of),
        layout.revision == "ngraph-layout/0",
        layout.regs,
        registers(&layout.reg),
        constants(&consts),
        nat_pairs(&layout.slot_regs),
        nat_pairs(&layout.layers),
        suffix_minima(&layout.fail_suffix_min),
    )
}

fn accepted_theorems(name: &str) -> String {
    format!(
        "theorem {name}_encoded_accepted :\n    validateEncoded (encodeInput {name}) = true := by\n  rw [validateEncoded_agrees]\n  decide\n\n"
    )
}

fn refused_theorems(name: &str) -> String {
    format!(
        "theorem {name}_encoded_rejected :\n    validateEncoded (encodeInput {name}) = false := by\n  rw [validateEncoded_agrees]\n  decide\n\n"
    )
}

fn lean_file() -> String {
    let mut text = String::from(HEADER);
    let (interleaved_checked, interleaved_layout) = checked_layout(super::interleaved());
    text.push_str(&input_definition(
        "interleaved_real",
        &interleaved_checked,
        &interleaved_layout,
    ));
    text.push_str(&accepted_theorems("interleaved_real"));

    let (checked, layout) = checked_layout(layout_sample());
    text.push_str(&input_definition("two_constants_real", &checked, &layout));
    text.push_str(&accepted_theorems("two_constants_real"));

    for (suffix, expected, mutated) in mutation_cases(&layout) {
        assert_eq!(
            layout::check(&checked, &mutated),
            Err(expected),
            "the real layout mutation {suffix} refuses at the expected obligation"
        );
        let name = format!("two_constants_{suffix}");
        text.push_str(&input_definition(&name, &checked, &mutated));
        text.push_str(&refused_theorems(&name));
    }
    text.push_str(CONTROLS);
    text.push_str("end NobleFoundation.PackedLayoutCorpus\n");
    text
}

#[test]
fn the_packed_layout_lean_file_is_the_real_emission() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proofs/foundation/NobleFoundation/PackedLayoutCorpus.lean"
    );
    let committed = std::fs::read_to_string(path).expect("committed packed-layout corpus exists");
    assert_eq!(
        lean_file(),
        committed,
        "the emitted layouts changed: regenerate `PackedLayoutCorpus.lean` with `cargo test -p noble-ngraph --test schedule_correspondence -- --ignored write_packed_layout_lean_file` and rerun the foundation lane"
    );
}

#[test]
#[ignore = "writes the generated PackedLayout corpus; run after a deliberate fixture change"]
fn write_packed_layout_lean_file() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proofs/foundation/NobleFoundation/PackedLayoutCorpus.lean"
    );
    std::fs::write(path, lean_file()).expect("generated packed-layout corpus written");
}
