use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use noble_ngraph::check;
use noble_ngraph::eval::{self, Control};
use noble_ngraph::ir::{
    Graph, GraphBuilder, Literal, Op as GraphOp, ScalarType, SubProfile, Subject,
};
use noble_ngraph::net::{self, Agent, Endpoint, Net, Op as NetOp, Origin, PortRef, Symbol};
use noble_ngraph::sem::Value;
use noble_ngraph_experiment::scheme::BLAKE3;

#[derive(Clone, Debug)]
struct Fixture {
    name: &'static str,
    graph: Graph,
    inputs: Vec<Value>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LeanPort {
    id: Vec<u32>,
    port: u8,
}

fn full_fixture() -> Fixture {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&BLAKE3, "net-i64-corpus/fanout-overflow"),
    );
    let left = builder.input(ScalarType::I64);
    let right = builder.input(ScalarType::I64);
    let _discarded = builder.input(ScalarType::I64);
    let three = builder.i64_const(3);
    let sum = builder.op(GraphOp::I64Add(left, three));
    let difference = builder.op(GraphOp::I64Sub(right, left));
    let product = builder.op(GraphOp::I64Mul(sum, difference));
    let equal = builder.op(GraphOp::I64Eq(left, right));
    builder.output(product);
    builder.output(equal);
    builder.output(product);
    Fixture {
        name: "fanout_overflow",
        graph: builder.finish(),
        inputs: vec![Value::I64(i64::MAX), Value::I64(-1), Value::I64(17)],
    }
}

fn discarded_fixture() -> Fixture {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&BLAKE3, "net-i64-corpus/discarded-unit"),
    );
    let _discarded = builder.input(ScalarType::I64);
    Fixture {
        name: "discarded_unit",
        graph: builder.finish(),
        inputs: vec![Value::I64(i64::MIN)],
    }
}

fn empty_fixture() -> Fixture {
    let builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&BLAKE3, "net-i64-corpus/empty"),
    );
    Fixture {
        name: "empty",
        graph: builder.finish(),
        inputs: Vec::new(),
    }
}

fn fixtures() -> [Fixture; 3] {
    [full_fixture(), discarded_fixture(), empty_fixture()]
}

fn checked_and_net(fixture: &Fixture) -> (noble_ngraph::CheckedGraph, Net) {
    let checked = check::check(&BLAKE3, fixture.graph.clone()).expect("corpus graph checks");
    let limits = net::Limits::default();
    let candidate = net::lower(&BLAKE3, &checked, &fixture.inputs, &limits)
        .expect("checked corpus graph lowers");
    assert_eq!(
        net::validate(&BLAKE3, &checked, &candidate, &fixture.inputs, &limits),
        Ok(()),
        "independent validator admits {}",
        fixture.name
    );
    (checked, candidate)
}

fn rust_result(
    fixture: &Fixture,
    checked: &noble_ngraph::CheckedGraph,
    candidate: &Net,
) -> (Vec<Value>, u64) {
    let mut control = Control::default();
    let reference = eval::reference(checked, &fixture.inputs, None, &mut control);
    let reference_outputs = match reference.obs {
        eval::Obs::Normal(outputs) => outputs,
        other => panic!("{} reference refused: {other:?}", fixture.name),
    };
    let limits = net::Limits::default();
    let plan = net::Plan {
        schedule: Some(net::Schedule::Fifo),
        ..net::Plan::default()
    };
    let report = net::reduce(&BLAKE3, checked, candidate, &fixture.inputs, &limits, &plan);
    let net_outputs = match report.obs {
        net::Obs::Normal(outputs) => outputs,
        other => panic!("{} net refused: {other:?}", fixture.name),
    };
    assert_eq!(
        net_outputs, reference_outputs,
        "{} Rust differential",
        fixture.name
    );
    let expected = candidate
        .expected_interactions()
        .expect("interaction count fits the contract counter");
    assert_eq!(report.interactions, expected, "{} Rust count", fixture.name);
    (net_outputs, report.interactions)
}

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

fn lean_port(id: Vec<u32>, port: u8) -> LeanPort {
    LeanPort { id, port }
}

fn endpoint_port(net: &Net, endpoint: PortRef) -> Result<LeanPort, String> {
    let index = usize::try_from(endpoint.agent).map_err(|error| error.to_string())?;
    let agent = net
        .agents
        .get(index)
        .ok_or_else(|| format!("agent {} is outside the Rust net", endpoint.agent))?;
    Ok(lean_port(node_id(agent)?, endpoint.port))
}

fn output_port(slot: u32) -> LeanPort {
    lean_port(vec![9, 9, slot], 0)
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

fn lean_op(op: NetOp) -> &'static str {
    match op {
        NetOp::Add => "add",
        NetOp::Sub => "sub",
        NetOp::Mul => "mul",
        NetOp::Eq => "eq",
    }
}

fn rust_net_literal(net: &Net) -> Result<String, String> {
    let mut agents = net
        .agents
        .iter()
        .map(|agent| Ok((node_id(agent)?, lean_symbol(agent.symbol)?)))
        .collect::<Result<Vec<_>, String>>()?;
    agents.sort_unstable_by(|left, right| left.0.cmp(&right.0));

    let mut wires = Vec::<(LeanPort, LeanPort)>::new();
    for (index, agent) in net.agents.iter().enumerate() {
        let id = node_id(agent)?;
        for port_index in 0..agent.symbol.port_count() {
            let source = lean_port(
                id.clone(),
                u8::try_from(port_index).map_err(|error| error.to_string())?,
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

    let agents = agents
        .iter()
        .map(|(id, symbol)| format!("({}, {symbol})", lean_id(id)))
        .collect::<Vec<_>>()
        .join(",\n    ");
    let free = (0..net.outputs.len())
        .map(|slot| {
            u32::try_from(slot)
                .map(output_port)
                .map(|port| format!("({}, {})", lean_id(&port.id), port.port))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, String>>()?
        .join(", ");
    let wires = wires
        .iter()
        .map(|(left, right)| {
            format!(
                "(({}, {}), ({}, {}))",
                lean_id(&left.id),
                left.port,
                lean_id(&right.id),
                right.port
            )
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    Ok(format!(
        "{{\n    agents := [{agents}],\n    free := [{free}],\n    wires := [{wires}]\n  }}"
    ))
}

fn lean_nodes(graph: &Graph) -> Result<String, String> {
    graph
        .nodes
        .iter()
        .map(|node| match &node.op {
            GraphOp::Input(slot) => Ok(format!(".input {slot}")),
            GraphOp::Const(Literal::I64(value)) => {
                let bits = u64::from_le_bytes(value.to_le_bytes());
                Ok(format!(".constant (BitVec.ofNat 64 {bits})"))
            }
            GraphOp::I64Add(a, b) => Ok(format!(".add {} {}", a.0, b.0)),
            GraphOp::I64Sub(a, b) => Ok(format!(".sub {} {}", a.0, b.0)),
            GraphOp::I64Mul(a, b) => Ok(format!(".mul {} {}", a.0, b.0)),
            GraphOp::I64Eq(a, b) => Ok(format!(".eq {} {}", a.0, b.0)),
            other => Err(format!("unsupported source op in corpus: {other:?}")),
        })
        .collect::<Result<Vec<_>, String>>()
        .map(|nodes| nodes.join(", "))
}

fn lean_value(value: Value) -> String {
    match value {
        Value::I64(value) => {
            let bits = u64::from_le_bytes(value.to_le_bytes());
            format!(".i64 (BitVec.ofNat 64 {bits})")
        }
        Value::Bool(value) => format!(".boolean {value}"),
        Value::F32(_) => panic!("I64 net returned an F32"),
    }
}

fn lean_corpus_preamble() -> &'static str {
    r#"import InteractionNets
import NobleFoundation.NetI64
import NobleFoundation.PackedSchedule

namespace NobleFoundation.NetI64Corpus

open NobleFoundation.NetI64
open NobleFoundation.PackedSchedule

private def compareId : List Nat → List Nat → Ordering
  | [], [] => .eq
  | [], _ :: _ => .lt
  | _ :: _, [] => .gt
  | left :: leftTail, right :: rightTail =>
      match compare left right with
      | .eq => compareId leftTail rightTail
      | order => order

private def comparePort (left right : INet.Port) : Ordering :=
  match compareId left.1 right.1 with
  | .eq => compare left.2 right.2
  | order => order

private def compareWire (left right : INet.Port × INet.Port) : Ordering :=
  match comparePort left.1 right.1 with
  | .eq => comparePort left.2 right.2
  | order => order

private def insertSorted {α : Type} (less : α → α → Bool) (value : α) : List α → List α
  | [] => [value]
  | head :: rest =>
      if less value head then value :: head :: rest
      else head :: insertSorted less value rest

private def sortSorted {α : Type} (less : α → α → Bool) : List α → List α
  | [] => []
  | head :: rest => insertSorted less head (sortSorted less rest)

private def canonicalNet (net : INet.XNet NetSym) : INet.XNet NetSym :=
  { agents := sortSorted (fun left right => compareId left.1 right.1 == .lt) net.agents
    free := sortSorted (fun left right => comparePort left right == .lt) net.free
    wires := sortSorted (fun left right => compareWire left right == .lt) net.wires }

private def normalizeReadBack (program : Program) (inputs : List (BitVec 64)) :=
  (Program.normalize program inputs).map fun (net, steps) =>
    (Program.readBack program net, steps)

-- Rust-produced input nets, differential outputs and interaction counts follow.

"#
}

fn corpus_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../proofs/foundation/NobleFoundation/NetI64Corpus.lean")
}

#[test]
fn generated_lean_corpus_is_fresh() {
    let path = corpus_path();
    let retained = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    assert_eq!(
        retained,
        render_corpus(),
        "regenerate the NetI64 Rust corpus"
    );
}

#[test]
#[ignore = "writes the kernel-checked corpus from freshly lowered Rust nets"]
fn regenerate_lean_corpus() {
    let path = corpus_path();
    std::fs::write(&path, render_corpus())
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

fn render_corpus() -> String {
    let mut source = String::from(lean_corpus_preamble());
    for fixture in fixtures() {
        let (checked, net) = checked_and_net(&fixture);
        let (outputs, interactions) = rust_result(&fixture, &checked, &net);
        let nodes = lean_nodes(checked.graph()).expect("whitelisted graph serializes");
        let inputs = fixture
            .inputs
            .iter()
            .map(|value| match value {
                Value::I64(value) => format!(
                    "BitVec.ofNat 64 {}",
                    u64::from_le_bytes(value.to_le_bytes())
                ),
                other => panic!("non-I64 corpus input: {other:?}"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let outputs = outputs
            .into_iter()
            .map(lean_value)
            .collect::<Vec<_>>()
            .join(", ");
        let net_literal = rust_net_literal(&net).expect("Rust net serializes");
        let output_nodes = checked
            .graph()
            .outputs
            .iter()
            .map(|node| node.0.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let input_arity = checked.graph().slots.len();
        let graph_id = noble_ngraph::ir::hex(&checked.id());
        let build_key = noble_ngraph::ir::hex(&net.build_key);
        writeln!(
            source,
            "-- graph={graph_id}; contract={}; lowering={}; build_key={build_key}; inputs=[{}]",
            net.contract_revision,
            net.lowering_revision,
            net.input_values
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
        .expect("writing to String");
        writeln!(
            source,
            "def {}Program : Program := {{ inputArity := {input_arity}, nodes := [{nodes}], outputs := [{output_nodes}] }}",
            fixture.name
        )
        .expect("writing to String");
        writeln!(
            source,
            "def {}Inputs : List (BitVec 64) := [{inputs}]",
            fixture.name
        )
        .expect("writing to String");
        writeln!(
            source,
            "def {}RustNet : INet.XNet NetSym := {net_literal}",
            fixture.name
        )
        .expect("writing to String");
        writeln!(
            source,
            "def {}RustOutputs : List PackedSchedule.Val := [{outputs}]",
            fixture.name
        )
        .expect("writing to String");
        writeln!(
            source,
            "def {}RustInteractions : Nat := {interactions}\n",
            fixture.name
        )
        .expect("writing to String");
        writeln!(
            source,
            "theorem {}_translation : (Program.translateChecked {}Program {}Inputs).map canonicalNet = some (canonicalNet {}RustNet) := by decide",
            fixture.name, fixture.name, fixture.name, fixture.name
        )
        .expect("writing to String");
        writeln!(
            source,
            "theorem {}_normalization : normalizeReadBack {}Program {}Inputs = some (some {}RustOutputs, {}RustInteractions) := by decide",
            fixture.name, fixture.name, fixture.name, fixture.name, fixture.name
        )
        .expect("writing to String");
        writeln!(
            source,
            "theorem {}_interaction_count : Program.interactionCount {}Program = {}RustInteractions := by decide\n",
            fixture.name, fixture.name, fixture.name
        )
        .expect("writing to String");
    }
    source.push_str("end NobleFoundation.NetI64Corpus\n");
    source
}
