//! Strict lowering for the external Nickel `noble-ngraph-nickel/v1` data
//! envelope. Evaluation stays in the pinned Node lane; this module only
//! translates exported records and invokes the independent native checker.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

const SCHEMA: &str = "noble-ngraph-nickel/v1";
const LIBRARY_VERSION: &str = "noble-ngraph-nickel/1";
const CONTRACT_VERSION: &str = "noble-ngraph-nickel-contracts/1";
pub(super) const LOWERER_REVISION: &str = "noble-ngraph-nickel-lowerer/1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: String,
    library_version: String,
    contract_version: String,
    profile: String,
    bundle_sha256: String,
    parameters: Vec<Parameter>,
    instances: Vec<Instance>,
    wires: Vec<Wire>,
    inputs: Vec<Handle>,
    outputs: Vec<Handle>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameter {
    name: String,
    value: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Instance {
    scope: String,
    template: String,
    inputs: Vec<String>,
    outputs: Vec<String>,
    nodes: Vec<Node>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Node {
    id: String,
    op: Operation,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    Input,
    Const { value: String },
    Add { left: String, right: String },
    Sub { left: String, right: String },
    Mul { left: String, right: String },
}

#[derive(Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct Handle {
    scope: String,
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    from: Handle,
    to: Handle,
}

type Key = (String, String);

fn key(scope: &str, id: &str) -> Key {
    (scope.to_owned(), id.to_owned())
}

fn malformed(detail: impl Into<String>) -> noble_ngraph::Diagnostic {
    noble_ngraph::Diagnostic::malformed(
        noble_ngraph::Malformed::Syntax,
        noble_ngraph::Site::Graph,
        detail,
    )
}

fn budget(kind: noble_ngraph::BudgetKind, limit: u64, observed: u64) -> noble_ngraph::Diagnostic {
    noble_ngraph::Diagnostic::budget(kind, limit, observed)
}

fn digest(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let mut out = [0; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let part = std::str::from_utf8(pair).ok()?;
        out[index] = u8::from_str_radix(part, 16).ok()?;
    }
    Some(out)
}

fn exact_i64(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    let digits = if bytes.first() == Some(&b'-') {
        bytes.get(1..)?
    } else {
        bytes
    };
    if digits.is_empty()
        || !digits.iter().all(u8::is_ascii_digit)
        || (digits.len() > 1 && digits[0] == b'0')
        || (bytes.first() == Some(&b'-') && digits == b"0")
    {
        return None;
    }
    let value = text.parse::<i64>().ok()?;
    (value.to_string() == text).then_some(value)
}

fn port(
    instance: &Instance,
    local_id: &str,
    input: bool,
    nodes: &BTreeMap<Key, (usize, usize)>,
) -> Result<Key, noble_ngraph::Diagnostic> {
    let id = key(&instance.scope, local_id);
    let Some(&(_instance_index, node_index)) = nodes.get(&id) else {
        return Err(malformed(format!("port {}:{} names no node", id.0, id.1)));
    };
    let matches = matches!(&instance.nodes[node_index].op, Operation::Input);
    if input && !matches {
        return Err(malformed(format!(
            "port {}:{} has the wrong node kind",
            id.0, id.1
        )));
    }
    Ok(id)
}

fn has_cycle(edges: &[Vec<usize>]) -> bool {
    let mut colors = vec![0u8; edges.len()];
    for start in 0..edges.len() {
        if colors[start] != 0 {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        colors[start] = 1;
        while let Some((vertex, next)) = stack.last_mut() {
            if let Some(&target) = edges[*vertex].get(*next) {
                *next += 1;
                match colors[target] {
                    0 => {
                        colors[target] = 1;
                        stack.push((target, 0));
                    }
                    1 => return true,
                    _ => {}
                }
            } else {
                colors[*vertex] = 2;
                stack.pop();
            }
        }
    }
    false
}

fn work_units(envelope: &Envelope) -> u64 {
    let nodes = envelope
        .instances
        .iter()
        .map(|instance| instance.nodes.len() as u64)
        .fold(0u64, u64::saturating_add);
    let ports = envelope
        .instances
        .iter()
        .map(|instance| instance.inputs.len().saturating_add(instance.outputs.len()) as u64)
        .fold(0u64, u64::saturating_add);
    (envelope.parameters.len() as u64)
        .saturating_add(envelope.instances.len() as u64)
        .saturating_add(nodes)
        .saturating_add(ports)
        .saturating_add(envelope.wires.len() as u64)
        .saturating_add(envelope.inputs.len() as u64)
        .saturating_add(envelope.outputs.len() as u64)
}

/// Lowers constructor data without simplifying it; `check_with` remains the
/// only route from producer data to an admitted graph.
pub(super) fn lower(
    bytes: &[u8],
    budgets: &noble_ngraph::Budgets,
) -> Result<noble_ngraph::Graph, noble_ngraph::Diagnostic> {
    if bytes.len() as u64 > budgets.graph_bytes {
        return Err(budget(
            noble_ngraph::BudgetKind::GraphBytes,
            budgets.graph_bytes,
            bytes.len() as u64,
        ));
    }
    let envelope: Envelope = serde_json::from_slice(bytes)
        .map_err(|error| malformed(format!("invalid Nickel constructor JSON: {error}")))?;
    if envelope.schema != SCHEMA
        || envelope.library_version != LIBRARY_VERSION
        || envelope.contract_version != CONTRACT_VERSION
    {
        return Err(malformed(
            "unsupported constructor schema, library, or contract version",
        ));
    }
    if envelope.profile != "ngraph-i64/0" {
        return Err(noble_ngraph::Diagnostic::new(
            noble_ngraph::Code::Profile,
            noble_ngraph::Site::Graph,
            "authored-nickel admits only ngraph-i64/0",
        ));
    }
    let bundle_digest = digest(&envelope.bundle_sha256)
        .ok_or_else(|| malformed("bundle_sha256 must be 64 lowercase hex digits"))?;
    if envelope.inputs.len() as u64 > budgets.inputs {
        return Err(budget(
            noble_ngraph::BudgetKind::Inputs,
            budgets.inputs,
            envelope.inputs.len() as u64,
        ));
    }
    if envelope.outputs.len() as u64 > budgets.outputs {
        return Err(budget(
            noble_ngraph::BudgetKind::Outputs,
            budgets.outputs,
            envelope.outputs.len() as u64,
        ));
    }
    let work = work_units(&envelope);
    if work > budgets.work_units {
        return Err(budget(
            noble_ngraph::BudgetKind::Work,
            budgets.work_units,
            work,
        ));
    }
    let value_nodes = envelope
        .instances
        .iter()
        .flat_map(|instance| &instance.nodes)
        .filter(|node| !matches!(&node.op, Operation::Input))
        .count() as u64;
    let graph_nodes = value_nodes.saturating_add(envelope.inputs.len() as u64);
    if graph_nodes > budgets.expanded_nodes {
        return Err(budget(
            noble_ngraph::BudgetKind::Expansion,
            budgets.expanded_nodes,
            graph_nodes,
        ));
    }
    if graph_nodes > budgets.live_nodes {
        return Err(budget(
            noble_ngraph::BudgetKind::LiveNodes,
            budgets.live_nodes,
            graph_nodes,
        ));
    }
    if let Some(parameter) = envelope.parameters.iter().find(|p| p.name.is_empty()) {
        return Err(malformed(format!(
            "construction parameter name is empty: {:?}",
            parameter.value
        )));
    }
    let mut parameter_names = BTreeSet::new();
    if envelope
        .parameters
        .iter()
        .any(|parameter| !parameter_names.insert(parameter.name.as_str()))
    {
        return Err(malformed("construction parameter names must be unique"));
    }

    let mut scopes = BTreeMap::new();
    let mut nodes = BTreeMap::new();
    for (instance_index, instance) in envelope.instances.iter().enumerate() {
        if instance.scope.is_empty() || instance.template.is_empty() {
            return Err(malformed(
                "instance scope and template names must be nonempty",
            ));
        }
        if scopes
            .insert(instance.scope.as_str(), instance_index)
            .is_some()
        {
            return Err(malformed(format!(
                "duplicate instance scope {:?}",
                instance.scope
            )));
        }
        for (node_index, node) in instance.nodes.iter().enumerate() {
            if node.id.is_empty() {
                return Err(malformed("node handle id must be nonempty"));
            }
            if nodes
                .insert(key(&instance.scope, &node.id), (instance_index, node_index))
                .is_some()
            {
                return Err(malformed(format!(
                    "duplicate scoped node id {}:{}",
                    instance.scope, node.id
                )));
            }
        }
    }

    let mut input_ports = BTreeSet::new();
    let mut output_ports = BTreeSet::new();
    for instance in &envelope.instances {
        let mut local_inputs = BTreeSet::new();
        for id in &instance.inputs {
            let handle = port(instance, id, true, &nodes)?;
            if !local_inputs.insert(handle.clone()) || !input_ports.insert(handle) {
                return Err(malformed("an input handle is declared more than once"));
            }
        }
        let declared_inputs: BTreeSet<_> = instance
            .nodes
            .iter()
            .filter(|node| matches!(&node.op, Operation::Input))
            .map(|node| key(&instance.scope, &node.id))
            .collect();
        if local_inputs != declared_inputs {
            return Err(malformed(
                "every input node must be declared exactly once as an instance input",
            ));
        }
        let mut local_outputs = BTreeSet::new();
        for id in &instance.outputs {
            let handle = port(instance, id, false, &nodes)?;
            if !local_outputs.insert(handle.clone()) || !output_ports.insert(handle) {
                return Err(malformed("an output handle is declared more than once"));
            }
        }
    }

    let mut wire_targets = BTreeMap::new();
    let mut instance_edges = vec![Vec::new(); envelope.instances.len()];
    for wire in &envelope.wires {
        let from = key(&wire.from.scope, &wire.from.id);
        let to = key(&wire.to.scope, &wire.to.id);
        if !output_ports.contains(&from) || !input_ports.contains(&to) {
            return Err(malformed(
                "wire endpoints must be declared output and input handles",
            ));
        }
        let source_instance = scopes[wire.from.scope.as_str()];
        let target_instance = scopes[wire.to.scope.as_str()];
        instance_edges[source_instance].push(target_instance);
        if wire_targets.insert(to, from).is_some() {
            return Err(malformed("an input handle has more than one wire"));
        }
    }
    if has_cycle(&instance_edges) {
        return Err(noble_ngraph::Diagnostic::malformed(
            noble_ngraph::Malformed::Cycle,
            noble_ngraph::Site::Graph,
            "instance wiring contains a cycle",
        ));
    }
    for wire in &envelope.wires {
        let source_instance = scopes[wire.from.scope.as_str()];
        let target_instance = scopes[wire.to.scope.as_str()];
        if source_instance >= target_instance {
            return Err(noble_ngraph::Diagnostic::malformed(
                noble_ngraph::Malformed::ForwardReference,
                noble_ngraph::Site::Graph,
                "instances must be listed in wiring dependency order",
            ));
        }
    }

    let mut value_ids = BTreeMap::new();
    let mut next = u32::try_from(envelope.inputs.len()).unwrap_or(u32::MAX);
    for instance in &envelope.instances {
        for node in &instance.nodes {
            if !matches!(&node.op, Operation::Input) {
                value_ids.insert(key(&instance.scope, &node.id), noble_ngraph::NodeId(next));
                next = next.checked_add(1).ok_or_else(|| {
                    budget(
                        noble_ngraph::BudgetKind::LiveNodes,
                        budgets.live_nodes,
                        u64::MAX,
                    )
                })?;
            }
        }
    }

    let mut input_values = BTreeMap::new();
    for (slot, handle) in envelope.inputs.iter().enumerate() {
        let id = key(&handle.scope, &handle.id);
        if !input_ports.contains(&id)
            || input_values.contains_key(&id)
            || wire_targets.contains_key(&id)
        {
            return Err(malformed(
                "exposed inputs must name distinct, unwired instance input handles",
            ));
        }
        input_values.insert(
            id,
            noble_ngraph::NodeId(u32::try_from(slot).unwrap_or(u32::MAX)),
        );
    }
    let mut ordered_wires: Vec<_> = envelope.wires.iter().collect();
    ordered_wires.sort_by_key(|wire| scopes[wire.to.scope.as_str()]);
    for wire in ordered_wires {
        let source = key(&wire.from.scope, &wire.from.id);
        let target = key(&wire.to.scope, &wire.to.id);
        let source_id = input_values
            .get(&source)
            .or_else(|| value_ids.get(&source))
            .copied()
            .ok_or_else(|| malformed("wire source has no lowered graph node"))?;
        input_values.insert(target, source_id);
    }
    if input_values.len() != input_ports.len() {
        return Err(malformed(
            "every instance input must be exposed or explicitly wired",
        ));
    }

    let mut builder = noble_ngraph::ir::GraphBuilder::new(
        noble_ngraph::SubProfile::I64,
        noble_ngraph::ir::Subject::authored_nickel(bundle_digest),
    );
    for _ in &envelope.inputs {
        builder.input(noble_ngraph::ScalarType::I64);
    }
    let resolve_local = |scope: &str, id: &str| {
        let local = key(scope, id);
        input_values
            .get(&local)
            .or_else(|| value_ids.get(&local))
            .copied()
            .ok_or_else(|| {
                noble_ngraph::Diagnostic::malformed(
                    noble_ngraph::Malformed::UnknownNode,
                    noble_ngraph::Site::Graph,
                    format!("node reference {scope}:{id} does not exist"),
                )
            })
    };
    for instance in &envelope.instances {
        for node in &instance.nodes {
            let op = match &node.op {
                Operation::Input => continue,
                Operation::Const { value } => {
                    let constant = exact_i64(value).ok_or_else(|| {
                        malformed(format!(
                            "constant {value:?} is not canonical signed-I64 decimal"
                        ))
                    })?;
                    noble_ngraph::Op::Const(noble_ngraph::ir::Literal::I64(constant))
                }
                Operation::Add { left, right } => noble_ngraph::Op::I64Add(
                    resolve_local(&instance.scope, left)?,
                    resolve_local(&instance.scope, right)?,
                ),
                Operation::Sub { left, right } => noble_ngraph::Op::I64Sub(
                    resolve_local(&instance.scope, left)?,
                    resolve_local(&instance.scope, right)?,
                ),
                Operation::Mul { left, right } => noble_ngraph::Op::I64Mul(
                    resolve_local(&instance.scope, left)?,
                    resolve_local(&instance.scope, right)?,
                ),
            };
            builder.op(op);
        }
    }
    for handle in &envelope.outputs {
        let id = key(&handle.scope, &handle.id);
        if !output_ports.contains(&id) {
            return Err(malformed(
                "outputs must name declared instance output handles",
            ));
        }
        let node = input_values
            .get(&id)
            .or_else(|| value_ids.get(&id))
            .copied()
            .ok_or_else(|| malformed("output handle has no lowered graph node"))?;
        builder.output(node);
    }
    Ok(builder.finish())
}

/// Parses and lowers the envelope, then admits it through Noble's checker.
pub(super) fn admit(
    bytes: &[u8],
    budgets: &noble_ngraph::Budgets,
) -> Result<noble_ngraph::CheckedGraph, noble_ngraph::Diagnostic> {
    let graph = lower(bytes, budgets)?;
    noble_ngraph::check_with(&super::IDS, graph, budgets)
}
