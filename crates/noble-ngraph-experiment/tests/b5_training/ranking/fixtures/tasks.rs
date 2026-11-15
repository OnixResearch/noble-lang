use super::*;

#[derive(Clone, Debug)]
pub(crate) struct TaskCase {
    pub(crate) inputs: Vec<Scalar>,
    pub(crate) outputs: Vec<Scalar>,
}

#[derive(Clone, Debug)]
pub(crate) struct TaskSpec {
    pub(crate) family: usize,
    pub(crate) block: u8,
    pub(crate) within_block: u8,
    pub(crate) stack_in: Vec<Ty>,
    pub(crate) stack_out: Vec<Ty>,
    pub(crate) cases: Vec<TaskCase>,
    pub(crate) primitive_source: Vec<u8>,
    pub(crate) target: Target,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Target {
    Arithmetic(Op),
    Swap,
    Boolean(u8),
    Integer(i64),
}

impl TaskSpec {
    pub(crate) fn id(&self) -> String {
        format!(
            "{}/b{:02}/t{:02}",
            FAMILY_NAMES[self.family], self.block, self.within_block
        )
    }

    pub(crate) fn split(&self) -> &'static str {
        match self.block {
            0..=6 => "train",
            7..=8 => "validation",
            _ => "test",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TaskCoordinate {
    pub(crate) family: usize,
    pub(crate) block: u8,
    pub(crate) within_block: u8,
}

fn task_case_seed(coordinate: TaskCoordinate) -> i64 {
    i64::try_from(coordinate.family.saturating_add(1)).expect("five families") * 1_000_003
        + i64::from(coordinate.block) * 8_191
        + i64::from(coordinate.within_block) * 257
}

fn arithmetic_task(coordinate: TaskCoordinate) -> TaskSpec {
    let seed = task_case_seed(coordinate);
    let operation = (coordinate.block + coordinate.within_block) % 3;
    let op = [Op::Add, Op::Sub, Op::Mul][usize::from(operation)];
    let cases = (0..FIXTURE_CASES)
        .map(|case| {
            let delta = i64::try_from(case).expect("four fixture cases");
            let left = (seed + delta * 19) % 97 - 48;
            let right = (seed * 3 - delta * 11) % 89 - 44;
            let output = match operation {
                0 => left.wrapping_add(right),
                1 => left.wrapping_sub(right),
                _ => left.wrapping_mul(right),
            };
            TaskCase {
                inputs: vec![Scalar::I64(left), Scalar::I64(right)],
                outputs: vec![Scalar::I64(output)],
            }
        })
        .collect();
    debug_assert_eq!(operation, (coordinate.block + coordinate.within_block) % 3);
    debug_assert!(matches!(op, Op::Add | Op::Sub | Op::Mul));
    TaskSpec {
        family: coordinate.family,
        block: coordinate.block,
        within_block: coordinate.within_block,
        stack_in: vec![Ty::I64, Ty::I64],
        stack_out: vec![Ty::I64],
        cases,
        primitive_source: op.spelling().as_bytes().to_vec(),
        target: Target::Arithmetic(op),
    }
}

fn routing_task(coordinate: TaskCoordinate) -> TaskSpec {
    let seed = task_case_seed(coordinate);
    let is_reversed = !coordinate.within_block.is_multiple_of(2);
    let stack_in = if is_reversed {
        vec![Ty::Bool, Ty::I64]
    } else {
        vec![Ty::I64, Ty::Bool]
    };
    let stack_out = if is_reversed {
        vec![Ty::I64, Ty::Bool]
    } else {
        vec![Ty::Bool, Ty::I64]
    };
    let cases = (0..FIXTURE_CASES)
        .map(|case| {
            let delta = i64::try_from(case).expect("four fixture cases");
            let integer = seed + delta * 31;
            let is_even = (coordinate.block
                + coordinate.within_block
                + u8::try_from(case).expect("four fixture cases"))
            .is_multiple_of(2);
            let inputs = if is_reversed {
                vec![Scalar::Bool(is_even), Scalar::I64(integer)]
            } else {
                vec![Scalar::I64(integer), Scalar::Bool(is_even)]
            };
            let outputs = if is_reversed {
                vec![Scalar::I64(integer), Scalar::Bool(is_even)]
            } else {
                vec![Scalar::Bool(is_even), Scalar::I64(integer)]
            };
            TaskCase { inputs, outputs }
        })
        .collect();
    debug_assert_eq!(stack_in.len(), 2);
    debug_assert_eq!(stack_out.len(), 2);
    TaskSpec {
        family: coordinate.family,
        block: coordinate.block,
        within_block: coordinate.within_block,
        stack_in,
        stack_out,
        cases,
        primitive_source: b"swap".to_vec(),
        target: Target::Swap,
    }
}

fn boolean_task(coordinate: TaskCoordinate) -> TaskSpec {
    let seed = task_case_seed(coordinate);
    let mode = (coordinate.block + coordinate.within_block) % 3;
    let source = match mode {
        0 => b"=".to_vec(),
        1 => b"+ 0 =".to_vec(),
        _ => b"- 0 =".to_vec(),
    };
    let cases = (0..FIXTURE_CASES)
        .map(|case| {
            let delta = i64::try_from(case).expect("four fixture cases");
            let left = seed + delta * 13;
            let right = seed / 3 - delta * 7;
            let is_match = match mode {
                0 => left == right,
                1 => left.wrapping_add(right) == 0,
                _ => left.wrapping_sub(right) == 0,
            };
            TaskCase {
                inputs: vec![Scalar::I64(left), Scalar::I64(right)],
                outputs: vec![Scalar::Bool(is_match)],
            }
        })
        .collect();
    debug_assert!(!source.is_empty());
    debug_assert!(mode < 3);
    TaskSpec {
        family: coordinate.family,
        block: coordinate.block,
        within_block: coordinate.within_block,
        stack_in: vec![Ty::I64, Ty::I64],
        stack_out: vec![Ty::Bool],
        cases,
        primitive_source: source,
        target: Target::Boolean(mode),
    }
}

fn integer_task(coordinate: TaskCoordinate) -> TaskSpec {
    let value = i64::from((coordinate.block * 8 + coordinate.within_block) % 3);
    let source = if coordinate.family == 3 {
        match value {
            0 => b"0".to_vec(),
            1 => b"[ 1 ] run".to_vec(),
            _ => b"[ [ 2 ] ] run run".to_vec(),
        }
    } else {
        value.to_string().into_bytes()
    };
    let cases = (0..FIXTURE_CASES)
        .map(|_| TaskCase {
            inputs: Vec::new(),
            outputs: vec![Scalar::I64(value)],
        })
        .collect();
    debug_assert!(matches!(coordinate.family, 3 | 4));
    debug_assert!(value < 3);
    TaskSpec {
        family: coordinate.family,
        block: coordinate.block,
        within_block: coordinate.within_block,
        stack_in: Vec::new(),
        stack_out: vec![Ty::I64],
        cases,
        primitive_source: source,
        target: Target::Integer(value),
    }
}

fn task_spec(coordinate: TaskCoordinate) -> TaskSpec {
    let mut task = match coordinate.family {
        0 => arithmetic_task(coordinate),
        1 => routing_task(coordinate),
        2 => boolean_task(coordinate),
        _ => integer_task(coordinate),
    };
    // Distinguish canonical reference programs across provenance blocks without
    // changing their stack effect, target, fixture output or candidate library.
    let instance = coordinate.family * 96
        + usize::from(coordinate.block) * 8
        + usize::from(coordinate.within_block)
        + 1;
    let mut source = format!("{instance} drop ").into_bytes();
    source.extend_from_slice(&task.primitive_source);
    task.primitive_source = source;
    task
}

pub(crate) fn generate() -> Vec<TaskSpec> {
    let mut tasks = Vec::with_capacity(480);
    for family in 0..FAMILY_NAMES.len() {
        for block in 0..12 {
            for within_block in 0..8 {
                tasks.push(task_spec(TaskCoordinate {
                    family,
                    block,
                    within_block,
                }));
            }
        }
    }
    tasks
}
