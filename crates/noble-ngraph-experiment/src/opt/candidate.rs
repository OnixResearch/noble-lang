//! Strict `ngraph-opt/1` wire parser and static admission.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use serde_json::{Map, Value};

use super::hash::digest;
use super::types::Digest;
use super::types::{Refusal, RefusalClass};

const MAX_NODES: usize = 16;
const MAX_LANES: usize = 64;
const MAX_WORK: u64 = 128;
const INPUTS: [&str; 5] = ["p", "g", "eta", "step", "state"];

/// An admitted immutable candidate, bound to the exact submitted JSON bytes.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub(crate) program: Program,
    bytes: Arc<[u8]>,
    id: Digest,
}

#[derive(Clone, Debug)]
pub(crate) struct Program {
    pub(crate) lanes: usize,
    pub(crate) nodes: Vec<Node>,
    pub(crate) output: u16,
    pub(crate) work: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub(crate) id: u16,
    pub(crate) op: Operation,
    pub(crate) operands: Vec<Operand>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    Constant(u32),
    Broadcast,
    Mul,
    Sub,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operand {
    Node(u16),
    Parameters,
    Gradients,
    Eta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Type {
    Scalar,
    Vector,
}

impl Candidate {
    /// Parses and admits the exact `ngraph-opt/1` JSON envelope.
    ///
    /// The only accepted numeric references are exact `0x`-prefixed eight-hex-digit bit strings.
    /// Node references are integer `u16` IDs and inputs are the strings `$p`, `$g`, or `$eta`.
    ///
    /// # Errors
    /// The first malformed, unsupported or over-budget field, with its JSON path.
    pub fn parse(bytes: &[u8]) -> Result<Self, Refusal> {
        let value: Value = serde_json::from_slice(bytes).map_err(|error| {
            Refusal::new(
                RefusalClass::Malformed,
                "$.envelope",
                format!("invalid JSON: {error}"),
            )
        })?;
        let duplicate = duplicate_key(bytes).map_err(|()| {
            Refusal::new(
                RefusalClass::Malformed,
                "$.envelope",
                "could not validate JSON object keys",
            )
        })?;
        if let Some((path, key)) = duplicate {
            return Err(Refusal::new(
                RefusalClass::Malformed,
                path,
                format!("duplicate object key {key:?}"),
            ));
        }
        let root = object(&value, "$.envelope")?;

        let format = string(root, "format", "$")?;
        if format != "ngraph-opt/1" {
            return Err(Refusal::new(RefusalClass::Malformed, "$.format", format));
        }
        let lanes = usize_field(root, "n", "$")?;
        if lanes == 0 {
            return Err(Refusal::new(RefusalClass::Malformed, "$.n", "0"));
        }
        if lanes > MAX_LANES {
            return Err(Refusal::new(
                RefusalClass::Exhausted,
                "$.outputs",
                format!("{lanes} output lanes exceeds {MAX_LANES}"),
            ));
        }

        let inputs = array_field(root, "inputs", "$")?;
        for (index, input) in inputs.iter().enumerate() {
            let path = format!("$.inputs[{index}]");
            let name = input.as_str().ok_or_else(|| {
                Refusal::new(RefusalClass::Malformed, path.clone(), input.to_string())
            })?;
            if !INPUTS.contains(&name) {
                return Err(Refusal::new(RefusalClass::Unsupported, path, name));
            }
            if INPUTS.get(index).copied() != Some(name) {
                return Err(Refusal::new(RefusalClass::Malformed, path, name));
            }
        }
        if inputs.len() != INPUTS.len() {
            let index = inputs.len();
            let expected = INPUTS.get(index).copied().unwrap_or("no additional input");
            return Err(Refusal::new(
                RefusalClass::Malformed,
                format!("$.inputs[{index}]"),
                format!(
                    "missing declared input {expected}; observed {} inputs",
                    inputs.len()
                ),
            ));
        }
        let state = string(root, "state", "$")?;
        if state != "State0" {
            return Err(Refusal::new(RefusalClass::Unsupported, "$.state", state));
        }
        let effects = array_field(root, "effects", "$")?;
        if let Some(effect) = effects.first() {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                "$.effects[0]",
                effect.to_string(),
            ));
        }
        parse_budgets(root)?;

        let raw_nodes = array_field(root, "nodes", "$")?;
        if raw_nodes.len() > MAX_NODES {
            return Err(Refusal::new(
                RefusalClass::Exhausted,
                "$.nodes",
                format!("{} nodes exceeds {MAX_NODES}", raw_nodes.len()),
            ));
        }
        let mut nodes = Vec::with_capacity(raw_nodes.len());
        let mut seen: BTreeMap<u16, Type> = BTreeMap::new();
        let mut work = 0u64;
        for (index, raw_node) in raw_nodes.iter().enumerate() {
            let path = format!("$.nodes[{index}]");
            let node = parse_node(raw_node, &path, lanes, &seen)?;
            if matches!(node.op, Operation::Mul | Operation::Sub) {
                work = work
                    .checked_add(u64::try_from(lanes).unwrap_or(u64::MAX))
                    .ok_or_else(|| {
                        Refusal::new(RefusalClass::Exhausted, "$.work", "measured work overflow")
                    })?;
                if work > MAX_WORK {
                    return Err(Refusal::new(
                        RefusalClass::Exhausted,
                        "$.work",
                        format!("measured {work} scalar lane operations exceeds {MAX_WORK}"),
                    ));
                }
            }
            let node_type = match node.op {
                Operation::Constant(_) => Type::Scalar,
                Operation::Broadcast | Operation::Mul | Operation::Sub => Type::Vector,
            };
            seen.insert(node.id, node_type);
            nodes.push(node);
        }
        let outputs = object_field(root, "outputs", "$")?;
        let output = u16_field(outputs, "parameters", "$.outputs")?;
        let output_type = seen.get(&output).copied().ok_or_else(|| {
            Refusal::new(
                RefusalClass::Malformed,
                "$.outputs.parameters",
                format!("unknown node id {output}"),
            )
        })?;
        if output_type != Type::Vector {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                "$.outputs.parameters",
                "expected F32[n]",
            ));
        }
        let output_state = string(outputs, "state", "$.outputs")?;
        if output_state != "State0" {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                "$.outputs.state",
                output_state,
            ));
        }
        reject_unknown(outputs, &["parameters", "state"], "$.outputs")?;
        reject_unknown(
            root,
            &[
                "format", "n", "inputs", "state", "effects", "budgets", "nodes", "outputs",
            ],
            "$",
        )?;
        Ok(Self {
            program: Program {
                lanes,
                nodes,
                output,
                work,
            },
            bytes: Arc::from(bytes),
            id: digest(b"candidate-code", &[bytes]),
        })
    }

    /// Number of exact F32 lanes accepted by the program.
    #[must_use]
    pub const fn lanes(&self) -> usize {
        self.program.lanes
    }

    /// Identity of the exact submitted artifact bytes.
    #[must_use]
    pub const fn id(&self) -> Digest {
        self.id
    }

    /// Exact candidate bytes used for the identity and future replay.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn bytes_arc(&self) -> &Arc<[u8]> {
        &self.bytes
    }
}

fn parse_budgets(root: &Map<String, Value>) -> Result<(), Refusal> {
    let budgets = object_field(root, "budgets", "$")?;
    for (field, expected) in [
        ("nodes", 16),
        ("state_bytes", 0),
        ("work", MAX_WORK),
        ("output_lanes", 64),
    ] {
        let observed = u64_field(budgets, field, "$.budgets")?;
        if observed != expected {
            return Err(Refusal::new(
                RefusalClass::Malformed,
                format!("$.budgets.{field}"),
                observed.to_string(),
            ));
        }
    }
    reject_unknown(
        budgets,
        &["nodes", "state_bytes", "work", "output_lanes"],
        "$.budgets",
    )?;
    Ok(())
}

fn parse_node(
    value: &Value,
    path: &str,
    lanes: usize,
    seen: &BTreeMap<u16, Type>,
) -> Result<Node, Refusal> {
    let fields = object(value, path)?;
    let id = u16_field(fields, "id", path)?;
    if seen.contains_key(&id) {
        return Err(Refusal::new(
            RefusalClass::Malformed,
            format!("{path}.id"),
            id.to_string(),
        ));
    }
    let op_name = string(fields, "op", path)?;
    let (op, allowed_fields, expected_operands) = match op_name {
        "constant" => (None, &["id", "op", "operands", "type", "bits"][..], 0),
        "broadcast" => (
            Some(Operation::Broadcast),
            &["id", "op", "operands", "type"][..],
            1,
        ),
        "Binary32.mul" => (
            Some(Operation::Mul),
            &["id", "op", "operands", "type"][..],
            2,
        ),
        "Binary32.sub" => (
            Some(Operation::Sub),
            &["id", "op", "operands", "type"][..],
            2,
        ),
        other => {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                format!("{path}.op"),
                other,
            ));
        }
    };
    let operands_value = array_field(fields, "operands", path)?;
    if operands_value.len() != expected_operands {
        return Err(Refusal::new(
            RefusalClass::Malformed,
            format!("{path}.operands"),
            format!(
                "expected {expected_operands}, observed {}",
                operands_value.len()
            ),
        ));
    }
    let mut operands = Vec::with_capacity(operands_value.len());
    let mut operand_types = Vec::with_capacity(operands_value.len());
    for (operand_index, value) in operands_value.iter().enumerate() {
        let operand_path = format!("{path}.operands[{operand_index}]");
        let (operand, ty) = match value {
            Value::Number(number) => {
                let raw = number.as_u64().ok_or_else(|| {
                    Refusal::new(
                        RefusalClass::Malformed,
                        operand_path.clone(),
                        value.to_string(),
                    )
                })?;
                let node_id = u16::try_from(raw).map_err(|_| {
                    Refusal::new(
                        RefusalClass::Malformed,
                        operand_path.clone(),
                        raw.to_string(),
                    )
                })?;
                let ty = seen.get(&node_id).copied().ok_or_else(|| {
                    Refusal::new(
                        RefusalClass::Malformed,
                        operand_path.clone(),
                        format!("node id {node_id} is unknown or forward-referenced"),
                    )
                })?;
                (Operand::Node(node_id), ty)
            }
            Value::String(reference) => match reference.as_str() {
                "$p" => (Operand::Parameters, Type::Vector),
                "$g" => (Operand::Gradients, Type::Vector),
                "$eta" => (Operand::Eta, Type::Scalar),
                "$step" | "$state" => {
                    return Err(Refusal::new(
                        RefusalClass::Unsupported,
                        operand_path,
                        reference.clone(),
                    ));
                }
                _ => {
                    return Err(Refusal::new(
                        RefusalClass::Unsupported,
                        operand_path,
                        reference.clone(),
                    ));
                }
            },
            _ => {
                return Err(Refusal::new(
                    RefusalClass::Malformed,
                    operand_path,
                    value.to_string(),
                ));
            }
        };
        operands.push(operand);
        operand_types.push(ty);
    }
    let operation = match op {
        None => Operation::Constant(parse_bits(
            string(fields, "bits", path)?,
            &format!("{path}.bits"),
        )?),
        Some(operation) => operation,
    };
    let declared_type = string(fields, "type", path)?;
    let expected_type = match operation {
        Operation::Constant(_) => "F32",
        Operation::Broadcast | Operation::Mul | Operation::Sub => "F32[n]",
    };
    if declared_type != expected_type {
        return Err(Refusal::new(
            RefusalClass::Malformed,
            format!("{path}.type"),
            declared_type,
        ));
    }
    let operand_shape_ok = match operation {
        Operation::Constant(_) => true,
        Operation::Broadcast => operand_types.first() == Some(&Type::Scalar),
        Operation::Mul | Operation::Sub => {
            operand_types.len() == 2
                && operand_types.iter().all(|ty| *ty == Type::Vector)
                && lanes <= MAX_LANES
        }
    };
    if !operand_shape_ok {
        return Err(Refusal::new(
            RefusalClass::Unsupported,
            format!("{path}.operands"),
            "operand shape does not match the opcode",
        ));
    }
    reject_unknown(fields, allowed_fields, path)?;
    Ok(Node {
        id,
        op: operation,
        operands,
    })
}

fn parse_bits(text: &str, path: &str) -> Result<u32, Refusal> {
    let digits = text
        .strip_prefix("0x")
        .ok_or_else(|| Refusal::new(RefusalClass::Malformed, path, text))?;
    if digits.len() != 8 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Refusal::new(RefusalClass::Malformed, path, text));
    }
    u32::from_str_radix(digits, 16).map_err(|_| Refusal::new(RefusalClass::Malformed, path, text))
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, Refusal> {
    value
        .as_object()
        .ok_or_else(|| Refusal::new(RefusalClass::Malformed, path, value.to_string()))
}

fn object_field<'a>(
    fields: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Map<String, Value>, Refusal> {
    let value = fields
        .get(key)
        .ok_or_else(|| Refusal::new(RefusalClass::Malformed, format!("{path}.{key}"), "missing"))?;
    object(value, &format!("{path}.{key}"))
}

fn array_field<'a>(
    fields: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Vec<Value>, Refusal> {
    let value = fields
        .get(key)
        .ok_or_else(|| Refusal::new(RefusalClass::Malformed, format!("{path}.{key}"), "missing"))?;
    value.as_array().ok_or_else(|| {
        Refusal::new(
            RefusalClass::Malformed,
            format!("{path}.{key}"),
            value.to_string(),
        )
    })
}

fn string<'a>(fields: &'a Map<String, Value>, key: &str, path: &str) -> Result<&'a str, Refusal> {
    let value = fields
        .get(key)
        .ok_or_else(|| Refusal::new(RefusalClass::Malformed, format!("{path}.{key}"), "missing"))?;
    value.as_str().ok_or_else(|| {
        Refusal::new(
            RefusalClass::Malformed,
            format!("{path}.{key}"),
            value.to_string(),
        )
    })
}

fn u64_field(fields: &Map<String, Value>, key: &str, path: &str) -> Result<u64, Refusal> {
    let value = fields
        .get(key)
        .ok_or_else(|| Refusal::new(RefusalClass::Malformed, format!("{path}.{key}"), "missing"))?;
    value.as_u64().ok_or_else(|| {
        Refusal::new(
            RefusalClass::Malformed,
            format!("{path}.{key}"),
            value.to_string(),
        )
    })
}

fn usize_field(fields: &Map<String, Value>, key: &str, path: &str) -> Result<usize, Refusal> {
    let raw = u64_field(fields, key, path)?;
    usize::try_from(raw).map_err(|_| {
        Refusal::new(
            RefusalClass::Malformed,
            format!("{path}.{key}"),
            raw.to_string(),
        )
    })
}

fn u16_field(fields: &Map<String, Value>, key: &str, path: &str) -> Result<u16, Refusal> {
    let raw = u64_field(fields, key, path)?;
    u16::try_from(raw).map_err(|_| {
        Refusal::new(
            RefusalClass::Malformed,
            format!("{path}.{key}"),
            raw.to_string(),
        )
    })
}

fn reject_unknown(
    fields: &Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), Refusal> {
    if let Some(key) = fields.keys().find(|key| !allowed.contains(&key.as_str())) {
        let class = match key.as_str() {
            "effect" | "effects" | "capability" | "capabilities" | "handle" | "host"
            | "host_call" | "memory" | "io" | "callback" | "filesystem" | "network" => {
                RefusalClass::Unsupported
            }
            _ => RefusalClass::Malformed,
        };
        return Err(Refusal::new(
            class,
            format!("{path}.{key}"),
            fields.get(key).map_or_else(String::new, Value::to_string),
        ));
    }
    Ok(())
}

fn duplicate_key(bytes: &[u8]) -> Result<Option<(String, String)>, ()> {
    let mut cursor = 0usize;
    scan_value(bytes, &mut cursor, "$", 0)
}

fn scan_value(
    bytes: &[u8],
    cursor: &mut usize,
    path: &str,
    depth: usize,
) -> Result<Option<(String, String)>, ()> {
    if depth > 128 {
        return Err(());
    }
    skip_whitespace(bytes, cursor);
    match bytes.get(*cursor).copied() {
        Some(b'{') => scan_object(bytes, cursor, path, depth + 1),
        Some(b'[') => scan_array(bytes, cursor, path, depth + 1),
        Some(b'"') => {
            scan_string(bytes, cursor)?;
            Ok(None)
        }
        Some(_) => {
            while bytes.get(*cursor).is_some_and(|byte| {
                !matches!(byte, b',' | b']' | b'}' | b' ' | b'\n' | b'\r' | b'\t')
            }) {
                *cursor = cursor.checked_add(1).ok_or(())?;
            }
            Ok(None)
        }
        None => Err(()),
    }
}

fn scan_object(
    bytes: &[u8],
    cursor: &mut usize,
    path: &str,
    depth: usize,
) -> Result<Option<(String, String)>, ()> {
    *cursor = cursor.checked_add(1).ok_or(())?;
    let mut keys = HashSet::new();
    loop {
        skip_whitespace(bytes, cursor);
        if bytes.get(*cursor) == Some(&b'}') {
            *cursor = cursor.checked_add(1).ok_or(())?;
            return Ok(None);
        }
        let start = *cursor;
        scan_string(bytes, cursor)?;
        let key: String = serde_json::from_slice(&bytes[start..*cursor]).map_err(|_| ())?;
        if !keys.insert(key.clone()) {
            return Ok(Some((format!("{path}.{key}"), key)));
        }
        skip_whitespace(bytes, cursor);
        *cursor = cursor.checked_add(1).ok_or(())?;
        let child_path = format!("{path}.{key}");
        if let Some(duplicate) = scan_value(bytes, cursor, &child_path, depth)? {
            return Ok(Some(duplicate));
        }
        skip_whitespace(bytes, cursor);
        if bytes.get(*cursor) == Some(&b',') {
            *cursor = cursor.checked_add(1).ok_or(())?;
        } else {
            *cursor = cursor.checked_add(1).ok_or(())?;
            return Ok(None);
        }
    }
}

fn scan_array(
    bytes: &[u8],
    cursor: &mut usize,
    path: &str,
    depth: usize,
) -> Result<Option<(String, String)>, ()> {
    *cursor = cursor.checked_add(1).ok_or(())?;
    let mut index = 0usize;
    loop {
        skip_whitespace(bytes, cursor);
        if bytes.get(*cursor) == Some(&b']') {
            *cursor = cursor.checked_add(1).ok_or(())?;
            return Ok(None);
        }
        if let Some(duplicate) = scan_value(bytes, cursor, &format!("{path}[{index}]"), depth)? {
            return Ok(Some(duplicate));
        }
        index = index.checked_add(1).ok_or(())?;
        skip_whitespace(bytes, cursor);
        if bytes.get(*cursor) == Some(&b',') {
            *cursor = cursor.checked_add(1).ok_or(())?;
        } else {
            *cursor = cursor.checked_add(1).ok_or(())?;
            return Ok(None);
        }
    }
}

fn scan_string(bytes: &[u8], cursor: &mut usize) -> Result<(), ()> {
    if bytes.get(*cursor) != Some(&b'"') {
        return Err(());
    }
    *cursor = cursor.checked_add(1).ok_or(())?;
    loop {
        match bytes.get(*cursor).copied() {
            Some(b'\\') => *cursor = cursor.checked_add(2).ok_or(())?,
            Some(b'"') => {
                *cursor = cursor.checked_add(1).ok_or(())?;
                return Ok(());
            }
            Some(_) => *cursor = cursor.checked_add(1).ok_or(())?,
            None => return Err(()),
        }
    }
}

fn skip_whitespace(bytes: &[u8], cursor: &mut usize) {
    while bytes
        .get(*cursor)
        .is_some_and(|byte| matches!(byte, b' ' | b'\n' | b'\r' | b'\t'))
    {
        if let Some(next) = cursor.checked_add(1) {
            *cursor = next;
        } else {
            break;
        }
    }
}
