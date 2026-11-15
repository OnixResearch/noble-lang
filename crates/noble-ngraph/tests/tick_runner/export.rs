//! Clean-source producer of the actual runner's finite compiler-lab replay trace.
//! Enabled only by NOBLE_TICK_EXPORT_PATH; ordinary tests never write files.

use super::{add_graph, runner, sub_graph, two_graph, ONE, TWO};
use crate::common::IDS;
use noble_ngraph::eval::{Control, Obs};
use noble_ngraph::ir::{hex, Graph, Op};
use noble_ngraph::tick::{Refusal, Runner};
use std::io::Write;
use std::process::{Command, Stdio};

fn command(program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .output()
        .expect("pinned executable");
    assert!(output.status.success(), "{program} {args:?}: {output:?}");
    String::from_utf8(output.stdout)
        .expect("UTF8 tool output")
        .trim()
        .to_owned()
}

fn digest(bytes: &[u8]) -> String {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(bytes)
        .expect("hash input");
    let result = child.wait_with_output().expect("sha256sum output");
    assert!(result.status.success());
    String::from_utf8(result.stdout).unwrap()[..64].to_owned()
}

fn words(values: &[i64]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|v| format!("\"{v}\""))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn model(graph: &Graph, p: noble_ngraph::tick::Partition) -> String {
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

fn graph_entry(
    key: &str,
    graph: Graph,
    p: noble_ngraph::tick::Partition,
    initial: &[i64],
    controls: &[i64],
) -> String {
    // This projection iterates the real authored, checked graph's nodes and
    // outputs; the actual runner's graph/schedule identities are not guessed.
    let selected = runner(graph.clone(), p, initial, controls).selection();
    let m = model(&graph, p);
    format!("{{\"key\":\"{key}\",\"graph_logical_id\":\"{}\",\"selected_schedule_id\":\"{}\",\"export_sha256\":\"{}\",{}",
        hex(&selected.graph),hex(&selected.schedule),digest(m.as_bytes()), &m[1..])
}

fn observation(r: &Runner, key: &str, controls: &[i64]) -> String {
    let selected = r.selection();
    let output = r.output().map_or("null".to_owned(), words);
    format!("{{\"graph_epoch\":{},\"control_epoch\":{},\"next_tick\":{},\"graph_key\":\"{key}\",\"state\":{},\"controls\":{},\"output\":{output}}}",
        selected.graph_epoch,selected.control_epoch,selected.next_tick,words(r.state()),words(controls))
}

fn row(
    seq: usize,
    op: &str,
    fields: &str,
    kind: &str,
    r: &Runner,
    key: &str,
    controls: &[i64],
) -> String {
    let outcome = if kind == "ok" {
        "{\"kind\":\"ok\"}".to_owned()
    } else {
        format!("{{\"kind\":\"refused\",\"class\":\"{kind}\"}}")
    };
    format!(
        "{{\"seq\":{seq},\"op\":\"{op}\",{fields},\"outcome\":{outcome},\"observed\":{}}}",
        observation(r, key, controls)
    )
}

fn tick_rows(
    lines: &mut Vec<String>,
    r: &mut Runner,
    key: &str,
    controls: &[i64],
    expected: (u64, u64, u64),
    input: &[i64],
    control: &mut Control<'_>,
    extra: &str,
    class: &str,
) {
    let (g, c, t) = expected;
    let result = r.tick(g, c, t, input, control);
    match class {
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
    let fields = format!("\"expected_graph_epoch\":{g},\"expected_control_epoch\":{c},\"expected_tick\":{t},\"input\":{}{extra}",words(input));
    lines.push(row(
        lines.len() - 1,
        "tick",
        &fields,
        class,
        r,
        key,
        controls,
    ));
}

fn update_rows(
    lines: &mut Vec<String>,
    r: &mut Runner,
    key: &str,
    controls: &[i64],
    expected: (u64, u64),
    next: &[i64],
    class: &str,
) {
    let (g, c) = expected;
    let result = r.update_controls(g, c, next, &IDS);
    if class == "ok" {
        assert!(result.is_ok(), "actual update: {result:?}");
    } else {
        assert_eq!(result, Err(Refusal::Partition));
    }
    let fields = format!(
        "\"expected_graph_epoch\":{g},\"expected_control_epoch\":{c},\"controls\":{}",
        words(next)
    );
    lines.push(row(
        lines.len() - 1,
        "update_controls",
        &fields,
        class,
        r,
        key,
        controls,
    ));
}

fn reset_rows(
    lines: &mut Vec<String>,
    r: &mut Runner,
    active: (&str, &[i64]),
    expected: (u64, u64),
    next: (&str, Graph, noble_ngraph::tick::Partition, &[i64], &[i64]),
    class: &str,
) {
    let (g, c) = expected;
    let (next_key, graph, partition, state, controls) = next;
    let result = r.reset(&IDS, g, c, graph, partition, state, controls);
    if class == "ok" {
        assert!(result.is_ok(), "actual reset: {result:?}");
    } else {
        assert_eq!(result, Err(Refusal::Partition));
    }
    let fields = format!("\"expected_graph_epoch\":{g},\"expected_control_epoch\":{c},\"graph_key\":\"{next_key}\",\"state\":{},\"controls\":{}",words(state),words(controls));
    let (key, active_controls) = if class == "ok" {
        (next_key, controls)
    } else {
        active
    };
    lines.push(row(
        lines.len() - 1,
        "reset",
        &fields,
        class,
        r,
        key,
        active_controls,
    ));
}

#[test]
fn export_exact_clean_source_trace_when_requested() {
    let Some(path) = std::env::var_os("NOBLE_TICK_EXPORT_PATH") else {
        return;
    };
    assert!(
        command("git", &["status", "--porcelain"]).is_empty(),
        "the producer must run from a clean committed tree"
    );
    let source_commit = command("git", &["rev-parse", "HEAD"]);
    let source_tree = command("git", &["rev-parse", "HEAD^{tree}"]);
    let rustc = command("rustc", &["--version"]);
    let test_file = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/tick_runner/export.rs");
    let producer_sha256 = digest(&std::fs::read(test_file).unwrap());
    let runner_artifact_sha256 = digest(&std::fs::read(std::env::current_exe().unwrap()).unwrap());
    let graphs = [
        graph_entry("add", add_graph(), ONE, &[13], &[2]),
        graph_entry("sub", sub_graph(), ONE, &[101], &[7]),
        graph_entry("two", two_graph(), TWO, &[17, -13], &[2, 3]),
    ]
    .join(",");
    let mut lines = vec![format!("{{\"schema\":\"noble-tick-export/v1\",\"profile\":\"noble-tick-i64/v1\",\"source_commit\":\"{source_commit}\",\"source_tree\":\"{source_tree}\",\"rustc\":\"{rustc}\",\"producer_sha256\":\"{producer_sha256}\",\"runner_artifact_sha256\":\"{runner_artifact_sha256}\",\"graphs\":[{graphs}],\"initial\":{{\"graph_key\":\"add\",\"state\":[\"13\"],\"controls\":[\"2\"]}},\"limits\":{{\"max_graphs\":16,\"max_nodes\":4096,\"max_words\":256,\"max_events\":65536,\"max_ticks\":3,\"max_work_per_tick\":4096,\"max_trace_bytes\":16777216}}}}")];
    let mut r = Runner::prepare(
        &IDS,
        add_graph(),
        ONE,
        &[13],
        &[2],
        noble_ngraph::tick::Limits {
            ticks: 3,
            ..Default::default()
        },
    )
    .unwrap();
    tick_rows(
        &mut lines,
        &mut r,
        "add",
        &[2],
        (0, 0, 0),
        &[3],
        &mut Control::default(),
        "",
        "ok",
    );
    tick_rows(
        &mut lines,
        &mut r,
        "add",
        &[2],
        (0, 0, 0),
        &[3],
        &mut Control::default(),
        "",
        "stale",
    );
    update_rows(&mut lines, &mut r, "add", &[2], (0, 0), &[], "malformed");
    update_rows(&mut lines, &mut r, "add", &[4], (0, 0), &[4], "ok");
    tick_rows(
        &mut lines,
        &mut r,
        "add",
        &[4],
        (0, 0, 1),
        &[5],
        &mut Control::default(),
        "",
        "stale",
    );
    let cancel = |_: u32| true;
    tick_rows(
        &mut lines,
        &mut r,
        "add",
        &[4],
        (0, 1, 1),
        &[5],
        &mut Control {
            cancel: Some(&cancel),
            ..Default::default()
        },
        ",\"cancel_before_commit\":true",
        "cancelled",
    );
    tick_rows(
        &mut lines,
        &mut r,
        "add",
        &[4],
        (0, 1, 1),
        &[5],
        &mut Control::default(),
        "",
        "ok",
    );
    reset_rows(
        &mut lines,
        &mut r,
        ("add", &[4]),
        (0, 1),
        ("sub", sub_graph(), ONE, &[101], &[7]),
        "ok",
    );
    tick_rows(
        &mut lines,
        &mut r,
        "sub",
        &[7],
        (1, 2, 0),
        &[3],
        &mut Control::default(),
        "",
        "ok",
    );
    reset_rows(
        &mut lines,
        &mut r,
        ("sub", &[7]),
        (1, 2),
        ("two", two_graph(), TWO, &[17, -13], &[2, 3]),
        "ok",
    );
    tick_rows(
        &mut lines,
        &mut r,
        "two",
        &[2, 3],
        (2, 3, 0),
        &[5, -4],
        &mut Control::default(),
        "",
        "ok",
    );
    update_rows(&mut lines, &mut r, "two", &[9, -7], (2, 3), &[9, -7], "ok");
    tick_rows(
        &mut lines,
        &mut r,
        "two",
        &[9, -7],
        (2, 4, 1),
        &[-2, 1],
        &mut Control {
            fuel: Some(0),
            ..Default::default()
        },
        ",\"work_budget\":0",
        "budget",
    );
    tick_rows(
        &mut lines,
        &mut r,
        "two",
        &[9, -7],
        (2, 4, 1),
        &[-2, 1],
        &mut Control::default(),
        "",
        "ok",
    );
    tick_rows(
        &mut lines,
        &mut r,
        "two",
        &[9, -7],
        (2, 4, 2),
        &[1, 1],
        &mut Control::default(),
        "",
        "ok",
    );
    tick_rows(
        &mut lines,
        &mut r,
        "two",
        &[9, -7],
        (2, 4, 3),
        &[1, 1],
        &mut Control::default(),
        "",
        "exhausted",
    );
    reset_rows(
        &mut lines,
        &mut r,
        ("two", &[9, -7]),
        (2, 4),
        ("add", add_graph(), ONE, &[], &[3]),
        "malformed",
    );
    let trace = format!("{}\n", lines.join("\n"));
    assert!(trace.len() <= 16 * 1024 * 1024, "trace cap");
    std::fs::write(&path, trace.as_bytes()).expect("retained trace outside repository");
    println!(
        "TICK_EXPORT source_commit={source_commit} path={}",
        path.to_string_lossy()
    );
}
