//! Opt-in, source-bound sequential comparison. Never a default test-time benchmark.
use super::{add_graph, expected_one, expected_two, sub_graph, two_graph, ONE, TWO};
use crate::common::IDS;
use noble_ngraph::compiled::{Compiled, Workspace};
use noble_ngraph::eval::{Control, Obs};
use noble_ngraph::ir::{self, Graph};
use noble_ngraph::sem::Value;
use noble_ngraph::tick::{Limits, Runner};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::hash::{Hash, Hasher};
use std::hint::black_box;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Add,
    Two,
    Sub,
}

impl Kind {
    fn graph(self) -> Graph {
        match self {
            Self::Add => add_graph(),
            Self::Two => two_graph(),
            Self::Sub => sub_graph(),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Two => "two",
            Self::Sub => "sub",
        }
    }
    fn partition(self) -> noble_ngraph::tick::Partition {
        if self == Self::Two {
            TWO
        } else {
            ONE
        }
    }
    fn initial(self) -> (Vec<i64>, Vec<i64>) {
        if self == Self::Two {
            (vec![i64::MIN, 7], vec![3, -5])
        } else if self == Self::Sub {
            (vec![i64::MIN + 1], vec![7])
        } else {
            (vec![i64::MAX - 1], vec![3])
        }
    }
    fn oracle(self, s: &[i64], x: &[i64], c: &[i64]) -> (Vec<i64>, Vec<i64>) {
        if self == Self::Two {
            let (state, output) = expected_two([s[0], s[1]], [x[0], x[1]], [c[0], c[1]]);
            (state.to_vec(), output.to_vec())
        } else {
            let value = expected_one(s[0], x[0], c[0], self == Self::Sub);
            (vec![value], vec![value])
        }
    }
}

#[derive(Clone)]
enum Event {
    Tick {
        input: Vec<i64>,
        stale: bool,
        cancel: bool,
    },
    Update(Vec<i64>),
    RejectUpdate {
        controls: Vec<i64>,
        stale: bool,
    },
    Reset(Kind),
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct Snapshot {
    kind: &'static str,
    graph_epoch: u64,
    control_epoch: u64,
    next_tick: u64,
    state: Vec<i64>,
    controls: Vec<i64>,
    output: Option<Vec<i64>>,
}

struct Model {
    kind: Kind,
    graph_epoch: u64,
    control_epoch: u64,
    next_tick: u64,
    state: Vec<i64>,
    controls: Vec<i64>,
    output: Option<Vec<i64>>,
}

impl Model {
    fn new(kind: Kind) -> Self {
        let (state, controls) = kind.initial();
        Self {
            kind,
            graph_epoch: 0,
            control_epoch: 0,
            next_tick: 0,
            state,
            controls,
            output: None,
        }
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            kind: self.kind.name(),
            graph_epoch: self.graph_epoch,
            control_epoch: self.control_epoch,
            next_tick: self.next_tick,
            state: self.state.clone(),
            controls: self.controls.clone(),
            output: self.output.clone(),
        }
    }
    fn apply(&mut self, event: &Event) -> bool {
        match event {
            Event::Tick { stale, cancel, .. } if *stale || *cancel => false,
            Event::Tick { input, .. } => {
                let (state, output) = self.kind.oracle(&self.state, input, &self.controls);
                self.state = state;
                self.output = Some(output);
                self.next_tick += 1;
                true
            }
            Event::Update(controls) => {
                self.controls.clone_from(controls);
                self.control_epoch += 1;
                true
            }
            Event::RejectUpdate { .. } => false,
            Event::Reset(kind) => {
                let (state, controls) = kind.initial();
                self.kind = *kind;
                self.graph_epoch += 1;
                self.control_epoch += 1;
                self.next_tick = 0;
                self.state = state;
                self.controls = controls;
                self.output = None;
                true
            }
        }
    }
}

struct GraphMachine {
    compiled: Compiled,
    workspace: Workspace,
    inputs: Vec<Value>,
}

impl GraphMachine {
    fn prepare(kind: Kind) -> Self {
        let checked =
            noble_ngraph::check::check(&IDS, kind.graph()).expect("checked scenario graph");
        let compiled = Compiled::new(&IDS, &checked);
        noble_ngraph::schedule::check(
            &checked,
            &compiled.packed_layout(),
            compiled.i64_instructions().expect("I64 instruction body"),
        )
        .expect("selected schedule validated");
        let workspace = compiled.workspace();
        let p = kind.partition();
        Self {
            compiled,
            workspace,
            inputs: vec![Value::I64(0); p.state_words + p.input_words + p.control_words],
        }
    }
    fn step(
        &mut self,
        state: &[i64],
        input: &[i64],
        controls: &[i64],
        cancel: bool,
    ) -> Option<(Vec<i64>, Vec<i64>)> {
        for (slot, value) in self
            .inputs
            .iter_mut()
            .zip(state.iter().chain(input).chain(controls))
        {
            *slot = Value::I64(*value);
        }
        let cancelled = |_: u32| true;
        let mut control = Control::default();
        if cancel {
            control.cancel = Some(&cancelled);
        }
        let report =
            self.compiled
                .evaluate_in(&mut self.workspace, &self.inputs, None, &mut control);
        let Obs::Normal(values) = report.obs else {
            return None;
        };
        let mut parts = values.into_iter().map(|value| {
            let Value::I64(word) = value else {
                panic!("admitted I64 graph returned another type")
            };
            word
        });
        let state: Vec<_> = parts.by_ref().take(state.len()).collect();
        let output: Vec<_> = parts.collect();
        Some((state, output))
    }
    fn footprint(&self) -> (u64, u64) {
        let scratch = self.compiled.scratch_bytes();
        (
            self.compiled.retained_bytes()
                + scratch
                + noble_ngraph::check::retained_bytes(self.compiled.checked().graph())
                + (self.inputs.capacity() * std::mem::size_of::<Value>()) as u64,
            scratch,
        )
    }
}

struct HostMachine {
    kind: Kind,
    graph_epoch: u64,
    control_epoch: u64,
    next_tick: u64,
    state: Vec<i64>,
    controls: Vec<i64>,
    output: Option<Vec<i64>>,
    graph: Option<GraphMachine>,
}

impl HostMachine {
    fn new(kind: Kind, graph: bool) -> Self {
        let (state, controls) = kind.initial();
        Self {
            kind,
            graph_epoch: 0,
            control_epoch: 0,
            next_tick: 0,
            state,
            controls,
            output: None,
            graph: graph.then(|| GraphMachine::prepare(kind)),
        }
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            kind: self.kind.name(),
            graph_epoch: self.graph_epoch,
            control_epoch: self.control_epoch,
            next_tick: self.next_tick,
            state: self.state.clone(),
            controls: self.controls.clone(),
            output: self.output.clone(),
        }
    }
    fn tick(&mut self, input: &[i64], stale: bool, cancel: bool) -> bool {
        if stale {
            return false;
        }
        let result = if let Some(graph) = &mut self.graph {
            graph.step(&self.state, input, &self.controls, cancel)
        } else if cancel {
            None
        } else {
            Some(self.kind.oracle(&self.state, input, &self.controls))
        };
        let Some((state, output)) = result else {
            return false;
        };
        self.state = state;
        self.output = Some(output);
        self.next_tick += 1;
        true
    }
    fn footprint(&self) -> (u64, Option<u64>) {
        let vectors = (self.state.capacity()
            + self.controls.capacity()
            + self.output.as_ref().map_or(0, Vec::capacity)) as u64
            * 8;
        match &self.graph {
            Some(graph) => {
                let (retained, scratch) = graph.footprint();
                (vectors + retained, Some(scratch))
            }
            None => (vectors, None),
        }
    }
}

enum Backend {
    Runner(Runner),
    Graph(HostMachine),
    Native(HostMachine),
}
impl Backend {
    fn new(arm: &'static str, kind: Kind) -> Self {
        match arm {
            "runner" => {
                let (state, controls) = kind.initial();
                Self::Runner(
                    Runner::prepare(
                        &IDS,
                        kind.graph(),
                        kind.partition(),
                        &state,
                        &controls,
                        Limits::default(),
                    )
                    .expect("admitted runner"),
                )
            }
            "graph" => Self::Graph(HostMachine::new(kind, true)),
            "native" => Self::Native(HostMachine::new(kind, false)),
            _ => panic!("unsupported benchmark arm"),
        }
    }
    fn snapshot(&self, kind: Kind, controls: &[i64]) -> Snapshot {
        match self {
            Self::Runner(r) => {
                let selected = r.selection();
                Snapshot {
                    kind: kind.name(),
                    graph_epoch: selected.graph_epoch,
                    control_epoch: selected.control_epoch,
                    next_tick: selected.next_tick,
                    state: r.state().to_vec(),
                    controls: controls.to_vec(),
                    output: r.output().map(<[i64]>::to_vec),
                }
            }
            Self::Graph(h) | Self::Native(h) => h.snapshot(),
        }
    }
    fn footprint(&self) -> (u64, Option<u64>) {
        match self {
            Self::Runner(r) => (r.retained_bytes(), Some(r.scratch_bytes())),
            Self::Graph(h) | Self::Native(h) => h.footprint(),
        }
    }
    fn next_tick(&self) -> u64 {
        match self {
            Self::Runner(r) => r.selection().next_tick,
            Self::Graph(h) | Self::Native(h) => h.next_tick,
        }
    }
}

struct Timings {
    by_stage: BTreeMap<&'static str, Vec<u128>>,
    recording: bool,
    whole_ns: u128,
    retained_bytes: u64,
    scratch_bytes: Option<u64>,
    rss_kib_after: Option<u64>,
    trace_digest: Option<u64>,
}
impl Timings {
    fn record(&mut self, stage: &'static str, ns: u128) {
        if self.recording {
            self.by_stage.entry(stage).or_default().push(ns);
        }
    }
    fn median(&self, stage: &str) -> Option<u128> {
        self.by_stage.get(stage).map(|v| median(v))
    }
}

fn median(values: &[u128]) -> u128 {
    assert!(!values.is_empty(), "no sampled stage");
    let mut v = values.to_vec();
    v.sort_unstable();
    v[(v.len() - 1) / 2]
}

fn events(kind: Kind, length: usize) -> Vec<Event> {
    let mut events = Vec::new();
    let mut current = kind;
    for count in 0..length {
        if count == 4 {
            events.push(Event::Tick {
                input: vec![1; current.partition().input_words],
                stale: true,
                cancel: false,
            });
        }
        if count == 5 {
            events.push(Event::Tick {
                input: vec![1; current.partition().input_words],
                stale: false,
                cancel: true,
            });
        }
        if count == 6 {
            events.push(Event::RejectUpdate {
                controls: current.initial().1,
                stale: true,
            });
        }
        if count == 9 {
            events.push(Event::RejectUpdate {
                controls: Vec::new(),
                stale: false,
            });
        }
        let input = if current == Kind::Two {
            vec![
                if count % 2 == 0 { -1 } else { i64::MAX },
                if count % 3 == 0 { i64::MIN } else { -7 },
            ]
        } else {
            vec![if count % 3 == 0 {
                i64::MIN
            } else if count % 2 == 0 {
                -2
            } else {
                1
            }]
        };
        events.push(Event::Tick {
            input,
            stale: false,
            cancel: false,
        });
        let completed = count + 1;
        if kind == Kind::Sub && completed == (if length == 32 { 16 } else { 256 }) {
            current = Kind::Sub;
            events.push(Event::Reset(current));
        }
        if completed == 8 || completed % 64 == 0 {
            let controls = if current == Kind::Two {
                vec![-4 - count as i64, 5 + count as i64]
            } else {
                vec![-4 - count as i64]
            };
            events.push(Event::Update(controls));
            events.push(Event::Tick {
                input: vec![1; current.partition().input_words],
                stale: true,
                cancel: false,
            });
        }
    }
    events
}

fn event_stage(event: &Event, tick_index: u64, previous: &'static str) -> &'static str {
    match event {
        Event::Tick { stale: true, .. } | Event::Tick { cancel: true, .. } => "refused_tick",
        Event::Tick { .. } if previous == "reset" => "after_reset_tick",
        Event::Tick { .. } if previous == "update" => "after_update_tick",
        Event::Tick { .. } if tick_index == 0 => "first_tick",
        Event::Tick { .. } => "warm_tick",
        Event::Update(_) => "control_update",
        Event::RejectUpdate { .. } => "refused_update",
        Event::Reset(_) => "reset",
    }
}

fn run_operation(
    backend: &mut Backend,
    event: &Event,
    timing: &mut Timings,
    current: &mut Kind,
    controls: &mut Vec<i64>,
) -> bool {
    match event {
        Event::Tick {
            input,
            stale,
            cancel,
        } => match backend {
            Backend::Runner(r) => {
                let s = r.selection();
                let cancelled = |_: u32| true;
                let mut control = Control::default();
                if *cancel {
                    control.cancel = Some(&cancelled);
                }
                let answer = r.tick(
                    s.graph_epoch + u64::from(*stale),
                    s.control_epoch,
                    s.next_tick,
                    black_box(input),
                    &mut control,
                );
                answer.is_ok()
            }
            Backend::Graph(h) | Backend::Native(h) => h.tick(black_box(input), *stale, *cancel),
        },
        Event::Update(words) => {
            let start = Instant::now();
            let admitted = match backend {
                Backend::Runner(r) => {
                    let s = r.selection();
                    Some(
                        r.prepare_control_update(
                            s.graph_epoch,
                            s.control_epoch,
                            black_box(words),
                            &IDS,
                        )
                        .expect("update admitted"),
                    )
                }
                Backend::Graph(_) | Backend::Native(_) => None,
            };
            let staged = if admitted.is_none() {
                Some(words.clone())
            } else {
                None
            };
            timing.record("control_admission", start.elapsed().as_nanos());
            let activation = Instant::now();
            match backend {
                Backend::Runner(r) => {
                    r.activate_control_update(admitted.expect("token"))
                        .expect("activate");
                }
                Backend::Graph(h) | Backend::Native(h) => {
                    h.controls = staged.expect("host staged words");
                    h.control_epoch += 1;
                }
            }
            timing.record("control_activation", activation.elapsed().as_nanos());
            controls.clone_from(words);
            true
        }
        Event::RejectUpdate {
            controls: words,
            stale,
        } => {
            match backend {
                Backend::Runner(r) => {
                    let selected = r.selection();
                    assert!(
                        r.update_controls(
                            selected.graph_epoch + u64::from(*stale),
                            selected.control_epoch,
                            words,
                            &IDS
                        )
                        .is_err(),
                        "rejected update admitted"
                    );
                }
                Backend::Graph(h) | Backend::Native(h) => {
                    assert!(
                        *stale || words.len() != h.controls.len(),
                        "host rejected valid update"
                    );
                }
            }
            false
        }
        Event::Reset(kind) => {
            let (state, new_controls) = kind.initial();
            match backend {
                Backend::Runner(r) => {
                    let s = r.selection();
                    r.reset(
                        &IDS,
                        s.graph_epoch,
                        s.control_epoch,
                        kind.graph(),
                        kind.partition(),
                        &state,
                        &new_controls,
                    )
                    .expect("explicit reset");
                }
                Backend::Graph(h) => {
                    h.graph = Some(GraphMachine::prepare(*kind));
                    h.state = state;
                    h.controls = new_controls.clone();
                    h.output = None;
                    h.next_tick = 0;
                    h.kind = *kind;
                    h.graph_epoch += 1;
                    h.control_epoch += 1;
                }
                Backend::Native(h) => {
                    h.state = state;
                    h.controls = new_controls.clone();
                    h.output = None;
                    h.next_tick = 0;
                    h.kind = *kind;
                    h.graph_epoch += 1;
                    h.control_epoch += 1;
                }
            }
            *current = *kind;
            controls.clone_from(&new_controls);
            true
        }
    }
}

fn run(
    arm: &'static str,
    kind: Kind,
    operations: &[Event],
    verify: bool,
    recording: bool,
) -> Timings {
    let whole = Instant::now();
    let prepare = Instant::now();
    let mut backend = Backend::new(arm, if kind == Kind::Sub { Kind::Add } else { kind });
    let prepare_ns = prepare.elapsed().as_nanos();
    let mut current = if kind == Kind::Sub { Kind::Add } else { kind };
    let (_, mut controls) = current.initial();
    let mut model = Model::new(current);
    let mut timing = Timings {
        by_stage: BTreeMap::new(),
        recording,
        whole_ns: 0,
        retained_bytes: 0,
        scratch_bytes: None,
        rss_kib_after: None,
        trace_digest: None,
    };
    timing.record(
        if arm == "native" {
            "native_initialization"
        } else {
            "graph_prepare"
        },
        prepare_ns,
    );
    let mut previous = "prepare";
    let mut digest = std::collections::hash_map::DefaultHasher::new();
    for event in operations {
        let before = verify.then(|| backend.snapshot(current, &controls));
        let stage = event_stage(event, backend.next_tick(), previous);
        let start = Instant::now();
        let result = run_operation(
            &mut backend,
            event,
            &mut timing,
            &mut current,
            &mut controls,
        );
        let elapsed = start.elapsed().as_nanos();
        timing.record(stage, elapsed);
        if verify {
            let expected = model.apply(event);
            assert_eq!(result, expected, "{arm}: accepted/refused event diverged");
            let observed = backend.snapshot(current, &controls);
            assert_eq!(
                observed,
                model.snapshot(),
                "{arm}: complete observation diverged after operation"
            );
            if !result {
                assert_eq!(
                    Some(&observed),
                    before.as_ref(),
                    "{arm}: refusal mutated active state"
                );
            }
            observed.hash(&mut digest);
        }
        if result {
            previous = match event {
                Event::Tick { .. } => "tick",
                Event::Update(_) => "update",
                Event::RejectUpdate { .. } => unreachable!("refusal does not advance"),
                Event::Reset(_) => "reset",
            };
        }
    }
    let (retained, scratch) = backend.footprint();
    timing.retained_bytes = retained;
    timing.scratch_bytes = scratch;
    let cleanup = Instant::now();
    drop(backend);
    timing.record("cleanup", cleanup.elapsed().as_nanos());
    timing.whole_ns = whole.elapsed().as_nanos();
    timing.rss_kib_after = resident_kib();
    if verify {
        timing.trace_digest = Some(digest.finish());
    }
    timing
}

fn resident_kib() -> Option<u64> {
    let contents = fs::read_to_string("/proc/self/status").ok()?;
    contents.lines().find_map(|line| {
        line.strip_prefix("VmRSS:")?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
    })
}

fn checked_command(name: &str, args: &[&str]) -> String {
    let output = Command::new(name)
        .args(args)
        .output()
        .expect("required identity command");
    assert!(output.status.success(), "{name} identity command refused");
    String::from_utf8(output.stdout)
        .expect("UTF-8 identity")
        .trim()
        .to_owned()
}

fn tool_digest(name: &str, environment: &str) -> (String, String) {
    let selected = std::env::var_os(environment)
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("PATH").and_then(|paths| {
                std::env::split_paths(&paths)
                    .map(|dir| dir.join(name))
                    .find(|candidate| candidate.is_file())
            })
        })
        .expect("selected compiler tool path");
    let path = selected
        .canonicalize()
        .expect("canonical compiler tool path");
    let text = path.to_str().expect("UTF-8 compiler tool path");
    let digest = checked_command("sha256sum", &[text]);
    (
        text.to_owned(),
        digest
            .split_whitespace()
            .next()
            .expect("tool digest")
            .to_owned(),
    )
}

fn source_identity(output: &Path) -> (String, String, String, String) {
    let root = PathBuf::from(checked_command("git", &["rev-parse", "--show-toplevel"]));
    assert!(output.is_absolute(), "benchmark output must be absolute");
    let parent = output
        .parent()
        .expect("output parent")
        .canonicalize()
        .expect("existing output parent");
    assert!(
        !parent.starts_with(&root),
        "receipt output must be outside the repository"
    );
    assert!(
        !output.exists(),
        "never overwrite a retained measurement receipt"
    );
    assert!(
        checked_command("git", &["status", "--porcelain", "--untracked-files=all"]).is_empty(),
        "benchmark requires committed clean source"
    );
    let commit = checked_command("git", &["rev-parse", "HEAD"]);
    let tree = checked_command("git", &["rev-parse", "HEAD^{tree}"]);
    let exe = std::env::current_exe().expect("current test executable");
    let artifact = checked_command("sha256sum", &[exe.to_str().expect("UTF-8 executable path")]);
    let artifact = artifact
        .split_whitespace()
        .next()
        .expect("sha256sum digest")
        .to_owned();
    let criteria = root.join("verification/tick-runner/continuation-criteria.md");
    let criteria_digest = checked_command(
        "sha256sum",
        &[criteria.to_str().expect("UTF-8 criteria path")],
    );
    let criteria_digest = criteria_digest
        .split_whitespace()
        .next()
        .expect("criteria digest")
        .to_owned();
    (commit, tree, artifact, criteria_digest)
}

fn quote(input: &str) -> String {
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

fn json_words(values: &[i64]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| quote(&value.to_string()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn workload(initial: Kind, operations: &[Event]) -> (String, String) {
    let rows: Vec<_> = operations
        .iter()
        .map(|event| match event {
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
        })
        .collect();
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

fn limits_json() -> String {
    let l = Limits::default();
    format!("{{\"graph_budgets_debug\":{},\"state_bytes\":{},\"input_bytes\":{},\"control_bytes\":{},\"output_bytes\":{},\"update_bytes\":{},\"retained_bytes\":{},\"schedule_work\":{},\"tick_work\":{},\"ticks\":{},\"trace_bytes\":{}}}",
        quote(&format!("{:?}", l.graph)), l.state_bytes, l.input_bytes, l.control_bytes,
        l.output_bytes, l.update_bytes, l.retained_bytes, l.schedule_work,
        l.tick_work, l.ticks, l.trace_bytes)
}

fn report_arm(rounds: &[Timings], arm: &str) -> String {
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
    let whole: Vec<_> = rounds[1..].iter().map(|row| row.whole_ns).collect();
    let raw_rounds: Vec<_> = rounds
        .iter()
        .enumerate()
        .map(|(round, row)| {
            let mut raw = String::new();
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
                "{{\"round\":{},\"whole_ns\":{},\"stage_samples_ns\":{{{}}}}}",
                round, row.whole_ns, raw
            )
        })
        .collect();
    format!("{{\"first_process_round_ns\":{},\"whole_trace\":{},\"stages\":{{{}}},\"raw_rounds\":[{}],\"retained_bytes_estimate\":{},\"retained_note\":\"excludes allocator metadata and CheckedGraph derived caches; runner uses its own conservative charged footprint\",\"scratch_bytes_declared\":{},\"scratch_note\":{},\"rss_kib_after_process_observation\":{},\"rss_peak_comparison\":\"unmeasured: multiple arms share one process\",\"allocations\":{{\"status\":\"unmeasured\",\"reason\":\"no allocation instrumentation installed\"}}}}",
        rounds[0].whole_ns, distribution(&whole), stage_json, raw_rounds.join(","),
        rounds.last().expect("round").retained_bytes,
        rounds.last().expect("round").scratch_bytes.map_or("null".to_owned(), |v| v.to_string()),
        quote(if arm == "native" { "not applicable: native code has no graph register file" }
            else { "declared packed register workspace, not a measured heap peak" }),
        rounds.last().expect("round").rss_kib_after.map_or("null".to_owned(), |v| v.to_string()))
}

fn break_even(rows: &BTreeMap<&str, Vec<Timings>>, baseline: &str, operations: &[Event]) -> String {
    let runner = rows.get("runner").expect("runner rows");
    let other = rows.get(baseline).expect("baseline rows");
    let stage = |runs: &[Timings], name: &str| -> u128 {
        median(
            &runs[1..]
                .iter()
                .map(|run| run.median(name).expect("required warm stage"))
                .collect::<Vec<_>>(),
        )
    };
    let runner_tick = stage(runner, "warm_tick");
    let other_tick = stage(other, "warm_tick");
    if other_tick <= runner_tick {
        return "null".to_owned();
    }
    let setup = |runs: &[Timings]| {
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
    };
    let incremental = setup(runner).saturating_sub(setup(other));
    let savings = other_tick - runner_tick;
    (incremental.div_ceil(savings)).to_string()
}

fn scenario_receipt(kind: Kind, length: usize) -> String {
    let operations = events(kind, length);
    let initial = if kind == Kind::Sub { Kind::Add } else { kind };
    let (workload_bytes, workload_sha256) = workload(initial, &operations);
    let digests: Vec<_> = ["runner", "graph", "native"]
        .into_iter()
        .map(|arm| {
            run(arm, kind, &operations, true, false)
                .trace_digest
                .expect("untimed full trace")
        })
        .collect();
    assert_eq!(
        digests[0], digests[1],
        "runner versus ordinary complete trace"
    );
    assert_eq!(
        digests[1], digests[2],
        "ordinary versus native complete trace"
    );
    let (initial_state, initial_controls) = initial.initial();
    let selection = {
        Runner::prepare(
            &IDS,
            initial.graph(),
            initial.partition(),
            &initial_state,
            &initial_controls,
            Limits::default(),
        )
        .expect("identity selection")
        .selection()
    };
    let replacement = if kind == Kind::Sub {
        let (state, controls) = kind.initial();
        Some(
            Runner::prepare(
                &IDS,
                kind.graph(),
                kind.partition(),
                &state,
                &controls,
                Limits::default(),
            )
            .expect("replacement identity selection")
            .selection(),
        )
    } else {
        None
    };
    let mut rows: BTreeMap<&str, Vec<Timings>> = BTreeMap::new();
    let arms = ["runner", "graph", "native"];
    for round in 0..11 {
        for offset in 0..3 {
            let arm = arms[(round + offset) % 3];
            rows.entry(arm)
                .or_default()
                .push(run(arm, kind, &operations, false, true));
        }
    }
    let runner = rows.get("runner").expect("runner rows");
    let graph = rows.get("graph").expect("graph rows");
    let native = rows.get("native").expect("native rows");
    let median_stage = |runs: &[Timings], stage: &str| {
        median(
            &runs[1..]
                .iter()
                .map(|row| row.median(stage).expect("required warm stage"))
                .collect::<Vec<_>>(),
        )
    };
    let median_whole =
        |runs: &[Timings]| median(&runs[1..].iter().map(|r| r.whole_ns).collect::<Vec<_>>());
    let time_gate = median_stage(runner, "warm_tick") * 10 <= median_stage(graph, "warm_tick") * 9
        && median_whole(runner) * 20 <= median_whole(graph) * 19
        && median_whole(runner) * 4 <= median_whole(native) * 5;
    let rg = break_even(&rows, "graph", &operations);
    let rn = break_even(&rows, "native", &operations);
    format!("{{\"scenario\":{},\"successful_ticks\":{},\"operations\":{},\"initial\":{{\"graph\":{},\"state\":{},\"controls\":{}}},\"workload_encoding\":\"UTF-8 JSON initial-plus-operations object plus trailing LF; all I64 values decimal strings\",\"workload\":{},\"workload_sha256\":{},\"limits\":{},\"graph_logical_id\":{},\"replacement_graph_logical_id\":{},\"selected_schedule_id\":{},\"replacement_schedule_id\":{},\"selection_control_id\":{},\"untimed_complete_trace_digest64\":{},\"digest_note\":\"noncryptographic equality witness; source/artifact binding uses SHA-256\",\"comparison_time_thresholds_met\":{},\"break_even_graph_ticks\":{},\"break_even_native_ticks\":{},\"decision\":\"NO-GO / inconclusive: per-arm peak RSS and allocation measurements unavailable; no general speed claim\",\"runner\":{},\"ordinary_graph\":{},\"native\":{}}}",
        quote(if kind == Kind::Sub { "reset-sub" } else { kind.name() }), length, operations.len(),
        quote(initial.name()), json_words(&initial_state), json_words(&initial_controls),
        workload_bytes.trim_end(), quote(&workload_sha256), limits_json(),
        quote(&ir::hex(&selection.graph)),
        replacement.map_or("null".to_owned(), |s| quote(&ir::hex(&s.graph))),
        quote(&ir::hex(&selection.schedule)),
        replacement.map_or("null".to_owned(), |s| quote(&ir::hex(&s.schedule))),
        quote(&ir::hex(&selection.controls)),
        quote(&format!("{:016x}", digests[0])), time_gate, rg, rn,
        report_arm(runner, "runner"), report_arm(graph, "graph"), report_arm(native, "native"))
}

#[test]
fn opt_in_source_bound_tick_cost_comparison() {
    let Some(destination) = std::env::var_os("NOBLE_TICK_BENCH_PATH") else {
        return;
    };
    let path = PathBuf::from(destination);
    let (commit, tree, executable, criteria) = source_identity(&path);
    let (rustc_path, rustc_sha) = tool_digest("rustc", "RUSTC");
    let (cargo_path, cargo_sha) = tool_digest("cargo", "CARGO");
    let rustc = checked_command(&rustc_path, &["-Vv"]);
    let mut cases = Vec::new();
    for kind in [Kind::Add, Kind::Two, Kind::Sub] {
        for length in [32, 512, 4096] {
            cases.push(scenario_receipt(kind, length));
        }
    }
    let report = format!("{{\"schema\":\"noble-tick-cost/v1\",\"profile\":\"noble-tick-i64/v1\",\"source_commit\":{},\"source_tree\":{},\"test_executable_sha256\":{},\"criteria_sha256\":{},\"rustc\":{},\"rustc_path\":{},\"rustc_sha256\":{},\"cargo_path\":{},\"cargo_sha256\":{},\"identity_scheme\":\"test-siphash13x4 (not deployed)\",\"optimization\":{},\"host\":{},\"rounds\":11,\"first_process_round\":0,\"warm_rounds\":10,\"cases\":[{}]}}\n",
        quote(&commit), quote(&tree), quote(&executable), quote(&criteria), quote(&rustc),
        quote(&rustc_path), quote(&rustc_sha), quote(&cargo_path), quote(&cargo_sha),
        quote(if cfg!(debug_assertions) { "debug assertions enabled (optimization unspecified)" }
            else { "debug assertions disabled (optimization unspecified)" }),
        quote(&checked_command("uname", &["-smr"])), cases.join(","));
    fs::write(path, report).expect("write source-bound receipt to caller-selected output");
}

/// A separately selected, single-arm process for external DHAT. Allocation
/// totals include fixture construction and test-harness effects; report the
/// zero-operation control and inspect DHAT program points before attributing
/// any delta to Runner itself. No global allocator is installed by this test.
#[test]
fn opt_in_tick_allocation_observation() {
    let Some(arm) = std::env::var("NOBLE_TICK_ALLOC_ARM").ok() else {
        return;
    };
    assert!(
        ["runner", "graph", "native"].contains(&arm.as_str()),
        "unknown allocation arm"
    );
    let length = std::env::var("NOBLE_TICK_ALLOC_LENGTH")
        .expect("explicit allocation length")
        .parse::<usize>()
        .expect("numeric allocation length");
    assert!(
        [0, 32, 512].contains(&length),
        "allocation length outside frozen probes"
    );
    assert!(
        checked_command("git", &["status", "--porcelain", "--untracked-files=all"]).is_empty(),
        "allocation probe requires committed clean source"
    );
    let commit = checked_command("git", &["rev-parse", "HEAD"]);
    let exe = std::env::current_exe().expect("exact test executable");
    let digest = checked_command("sha256sum", &[exe.to_str().expect("UTF-8 executable path")]);
    let executable_sha = digest
        .split_whitespace()
        .next()
        .expect("test executable digest");
    let operations = events(Kind::Add, length);
    let result = run(
        if arm == "runner" {
            "runner"
        } else if arm == "graph" {
            "graph"
        } else {
            "native"
        },
        Kind::Add,
        &operations,
        false,
        false,
    );
    println!("ALLOC {{\"schema\":\"noble-tick-allocation-observation/v1\",\"arm\":{},\"successful_ticks\":{},\"source_commit\":{},\"test_executable_sha256\":{},\"operation_count\":{},\"harness_included\":true,\"scratch_bytes_declared\":{},\"retained_bytes_charged\":{}}}",
        quote(&arm), length, quote(&commit), quote(executable_sha), operations.len(),
        result.scratch_bytes.map_or("null".to_owned(), |v| v.to_string()),
        result.retained_bytes);
}
