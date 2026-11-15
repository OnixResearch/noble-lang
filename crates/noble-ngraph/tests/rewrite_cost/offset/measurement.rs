//! Ignored receipt producer: costs and retained sizes are separate from semantic proof.

use std::time::Instant;

use noble_ngraph::check::{self, Budgets, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::sem::Value;
use noble_ngraph::specialize::{self, offset, Request, Residual};

use super::super::common::IDS;
use super::fixtures::{compiled, graph, observed, vectors, Fixture};

#[expect(
    tigerstyle::ambient_clock,
    reason = "Owner: noble-maintainers; the ignored, frozen #32 cost lane times only the explicitly named specialization, admission and replay scopes."
)]
fn clock() -> Instant {
    Instant::now()
}

struct Side {
    emit_readmit_ns: u128,
    first_replay_ns: u128,
    warm_replay_ns: u128,
    l3_visits: u64,
    l4_visits: u64,
    graph_bytes: u64,
    compiled_bytes: u64,
    source_bytes: usize,
}

fn measure_side(
    original: &CheckedGraph,
    baseline: &Residual,
    checked: &CheckedGraph,
    map: &[Option<i64>],
    budgets: &Budgets,
) -> Side {
    let graph_bytes = check::retained_bytes(checked.graph());
    let packed = Compiled::new(&IDS, checked);
    let compiled_bytes = packed.retained_bytes();
    let start = clock();
    let source = specialize::emit_source(checked).expect("source");
    let source_bytes = source.len();
    let rechecked = specialize::readmit(
        &IDS,
        &source,
        &specialize::interface_types(checked).expect("types"),
        noble_contracts::Limits::default(),
        budgets,
    )
    .expect("ordinary admission");
    let emit_readmit_ns = start.elapsed().as_nanos();
    let inputs = vectors(map);
    let first = inputs[0]
        .iter()
        .copied()
        .map(Value::I64)
        .collect::<Vec<_>>();
    let start = clock();
    let first_result = compiled(&packed, &first);
    let first_replay_ns = start.elapsed().as_nanos();
    assert_eq!(first_result.0, observed(&rechecked, &first).0);
    let mut warm_replay_ns = 0;
    let mut l3_visits = 0;
    let mut l4_visits = 0;
    for row in inputs {
        let dynamic = row.iter().copied().map(Value::I64).collect::<Vec<_>>();
        let merged = baseline.merge(&dynamic).expect("ordered merge");
        let expected = observed(original, &merged).0;
        let (l3, visits) = observed(checked, &dynamic);
        let start = clock();
        let (l4, packed_visits) = compiled(&packed, &dynamic);
        warm_replay_ns += start.elapsed().as_nanos();
        assert_eq!(l3, expected, "L3");
        assert_eq!(l4, expected, "L4");
        assert_eq!(observed(&rechecked, &dynamic).0, expected, "readmission");
        l3_visits += visits;
        l4_visits += packed_visits;
    }
    Side {
        emit_readmit_ns,
        first_replay_ns,
        warm_replay_ns,
        l3_visits,
        l4_visits,
        graph_bytes,
        compiled_bytes,
        source_bytes,
    }
}

fn measure_fixture(kind: Fixture) -> String {
    let (original, map) = graph(kind);
    let budgets = Budgets::default();
    let request = Request {
        static_inputs: &map,
        budgets: &budgets,
    };
    let start = clock();
    let baseline = specialize::specialize(&IDS, &original, &request).expect("baseline");
    let baseline_specialization_ns = start.elapsed().as_nanos();
    let start = clock();
    let comparison = offset::compare(&IDS, &original, &request).expect("offset");
    let offset_specialization_ns = start.elapsed().as_nanos();
    assert_eq!(baseline.checked.id(), comparison.baseline.checked.id());
    let base = measure_side(&original, &baseline, &baseline.checked, &map, &budgets);
    let other = measure_side(&original, &baseline, &comparison.normalized, &map, &budgets);
    format!(
        "    {{\"fixture\":\"{}\",\"vectors\":69,\"baseline_nodes\":{},\"offset_nodes\":{},\"baseline_l3_visits\":{},\"offset_l3_visits\":{},\"baseline_l4_visits\":{},\"offset_l4_visits\":{},\"baseline_specialize_ns\":{},\"offset_specialize_total_ns\":{},\"baseline_emit_readmit_ns\":{},\"offset_emit_readmit_ns\":{},\"baseline_first_replay_ns\":{},\"offset_first_replay_ns\":{},\"baseline_warm_replay_ns\":{},\"offset_warm_replay_ns\":{},\"baseline_graph_bytes\":{},\"offset_graph_bytes\":{},\"baseline_compiled_bytes\":{},\"offset_compiled_bytes\":{},\"baseline_source_bytes\":{},\"offset_source_bytes\":{}}}",
        kind.name(), baseline.checked.node_count(), comparison.normalized.node_count(),
        base.l3_visits, other.l3_visits, base.l4_visits, other.l4_visits, baseline_specialization_ns,
        offset_specialization_ns, base.emit_readmit_ns, other.emit_readmit_ns,
        base.first_replay_ns, other.first_replay_ns, base.warm_replay_ns, other.warm_replay_ns,
        base.graph_bytes, other.graph_bytes, base.compiled_bytes, other.compiled_bytes,
        base.source_bytes, other.source_bytes,
    )
}

#[test]
#[ignore = "descriptive #32 measurement; requires clean committed source and an external receipt destination"]
#[expect(
    tigerstyle::ambient_env,
    reason = "Owner: noble-maintainers; this ignored cost producer reads source identity and destination only at its evidence boundary."
)]
fn report_costs() {
    let source_commit = std::env::var("NOBLE_SOURCE_COMMIT").expect("source commit");
    let source_tree = std::env::var("NOBLE_SOURCE_TREE").expect("source tree");
    let source_clean = std::env::var("NOBLE_SOURCE_CLEAN").expect("clean source assertion");
    assert_eq!(source_clean, "true");
    let output = std::env::var("NOBLE_PE_OFFSET_OUT").expect("external receipt destination");
    let mut records = Vec::with_capacity(7);
    for kind in [
        Fixture::Add,
        Fixture::Sub,
        Fixture::Wrap,
        Fixture::Shared,
        Fixture::Nonprefix,
        Fixture::Dead,
        Fixture::Growth,
    ] {
        records.push(measure_fixture(kind));
    }
    let report = format!(
        "{{\n  \"schema\":\"noble-pe-offset-comparison/v1\",\n  \"issue\":32,\n  \"protocol_commit\":\"fbd67e7\",\n  \"source_commit\":\"{source_commit}\",\n  \"source_tree\":\"{source_tree}\",\n  \"clean_source\":true,\n  \"timings\":\"single shared-host debug samples, descriptive not matched-budget speedup\",\n  \"identity\":\"test-siphash13x4\",\n  \"rows\":[\n{}\n  ]\n}}\n",
        records.join(",\n")
    );
    std::fs::write(output, report).expect("write receipt outside source tree");
}
