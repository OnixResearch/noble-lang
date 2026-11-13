#![allow(
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::assertion_density,
    tigerstyle::numeric_units,
    reason = "Owner: noble-maintainers; this ignored issue #108 measurement keeps its bounded fixtures, admission assertions, timing scopes and report format together in one test module."
)]

use std::time::Instant;

use noble_ngraph::cert_scoped::{self, Derivation, Outcome};
use noble_ngraph::ir::{
    Fact, Graph, GraphBuilder, Literal, Op, Provenance, ScalarType, SubProfile, Subject,
};

const PRIMING_ATTEMPTS: usize = 64;
const SAMPLES: usize = 200;
const MICROSECONDS_PER_SECOND: f64 = 1_000_000.0;

fn exact(value: i64) -> Fact {
    Fact::Range {
        lo: Literal::I64(value),
        hi: Literal::I64(value),
    }
}

fn guarded_input_graph() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "scoped-cost-guarded-input"),
    );
    let input = builder.input(ScalarType::I64);
    let seven = builder.i64_const(7);
    builder.guard(exact(0), input);
    let sum = builder.op(Op::I64Add(input, seven));
    let product = builder.op(Op::I64Mul(input, sum));
    builder.output(seven);
    builder.output(sum);
    builder.output(product);
    builder.finish()
}

fn input_fact_graph() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "scoped-cost-input-fact"),
    );
    let input = builder.input(ScalarType::I64);
    let nine = builder.i64_const(9);
    let difference = builder.op(Op::I64Sub(nine, input));
    let product = builder.op(Op::I64Mul(input, difference));
    builder.fact(input, exact(3), Provenance::InputValidated);
    builder.output(nine);
    builder.output(difference);
    builder.output(product);
    builder.finish()
}

struct Timings {
    proposal_us: Vec<f64>,
    checker_us: Vec<f64>,
    evidence_bytes: usize,
    steps: usize,
}

fn admitted(graph: &Graph, label: &str) {
    noble_ngraph::check::check(&super::common::IDS, graph.clone())
        .unwrap_or_else(|diagnostic| panic!("{label} endpoint is not admitted: {diagnostic:?}"));
}

#[expect(
    tigerstyle::ambient_clock,
    reason = "Owner: noble-maintainers; this test-only helper measures real scoped certificate proposal and checking."
)]
fn attempt(
    graph: &Graph,
    proposal_samples: &mut [f64],
    checker_samples: &mut [f64],
    sample_index: usize,
) -> (usize, usize) {
    let started = Instant::now();
    let (derivation, candidate): (Derivation, Graph) =
        cert_scoped::propose(&super::common::IDS, graph);
    let proposal_elapsed = started.elapsed().as_secs_f64() * MICROSECONDS_PER_SECOND;
    assert!(
        !derivation.steps.is_empty(),
        "the fixture must produce at least one scoped rewrite"
    );

    let started = Instant::now();
    let outcome = cert_scoped::check(&super::common::IDS, graph, &candidate, &derivation);
    let checker_elapsed = started.elapsed().as_secs_f64() * MICROSECONDS_PER_SECOND;
    assert_eq!(
        outcome,
        Outcome::Accepted,
        "only accepted certificates are measured"
    );
    admitted(&candidate, "candidate");

    proposal_samples[sample_index] = proposal_elapsed;
    checker_samples[sample_index] = checker_elapsed;
    let evidence_bytes = std::mem::size_of_val(&derivation)
        .saturating_add(derivation.revision.capacity())
        .saturating_add(
            derivation
                .steps
                .capacity()
                .saturating_mul(std::mem::size_of::<cert_scoped::Step>()),
        );
    (evidence_bytes, derivation.steps.len())
}

fn summary(values: &mut [f64]) -> (f64, f64) {
    values.sort_unstable_by(f64::total_cmp);
    let lower = values[values.len() / 4];
    let median = values[values.len() / 2];
    let upper = values[3 * values.len() / 4];
    (median, upper - lower)
}

fn measure(graph: Graph) -> Timings {
    admitted(&graph, "source");
    let mut proposal_samples = vec![0.0; SAMPLES];
    let mut checker_samples = vec![0.0; SAMPLES];
    let mut evidence_bytes = 0;
    let mut steps = 0;

    let mut unrecorded_proposal = vec![0.0; PRIMING_ATTEMPTS];
    let mut unrecorded_checker = vec![0.0; PRIMING_ATTEMPTS];
    for sample_index in 0..PRIMING_ATTEMPTS {
        (evidence_bytes, steps) = attempt(
            &graph,
            &mut unrecorded_proposal,
            &mut unrecorded_checker,
            sample_index,
        );
    }
    for sample_index in 0..SAMPLES {
        (evidence_bytes, steps) = attempt(
            &graph,
            &mut proposal_samples,
            &mut checker_samples,
            sample_index,
        );
    }
    assert_eq!(proposal_samples.len(), SAMPLES);
    assert_eq!(checker_samples.len(), SAMPLES);
    Timings {
        proposal_us: proposal_samples,
        checker_us: checker_samples,
        evidence_bytes,
        steps,
    }
}

fn render_row(label: &str, timings: &mut Timings) -> String {
    let (proposal_median, proposal_iqr) = summary(&mut timings.proposal_us);
    let (checker_median, checker_iqr) = summary(&mut timings.checker_us);
    format!(
        "    {{\"fixture\":\"{label}\",\"source_admitted\":true,\"certificate_accepted\":true,\"candidate_admitted\":true,\"samples\":{SAMPLES},\"priming_attempts\":{PRIMING_ATTEMPTS},\"steps\":{},\"proposal_median_us\":{proposal_median:.3},\"proposal_iqr_us\":{proposal_iqr:.3},\"checker_median_us\":{checker_median:.3},\"checker_iqr_us\":{checker_iqr:.3},\"evidence_structural_bytes\":{}}}",
        timings.steps, timings.evidence_bytes
    )
}

#[test]
#[ignore = "writes a descriptive #108 scoped-proposer/checker cost measurement to NOBLE_SCOPED_COST_OUT"]
#[expect(
    tigerstyle::ambient_env,
    reason = "Owner: noble-maintainers; this ignored evidence producer reads its frozen runner inputs at the test boundary."
)]
fn measure_proposer_and_checker() {
    let source_commit =
        std::env::var("NOBLE_SOURCE_COMMIT").expect("set the measured source commit");
    let source_tree = std::env::var("NOBLE_SOURCE_TREE").expect("set the measured source tree");
    let source_clean =
        std::env::var("NOBLE_SOURCE_CLEAN").expect("verify source cleanliness before measurement");
    assert_eq!(
        source_clean, "true",
        "cost measurements must run from a clean source commit"
    );
    let manifest_sha256 = std::env::var("NOBLE_SCOPED_COST_MANIFEST_SHA256")
        .expect("set the frozen protocol SHA-256");
    let output =
        std::env::var("NOBLE_SCOPED_COST_OUT").expect("set an output path outside the worktree");

    let mut guarded = measure(guarded_input_graph());
    let mut preflight = measure(input_fact_graph());
    let report = format!(
        "{{\n  \"schema\":\"noble-scoped-rewrite-cost-receipt/v1\",\n  \"issue\":108,\n  \"related_issue\":64,\n  \"source_commit\":\"{source_commit}\",\n  \"source_tree\":\"{source_tree}\",\n  \"clean_source\":true,\n  \"manifest\":\"verification/ngraph/scoped-rewrite-cost-v1.txt\",\n  \"manifest_sha256\":\"{manifest_sha256}\",\n  \"identity_scheme\":\"existing test-siphash13x4\",\n  \"timing_profile\":\"optimized release\",\n  \"timing_scope\":\"isolated cert_scoped::propose and cert_scoped::check calls; admission asserted outside timers on every retained sample\",\n  \"rows\":[\n{},\n{}\n  ]\n}}\n",
        render_row("guarded-input", &mut guarded),
        render_row("input-fact", &mut preflight)
    );
    std::fs::write(output, report).expect("measurement report written outside worktree");
}
