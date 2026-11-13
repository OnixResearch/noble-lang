use noble_ngraph::check::CheckedGraph;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{Fact, Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::layout::PackedLayout;
use noble_ngraph::schedule::I64Instruction;
use noble_ngraph::sem::Value;

use super::corpus_shared;
use super::emit::{artifact_text, GuardExecution, Mutation};
use super::emit_guard::GUARDED_HEADER;

/// A non-layer-sorted I64 graph whose canonical-first cube guard is in a later layer than the sum guard.
pub(crate) fn graph() -> Graph {
    let scheme = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "schedule-corpus-non-layer-sorted-guarded"),
    );
    let x = builder.input(ScalarType::I64);
    let square = builder.op(Op::I64Mul(x, x));
    let cube = builder.op(Op::I64Mul(square, x));
    builder.guard(Fact::Bit, cube);
    let y = builder.input(ScalarType::I64);
    let five = builder.i64_const(5);
    let sum = builder.op(Op::I64Add(y, five));
    builder.guard(Fact::Bit, sum);
    let difference = builder.op(Op::I64Sub(cube, sum));
    builder.output(difference);
    builder.output(sum);
    builder.output(difference);
    builder.output(cube);
    builder.finish()
}

const RUNS: [[i64; 2]; 7] = [
    [0, -5],
    [1, -5],
    [0, -4],
    [2, -5],
    [0, 0],
    [2, 0],
    [i64::MAX, -4],
];

/// Guarded packed observations checked against the reference interpreter before Lean emission.
pub(crate) fn executions(checked: &CheckedGraph) -> Vec<GuardExecution> {
    let compiled = noble_ngraph::compiled::Compiled::new(&super::common::IDS, checked);
    RUNS.iter()
        .map(|run| {
            let inputs: Vec<Value> = run.iter().map(|&value| Value::I64(value)).collect();
            let packed = compiled
                .evaluate(&inputs, None, &mut Control::default())
                .obs;
            let reference = eval::reference(checked, &inputs, None, &mut Control::default()).obs;
            assert_eq!(
                packed, reference,
                "guarded packed execution agrees with the reference interpreter"
            );
            let observation = match packed {
                Obs::Normal(values) => {
                    let outputs: Vec<String> = values.iter().map(super::emit::value_term).collect();
                    format!(
                        "NobleFoundation.PackedScheduleGuard.Obs.normal [{}]",
                        outputs.join(", ")
                    )
                }
                Obs::GuardFailed {
                    node,
                    fact: Fact::Bit,
                } => format!("NobleFoundation.PackedScheduleGuard.Obs.failed {}", node.0),
                other => panic!("unsupported guarded observation: {other:?}"),
            };
            (run.to_vec(), observation)
        })
        .collect()
}

/// Mutated copies of a guarded artifact that violate its exact guard obligations.
pub(crate) fn mutations(
    checked: &CheckedGraph,
    layout: &PackedLayout,
    stream: &[I64Instruction],
    instrs: &[String],
    out_regs: &[u32],
) -> Vec<Mutation> {
    let restream =
        |changed: Vec<I64Instruction>| corpus_shared::exported_stream(checked, layout, &changed);
    let guard_positions: Vec<usize> = stream
        .iter()
        .enumerate()
        .filter_map(|(index, instruction)| {
            matches!(instruction, I64Instruction::Guard { .. }).then_some(index)
        })
        .collect();
    let mut refused = Vec::new();
    if guard_positions.len() >= 2 {
        let mut reordered = stream.to_vec();
        reordered.swap(guard_positions[0], guard_positions[1]);
        refused.push((
            "reordered_guards",
            "the two guard bodies are exchanged, changing the first failing guard in layer order",
            restream(reordered),
            out_regs.to_vec(),
        ));
    }
    if let Some(position) = instrs
        .iter()
        .position(|instruction| instruction.starts_with(".guard "))
    {
        let mut missing_guard = instrs.to_vec();
        missing_guard.remove(position);
        refused.push((
            "missing_guard",
            "one required guard instruction is missing",
            missing_guard,
            out_regs.to_vec(),
        ));
    }
    if let Some(position) = guard_positions.first().copied() {
        if stream
            .last()
            .is_some_and(|instruction| matches!(instruction, I64Instruction::Sub(..)))
        {
            let mut after_use = stream.to_vec();
            let guard = after_use.remove(position);
            after_use.push(guard);
            refused.push((
                "guard_after_use",
                "the sum guard is moved after the output subtraction that uses its value",
                restream(after_use),
                out_regs.to_vec(),
            ));
        }
    }
    refused
}

fn lean_file() -> String {
    let artifact = super::corpus::export(
        "nonLayerSortedGuarded",
        "a two-guard, non-layer-sorted I64 graph with a canonical-first guard in a later layer",
        graph(),
        Some(vec![2, 0]),
    );
    let mut text = String::from(GUARDED_HEADER);
    text.push_str(&artifact_text(&artifact));
    text.push_str("\nend NobleFoundation.PackedScheduleCorpus\n");
    text
}

pub(crate) fn write_lean_file() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proofs/foundation/NobleFoundation/PackedScheduleGuardCorpus.lean"
    );
    std::fs::write(path, lean_file()).expect("generated guarded file written");
}

#[test]
fn the_guarded_lean_file_is_the_real_emission() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proofs/foundation/NobleFoundation/PackedScheduleGuardCorpus.lean"
    );
    let committed = std::fs::read_to_string(path).expect("committed guarded Lean corpus exists");
    assert_eq!(
        lean_file(),
        committed,
        "the guarded artifact changed: regenerate both corpora with `cargo test -p noble-ngraph --test schedule_correspondence -- --ignored write_the_lean_file` and rerun the foundation lane"
    );
}
