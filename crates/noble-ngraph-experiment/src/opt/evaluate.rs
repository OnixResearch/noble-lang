//! Exact-bit evaluation of the admitted closed instruction set.

use super::candidate::{Candidate, Node, Operand, Operation};
use super::types::{
    classify_bits, NonFinite, Refusal, RefusalClass, TransitionInput, TransitionOutput,
};

#[derive(Debug)]
enum RuntimeValue {
    Scalar(u32),
    Vector(Vec<u32>),
}

enum RuntimeRef<'a> {
    Scalar(u32),
    Vector(&'a [u32]),
}

impl Candidate {
    /// Evaluates this admitted candidate on explicit host-owned transition inputs.
    ///
    /// Static graph limits and measured work are fixed by the parser. Inputs and outputs are
    /// classified from bits, not host `is_finite` behavior. The descriptor is validated but cannot
    /// be read by a candidate opcode.
    pub fn evaluate(&self, input: &TransitionInput<'_>) -> Result<TransitionOutput, Refusal> {
        if input.step.version != 1 {
            return Err(Refusal::new(
                RefusalClass::Malformed,
                "$.inputs.step.version",
                input.step.version.to_string(),
            ));
        }
        if !input.state.is_empty() {
            return Err(Refusal::new(
                RefusalClass::Exhausted,
                "$.state",
                format!(
                    "{} state bytes exceeds the State0 limit 0",
                    input.state.len()
                ),
            ));
        }
        if input.parameters.len() != self.program.lanes {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                "$.inputs.p",
                format!(
                    "expected {} lanes, observed {}",
                    self.program.lanes,
                    input.parameters.len()
                ),
            ));
        }
        if input.gradients.len() != self.program.lanes {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                "$.inputs.g",
                format!(
                    "expected {} lanes, observed {}",
                    self.program.lanes,
                    input.gradients.len()
                ),
            ));
        }
        refuse_nonfinite(input.parameters, "$.inputs.p")?;
        refuse_nonfinite(input.gradients, "$.inputs.g")?;
        if let Some(kind) = classify_bits(input.eta) {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                "$.inputs.eta",
                format!(
                    "{} {}",
                    nonfinite_name(kind),
                    super::hash::bits_hex(input.eta)
                ),
            ));
        }

        let mut values = Vec::with_capacity(self.program.nodes.len());
        for node in &self.program.nodes {
            let value = evaluate_node(node, &values, input, self.program.lanes)?;
            values.push((node.id, value));
        }
        let result = values
            .into_iter()
            .find_map(|(id, value)| (id == self.program.output).then_some(value))
            .ok_or_else(|| internal("the admitted output node was not evaluated"))?;
        let RuntimeValue::Vector(parameters) = result else {
            return Err(internal("the admitted output node is not a vector"));
        };
        for (lane, bits) in parameters.iter().copied().enumerate() {
            if let Some(kind) = classify_bits(bits) {
                return Err(Refusal::new(
                    RefusalClass::Unsupported,
                    format!("$.outputs.parameters[{lane}]"),
                    format!("{} {}", nonfinite_name(kind), super::hash::bits_hex(bits)),
                ));
            }
        }
        Ok(TransitionOutput {
            parameters,
            state: Vec::new(),
            work: self.program.work,
        })
    }
}

fn evaluate_node(
    node: &Node,
    values: &[(u16, RuntimeValue)],
    input: &TransitionInput<'_>,
    lanes: usize,
) -> Result<RuntimeValue, Refusal> {
    match node.op {
        Operation::Constant(bits) => Ok(RuntimeValue::Scalar(bits)),
        Operation::Broadcast => {
            let operand = node
                .operands
                .first()
                .copied()
                .ok_or_else(|| internal("broadcast operand is absent"))?;
            let RuntimeRef::Scalar(bits) = operand_value(operand, values, input)? else {
                return Err(internal("admitted broadcast operand changed type"));
            };
            Ok(RuntimeValue::Vector(vec![bits; lanes]))
        }
        Operation::Mul | Operation::Sub => {
            let left = node
                .operands
                .first()
                .copied()
                .ok_or_else(|| internal("binary left operand is absent"))?;
            let right = node
                .operands
                .get(1)
                .copied()
                .ok_or_else(|| internal("binary right operand is absent"))?;
            let RuntimeRef::Vector(left) = operand_value(left, values, input)? else {
                return Err(internal("admitted binary left operand changed type"));
            };
            let RuntimeRef::Vector(right) = operand_value(right, values, input)? else {
                return Err(internal("admitted binary right operand changed type"));
            };
            if left.len() != lanes || right.len() != lanes {
                return Err(internal("admitted vector length changed during evaluation"));
            }
            let mut result = Vec::with_capacity(lanes);
            for (a, b) in left.iter().zip(right) {
                let a = f32::from_bits(*a);
                let b = f32::from_bits(*b);
                let value = match node.op {
                    Operation::Mul => noble_ngraph::sem::mul(a, b),
                    Operation::Sub => noble_ngraph::sem::sub(a, b),
                    Operation::Constant(_) | Operation::Broadcast => {
                        return Err(internal("non-binary operation reached binary evaluator"));
                    }
                };
                result.push(value.to_bits());
            }
            Ok(RuntimeValue::Vector(result))
        }
    }
}

fn operand_value<'a>(
    operand: Operand,
    values: &'a [(u16, RuntimeValue)],
    input: &'a TransitionInput<'_>,
) -> Result<RuntimeRef<'a>, Refusal> {
    match operand {
        Operand::Parameters => Ok(RuntimeRef::Vector(input.parameters)),
        Operand::Gradients => Ok(RuntimeRef::Vector(input.gradients)),
        Operand::Eta => Ok(RuntimeRef::Scalar(input.eta)),
        Operand::Node(id) => {
            let value = values
                .iter()
                .find_map(|(node_id, value)| (*node_id == id).then_some(value))
                .ok_or_else(|| internal("admitted node reference was not evaluated"))?;
            match value {
                RuntimeValue::Scalar(bits) => Ok(RuntimeRef::Scalar(*bits)),
                RuntimeValue::Vector(bits) => Ok(RuntimeRef::Vector(bits)),
            }
        }
    }
}

fn refuse_nonfinite(values: &[u32], base: &str) -> Result<(), Refusal> {
    for (lane, bits) in values.iter().copied().enumerate() {
        if let Some(kind) = classify_bits(bits) {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                format!("{base}[{lane}]"),
                format!("{} {}", nonfinite_name(kind), super::hash::bits_hex(bits)),
            ));
        }
    }
    Ok(())
}

fn nonfinite_name(value: NonFinite) -> &'static str {
    match value {
        NonFinite::Infinity => "infinity",
        NonFinite::NaN => "NaN",
    }
}

fn internal(observed: &str) -> Refusal {
    Refusal::new(RefusalClass::Internal, "$.envelope", observed)
}
