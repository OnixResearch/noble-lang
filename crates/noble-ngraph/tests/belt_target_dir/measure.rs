#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_unwrap,
    tigerstyle::no_panic,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this ignored fixed-protocol experiment records model and software-path measurements to an explicitly external receipt file."
)]

use noble_ngraph::belt::target::{self, Issue, Machine};
use noble_ngraph::check::CheckedGraph;
use noble_ngraph::ir::{GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;
use std::io::Write as _;
use std::time::Instant;

const WARMUP: usize = 20;
const ITERATIONS: usize = 200;

fn basic_case() -> (&'static str, CheckedGraph, Vec<Value>) {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "target-belt-114-measure-basic"),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let scale = builder.i64_const(-3);
    let sum = builder.op(Op::I64Add(x, y));
    let difference = builder.op(Op::I64Sub(y, x));
    let product = builder.op(Op::I64Mul(sum, scale));
    for node in [sum, difference, product] {
        builder.output(node);
    }
    (
        "reorderable-six-node",
        noble_ngraph::check(&super::common::IDS, builder.finish()).expect("fixed graph checks"),
        vec![Value::I64(5), Value::I64(3)],
    )
}

fn pressure_case() -> (&'static str, CheckedGraph, Vec<Value>) {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "target-belt-114-measure-pressure"),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let first = builder.i64_const(2);
    let second = builder.i64_const(3);
    let third = builder.i64_const(4);
    let last = builder.i64_const(5);
    let sum = builder.op(Op::I64Add(x, y));
    let result = builder.op(Op::I64Add(sum, last));
    for node in [first, second, third, result] {
        builder.output(node);
    }
    (
        "preservation-pressure-eight-node",
        noble_ngraph::check(&super::common::IDS, builder.finish()).expect("fixed graph checks"),
        vec![Value::I64(17), Value::I64(25)],
    )
}

fn mean_lower_ns(checked: &CheckedGraph, machine: Machine) -> u128 {
    let denominator = u128::try_from(ITERATIONS).expect("fixed iteration count fits u128");
    assert!(denominator != 0, "fixed iteration count is nonzero");
    for _ in 0..WARMUP {
        std::hint::black_box(
            target::lower(&super::common::IDS, checked, machine).expect("fixed profile lowers"),
        );
    }
    let start = start_clock();
    for _ in 0..ITERATIONS {
        std::hint::black_box(
            target::lower(&super::common::IDS, checked, machine).expect("fixed profile lowers"),
        );
    }
    start.elapsed().as_nanos() / denominator
}

fn mean_check_ns(checked: &CheckedGraph, machine: Machine, bytes: &[u8]) -> u128 {
    let denominator = u128::try_from(ITERATIONS).expect("fixed iteration count fits u128");
    assert!(denominator != 0, "fixed iteration count is nonzero");
    for _ in 0..WARMUP {
        std::hint::black_box(
            target::check(&super::common::IDS, checked, machine, bytes).expect("artifact checks"),
        );
    }
    let start = start_clock();
    for _ in 0..ITERATIONS {
        std::hint::black_box(
            target::check(&super::common::IDS, checked, machine, bytes).expect("artifact checks"),
        );
    }
    start.elapsed().as_nanos() / denominator
}

#[expect(
    tigerstyle::ambient_clock,
    reason = "Owner: noble-maintainers; this fixed, ignored benchmark reads monotonic time only at its measurement boundary."
)]
fn start_clock() -> Instant {
    Instant::now()
}

fn completion_cycle(machine: Machine, event: Issue) -> u32 {
    let descriptor = machine.descriptor();
    let class = descriptor
        .unit_class(event.unit)
        .expect("the emitted unit is in profile");
    event.cycle + u32::from(descriptor.completion_cycles(class)) - 1
}

fn peak_pending(machine: Machine, events: &[Issue], modeled_cycles: u32) -> usize {
    (0..modeled_cycles)
        .map(|cycle| {
            events
                .iter()
                .filter(|event| event.cycle <= cycle && completion_cycle(machine, **event) >= cycle)
                .count()
        })
        .max()
        .unwrap_or(0)
}

fn case_json(name: &str, checked: &CheckedGraph, inputs: &[Value], machine: Machine) -> String {
    let descriptor = machine.descriptor();
    let lowered =
        target::lower(&super::common::IDS, checked, machine).expect("fixed profile lowers");
    target::execute(
        &super::common::IDS,
        checked,
        machine,
        &lowered.bytes,
        inputs,
    )
    .expect("measured artifact executes");
    let decoded =
        target::decode(&super::common::IDS, &lowered.bytes).expect("measured artifact decodes");
    let utilization_denominator =
        u128::from(lowered.modeled_cycles) * u128::from(descriptor.issue_width);
    assert!(
        utilization_denominator != 0,
        "a nonempty schedule has nonzero modeled cycles and issue width"
    );
    let event_count = u128::try_from(lowered.event_count).expect("fixed workload event count fits");
    let utilization_milli = event_count * 1000 / utilization_denominator;
    format!(
        "{{\"machine\":\"{}\",\"workload\":\"{}\",\"graph_nodes\":{},\"outputs\":{},\"modeled_cycles\":{},\"issue_width\":{},\"issue_utilization_milli\":{},\"preservation_saves\":{},\"preservation_loads\":{},\"event_count\":{},\"scheduler_work\":{},\"artifact_bytes\":{},\"peak_pending_events\":{},\"max_pending_bound\":{},\"mean_lower_ns\":{},\"mean_check_ns\":{}}}",
        match machine { Machine::T1 => "T1", Machine::T2 => "T2" },
        name,
        checked.graph().nodes.len(),
        checked.graph().outputs.len(),
        lowered.modeled_cycles,
        descriptor.issue_width,
        utilization_milli,
        lowered.preservation_saves,
        lowered.preservation_loads,
        lowered.event_count,
        lowered.scheduling_work,
        lowered.bytes.len(),
        peak_pending(machine, decoded.events(), lowered.modeled_cycles),
        target::MAX_PENDING,
        mean_lower_ns(checked, machine),
        mean_check_ns(checked, machine, &lowered.bytes),
    )
}

fn report(source: &str) -> String {
    assert_eq!(source.len(), 40, "source commit is a full object ID");
    assert!(
        source.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "source commit contains only hexadecimal digits"
    );
    let cases = [basic_case(), pressure_case()];
    let mut rows = Vec::with_capacity(4);
    for (name, checked, inputs) in &cases {
        for machine in [Machine::T1, Machine::T2] {
            rows.push(case_json(name, checked, inputs, machine));
        }
    }
    format!(
        "{{\n  \"schema\": \"ngraph-belt-target-model-cost/0\",\n  \"source_commit\": \"{source}\",\n  \"profile\": \"Rust test profile (debug)\",\n  \"warmup_calls\": {WARMUP},\n  \"measured_calls_per_phase\": {ITERATIONS},\n  \"cases\": [\n    {}\n  ]\n}}\n",
        rows.join(",\n    ")
    )
}

struct MeasurementConfig {
    source_commit: String,
    destination: std::path::PathBuf,
}

#[expect(
    tigerstyle::ambient_env,
    reason = "Owner: noble-maintainers; the ignored benchmark captures only its explicit source-commit and external-report-path inputs at the test shell boundary."
)]
fn measurement_config() -> MeasurementConfig {
    MeasurementConfig {
        source_commit: std::env::var("NOBLE_TARGET_SOURCE_COMMIT")
            .expect("NOBLE_TARGET_SOURCE_COMMIT must name the committed source revision"),
        destination: std::path::PathBuf::from(
            std::env::var_os("NOBLE_TARGET_REPORT")
                .expect("NOBLE_TARGET_REPORT must name an external report file"),
        ),
    }
}

#[test]
#[ignore = "frozen model/software-path measurement; report destination and source commit are explicit environment inputs"]
fn fixed_model_cost_report() {
    let config = measurement_config();
    let destination = config.destination;
    assert!(destination.is_absolute());
    let parent = destination
        .parent()
        .expect("report has a parent directory")
        .canonicalize()
        .expect("report parent exists");
    let repository = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("the crate path exists");
    assert!(
        !parent.starts_with(repository),
        "report must be outside the repository"
    );
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .expect("the report path is new");
    output
        .write_all(report(&config.source_commit).as_bytes())
        .expect("report bytes write");
}
