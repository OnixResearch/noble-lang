#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; this exporter writes fixed fixtures as Lean text and must stop at anything outside the exported slice, so expect/panic on unsupported input and owner-path imports are the readable form."
)]
//! The real packed I64/Bool artifacts exported into the Lean model vocabulary (issue #43).

use noble_ngraph::check::CheckedGraph;
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::layout::PackedLayout;
use noble_ngraph::schedule::I64Instruction;

use super::corpus_shared;
use super::emit::{Artifact, HEADER};

/// An input created after an addition it does not feed, moved ahead of it in packed order.
fn late_input() -> Graph {
    let scheme = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "schedule-corpus-late-input"),
    );
    let first = builder.input(ScalarType::I64);
    let five = builder.i64_const(5);
    let sum = builder.op(Op::I64Add(first, five));
    let second = builder.input(ScalarType::I64);
    let equal = builder.op(Op::I64Eq(sum, second));
    builder.output(equal);
    builder.output(equal);
    builder.finish()
}

/// Export one graph and the exact schedule, declaration and observation the packed evaluator uses.
pub(crate) fn export(
    name: &'static str,
    doc: &'static str,
    graph: Graph,
    layer_order_witness: Option<Vec<i64>>,
) -> Artifact {
    let (checked, layout, stream) = super::packed(graph);
    artifact_from_checked(ArtifactInput {
        name,
        doc,
        checked: &checked,
        layout: &layout,
        stream: &stream,
        layer_order_witness,
    })
}

struct ArtifactInput<'a> {
    name: &'static str,
    doc: &'static str,
    checked: &'a CheckedGraph,
    layout: &'a PackedLayout,
    stream: &'a [I64Instruction],
    layer_order_witness: Option<Vec<i64>>,
}

struct GraphTerms {
    nodes: Vec<String>,
    outputs: Vec<u32>,
    out_regs: Vec<u32>,
    layers: Vec<Vec<u32>>,
    layer_guard_order: Vec<u32>,
}

fn graph_terms(checked: &CheckedGraph, layout: &PackedLayout) -> GraphTerms {
    let nodes = checked
        .graph()
        .nodes
        .iter()
        .map(|node| super::emit::node_term(checked.graph(), &node.op))
        .collect();
    let outputs: Vec<u32> = checked
        .graph()
        .outputs
        .iter()
        .map(|output| output.0)
        .collect();
    let out_regs: Vec<u32> = outputs
        .iter()
        .map(|&output| layout.reg[corpus_shared::at(output)])
        .collect();
    let layers = corpus_shared::packed_layers(checked);
    let layer_guard_order = corpus_shared::layer_guard_order(checked, &layers);
    GraphTerms {
        nodes,
        outputs,
        out_regs,
        layers,
        layer_guard_order,
    }
}

fn artifact_from_checked(input: ArtifactInput<'_>) -> Artifact {
    let ArtifactInput {
        name,
        doc,
        checked,
        layout,
        stream,
        layer_order_witness,
    } = input;
    let instrs = corpus_shared::exported_stream(checked, layout, stream);
    let graph = graph_terms(checked, layout);
    let is_guarded = layer_order_witness.is_some();
    if is_guarded {
        assert!(
            graph.layer_guard_order.len() >= 2,
            "the guarded artifact has both guard nodes"
        );
    } else {
        assert!(
            graph.layer_guard_order.is_empty(),
            "unguarded artifacts have no guard nodes"
        );
    }
    let mut mutations =
        corpus_shared::base_mutations(checked, layout, stream, &instrs, &graph.out_regs);
    if is_guarded {
        mutations.extend(super::corpus_guard::mutations(
            checked,
            layout,
            stream,
            &instrs,
            &graph.out_regs,
        ));
    }
    Artifact {
        name,
        doc,
        nodes: graph.nodes,
        regs: layout.reg.clone(),
        order: corpus_shared::packed_order(checked),
        layers: graph.layers,
        suffix_min: layout
            .fail_suffix_min
            .iter()
            .map(|&index| (index != u32::MAX).then_some(index))
            .collect(),
        layer_guard_order: graph.layer_guard_order,
        layer_order_witness,
        instrs,
        outputs: graph.outputs,
        out_regs: graph.out_regs,
        mutations,
        executions: if is_guarded {
            Vec::new()
        } else {
            corpus_shared::executions(checked)
        },
        is_guarded,
        guarded_executions: if is_guarded {
            super::corpus_guard::executions(checked)
        } else {
            Vec::new()
        },
    }
}

fn lean_file() -> String {
    let artifacts = [
        export(
            "interleaved",
            "two branches sharing both inputs, a constant, a Bool equality and two outputs",
            super::interleaved(),
            None,
        ),
        export(
            "lateInput",
            "an input created after an addition it does not feed, moved ahead of it in packed order, and a repeated output",
            late_input(),
            None,
        ),
    ];
    let mut text = String::from(HEADER);
    for artifact in &artifacts {
        text.push_str(&super::emit::artifact_text(artifact));
        text.push('\n');
    }
    text.push_str("end NobleFoundation.PackedScheduleCorpus\n");
    text
}

#[test]
fn the_lean_file_is_the_real_emission() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proofs/foundation/NobleFoundation/PackedScheduleCorpus.lean"
    );
    let committed = std::fs::read_to_string(path).expect("committed Lean corpus exists");
    assert_eq!(
        lean_file(),
        committed,
        "the emitted artifacts changed: regenerate both corpus files with `cargo test -p noble-ngraph --test schedule_correspondence -- --ignored write_the_lean_file` and rerun the foundation lane"
    );
}

#[test]
#[ignore = "writes the generated PackedSchedule corpus files; run after a deliberate emission change"]
fn write_the_lean_file() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proofs/foundation/NobleFoundation/PackedScheduleCorpus.lean"
    );
    std::fs::write(path, lean_file()).expect("generated base corpus written");
    super::corpus_guard::write_lean_file();
}
