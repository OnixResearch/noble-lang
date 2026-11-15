//! The independent replay graph projection from the checked runner fixtures.

use noble_ngraph::ir::{hex, Graph, Op};
use noble_ngraph::tick::Partition;

pub(super) fn words(values: &[i64]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|v| format!("\"{v}\""))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn graph_json(graph: &Graph, p: Partition) -> String {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| match &node.op {
            Op::Input(k) => format!("{{\"op\":\"input\",\"index\":{k}}}"),
            Op::Const(noble_ngraph::ir::Literal::I64(value)) => {
                format!("{{\"op\":\"const\",\"value\":\"{value}\"}}")
            }
            Op::I64Add(a, b) => format!("{{\"op\":\"add\",\"lhs\":{},\"rhs\":{}}}", a.0, b.0),
            Op::I64Sub(a, b) => format!("{{\"op\":\"sub\",\"lhs\":{},\"rhs\":{}}}", a.0, b.0),
            Op::I64Mul(a, b) => format!("{{\"op\":\"mul\",\"lhs\":{},\"rhs\":{}}}", a.0, b.0),
            other => panic!("outside exported graph subset: {other:?}"),
        })
        .collect::<Vec<_>>()
        .join(",");
    let outputs = graph
        .outputs
        .iter()
        .map(|id| id.0.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"partition\":{{\"state_words\":{},\"input_words\":{},\"control_words\":{},\"output_words\":{}}},\"nodes\":[{nodes}],\"outputs\":[{outputs}]}}",
        p.state_words,p.input_words,p.control_words,p.output_words)
}

pub(super) fn graph_entry(
    key: &str,
    graph: Graph,
    p: Partition,
    initial: &[i64],
    controls: &[i64],
) -> String {
    // This projection iterates the real authored, checked graph's nodes and
    // outputs; the actual runner's graph/schedule identities are not guessed.
    let selected = super::super::fixture(graph.clone(), p, initial, controls).selection();
    let m = graph_json(&graph, p);
    format!("{{\"key\":\"{key}\",\"graph_logical_id\":\"{}\",\"selected_schedule_id\":\"{}\",\"export_sha256\":\"{}\",{}",
        hex(&selected.graph),hex(&selected.schedule),super::identity::digest(m.as_bytes()), &m[1..])
}
