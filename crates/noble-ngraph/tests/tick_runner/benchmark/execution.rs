use super::domain::{Event, Kind, Model, Snapshot};
use super::engines::Backend;
use std::collections::{hash_map::DefaultHasher, BTreeMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::time::Instant;

pub(super) struct Timings {
    pub(super) by_stage: BTreeMap<&'static str, Vec<u128>>,
    recording: bool,
    pub(super) whole_ns: u128,
    pub(super) whole_cpu_ns: Option<u128>,
    pub(super) retained_bytes: u64,
    pub(super) scratch_bytes: Option<u64>,
    pub(super) rss_kib_after: Option<u64>,
    pub(super) trace_digest: Option<u64>,
}

impl Timings {
    pub(super) fn record(&mut self, stage: &'static str, ns: u128) {
        if self.recording {
            self.by_stage.entry(stage).or_default().push(ns);
        }
    }

    pub(super) fn median(&self, stage: &str) -> Option<u128> {
        self.by_stage.get(stage).map(|v| median(v))
    }
}

pub(super) fn median(values: &[u128]) -> u128 {
    assert!(!values.is_empty(), "no sampled stage");
    let mut v = values.to_vec();
    v.sort_unstable();
    v[(v.len() - 1) / 2]
}

// The stage labels are a frozen part of the measurement schema.
#[allow(tigerstyle::fragile_exhaustive_enum_match)]
fn event_stage(
    event: &Event,
    tick_index: u64,
    previous: &'static str,
    is_reset_pending: bool,
) -> &'static str {
    match event {
        Event::Tick { stale: true, .. } | Event::Tick { cancel: true, .. } => "refused_tick",
        Event::Tick { .. } if is_reset_pending => "after_reset_tick",
        Event::Tick { .. } if previous == "update" => "after_update_tick",
        Event::Tick { .. } if tick_index == 0 => "first_tick",
        Event::Tick { .. } => "warm_tick",
        Event::Update(_) => "control_update",
        Event::RejectUpdate { .. } => "refused_update",
        Event::Reset(_) => "reset",
    }
}

pub(super) struct Replay {
    pub(super) backend: Backend,
    pub(super) current: Kind,
    pub(super) controls: Vec<i64>,
    model: Model,
    previous: &'static str,
    is_reset_pending: bool,
    digest: DefaultHasher,
}

impl Replay {
    fn new(backend: Backend, initial: Kind) -> Self {
        let (_, controls) = initial.initial();
        Self {
            backend,
            current: initial,
            controls,
            model: Model::new(initial),
            previous: "prepare",
            is_reset_pending: false,
            digest: DefaultHasher::new(),
        }
    }

    // Accepted events alone advance the stage-transition history.
    #[allow(tigerstyle::fragile_exhaustive_enum_match)]
    fn step(&mut self, arm: &str, event: &Event, timing: &mut Timings, verify: bool) {
        let before = verify.then(|| self.backend.snapshot(self.current, &self.controls));
        let stage = event_stage(
            event,
            self.backend.next_tick(),
            self.previous,
            self.is_reset_pending,
        );
        let start = Instant::now();
        let is_accepted = self.run_operation(event, timing);
        let elapsed = start.elapsed().as_nanos();
        timing.record(stage, elapsed);
        if verify {
            self.verify_event(arm, event, before.as_ref(), is_accepted);
        }
        if is_accepted {
            self.previous = match event {
                Event::Tick { .. } => "tick",
                Event::Update(_) => "update",
                Event::RejectUpdate { .. } => unreachable!("refusal does not advance"),
                Event::Reset(_) => "reset",
            };
            if matches!(event, Event::Reset(_)) {
                self.is_reset_pending = true;
            } else if matches!(event, Event::Tick { .. }) {
                self.is_reset_pending = false;
            }
        }
    }

    fn verify_event(
        &mut self,
        arm: &str,
        event: &Event,
        before: Option<&Snapshot>,
        is_accepted: bool,
    ) {
        let is_expected = self.model.apply(event);
        assert_eq!(
            is_accepted, is_expected,
            "{arm}: accepted/refused event diverged"
        );
        let observed = self.backend.snapshot(self.current, &self.controls);
        assert_eq!(
            observed,
            self.model.snapshot(),
            "{arm}: complete observation diverged after operation"
        );
        if !is_accepted {
            assert_eq!(
                Some(&observed),
                before,
                "{arm}: refusal mutated active state"
            );
        }
        observed.hash(&mut self.digest);
    }
}

pub(super) fn run(
    arm: &'static str,
    kind: Kind,
    operations: &[Event],
    verify: bool,
    recording: bool,
) -> Timings {
    let start_cpu_ns = thread_cpu_ns();
    let whole = Instant::now();
    let prepare = Instant::now();
    let initial = if kind == Kind::Sub { Kind::Add } else { kind };
    let backend = Backend::new(arm, initial);
    let prepare_ns = prepare.elapsed().as_nanos();
    let mut replay = Replay::new(backend, initial);
    let mut timing = Timings {
        by_stage: BTreeMap::new(),
        recording,
        whole_ns: 0,
        whole_cpu_ns: None,
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
    for event in operations {
        replay.step(arm, event, &mut timing, verify);
    }
    let (retained, scratch) = replay.backend.footprint();
    timing.retained_bytes = retained;
    timing.scratch_bytes = scratch;
    let cleanup = Instant::now();
    drop(replay.backend);
    timing.record("cleanup", cleanup.elapsed().as_nanos());
    let end_cpu_ns = thread_cpu_ns();
    timing.whole_ns = whole.elapsed().as_nanos();
    timing.whole_cpu_ns = start_cpu_ns
        .zip(end_cpu_ns)
        .and_then(|(start_ns, end_ns)| end_ns.checked_sub(start_ns))
        .filter(|elapsed_ns| *elapsed_ns > 0);
    timing.rss_kib_after = resident_kib();
    if verify {
        timing.trace_digest = Some(replay.digest.finish());
    }
    timing
}

fn thread_cpu_ns() -> Option<u128> {
    // The test runs in a harness worker thread; /proc/self/schedstat would
    // sample the process leader instead of the thread running these operations.
    let contents = fs::read_to_string("/proc/thread-self/schedstat").ok()?;
    let cpu_ns = contents.split_whitespace().next()?.parse::<u128>().ok()?;
    (cpu_ns > 0).then_some(cpu_ns)
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
