#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the test-only model modules import their sibling relation, identity and fixture types directly so the bounded model stays readable"
)]
use super::model::{program, MergeResult, Op, Program};
pub(super) const MAX: u64 = u64::MAX;

pub(super) fn inputs() -> Vec<Vec<u64>> {
    let values = [0u64, 1, 2, MAX - 1, MAX];
    let mut inputs = Vec::with_capacity(values.len() * values.len());
    let mut left = 0usize;
    while left < values.len() {
        let mut right = 0usize;
        while right < values.len() {
            inputs.push(vec![values[left], values[right]]);
            right += 1;
        }
        left += 1;
    }
    inputs
}

pub(super) struct Fixture {
    pub(super) name: &'static str,
    pub(super) base: Program,
    pub(super) first: Program,
    pub(super) second: Program,
    pub(super) candidate: Program,
    pub(super) output_len: usize,
    pub(super) expected: MergeResult,
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-maintainers; the fixture table is intentionally one linear review block covering every positive and negative SMERGE case"
)]
pub(super) fn fixtures() -> Vec<Fixture> {
    let identity = program(Vec::new(), Vec::new(), 0);
    let increment_top = program(vec![Op::Push(1), Op::Add], Vec::new(), 0);
    let increment_bottom = program(
        vec![Op::Swap, Op::Push(1), Op::Add, Op::Swap],
        Vec::new(),
        0,
    );
    let increment_both = program(
        vec![
            Op::Push(1),
            Op::Add,
            Op::Swap,
            Op::Push(1),
            Op::Add,
            Op::Swap,
        ],
        Vec::new(),
        0,
    );
    let equivalent = program(
        vec![Op::Push(2), Op::Add, Op::Push(1), Op::Sub],
        Vec::new(),
        0,
    );
    let increment_two = program(vec![Op::Push(2), Op::Add], Vec::new(), 0);
    let missing = program(vec![Op::Drop], Vec::new(), 0);
    let captured_five = program(vec![Op::PushCapture(0), Op::Add], vec![5], 0);
    let captured_six = program(vec![Op::PushCapture(0), Op::Add], vec![6], 0);
    let abnormal = program(vec![Op::Drop, Op::Drop, Op::Drop], Vec::new(), 0);
    vec![
        Fixture {
            name: "independent-changes",
            base: identity.clone(),
            first: increment_top.clone(),
            second: increment_bottom.clone(),
            candidate: increment_both,
            output_len: 2,
            expected: MergeResult::Verified,
        },
        Fixture {
            name: "equivalent-edits",
            base: identity.clone(),
            first: increment_top.clone(),
            second: equivalent,
            candidate: increment_top.clone(),
            output_len: 2,
            expected: MergeResult::Verified,
        },
        Fixture {
            name: "one-sided-change",
            base: identity.clone(),
            first: increment_top.clone(),
            second: identity.clone(),
            candidate: increment_top.clone(),
            output_len: 2,
            expected: MergeResult::Verified,
        },
        Fixture {
            name: "both-change-identically",
            base: identity.clone(),
            first: increment_top.clone(),
            second: increment_top.clone(),
            candidate: increment_top.clone(),
            output_len: 2,
            expected: MergeResult::Verified,
        },
        Fixture {
            name: "lost-change",
            base: identity.clone(),
            first: increment_top.clone(),
            second: identity.clone(),
            candidate: identity.clone(),
            output_len: 2,
            expected: MergeResult::Counterexample {
                input: vec![0, 0],
                position: 1,
                expected: 1,
                actual: 0,
            },
        },
        Fixture {
            name: "incompatible-edits",
            base: identity.clone(),
            first: increment_top.clone(),
            second: increment_two.clone(),
            candidate: increment_top.clone(),
            output_len: 2,
            expected: MergeResult::Conflict {
                input: vec![0, 0],
                position: 1,
            },
        },
        Fixture {
            name: "candidate-differs",
            base: identity.clone(),
            first: increment_top.clone(),
            second: identity.clone(),
            candidate: increment_two,
            output_len: 2,
            expected: MergeResult::Counterexample {
                input: vec![0, 0],
                position: 1,
                expected: 1,
                actual: 2,
            },
        },
        Fixture {
            name: "missing-observation",
            base: identity.clone(),
            first: increment_top.clone(),
            second: identity.clone(),
            candidate: missing,
            output_len: 2,
            expected: MergeResult::IncompatibleInterface,
        },
        Fixture {
            name: "changed-capture",
            base: identity.clone(),
            first: captured_five,
            second: identity.clone(),
            candidate: captured_six,
            output_len: 2,
            expected: MergeResult::Counterexample {
                input: vec![0, 0],
                position: 1,
                expected: 5,
                actual: 6,
            },
        },
        Fixture {
            name: "abnormal",
            base: abnormal.clone(),
            first: abnormal.clone(),
            second: abnormal.clone(),
            candidate: abnormal,
            output_len: 2,
            expected: MergeResult::Abnormal,
        },
    ]
}
