use super::domain::{events, Event, Kind};
use super::encoding::{json_words, limits_json, quote, report_arm, workload};
use super::execution::{median, run, Timings};
use crate::common::IDS;
use noble_ngraph::ir;
use noble_ngraph::tick::{Limits, Runner, Selection};
use std::collections::BTreeMap;

fn median_warm_stage(runs: &[Timings], stage: &str) -> u128 {
    median(
        &runs[1..]
            .iter()
            .map(|run| run.median(stage).expect("required warm stage"))
            .collect::<Vec<_>>(),
    )
}

fn setup_cost(runs: &[Timings], operations: &[Event]) -> u128 {
    [
        "graph_prepare",
        "native_initialization",
        "reset",
        "control_admission",
        "control_activation",
    ]
    .iter()
    .map(|label| {
        let count = match *label {
            "graph_prepare" => usize::from(runs[0].by_stage.contains_key("graph_prepare")),
            "native_initialization" => {
                usize::from(runs[0].by_stage.contains_key("native_initialization"))
            }
            "reset" => operations
                .iter()
                .filter(|e| matches!(e, Event::Reset(_)))
                .count(),
            "control_admission" | "control_activation" => operations
                .iter()
                .filter(|e| matches!(e, Event::Update(_)))
                .count(),
            _ => unreachable!(),
        };
        if count == 0 {
            return 0;
        } // Inapplicable category has zero events, not zero cost.
        median(
            &runs[1..]
                .iter()
                .map(|run| {
                    let samples = run
                        .by_stage
                        .get(label)
                        .expect("declared setup stage measured");
                    assert_eq!(samples.len(), count, "missing setup event measurement");
                    samples.iter().sum::<u128>()
                })
                .collect::<Vec<_>>(),
        )
    })
    .sum::<u128>()
}

fn break_even(rows: &BTreeMap<&str, Vec<Timings>>, baseline: &str, operations: &[Event]) -> String {
    let runner = rows.get("runner").expect("runner rows");
    let other = rows.get(baseline).expect("baseline rows");
    let runner_tick = median_warm_stage(runner, "warm_tick");
    let other_tick = median_warm_stage(other, "warm_tick");
    if other_tick <= runner_tick {
        return "null".to_owned();
    }
    let incremental = setup_cost(runner, operations).saturating_sub(setup_cost(other, operations));
    let savings = other_tick - runner_tick;
    (incremental.div_ceil(savings)).to_string()
}

fn trace_digest(kind: Kind, operations: &[Event]) -> u64 {
    let digests = ["runner", "graph", "native"].map(|arm| {
        run(arm, kind, operations, true, false)
            .trace_digest
            .expect("untimed full trace")
    });
    assert_eq!(
        digests[0], digests[1],
        "runner versus ordinary complete trace"
    );
    assert_eq!(
        digests[1], digests[2],
        "ordinary versus native complete trace"
    );
    digests[0]
}

fn selections(kind: Kind, initial: Kind) -> (Selection, Option<Selection>) {
    let (initial_state, initial_controls) = initial.initial();
    let selection = Runner::prepare(
        &IDS,
        super::super::start(
            initial.graph(),
            initial.partition(),
            &initial_state,
            &initial_controls,
        ),
        Limits::default(),
    )
    .expect("identity selection")
    .selection();
    let replacement = if kind == Kind::Sub {
        let (state, controls) = kind.initial();
        Some(
            Runner::prepare(
                &IDS,
                super::super::start(kind.graph(), kind.partition(), &state, &controls),
                Limits::default(),
            )
            .expect("replacement identity selection")
            .selection(),
        )
    } else {
        None
    };
    (selection, replacement)
}

fn measured_rounds(kind: Kind, operations: &[Event]) -> BTreeMap<&'static str, Vec<Timings>> {
    let mut rows: BTreeMap<&'static str, Vec<Timings>> = BTreeMap::new();
    let arms = ["runner", "graph", "native"];
    for round in 0..11 {
        for offset in 0..3 {
            let arm = arms[(round + offset) % 3];
            rows.entry(arm)
                .or_insert_with(|| Vec::with_capacity(11))
                .push(run(arm, kind, operations, false, true));
        }
    }
    rows
}

fn threshold_decision(runner: &[Timings], graph: &[Timings], native: &[Timings]) -> bool {
    let median_whole =
        |runs: &[Timings]| median(&runs[1..].iter().map(|r| r.whole_ns).collect::<Vec<_>>());
    let is_tick_faster =
        median_warm_stage(runner, "warm_tick") * 10 <= median_warm_stage(graph, "warm_tick") * 9;
    let is_trace_faster = median_whole(runner) * 20 <= median_whole(graph) * 19;
    let is_within_native = median_whole(runner) * 4 <= median_whole(native) * 5;
    is_tick_faster && is_trace_faster && is_within_native
}

pub(super) fn scenario_receipt(kind: Kind, tick_count: usize) -> String {
    let operations = events(kind, tick_count);
    let initial = if kind == Kind::Sub { Kind::Add } else { kind };
    let (workload_bytes, workload_sha256) = workload(initial, &operations);
    let digest = trace_digest(kind, &operations);
    let (initial_state, initial_controls) = initial.initial();
    let (selection, replacement) = selections(kind, initial);
    let rows = measured_rounds(kind, &operations);
    let runner = rows.get("runner").expect("runner rows");
    let graph = rows.get("graph").expect("graph rows");
    let native = rows.get("native").expect("native rows");
    let is_time_threshold_met = threshold_decision(runner, graph, native);
    let rg = break_even(&rows, "graph", &operations);
    let rn = break_even(&rows, "native", &operations);
    format!("{{\"scenario\":{},\"successful_ticks\":{},\"operations\":{},\"initial\":{{\"graph\":{},\"state\":{},\"controls\":{}}},\"workload_encoding\":\"UTF-8 JSON initial-plus-operations object plus trailing LF; all I64 values decimal strings\",\"workload\":{},\"workload_sha256\":{},\"limits\":{},\"graph_logical_id\":{},\"replacement_graph_logical_id\":{},\"selected_schedule_id\":{},\"replacement_schedule_id\":{},\"selection_control_id\":{},\"untimed_complete_trace_digest64\":{},\"digest_note\":\"noncryptographic equality witness; source/artifact binding uses SHA-256\",\"comparison_time_thresholds_met\":{},\"break_even_graph_ticks\":{},\"break_even_native_ticks\":{},\"decision\":\"NO-GO / inconclusive: per-arm peak RSS and allocation measurements unavailable; no general speed claim\",\"runner\":{},\"ordinary_graph\":{},\"native\":{}}}",
        quote(if kind == Kind::Sub { "reset-sub" } else { kind.name() }), tick_count, operations.len(),
        quote(initial.name()), json_words(&initial_state), json_words(&initial_controls),
        workload_bytes.trim_end(), quote(&workload_sha256), limits_json(),
        quote(&ir::hex(&selection.graph)),
        replacement.map_or("null".to_owned(), |s| quote(&ir::hex(&s.graph))),
        quote(&ir::hex(&selection.schedule)),
        replacement.map_or("null".to_owned(), |s| quote(&ir::hex(&s.schedule))),
        quote(&ir::hex(&selection.controls)),
        quote(&format!("{:016x}", digest)), is_time_threshold_met, rg, rn,
        report_arm(runner, "runner"), report_arm(graph, "graph"), report_arm(native, "native"))
}
