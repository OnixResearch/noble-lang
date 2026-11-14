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

pub(crate) fn task_spec(family: usize, block: u8, within_block: u8) -> TaskSpec {
    let case_seed = i64::try_from(family.saturating_add(1)).unwrap_or(0) * 1_000_003
        + i64::from(block) * 8_191
        + i64::from(within_block) * 257;
    let (stack_in, stack_out, target, primitive_source, cases) = match family {
        0 => {
            let operation = (block + within_block) % 3;
            let op = [Op::Add, Op::Sub, Op::Mul][usize::from(operation)];
            let source = op.spelling().as_bytes().to_vec();
            let cases = (0..FIXTURE_CASES)
                .map(|case| {
                    let delta = i64::try_from(case).unwrap_or(0);
                    let left = (case_seed + delta * 19) % 97 - 48;
                    let right = (case_seed * 3 - delta * 11) % 89 - 44;
                    let output = match op {
                        Op::Add => left.wrapping_add(right),
                        Op::Sub => left.wrapping_sub(right),
                        Op::Mul => left.wrapping_mul(right),
                        _ => 0,
                    };
                    TaskCase {
                        inputs: vec![Scalar::I64(left), Scalar::I64(right)],
                        outputs: vec![Scalar::I64(output)],
                    }
                })
                .collect();
            (
                vec![Ty::I64, Ty::I64],
                vec![Ty::I64],
                Target::Arithmetic(op),
                source,
                cases,
            )
        }
        1 => {
            let reversed = !within_block.is_multiple_of(2);
            let stack_in = if reversed {
                vec![Ty::Bool, Ty::I64]
            } else {
                vec![Ty::I64, Ty::Bool]
            };
            let stack_out = if reversed {
                vec![Ty::I64, Ty::Bool]
            } else {
                vec![Ty::Bool, Ty::I64]
            };
            let cases = (0..FIXTURE_CASES)
                .map(|case| {
                    let delta = i64::try_from(case).unwrap_or(0);
                    let integer = case_seed + delta * 31;
                    let boolean =
                        (block + within_block + u8::try_from(case).unwrap_or(0)).is_multiple_of(2);
                    let inputs = if reversed {
                        vec![Scalar::Bool(boolean), Scalar::I64(integer)]
                    } else {
                        vec![Scalar::I64(integer), Scalar::Bool(boolean)]
                    };
                    let outputs = if reversed {
                        vec![Scalar::I64(integer), Scalar::Bool(boolean)]
                    } else {
                        vec![Scalar::Bool(boolean), Scalar::I64(integer)]
                    };
                    TaskCase { inputs, outputs }
                })
                .collect();
            (stack_in, stack_out, Target::Swap, b"swap".to_vec(), cases)
        }
        2 => {
            let mode = (block + within_block) % 3;
            let source = match mode {
                0 => b"=".to_vec(),
                1 => b"+ 0 =".to_vec(),
                _ => b"- 0 =".to_vec(),
            };
            let cases = (0..FIXTURE_CASES)
                .map(|case| {
                    let delta = i64::try_from(case).unwrap_or(0);
                    let left = case_seed + delta * 13;
                    let right = case_seed / 3 - delta * 7;
                    let compared = match mode {
                        0 => left == right,
                        1 => left.wrapping_add(right) == 0,
                        _ => left.wrapping_sub(right) == 0,
                    };
                    TaskCase {
                        inputs: vec![Scalar::I64(left), Scalar::I64(right)],
                        outputs: vec![Scalar::Bool(compared)],
                    }
                })
                .collect();
            (
                vec![Ty::I64, Ty::I64],
                vec![Ty::Bool],
                Target::Boolean(mode),
                source,
                cases,
            )
        }
        3 | 4 => {
            let value = i64::from((block * 8 + within_block) % 3);
            let source = if family == 3 {
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
            (
                Vec::new(),
                vec![Ty::I64],
                Target::Integer(value),
                source,
                cases,
            )
        }
        _ => unreachable!(),
    };
    TaskSpec {
        family,
        block,
        within_block,
        stack_in,
        stack_out,
        cases,
        primitive_source,
        target,
    }
}

pub(crate) fn corpus_tasks() -> Vec<TaskSpec> {
    let mut tasks = Vec::with_capacity(480);
    for family in 0..FAMILY_NAMES.len() {
        for block in 0..12 {
            for within_block in 0..8 {
                tasks.push(task_spec(family, block, within_block));
            }
        }
    }
    tasks
}
