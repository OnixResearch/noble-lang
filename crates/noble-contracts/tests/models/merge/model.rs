#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Op {
    Push(u64),
    PushCapture(usize),
    Add,
    Sub,
    Swap,
    Drop,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Program {
    pub(super) ops: Vec<Op>,
    pub(super) captures: Vec<u64>,
    pub(super) callee: u64,
}

pub(super) fn program(ops: Vec<Op>, captures: Vec<u64>, callee: u64) -> Program {
    Program {
        ops,
        captures,
        callee,
    }
}

pub(super) fn run(program: &Program, input: &[u64]) -> Option<Vec<u64>> {
    let mut stack = input.to_vec();
    let mut at = 0usize;
    while at < program.ops.len() {
        match program.ops[at] {
            Op::Push(value) => stack.push(value),
            Op::PushCapture(index) => stack.push(*program.captures.get(index)?),
            Op::Add => {
                let right = stack.pop()?;
                let left = stack.pop()?;
                stack.push(left.wrapping_add(right));
            }
            Op::Sub => {
                let right = stack.pop()?;
                let left = stack.pop()?;
                stack.push(left.wrapping_sub(right));
            }
            Op::Swap => {
                let length = stack.len();
                if length < 2 {
                    return None;
                }
                stack.swap(length - 1, length - 2);
            }
            Op::Drop => {
                stack.pop()?;
            }
        }
        at += 1;
    }
    Some(stack)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum MergeResult {
    Verified,
    Conflict {
        input: Vec<u64>,
        position: usize,
    },
    Counterexample {
        input: Vec<u64>,
        position: usize,
        expected: u64,
        actual: u64,
    },
    IncompatibleInterface,
    Abnormal,
    VacuousPrecondition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Precondition {
    True,
    False,
}

pub(super) fn combined_value(base: u64, first: u64, second: u64) -> Option<u64> {
    let is_first_unchanged = first == base;
    let is_second_unchanged = second == base;
    if is_first_unchanged {
        return if is_second_unchanged {
            Some(base)
        } else {
            Some(second)
        };
    }
    if is_second_unchanged {
        return Some(first);
    }
    if first == second {
        Some(first)
    } else {
        None
    }
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-maintainers; the test-only relation takes the four exact subjects plus the selected input and output width explicitly so each fixture remains readable"
)]
pub(super) fn check_relation(
    base: &Program,
    first: &Program,
    second: &Program,
    candidate: &Program,
    input: &[u64],
    output_len: usize,
    precondition: Precondition,
) -> MergeResult {
    if precondition == Precondition::False {
        return MergeResult::VacuousPrecondition;
    }
    let Some(base_out) = run(base, input) else {
        return MergeResult::Abnormal;
    };
    let Some(first_out) = run(first, input) else {
        return MergeResult::Abnormal;
    };
    let Some(second_out) = run(second, input) else {
        return MergeResult::Abnormal;
    };
    let Some(candidate_out) = run(candidate, input) else {
        return MergeResult::Abnormal;
    };
    if base_out.len() != output_len
        || first_out.len() != output_len
        || second_out.len() != output_len
        || candidate_out.len() != output_len
    {
        return MergeResult::IncompatibleInterface;
    }
    let mut position = 0usize;
    while position < output_len {
        let Some(expected) = combined_value(
            base_out[position],
            first_out[position],
            second_out[position],
        ) else {
            return MergeResult::Conflict {
                input: input.to_vec(),
                position,
            };
        };
        if candidate_out[position] != expected {
            return MergeResult::Counterexample {
                input: input.to_vec(),
                position,
                expected,
                actual: candidate_out[position],
            };
        }
        position += 1;
    }
    MergeResult::Verified
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-maintainers; the aggregate relation takes the four exact subjects, the input corpus and the selected output width explicitly so the finite fixture lane remains readable"
)]
pub(super) fn check_relation_over(
    base: &Program,
    first: &Program,
    second: &Program,
    candidate: &Program,
    inputs: &[Vec<u64>],
    output_len: usize,
    precondition: Precondition,
) -> MergeResult {
    let mut at = 0usize;
    while at < inputs.len() {
        let result = check_relation(
            base,
            first,
            second,
            candidate,
            &inputs[at],
            output_len,
            precondition,
        );
        if result != MergeResult::Verified {
            return result;
        }
        at += 1;
    }
    MergeResult::Verified
}
