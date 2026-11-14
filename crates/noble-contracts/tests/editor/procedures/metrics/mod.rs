#![allow(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the metrics fixture checks manifest binding, size refusals, and teacher-call claim withdrawal as one contract case"
)]

use super::super::procedures::support::{literal_record, revision};
use noble_contracts::source::procedures::{
    ArmMetrics, Metrics, ProcedureLibrary, Producer, MAX_METRICS_BYTES,
};
use noble_contracts::source::Session;
use std::string::String;
use std::vec;

#[test]
fn bind_manifest_and_withdraw_transfer_on_any_producer_call() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    library
        .admit(
            &namespace,
            None,
            literal_record(1, Producer::Chunk).map_err(|error| format!("record: {error:?}"))?,
        )
        .map_err(|error| format!("admit: {error:?}"))?;
    let manifest = library
        .identity_manifest()
        .map_err(|error| format!("identity manifest: {error:?}"))?;
    let decoded: serde_json::Value = serde_json::from_slice(&manifest)
        .map_err(|error| format!("malformed identity manifest: {error}"))?;
    assert_eq!(decoded["entries"][0]["chunk_format"], 1);
    assert_eq!(decoded["entries"][0]["interface"]["stack_out"][0], "I64");
    let arm = ArmMetrics {
        arm: String::from("accumulated-learned"),
        seed: 1,
        acquisition_entries: 1,
        acquisition_tasks: 1,
        surviving_generations: 1,
        held_out_tasks: 2,
        transfer_passed: 2,
        earlier_tasks: 3,
        regression_failed: 0,
        growth_entries_added: 1,
        growth_bytes_added: 256,
        retrieval_queries: 2,
        retrieval_examined: 4,
        max_examined: 2,
        retrieval_overhead_ns: 4_000,
        restart_identity_match: true,
        proposer_calls: 0,
        teacher_calls: 0,
        break_even_invocations: None,
        break_even_reason: String::from("no valid baseline amortization comparison"),
    };
    let metrics = Metrics {
        index_build_steps: 1,
        index_build_monotonic_ns: 100,
        arms: vec![arm.clone()],
        model_synthesis: String::from("producer receipt retained separately"),
        executed_mechanics: String::from("typed-chunk search and fresh source re-admission"),
        correspondence: String::from("no source-to-model correspondence claim"),
        assumptions: String::from("held-out family is the frozen arithScaled split"),
    };
    let clean = library
        .metrics_record(&metrics)
        .map_err(|error| format!("metrics: {error:?}"))?;
    let clean = core::str::from_utf8(&clean).map_err(|error| format!("metrics utf8: {error}"))?;
    let parsed: serde_json::Value = serde_json::from_str(clean)
        .map_err(|error| format!("malformed metrics record: {error}"))?;
    assert_eq!(parsed["transfer_valid"], true);
    assert!(clean.contains("noble-procedure-library-metrics/1"));
    assert!(clean.contains("manifest_digest"));
    assert!(clean.contains("\"transfer_valid\":true"));
    assert!(clean.contains("\"teacher_calls\":0"));
    assert!(clean.contains("\"retrieval_overhead_ns\":4000"));
    for forbidden in [
        "absolute_path",
        "process_id",
        "host_name",
        "secret",
        "teacher_config",
    ] {
        assert!(!clean.contains(forbidden));
    }

    let mut oversized_arm = metrics.clone();
    oversized_arm.arms[0].arm = "x".repeat(MAX_METRICS_BYTES.saturating_add(1));
    assert!(library.metrics_record(&oversized_arm).is_err());

    let mut oversized_claim = metrics.clone();
    oversized_claim.model_synthesis = "x".repeat(MAX_METRICS_BYTES.saturating_add(1));
    assert!(library.metrics_record(&oversized_claim).is_err());

    let mut invalid = metrics;
    let mut called = arm;
    called.teacher_calls = 1;
    invalid.arms = vec![called];
    let withdrawn = library
        .metrics_record(&invalid)
        .map_err(|error| format!("withdrawn metrics: {error:?}"))?;
    let withdrawn =
        core::str::from_utf8(&withdrawn).map_err(|error| format!("withdrawn utf8: {error}"))?;
    assert!(withdrawn.contains("\"transfer_valid\":false"));
    assert!(withdrawn.contains("\"transfer\":[]"));
    assert!(withdrawn.contains("\"teacher_calls\":1"));
    Ok(())
}
