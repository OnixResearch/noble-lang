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
use noble_ngraph::layout::{self, PackedLayout};

use super::super::{common, emit};

#[path = "layout_corpus/controls.rs"]
mod controls;
#[path = "layout_corpus/mutations.rs"]
mod mutations;

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
        .map(|node| emit::node_term(graph, &node.op))
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
        emit::term_list(&nodes),
        checked.layer_count(),
        emit::nat_list(&layer_of),
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
    let (interleaved_checked, interleaved_layout) = checked_layout(super::super::interleaved());
    text.push_str(&input_definition(
        "interleaved_real",
        &interleaved_checked,
        &interleaved_layout,
    ));
    text.push_str(&accepted_theorems("interleaved_real"));

    let (checked, layout) = checked_layout(layout_sample());
    text.push_str(&input_definition("two_constants_real", &checked, &layout));
    text.push_str(&accepted_theorems("two_constants_real"));

    for (suffix, expected, mutated) in mutations::all(&layout) {
        assert_eq!(
            layout::check(&checked, &mutated),
            Err(expected),
            "the real layout mutation {suffix} refuses at the expected obligation"
        );
        let name = format!("two_constants_{suffix}");
        text.push_str(&input_definition(&name, &checked, &mutated));
        text.push_str(&refused_theorems(&name));
    }
    text.push_str(controls::LEAN_FRAGMENT);
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
