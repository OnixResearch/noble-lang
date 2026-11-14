use noble_ngraph::ir::{Graph, Literal};
use noble_ngraph::net::{Agent, Endpoint, InteractionGraph, Origin, PortIndex, PortRef, Symbol};
use noble_ngraph::sem::Value;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct LeanPort {
    id: Vec<u32>,
    port: PortIndex,
}

struct WireEntryCapacity(usize);

fn node_id(agent: &Agent) -> Result<Vec<u32>, String> {
    match (agent.origin, agent.symbol) {
        (Origin::Node(node), Symbol::Num(_)) => Ok(vec![0, 0, node.0]),
        (Origin::Node(node), Symbol::Op2(_)) => Ok(vec![0, 1, node.0]),
        (Origin::Dup { value, ordinal }, Symbol::Dup) => Ok(vec![0, 2, value.0, ordinal]),
        (Origin::Era(value), Symbol::Era) => Ok(vec![0, 3, value.0]),
        (origin, symbol) => Err(format!(
            "unexpected initial net origin/symbol: {origin:?}/{symbol:?}"
        )),
    }
}

fn lean_id(id: &[u32]) -> String {
    let parts = id.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
    format!("[{parts}]")
}

fn lean_port(id: Vec<u32>, port_index: PortIndex) -> LeanPort {
    LeanPort {
        id,
        port: port_index,
    }
}

fn endpoint_port(net: &InteractionGraph, endpoint: PortRef) -> Result<LeanPort, String> {
    let index = usize::try_from(endpoint.agent).map_err(|error| error.to_string())?;
    let agent = net
        .agents
        .get(index)
        .ok_or_else(|| format!("agent {} is outside the Rust net", endpoint.agent))?;
    Ok(lean_port(node_id(agent)?, endpoint.port_index))
}

fn output_port(slot: u32) -> LeanPort {
    lean_port(vec![9, 9, slot], PortIndex::from(0))
}

fn lean_symbol(symbol: Symbol) -> Result<String, String> {
    match symbol {
        Symbol::Num(bits) => Ok(format!(".num (BitVec.ofNat 64 {bits})")),
        Symbol::Op2(op) => Ok(format!(".op2 .{}", lean_op(op))),
        Symbol::Dup => Ok(".dup".to_owned()),
        Symbol::Era => Ok(".era".to_owned()),
        Symbol::Op1(..) => Err("an initial translated net contains an op1".to_owned()),
    }
}

fn lean_op(op: noble_ngraph::net::Op) -> &'static str {
    match op {
        noble_ngraph::net::Op::Add => "add",
        noble_ngraph::net::Op::Sub => "sub",
        noble_ngraph::net::Op::Mul => "mul",
        noble_ngraph::net::Op::Eq => "eq",
    }
}

fn wire_entry_capacity(net: &InteractionGraph) -> Option<WireEntryCapacity> {
    net.agents
        .iter()
        .try_fold(WireEntryCapacity(net.outputs.len()), |current, agent| {
            current
                .0
                .checked_add(agent.symbol.port_count())
                .map(WireEntryCapacity)
        })
}

fn agent_literals(net: &InteractionGraph) -> Result<Vec<(Vec<u32>, String)>, String> {
    let mut agents = net
        .agents
        .iter()
        .map(|agent| Ok((node_id(agent)?, lean_symbol(agent.symbol)?)))
        .collect::<Result<Vec<_>, String>>()?;
    agents.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    Ok(agents)
}

fn wire_pairs(net: &InteractionGraph) -> Result<Vec<(LeanPort, LeanPort)>, String> {
    // Every agent has at most three ports; one extra slot per output bounds the
    // symmetric representation of each free-interface wire. Its exact capacity
    // is the port total plus one duplicate endpoint per output.
    let mut wires = Vec::<(LeanPort, LeanPort)>::with_capacity(
        wire_entry_capacity(net)
            .ok_or_else(|| "net wire capacity overflows usize".to_owned())?
            .0,
    );
    for (index, agent) in net.agents.iter().enumerate() {
        let id = node_id(agent)?;
        for port_index in 0..agent.symbol.port_count() {
            let source = lean_port(
                id.clone(),
                PortIndex::from(u8::try_from(port_index).map_err(|error| error.to_string())?),
            );
            let endpoint = agent
                .ports
                .get(port_index)
                .and_then(Option::as_ref)
                .ok_or_else(|| format!("agent {index} port {port_index} has no endpoint"))?;
            match endpoint {
                Endpoint::Port(target) => wires.push((source, endpoint_port(net, *target)?)),
                Endpoint::Output(slot) => {
                    let target = output_port(*slot);
                    wires.push((source.clone(), target.clone()));
                    wires.push((target, source));
                }
            }
        }
    }
    wires.sort_unstable();
    Ok(wires)
}

fn render_agents(agents: &[(Vec<u32>, String)]) -> String {
    agents
        .iter()
        .map(|(id, symbol)| format!("({}, {symbol})", lean_id(id)))
        .collect::<Vec<_>>()
        .join(",\n    ")
}

fn render_free_ports(net: &InteractionGraph) -> Result<String, String> {
    let ports = (0..net.outputs.len())
        .map(|slot| {
            u32::try_from(slot)
                .map(output_port)
                .map(|port| format!("({}, {})", lean_id(&port.id), u8::from(port.port)))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(ports.join(", "))
}

fn render_wires(wires: &[(LeanPort, LeanPort)]) -> String {
    wires
        .iter()
        .map(|(left, right)| {
            format!(
                "(({}, {}), ({}, {}))",
                lean_id(&left.id),
                u8::from(left.port),
                lean_id(&right.id),
                u8::from(right.port)
            )
        })
        .collect::<Vec<_>>()
        .join(",\n    ")
}

pub(super) fn rust_net_literal(net: &InteractionGraph) -> Result<String, String> {
    let agents = render_agents(&agent_literals(net)?);
    let free = render_free_ports(net)?;
    let wires = render_wires(&wire_pairs(net)?);
    Ok(format!(
        "{{\n    agents := [{agents}],\n    free := [{free}],\n    wires := [{wires}]\n  }}"
    ))
}

pub(super) fn lean_nodes(graph: &Graph) -> Result<String, String> {
    graph
        .nodes
        .iter()
        .map(|node| match &node.op {
            noble_ngraph::ir::Op::Input(slot) => Ok(format!(".input {slot}")),
            noble_ngraph::ir::Op::Const(Literal::I64(value)) => {
                let bits = u64::from_le_bytes(value.to_le_bytes());
                Ok(format!(".constant (BitVec.ofNat 64 {bits})"))
            }
            noble_ngraph::ir::Op::I64Add(a, b) => Ok(format!(".add {} {}", a.0, b.0)),
            noble_ngraph::ir::Op::I64Sub(a, b) => Ok(format!(".sub {} {}", a.0, b.0)),
            noble_ngraph::ir::Op::I64Mul(a, b) => Ok(format!(".mul {} {}", a.0, b.0)),
            noble_ngraph::ir::Op::I64Eq(a, b) => Ok(format!(".eq {} {}", a.0, b.0)),
            other => Err(format!("unsupported source op in corpus: {other:?}")),
        })
        .collect::<Result<Vec<_>, String>>()
        .map(|nodes| nodes.join(", "))
}

pub(super) fn lean_value(value: Value) -> String {
    match value {
        Value::I64(value) => {
            let bits = u64::from_le_bytes(value.to_le_bytes());
            format!(".i64 (BitVec.ofNat 64 {bits})")
        }
        Value::Bool(value) => format!(".boolean {value}"),
        Value::F32(_) => panic!("I64 net returned an F32"),
    }
}
