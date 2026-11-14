#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    tigerstyle::assertion_density,
    tigerstyle::ambiguous_params,
    tigerstyle::fallible_int_fallback,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; this integration harness reads frozen fixtures and prices their measured costs, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! End-to-end cost accounting for certified graph rewrites (issue #64 items 5-7).
//!
//! The quality lane proves that correctness alone cannot satisfy a quality target. This lane prices
//! both raw certificate components and the fail-closed admitted execution pipeline. Certificate
//! validity alone is not executable admission: unreachable source/candidate nodes and unsupported
//! derivations stay in the denominator, with later stages explicitly not reached.
//!
//! The continuation thresholds come from the frozen manifest
//! `verification/ngraph/rewrite-cost-v4.txt`. Fresh attempts and reused compiled/workspace execution
//! are measured separately; no OS-cache-cold, allocator/RSS or Lean correspondence claim is made.

mod common;

#[path = "rewrite_cost/fixtures.rs"]
mod fixtures;
#[path = "rewrite_cost/measure.rs"]
mod measure;
#[path = "rewrite_cost/pipeline.rs"]
mod pipeline;
#[path = "rewrite_cost/report.rs"]
mod report;
#[path = "rewrite_cost/scoped.rs"]
mod scoped;

#[test]
fn certified_rewrite_costs_are_reported_apart_from_validity_and_coverage(
) -> Result<(), Box<dyn std::error::Error>> {
    let (manifest, corpus) = fixtures::load()?;
    let threshold = manifest.threshold.as_ref().ok_or("threshold")?;
    let samples = 200;
    let run = measure::collect(&corpus, threshold, samples)?;
    assert!(
        run.reached_target > 0,
        "the corpus must contain at least one reachable target, or the thresholds are vacuous"
    );
    assert_eq!(run.accepted + run.refused, corpus.fixtures.len() * 4);
    assert!(run.pipeline.executed > 0, "candidate execution is measured");
    let report = report::render(&manifest, threshold, samples, &run);
    println!("{report}");
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this helper reads the frozen fixture files and allocates graph nodes at runtime."
)]
fn corpus_graph(name: &str) -> noble_ngraph::Graph {
    let (_, corpus) = fixtures::load().expect("frozen corpus");
    let fixture = corpus
        .fixtures
        .iter()
        .find(|fixture| fixture.name == name)
        .expect("named fixture");
    common::rewrite_corpus::build(&common::IDS, fixture).expect("fixture graph")
}

#[test]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this regression calls the runtime proposer, admission checks and measured execution pipeline."
)]
fn a_chained_non_identity_certificate_passes_admission_and_executes() {
    let graph = corpus_graph("single-fold");
    let (certificate, candidate) = noble_ngraph::cert::propose_chained(
        &common::IDS,
        noble_ngraph::cert::Proposer::FirstEligible,
        &graph,
    )
    .expect("bounded chained proposal");
    assert_eq!(
        noble_ngraph::cert::check_chained(&common::IDS, &graph, &candidate, &certificate),
        Ok(())
    );
    let measured = pipeline::measure(&graph, noble_ngraph::cert::Proposer::FirstEligible, 2);
    assert_eq!(measured.disposition, "executed");
    assert!(measured.candidate_execution_calls > 0);
    assert!(measured.baseline_execution_calls > 0);
}

#[test]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this regression executes source and certificate refusal paths and observes actual evaluation counters."
)]
fn source_refusals_stop_and_chained_candidates_execute() {
    let mut invalid_source = corpus_graph("independent-folds");
    invalid_source.outputs = vec![noble_ngraph::ir::NodeId(5)];
    let input = pipeline::measure(&invalid_source, noble_ngraph::cert::Proposer::Identity, 2);
    assert_eq!(input.disposition, "input-refused");
    assert_eq!(input.baseline_execution_calls, 0);
    assert_eq!(input.candidate_execution_calls, 0);

    let chained = corpus_graph("chained-folds");
    let certificate = pipeline::measure(&chained, noble_ngraph::cert::Proposer::FixedPoint, 2);
    assert_eq!(certificate.disposition, "executed");
    assert!(certificate.baseline_execution_calls > 0);
    assert!(certificate.candidate_execution_calls > 0);
}

#[test]
fn v2_ordered_repeated_and_chained_candidates_execute_in_output_order() {
    let cases: [(&str, &[u32], &[i64]); 4] = [
        ("independent-folds", &[2, 5], &[12, 7]),
        ("ordered-outputs", &[4, 2], &[12, 5]),
        ("chained-folds", &[4], &[15]),
        ("repeated-outputs", &[2, 2], &[12, 12]),
    ];
    for (name, expected_nodes, expected_values) in cases {
        let graph = corpus_graph(name);
        assert_eq!(
            graph
                .outputs
                .iter()
                .map(|output| output.0)
                .collect::<Vec<_>>(),
            expected_nodes,
            "{name}: output order and multiplicity are frozen"
        );
        let checked = noble_ngraph::check(&common::IDS, graph.clone())
            .expect("each explicit-output graph is admitted by the source checker");
        let observed =
            noble_ngraph::reference(&checked, &[], None, &mut noble_ngraph::Control::default());
        assert_eq!(
            observed.obs,
            noble_ngraph::Obs::Normal(
                expected_values
                    .iter()
                    .copied()
                    .map(noble_ngraph::Value::I64)
                    .collect()
            ),
            "{name}: evaluated values preserve the declared output vector"
        );
        let (certificate, candidate) = noble_ngraph::cert::propose_chained(
            &common::IDS,
            noble_ngraph::cert::Proposer::FixedPoint,
            &graph,
        )
        .expect("bounded chained proposal");
        assert_eq!(
            noble_ngraph::cert::check_chained(&common::IDS, &graph, &candidate, &certificate),
            Ok(()),
            "{name}: the independent v2 checker accepts its pruned endpoint"
        );
        let measured = pipeline::measure(
            &graph,
            noble_ngraph::cert::Proposer::FixedPoint,
            2,
        );
        assert_eq!(measured.disposition, "executed", "{name}");
        assert!(measured.candidate_execution_calls > 0, "{name}");
    }
}

#[test]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this regression invokes the runtime reference evaluator and packed execution pipeline."
)]
fn an_admitted_wrapping_identity_executes_both_paths() {
    let graph = corpus_graph("wrapping-boundary");
    let checked = noble_ngraph::check(&common::IDS, graph.clone()).expect("input admitted");
    let expected =
        noble_ngraph::reference(&checked, &[], None, &mut noble_ngraph::Control::default());
    assert_eq!(
        expected.obs,
        noble_ngraph::Obs::Normal(vec![noble_ngraph::Value::I64(i64::MIN)])
    );
    let measured = pipeline::measure(&graph, noble_ngraph::cert::Proposer::Identity, 2);
    assert_eq!(measured.disposition, "executed");
    assert!(measured.baseline_execution_calls > 0);
    assert!(measured.candidate_execution_calls > 0);
}
