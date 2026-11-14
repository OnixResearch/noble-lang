use super::*;

#[derive(Clone, Debug)]
pub(crate) enum OracleValue {
    I64(i64),
    Bool(bool),
    Quotation(Vec<u32>),
}

pub(crate) fn pop_i64(stack: &mut Vec<OracleValue>) -> Result<i64, String> {
    match stack.pop() {
        Some(OracleValue::I64(value)) => Ok(value),
        _ => Err(String::from("independent replay expected I64")),
    }
}

pub(crate) fn evaluate_recipe(
    recipe: &Recipe,
    references: &[u32],
    stack: &mut Vec<OracleValue>,
    depth: u32,
) -> Result<(), String> {
    if depth > 8 {
        return Err(String::from(
            "independent quotation depth exceeded its fixed bound",
        ));
    }
    for reference in references {
        let index = usize::try_from(*reference).map_err(|error| error.to_string())?;
        let node = recipe
            .nodes
            .get(index)
            .ok_or_else(|| String::from("independent recipe reference is outside its arena"))?;
        match node {
            Node::I64(value) => stack.push(OracleValue::I64(*value)),
            Node::Bool(value) => stack.push(OracleValue::Bool(*value)),
            Node::Capture { .. } | Node::Invoke { .. } => {
                return Err(String::from(
                    "fixture oracle does not admit captures or dependencies",
                ));
            }
            Node::Quotation { body, .. } => stack.push(OracleValue::Quotation(body.clone())),
            Node::Operation { op, .. } => match op {
                Op::Dup => {
                    let value = stack.last().cloned().ok_or("independent dup underflow")?;
                    stack.push(value);
                }
                Op::Drop => {
                    let _ = stack.pop().ok_or("independent drop underflow")?;
                }
                Op::Swap => {
                    if stack.len() < 2 {
                        return Err(String::from("independent swap underflow"));
                    }
                    let top = stack.len().saturating_sub(1);
                    let below = stack.len().saturating_sub(2);
                    stack.swap(top, below);
                }
                Op::Add | Op::Sub | Op::Mul | Op::Eq => {
                    let right = pop_i64(stack)?;
                    let left = pop_i64(stack)?;
                    match op {
                        Op::Add => stack.push(OracleValue::I64(left.wrapping_add(right))),
                        Op::Sub => stack.push(OracleValue::I64(left.wrapping_sub(right))),
                        Op::Mul => stack.push(OracleValue::I64(left.wrapping_mul(right))),
                        Op::Eq => stack.push(OracleValue::Bool(left == right)),
                        _ => return Err(String::from("unreachable arithmetic oracle branch")),
                    }
                }
                Op::Run => {
                    let Some(OracleValue::Quotation(body)) = stack.pop() else {
                        return Err(String::from("independent run received a non-quotation"));
                    };
                    evaluate_recipe(recipe, &body, stack, depth.saturating_add(1))?;
                }
            },
        }
    }
    Ok(())
}

pub(crate) fn independent_recipe_output(
    envelope: &Envelope,
    inputs: &[Scalar],
) -> Result<Vec<Scalar>, String> {
    let mut stack: Vec<OracleValue> = inputs
        .iter()
        .map(|value| match value {
            Scalar::I64(value) => OracleValue::I64(*value),
            Scalar::Bool(value) => OracleValue::Bool(*value),
        })
        .collect();
    evaluate_recipe(&envelope.body, &envelope.body.entry, &mut stack, 0)?;
    stack
        .into_iter()
        .map(|value| match value {
            OracleValue::I64(value) => Ok(Scalar::I64(value)),
            OracleValue::Bool(value) => Ok(Scalar::Bool(value)),
            OracleValue::Quotation(_) => {
                Err(String::from("independent replay left quotation data"))
            }
        })
        .collect()
}

pub(crate) fn oracle_target(target: Target, inputs: &[Scalar]) -> Result<Vec<Scalar>, String> {
    match target {
        Target::Arithmetic(op) => {
            let [Scalar::I64(left), Scalar::I64(right)] = inputs else {
                return Err(String::from("arithmetic oracle received malformed inputs"));
            };
            let value = match op {
                Op::Add => left.wrapping_add(*right),
                Op::Sub => left.wrapping_sub(*right),
                Op::Mul => left.wrapping_mul(*right),
                _ => {
                    return Err(String::from(
                        "arithmetic oracle received a non-arithmetic operation",
                    ))
                }
            };
            Ok(vec![Scalar::I64(value)])
        }
        Target::Swap => {
            let mut output = inputs.to_vec();
            output.reverse();
            Ok(output)
        }
        Target::Boolean(mode) => {
            let [Scalar::I64(left), Scalar::I64(right)] = inputs else {
                return Err(String::from("boolean oracle received malformed inputs"));
            };
            let result = match mode {
                0 => left == right,
                1 => left.wrapping_add(*right) == 0,
                _ => left.wrapping_sub(*right) == 0,
            };
            Ok(vec![Scalar::Bool(result)])
        }
        Target::Integer(value) => Ok(vec![Scalar::I64(value)]),
    }
}

pub(crate) fn primitive_envelope(spec: &TaskSpec) -> Envelope {
    match (spec.family, spec.target) {
        (0, Target::Arithmetic(op)) => {
            operation_envelope(op, &[op.spelling().as_bytes(), b" "].concat())
        }
        (1, Target::Swap) => family_envelopes(1)
            .into_iter()
            .find(|envelope| envelope.interface.stack_in == spec.stack_in)
            .unwrap_or_else(|| unreachable!()),
        (2, Target::Boolean(mode)) => match mode {
            0 => boolean_comparison_envelope(Op::Eq, b"= "),
            1 => boolean_comparison_envelope(Op::Add, b"+ 0 = "),
            _ => boolean_comparison_envelope(Op::Sub, b"- 0 = "),
        },
        (3, Target::Integer(0)) => literal_envelope(0, b"0 "),
        (3, Target::Integer(1)) => quotation_envelope(1, 1, b"[ 1 ] run "),
        (3, Target::Integer(_)) => quotation_envelope(2, 2, b"[ [ 2 ] ] run run "),
        (4, Target::Integer(value)) => {
            let source = format!("{value} ");
            literal_envelope(value, source.as_bytes())
        }
        _ => unreachable!(),
    }
}
