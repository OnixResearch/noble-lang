use super::*;

#[octet::sealed_enum]
#[derive(Clone, Debug)]
enum StackValue {
    I64(i64),
    Bool(bool),
    Quotation(Vec<u32>),
}

struct ValueStack(Vec<StackValue>);

impl ValueStack {
    fn pop(&mut self) -> Option<StackValue> {
        self.0.pop()
    }

    fn push(&mut self, value: StackValue) {
        self.0.push(value);
    }

    fn pop_i64(&mut self) -> Result<i64, String> {
        match self.pop() {
            Some(StackValue::I64(value)) => Ok(value),
            Some(StackValue::Bool(_) | StackValue::Quotation(_)) | None => {
                Err(String::from("independent replay expected I64"))
            }
        }
    }

    fn swap_top(&mut self) -> Result<(), String> {
        if self.0.len() < 2 {
            return Err(String::from("independent swap underflow"));
        }
        let top = self.0.len().saturating_sub(1);
        let below = self.0.len().saturating_sub(2);
        self.0.swap(top, below);
        Ok(())
    }

    fn duplicate_top(&mut self) -> Result<(), String> {
        let value = self.0.last().cloned().ok_or("independent dup underflow")?;
        self.push(value);
        Ok(())
    }

    fn into_scalars(self) -> Result<Vec<Scalar>, String> {
        self.0.into_iter().map(output_scalar).collect()
    }
}

fn arithmetic_operation(op: Op, stack: &mut ValueStack) -> Result<(), String> {
    let right = stack.pop_i64()?;
    let left = stack.pop_i64()?;
    match op {
        Op::Add => stack.push(StackValue::I64(left.wrapping_add(right))),
        Op::Sub => stack.push(StackValue::I64(left.wrapping_sub(right))),
        Op::Mul => stack.push(StackValue::I64(left.wrapping_mul(right))),
        Op::Eq => stack.push(StackValue::Bool(left == right)),
        Op::Dup | Op::Drop | Op::Swap | Op::Run => {
            return Err(String::from("unreachable arithmetic oracle branch"));
        }
    }
    Ok(())
}

fn run_quotation(recipe: &Recipe, stack: &mut ValueStack, depth: u32) -> Result<(), String> {
    let Some(StackValue::Quotation(body)) = stack.pop() else {
        return Err(String::from("independent run received a non-quotation"));
    };
    evaluate_recipe(recipe, &body, stack, depth.saturating_add(1))
}

fn execute_operation(
    recipe: &Recipe,
    op: Op,
    stack: &mut ValueStack,
    depth: u32,
) -> Result<(), String> {
    match op {
        Op::Dup => stack.duplicate_top(),
        Op::Drop => stack
            .pop()
            .map(|_| ())
            .ok_or_else(|| String::from("independent drop underflow")),
        Op::Swap => stack.swap_top(),
        Op::Add | Op::Sub | Op::Mul | Op::Eq => arithmetic_operation(op, stack),
        Op::Run => run_quotation(recipe, stack, depth),
    }
}

fn evaluate_node(
    recipe: &Recipe,
    node: &Node,
    stack: &mut ValueStack,
    depth: u32,
) -> Result<(), String> {
    match node {
        Node::I64(value) => stack.push(StackValue::I64(*value)),
        Node::Bool(value) => stack.push(StackValue::Bool(*value)),
        Node::Capture { .. } | Node::Invoke { .. } => {
            return Err(String::from(
                "fixture oracle does not admit captures or dependencies",
            ));
        }
        Node::Quotation { body, .. } => stack.push(StackValue::Quotation(body.clone())),
        Node::Operation { op, .. } => return execute_operation(recipe, *op, stack, depth),
    }
    Ok(())
}

fn evaluate_recipe(
    recipe: &Recipe,
    references: &[u32],
    stack: &mut ValueStack,
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
        evaluate_node(recipe, node, stack, depth)?;
    }
    Ok(())
}

fn input_value(value: &Scalar) -> StackValue {
    match value {
        Scalar::I64(value) => StackValue::I64(*value),
        Scalar::Bool(value) => StackValue::Bool(*value),
    }
}

fn output_scalar(value: StackValue) -> Result<Scalar, String> {
    match value {
        StackValue::I64(value) => Ok(Scalar::I64(value)),
        StackValue::Bool(value) => Ok(Scalar::Bool(value)),
        StackValue::Quotation(_) => Err(String::from("independent replay left quotation data")),
    }
}

pub(crate) fn independent_recipe_output(
    envelope: &Envelope,
    inputs: &[Scalar],
) -> Result<Vec<Scalar>, String> {
    let mut stack = ValueStack(inputs.iter().map(input_value).collect());
    evaluate_recipe(&envelope.body, &envelope.body.entry, &mut stack, 0)?;
    stack.into_scalars()
}

fn arithmetic_target(op: Op, inputs: &[Scalar]) -> Result<Vec<Scalar>, String> {
    let [Scalar::I64(left), Scalar::I64(right)] = inputs else {
        return Err(String::from("arithmetic oracle received malformed inputs"));
    };
    let value = match op {
        Op::Add => left.wrapping_add(*right),
        Op::Sub => left.wrapping_sub(*right),
        Op::Mul => left.wrapping_mul(*right),
        Op::Dup | Op::Drop | Op::Swap | Op::Eq | Op::Run => {
            return Err(String::from(
                "arithmetic oracle received a non-arithmetic operation",
            ));
        }
    };
    Ok(vec![Scalar::I64(value)])
}

fn boolean_target(mode: u8, inputs: &[Scalar]) -> Result<Vec<Scalar>, String> {
    let [Scalar::I64(left), Scalar::I64(right)] = inputs else {
        return Err(String::from("boolean oracle received malformed inputs"));
    };
    let is_match = match mode {
        0 => left == right,
        1 => left.wrapping_add(*right) == 0,
        _ => left.wrapping_sub(*right) == 0,
    };
    Ok(vec![Scalar::Bool(is_match)])
}

fn integer_target(target: Target) -> Result<Vec<Scalar>, String> {
    let Target::Integer(value) = target else {
        return Err(String::from(
            "independent oracle received an unsupported target",
        ));
    };
    Ok(vec![Scalar::I64(value)])
}

pub(crate) fn target(target: Target, inputs: &[Scalar]) -> Result<Vec<Scalar>, String> {
    match target {
        Target::Arithmetic(op) => arithmetic_target(op, inputs),
        Target::Swap => {
            let mut output = inputs.to_vec();
            output.reverse();
            Ok(output)
        }
        Target::Boolean(mode) => boolean_target(mode, inputs),
        _ => integer_target(target),
    }
}

fn boolean_primitive(mode: u8) -> Envelope {
    match mode {
        0 => boolean_comparison_envelope(Op::Eq, b"= "),
        1 => boolean_comparison_envelope(Op::Add, b"+ 0 = "),
        _ => boolean_comparison_envelope(Op::Sub, b"- 0 = "),
    }
}

fn quotation_primitive(value: i64) -> Envelope {
    match value {
        0 => literal_envelope(0, b"0 "),
        1 => quotation_envelope(1, 1, b"[ 1 ] run "),
        _ => quotation_envelope(2, 2, b"[ [ 2 ] ] run run "),
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
        (2, Target::Boolean(mode)) => boolean_primitive(mode),
        (3, Target::Integer(value)) => quotation_primitive(value),
        (4, Target::Integer(value)) => {
            let source = format!("{value} ");
            literal_envelope(value, source.as_bytes())
        }
        _ => unreachable!(),
    }
}
