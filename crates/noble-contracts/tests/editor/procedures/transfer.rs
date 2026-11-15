#![allow(
    tigerstyle::assertion_density,
    tigerstyle::function_length,
    tigerstyle::ambient_clock,
    tigerstyle::contradictory_time,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; this frozen transfer harness keeps its paired multi-arm acceptance trace in one test module, uses a deliberately closed task taxonomy, and measures retrieval/build overhead with monotonic time only"
)]

use super::super::procedures::support::{envelope, limits, provenance, revision};
use noble_contracts::source::chunks::{
    self, Binding, Budget, DefinitionRef, Fixture, FixtureSet, Instantiation, Node, Request,
    Response, Scalar, Task,
};
use noble_contracts::source::editor::{Candidate, Node as EditorNode};
use noble_contracts::source::procedures::{
    ArmMetrics, ComposeContext, EvidenceClass, EvidenceOutcome, EvidenceRecord, Metrics,
    ProcedureLibrary, ProcedureRecord, Producer, Provenance, Query, QueryBudget,
};
use noble_contracts::source::Session;
use noble_kernel::types::{EffSet, Ty};
use std::string::{String, ToString};
use std::time::Instant;
use std::vec;
use std::vec::Vec;

const SEEDS: [u64; 4] = [1, 7, 23, 101];
const HELD_OUT_INPUTS: [[[i64; 3]; 3]; 4] = [
    [[1, 2, 3], [5, -5, 7], [-12, 23, -8]],
    [[17, -4, 9], [9, 13, 21], [-4, 5, -11]],
    [[-3, 8, 4], [31, 7, -19], [6, -14, 2]],
    [[0, 0, 0], [19, -23, 41], [-55, 13, 9]],
];
const SOURCE_TASKS: [SourceTask; 4] = [
    SourceTask::Commuted,
    SourceTask::Subtract,
    SourceTask::Scaled,
    SourceTask::Product,
];
const HELD_OUT_TASKS: [HeldOutTask; 2] = [
    HeldOutTask {
        name: "commuted_then_subtract",
        kind: TargetKind::CommutedThenSubtract,
    },
    HeldOutTask {
        name: "subtract_then_commuted",
        kind: TargetKind::SubtractThenCommuted,
    },
];
const CONTROL_POOL: [SourceTask; 2] = [SourceTask::Product, SourceTask::Scaled];
const TRANSFER_ARMS: [&str; 4] = [
    "no_library",
    "frozen_hand_library",
    "accumulated_learned_library",
    "size_matched_nontransfer",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceTask {
    Commuted,
    Subtract,
    Scaled,
    Product,
}

impl SourceTask {
    const fn name(self) -> &'static str {
        match self {
            Self::Commuted => "arithCommuted",
            Self::Subtract => "arithSubtract",
            Self::Scaled => "arithScaled",
            Self::Product => "arithProduct",
        }
    }

    const fn source(self) -> &'static [u8] {
        match self {
            Self::Commuted => b"7 + + ",
            Self::Subtract => b"- 7 + ",
            Self::Scaled => b"7 + 5 * + ",
            Self::Product => b"5 + dup 2 - * + ",
        }
    }

    fn nodes(self) -> Vec<Node> {
        match self {
            Self::Commuted => vec![
                Node::I64(7),
                Node::Operation {
                    op: chunks::Op::Add,
                    instantiation: 0,
                },
                Node::Operation {
                    op: chunks::Op::Add,
                    instantiation: 0,
                },
            ],
            Self::Subtract => vec![
                Node::Operation {
                    op: chunks::Op::Sub,
                    instantiation: 0,
                },
                Node::I64(7),
                Node::Operation {
                    op: chunks::Op::Add,
                    instantiation: 0,
                },
            ],
            Self::Scaled => vec![
                Node::I64(7),
                Node::Operation {
                    op: chunks::Op::Add,
                    instantiation: 0,
                },
                Node::I64(5),
                Node::Operation {
                    op: chunks::Op::Mul,
                    instantiation: 0,
                },
                Node::Operation {
                    op: chunks::Op::Add,
                    instantiation: 0,
                },
            ],
            Self::Product => vec![
                Node::I64(5),
                Node::Operation {
                    op: chunks::Op::Add,
                    instantiation: 0,
                },
                Node::Operation {
                    op: chunks::Op::Dup,
                    instantiation: 1,
                },
                Node::I64(2),
                Node::Operation {
                    op: chunks::Op::Sub,
                    instantiation: 0,
                },
                Node::Operation {
                    op: chunks::Op::Mul,
                    instantiation: 0,
                },
                Node::Operation {
                    op: chunks::Op::Add,
                    instantiation: 0,
                },
            ],
        }
    }

    const fn evidence(self) -> (EvidenceClass, EvidenceOutcome, &'static str) {
        match self {
            Self::Commuted => (
                EvidenceClass::Lean,
                EvidenceOutcome::Proved,
                "NobleFoundation.Synthesis.arithCommuted_certified",
            ),
            Self::Subtract => (
                EvidenceClass::Lean,
                EvidenceOutcome::Proved,
                "NobleFoundation.Synthesis.arithSubtract_certified",
            ),
            Self::Scaled => (
                EvidenceClass::Lean,
                EvidenceOutcome::Proved,
                "NobleFoundation.Synthesis.arithScaled_certified",
            ),
            Self::Product => (
                EvidenceClass::Test,
                EvidenceOutcome::Passed,
                "producer status unproven; held-out producer examples passed",
            ),
        }
    }
}

#[derive(Clone, Copy)]
enum TargetKind {
    CommutedThenSubtract,
    SubtractThenCommuted,
}

#[derive(Clone, Copy)]
struct HeldOutTask {
    name: &'static str,
    kind: TargetKind,
}

#[derive(Clone, Copy)]
struct RetainedSource {
    task: SourceTask,
    id: [u8; 32],
}

struct ControlSelection {
    tasks: [SourceTask; 2],
    target_bytes: usize,
    candidate_bytes: usize,
    within_ten_percent: bool,
}

type CaseOutcome = (Option<i64>, i64, bool);
type FixtureOutcome = (&'static str, u32, Vec<CaseOutcome>);

#[derive(Default)]
struct Counts {
    retrieval_queries: u32,
    retrieval_examined: u32,
    max_examined: u32,
    retrieval_ns: u64,
    heldout_tasks: u32,
    transfer_passed: u32,
    heldout_examples: u32,
    heldout_examples_passed: u32,
    earlier_tasks: u32,
    regression_failed: u32,
}

struct ReportInput<'a> {
    namespace: &'a Session,
    arm: &'a str,
    seed: u64,
    library: &'a ProcedureLibrary,
    sources: &'a [RetainedSource],
    build_ns: u64,
    sweeps: &'a [(String, Counts)],
    control: &'a ControlSelection,
}

struct SweepInput<'a> {
    namespace: &'a Session,
    arm: &'a str,
    seed_index: usize,
    library: &'a ProcedureLibrary,
    sources: &'a [RetainedSource],
    label: &'a str,
    control: &'a ControlSelection,
}

struct TaskReport<'a> {
    task: &'a str,
    order: [SourceTask; 2],
    outcome: &'a str,
    passed: u32,
    cases: &'a [CaseOutcome],
    references: &'a [([i64; 3], i64)],
}

const TRANSFER_ROW_CAPACITY: usize = 16;

#[path = "transfer/fixtures.rs"]
mod fixtures;
#[path = "transfer/producer.rs"]
mod producer;
#[path = "transfer/report.rs"]
mod report;
#[path = "transfer/runner.rs"]
mod runner;
#[path = "transfer/selection.rs"]
mod selection;
#[path = "transfer/sweep.rs"]
mod sweep;
#[path = "transfer/tests.rs"]
mod tests;
