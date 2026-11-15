//! Operations are executed on the real runner before their observed rows are recorded.

use crate::common::IDS;
use noble_ngraph::eval::{Control, Obs};
use noble_ngraph::ir::Graph;
use noble_ngraph::tick::{Partition, Refusal, Runner};

use super::model::words;

pub(super) struct Trace {
    lines: Vec<String>,
}

pub(super) struct Step<'a> {
    pub(super) key: &'a str,
    pub(super) controls: &'a [i64],
    pub(super) expected: (u64, u64, u64),
    pub(super) input: &'a [i64],
    pub(super) control: Control<'a>,
    pub(super) extra: &'a str,
    pub(super) class: &'a str,
}

pub(super) struct Update<'a> {
    pub(super) key: &'a str,
    pub(super) controls: &'a [i64],
    pub(super) expected: (u64, u64),
    pub(super) next: &'a [i64],
    pub(super) class: &'a str,
}

pub(super) struct Reset<'a> {
    pub(super) active_key: &'a str,
    pub(super) active_controls: &'a [i64],
    pub(super) expected: (u64, u64),
    pub(super) next: Replacement<'a>,
    pub(super) class: &'a str,
}

pub(super) struct Replacement<'a> {
    pub(super) key: &'a str,
    pub(super) graph: Graph,
    pub(super) partition: Partition,
    pub(super) state: &'a [i64],
    pub(super) controls: &'a [i64],
}

struct Record<'a> {
    op: &'a str,
    fields: String,
    class: &'a str,
    runner: &'a Runner,
    key: &'a str,
    controls: &'a [i64],
}

fn observation(r: &Runner, key: &str, controls: &[i64]) -> String {
    let selected = r.selection();
    let output = r.output().map_or("null".to_owned(), words);
    format!("{{\"graph_epoch\":{},\"control_epoch\":{},\"next_tick\":{},\"graph_key\":\"{key}\",\"state\":{},\"controls\":{},\"output\":{output}}}",
        selected.graph_epoch,selected.control_epoch,selected.next_tick,words(r.state()),words(controls))
}

impl Trace {
    pub(super) fn new(header: String) -> Self {
        Self {
            lines: vec![header],
        }
    }

    fn record(&mut self, row: Record<'_>) {
        let seq = self.lines.len() - 1;
        let outcome = if row.class == "ok" {
            "{\"kind\":\"ok\"}".to_owned()
        } else {
            format!("{{\"kind\":\"refused\",\"class\":\"{}\"}}", row.class)
        };
        self.lines.push(format!(
            "{{\"seq\":{seq},\"op\":\"{}\",{},\"outcome\":{outcome},\"observed\":{}}}",
            row.op,
            row.fields,
            observation(row.runner, row.key, row.controls)
        ));
    }

    pub(super) fn advance(&mut self, r: &mut Runner, mut request: Step<'_>) {
        let (g, c, t) = request.expected;
        let result = r.tick(g, c, t, request.input, &mut request.control);
        match request.class {
            "ok" => assert!(result.is_ok(), "actual tick: {result:?}"),
            "stale" => assert!(matches!(result, Err(Refusal::Stale | Refusal::TickOrder))),
            "cancelled" => assert_eq!(result, Err(Refusal::Evaluation(Obs::Cancelled))),
            "budget" => assert_eq!(
                result,
                Err(Refusal::Evaluation(Obs::Exhausted(
                    noble_ngraph::diag::BudgetKind::Fuel
                )))
            ),
            "exhausted" => assert_eq!(result, Err(Refusal::Terminal)),
            _ => panic!("unsupported tick refusal class"),
        }
        let fields = format!("\"expected_graph_epoch\":{g},\"expected_control_epoch\":{c},\"expected_tick\":{t},\"input\":{}{}",words(request.input),request.extra);
        self.record(Record {
            op: "tick",
            fields,
            class: request.class,
            runner: r,
            key: request.key,
            controls: request.controls,
        });
    }

    pub(super) fn update(&mut self, r: &mut Runner, request: Update<'_>) {
        let (g, c) = request.expected;
        let result = r.update_controls(g, c, request.next, &IDS);
        if request.class == "ok" {
            assert!(result.is_ok(), "actual update: {result:?}");
        } else {
            assert_eq!(result, Err(Refusal::Partition));
        }
        let fields = format!(
            "\"expected_graph_epoch\":{g},\"expected_control_epoch\":{c},\"controls\":{}",
            words(request.next)
        );
        self.record(Record {
            op: "update_controls",
            fields,
            class: request.class,
            runner: r,
            key: request.key,
            controls: request.controls,
        });
    }

    pub(super) fn reset(&mut self, r: &mut Runner, request: Reset<'_>) {
        let (g, c) = request.expected;
        let Replacement {
            key: next_key,
            graph,
            partition,
            state,
            controls,
        } = request.next;
        let result = r.reset(
            &IDS,
            super::super::epochs(g, c),
            super::super::start(graph, partition, state, controls),
        );
        if request.class == "ok" {
            assert!(result.is_ok(), "actual reset: {result:?}");
        } else {
            assert_eq!(result, Err(Refusal::Partition));
        }
        let fields = format!("\"expected_graph_epoch\":{g},\"expected_control_epoch\":{c},\"graph_key\":\"{next_key}\",\"state\":{},\"controls\":{}",words(state),words(controls));
        let (key, active_controls) = if request.class == "ok" {
            (next_key, controls)
        } else {
            (request.active_key, request.active_controls)
        };
        self.record(Record {
            op: "reset",
            fields,
            class: request.class,
            runner: r,
            key,
            controls: active_controls,
        });
    }

    pub(super) fn finish(self) -> String {
        format!("{}\n", self.lines.join("\n"))
    }
}
