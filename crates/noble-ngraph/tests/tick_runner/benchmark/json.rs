use super::domain::{Event, Kind};
use super::execution::{median, Timings};
use noble_ngraph::tick::Limits;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::{Command, Stdio};

pub(super) fn quote(input: &str) -> String {
    let mut text = String::from("\"");
    for ch in input.chars() {
        match ch {
            '"' => text.push_str("\\\""),
            '\\' => text.push_str("\\\\"),
            '\n' => text.push_str("\\n"),
            '\r' => text.push_str("\\r"),
            '\t' => text.push_str("\\t"),
            c if c < ' ' => write!(text, "\\u{:04x}", c as u32).expect("JSON escape"),
            c => text.push(c),
        }
    }
    text.push('"');
    text
}

fn distribution(values: &[u128]) -> String {
    if values.is_empty() {
        return "null".to_owned();
    }
    let mut v = values.to_vec();
    v.sort_unstable();
    format!(
        "{{\"samples\":{},\"p25_ns\":{},\"median_ns\":{},\"p75_ns\":{}}}",
        v.len(),
        v[(v.len() - 1) / 4],
        v[(v.len() - 1) / 2],
        v[3 * (v.len() - 1) / 4]
    )
}

pub(super) fn json_words(values: &[i64]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| quote(&value.to_string()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

// Each event variant is part of the committed workload format; require an
// explicit JSON branch for any future event rather than a catch-all fallback.
#[allow(tigerstyle::fragile_exhaustive_enum_match)]
fn event_json(event: &Event) -> String {
    match event {
        Event::Tick {
            input,
            stale,
            cancel,
        } => format!(
            "{{\"op\":\"tick\",\"input\":{},\"stale\":{},\"cancel\":{}}}",
            json_words(input),
            stale,
            cancel
        ),
        Event::Update(controls) => format!(
            "{{\"op\":\"update\",\"controls\":{}}}",
            json_words(controls)
        ),
        Event::RejectUpdate { controls, stale } => format!(
            "{{\"op\":\"reject_update\",\"controls\":{},\"stale\":{}}}",
            json_words(controls),
            stale
        ),
        Event::Reset(kind) => {
            let (state, controls) = kind.initial();
            format!(
                "{{\"op\":\"reset\",\"graph\":{},\"state\":{},\"controls\":{}}}",
                quote(kind.name()),
                json_words(&state),
                json_words(&controls)
            )
        }
    }
}

pub(super) fn workload(initial: Kind, operations: &[Event]) -> (String, String) {
    let rows: Vec<_> = operations.iter().map(event_json).collect();
    let (state, controls) = initial.initial();
    let bytes = format!(
        "{{\"initial\":{{\"graph\":{},\"state\":{},\"controls\":{}}},\"operations\":[{}]}}\n",
        quote(initial.name()),
        json_words(&state),
        json_words(&controls),
        rows.join(",")
    );
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("workload digest process");
    child
        .stdin
        .take()
        .expect("digest input")
        .write_all(bytes.as_bytes())
        .expect("write workload bytes");
    let output = child.wait_with_output().expect("workload digest result");
    assert!(output.status.success(), "workload digest refused");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 digest");
    (
        bytes,
        stdout
            .split_whitespace()
            .next()
            .expect("workload digest")
            .to_owned(),
    )
}

pub(super) fn limits_json() -> String {
    let l = Limits::default();
    format!("{{\"graph_budgets_debug\":{},\"state_bytes\":{},\"input_bytes\":{},\"control_bytes\":{},\"output_bytes\":{},\"update_bytes\":{},\"retained_bytes\":{},\"schedule_work\":{},\"tick_work\":{},\"ticks\":{},\"trace_bytes\":{}}}",
        quote(&format!("{:?}", l.graph)), l.state_bytes, l.input_bytes, l.control_bytes,
        l.output_bytes, l.update_bytes, l.retained_bytes, l.schedule_work,
        l.tick_work, l.ticks, l.trace_bytes)
}

fn stages_json(rounds: &[Timings], arm: &str) -> String {
    let mut stages: BTreeMap<&str, Vec<u128>> = BTreeMap::new();
    for run in &rounds[1..] {
        for (stage, values) in &run.by_stage {
            stages.entry(stage).or_default().push(median(values));
        }
    }
    let mut stage_json = String::new();
    for (stage, values) in stages {
        if !stage_json.is_empty() {
            stage_json.push(',');
        }
        write!(stage_json, "{}:{}", quote(stage), distribution(&values)).expect("JSON report");
    }
    if arm == "native" {
        stage_json.push_str(",\"graph_prepare\":null");
    }
    stage_json
}

fn raw_round_json(round: usize, row: &Timings) -> String {
    // Each u128 sample occupies at most 39 decimal digits plus a separator.
    let reserve = row
        .by_stage
        .values()
        .map(|samples| 64 + 40 * samples.len())
        .sum();
    let mut raw = String::with_capacity(reserve);
    for (stage, samples) in &row.by_stage {
        if !raw.is_empty() {
            raw.push(',');
        }
        write!(
            raw,
            "{}:[{}]",
            quote(stage),
            samples
                .iter()
                .map(u128::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
        .expect("raw stage samples");
    }
    format!(
        "{{\"round\":{},\"whole_ns\":{},\"whole_cpu_ns\":{},\"stage_samples_ns\":{{{}}}}}",
        round,
        row.whole_ns,
        row.whole_cpu_ns
            .map_or("null".to_owned(), |ns| ns.to_string()),
        raw
    )
}

pub(super) fn report_arm(rounds: &[Timings], arm: &str) -> String {
    let stage_json = stages_json(rounds, arm);
    let whole: Vec<_> = rounds[1..].iter().map(|row| row.whole_ns).collect();
    let cpu: Option<Vec<u128>> = rounds[1..].iter().map(|row| row.whole_cpu_ns).collect();
    let cpu_json = cpu.as_deref().map_or("null".to_owned(), distribution);
    let raw_rounds: Vec<_> = rounds
        .iter()
        .enumerate()
        .map(|(round, row)| raw_round_json(round, row))
        .collect();
    format!("{{\"first_process_round_ns\":{},\"first_process_round_cpu_ns\":{},\"whole_trace\":{},\"whole_trace_cpu\":{},\"whole_trace_cpu_note\":\"current benchmark thread scheduled CPU ns from /proc/thread-self/schedstat; null when unavailable or nonmonotone; excludes other threads and processes\",\"stages\":{{{}}},\"raw_rounds\":[{}],\"retained_bytes_estimate\":{},\"retained_note\":\"excludes allocator metadata and CheckedGraph derived caches; runner uses its own conservative charged footprint\",\"scratch_bytes_declared\":{},\"scratch_note\":{},\"rss_kib_after_process_observation\":{},\"rss_peak_comparison\":\"unmeasured: multiple arms share one process\",\"allocations\":{{\"status\":\"unmeasured\",\"reason\":\"no allocation instrumentation installed\"}}}}",
        rounds[0].whole_ns,
        rounds[0].whole_cpu_ns.map_or("null".to_owned(), |ns| ns.to_string()),
        distribution(&whole), cpu_json, stage_json, raw_rounds.join(","),
        rounds.last().expect("round").retained_bytes,
        rounds.last().expect("round").scratch_bytes.map_or("null".to_owned(), |v| v.to_string()),
        quote(if arm == "native" { "not applicable: native code has no graph register file" }
            else { "declared packed register workspace, not a measured heap peak" }),
        rounds.last().expect("round").rss_kib_after.map_or("null".to_owned(), |v| v.to_string()))
}
