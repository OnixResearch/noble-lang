#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this exporter writes small fixed fixtures as Lean text and must stop at anything outside the exported slice, so expect/panic on unsupported input, owner-path imports and string building are the readable form."
)]
//! The real packed I64/Bool artifacts exported into the vocabulary of the Lean model
//! `NobleFoundation.PackedSchedule` (issue #43). `PackedScheduleCorpus.lean` is generated from them;
//! its header states what the kernel checks there and which translation is trusted. Regenerate after
//! a deliberate emission change with `cargo test -p noble-ngraph --test schedule_correspondence --
//! --ignored write_the_lean_file`, then rerun the foundation lane.

use noble_ngraph::check::CheckedGraph;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{Graph, GraphBuilder, NodeId, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::layout::PackedLayout;
use noble_ngraph::schedule::I64Instruction;
use noble_ngraph::sem::Value;

use super::emit::{
    artifact_text, hex, instruction_term, node_term, value_term, Artifact, Execution, Mutation,
    HEADER,
};

/// The generated file the foundation lane kernel-checks.
const COMMITTED: &str =
    include_str!("../../../../proofs/foundation/NobleFoundation/PackedScheduleCorpus.lean");

/// An input created after an addition it does not feed: packed order moves it ahead of the
/// addition, and the equality's result is requested twice.
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

/// A `u32` index into a slice.
fn at(index: u32) -> usize {
    usize::try_from(index).expect("a u32 index fits usize")
}

/// The exporter's claim: nodes in packed order, by layer and then canonical index.
fn packed_order(checked: &CheckedGraph) -> Vec<u32> {
    let count = u32::try_from(checked.graph().nodes.len()).expect("node count fits");
    let mut order: Vec<u32> = (0..count).collect();
    order.sort_by_key(|&index| (checked.layer(NodeId(index)), index));
    order
}

/// One preload (slot or constant table entry) or one stream instruction per node, in packed order.
fn exported_stream(
    checked: &CheckedGraph,
    layout: &PackedLayout,
    stream: &[I64Instruction],
) -> Vec<String> {
    let mut computed = stream.iter();
    let mut terms = Vec::new();
    for index in packed_order(checked) {
        let register = layout.reg[at(index)];
        // The preload tables, not the graph, decide which registers are loaded before the first
        // layer; every other node takes the next stream instruction.
        let slot = layout.slot_regs.iter().find(|(reg, _)| *reg == register);
        let constant = layout.consts.iter().find(|(reg, _)| *reg == register);
        terms.push(match (slot, constant) {
            (Some((_, slot)), None) => format!(".setInput {register} {slot}"),
            (None, Some((_, bits))) => format!(".setConst {register} {}", hex(*bits)),
            (None, None) => {
                instruction_term(computed.next().expect("one instruction per computed node"))
            }
            (Some(_), Some(_)) => panic!("register {register} is in both preload tables"),
        });
    }
    assert!(
        computed.next().is_none(),
        "no instruction is left without a node"
    );
    terms
}

/// Boundary inputs every artifact runs on, one I64 per input slot, including wrap-around.
const RUNS: [[i64; 2]; 5] = [
    [3, 8],
    [0, 0],
    [-1, 1],
    [i64::MAX, i64::MIN + 4],
    [i64::MIN, -1],
];

/// The real packed machine's outputs at each boundary input, checked against the reference
/// interpreter before they are written down.
fn executions(checked: &CheckedGraph) -> Vec<Execution> {
    let compiled = Compiled::new(&super::common::IDS, checked);
    RUNS.iter()
        .map(|run| {
            let inputs: Vec<Value> = run.iter().map(|&value| Value::I64(value)).collect();
            let packed = compiled
                .evaluate(&inputs, None, &mut Control::default())
                .obs;
            let reference = eval::reference(checked, &inputs, None, &mut Control::default()).obs;
            assert_eq!(
                packed, reference,
                "packed execution agrees with the reference interpreter"
            );
            let Obs::Normal(values) = packed else {
                panic!("a normal observation")
            };
            (run.to_vec(), values.iter().map(value_term).collect())
        })
        .collect()
}

fn export(name: &'static str, doc: &'static str, graph: Graph) -> Artifact {
    let (checked, layout, stream) = super::packed(graph);
    let instrs = exported_stream(&checked, &layout, &stream);
    let nodes = checked
        .graph()
        .nodes
        .iter()
        .map(|node| node_term(checked.graph(), &node.op))
        .collect();
    let outputs: Vec<u32> = checked
        .graph()
        .outputs
        .iter()
        .map(|output| output.0)
        .collect();
    let out_regs: Vec<u32> = outputs
        .iter()
        .map(|&output| layout.reg[at(output)])
        .collect();
    let mutations = mutations(&checked, &layout, &stream, &instrs, &out_regs);
    Artifact {
        name,
        doc,
        nodes,
        regs: layout.reg.clone(),
        order: packed_order(&checked),
        instrs,
        outputs,
        out_regs,
        mutations,
        executions: executions(&checked),
    }
}

/// Mutated copies of the real artifact, each one changed obligation.
fn mutations(
    checked: &CheckedGraph,
    layout: &PackedLayout,
    stream: &[I64Instruction],
    instrs: &[String],
    out_regs: &[u32],
) -> Vec<Mutation> {
    let restream = |changed: Vec<I64Instruction>| exported_stream(checked, layout, &changed);
    let mut refused = Vec::new();
    if let Some(index) = stream
        .iter()
        .position(|i| matches!(i, I64Instruction::Add(..)))
    {
        let mut changed = stream.to_vec();
        if let I64Instruction::Add(d, a, b) = changed[index] {
            changed[index] = I64Instruction::Sub(d, a, b);
        }
        refused.push((
            "wrong_operation",
            "the first addition emitted as a subtraction",
            restream(changed),
            out_regs.to_vec(),
        ));
    }
    if let Some(index) = stream
        .iter()
        .position(|i| matches!(i, I64Instruction::Sub(_, a, b) if a != b))
    {
        let mut changed = stream.to_vec();
        if let I64Instruction::Sub(d, a, b) = changed[index] {
            changed[index] = I64Instruction::Sub(d, b, a);
        }
        refused.push((
            "swapped_operands",
            "the first subtraction's operands swapped",
            restream(changed),
            out_regs.to_vec(),
        ));
    }
    if let Some(&(start, _)) = layout.layers.iter().find(|(start, end)| end - start >= 2) {
        let mut changed = stream.to_vec();
        changed.swap(at(start), at(start) + 1);
        refused.push((
            "reordered_within_layer",
            "two instructions of one layer swapped",
            restream(changed),
            out_regs.to_vec(),
        ));
    }
    let mut dropped = instrs.to_vec();
    dropped.pop();
    refused.push((
        "dropped_instruction",
        "the last instruction dropped",
        dropped,
        out_regs.to_vec(),
    ));
    if out_regs.len() >= 2 && out_regs[0] != out_regs[1] {
        let mut swapped = out_regs.to_vec();
        swapped.swap(0, 1);
        refused.push((
            "swapped_outputs",
            "the first two output registers swapped",
            instrs.to_vec(),
            swapped,
        ));
    }
    refused
}

/// The whole generated Lean module.
fn lean_file() -> String {
    let artifacts = [
        export(
            "interleaved",
            "two branches sharing both inputs, a constant, a Bool equality and two outputs",
            super::interleaved(),
        ),
        export(
            "lateInput",
            "an input created after an addition it does not feed, moved ahead of it in packed order, and a repeated output",
            late_input(),
        ),
    ];
    let mut text = String::from(HEADER);
    for artifact in &artifacts {
        text.push_str(&artifact_text(artifact));
        text.push('\n');
    }
    text.push_str("end NobleFoundation.PackedScheduleCorpus\n");
    text
}

#[test]
fn the_lean_file_is_the_real_emission() {
    assert!(
        lean_file() == COMMITTED,
        "the emitted artifacts changed: regenerate PackedScheduleCorpus.lean with `cargo test -p noble-ngraph --test schedule_correspondence -- --ignored write_the_lean_file` and rerun the foundation lane"
    );
}

#[test]
#[ignore = "writes proofs/foundation/NobleFoundation/PackedScheduleCorpus.lean; run after a deliberate emission change"]
fn write_the_lean_file() {
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proofs/foundation/NobleFoundation/PackedScheduleCorpus.lean"
        ),
        lean_file(),
    )
    .expect("generated file written");
}
