use noble_contracts::source::procedures::{
    ArmMetrics, EvidenceClass, EvidenceOutcome, EvidenceRecord, Metrics, ProcedureRecord,
};
use std::string::String;
use std::vec;
use std::vec::Vec;

pub(super) fn fixture_evidence(
    record: &ProcedureRecord,
    obligation: u32,
    class: EvidenceClass,
    outcome: EvidenceOutcome,
) -> EvidenceRecord {
    EvidenceRecord {
        obligation,
        class,
        outcome,
        toolchain: vec![String::from("synthetic-schema-fixture")],
        claim: String::from("unverified fixture reference; not a checked proof"),
        subject_binding: record.procedure_id,
        policy_revision: 3,
        capability: None,
    }
}

pub(super) fn hex_identity(id: &[u8; 32]) -> String {
    id.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn assert_no_authority_fields(root: &serde_json::Value) {
    assert!(root.is_object(), "library export root must be an object");
    let mut pending: Vec<&serde_json::Value> = vec![root];
    while let Some(value) = pending.pop() {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, child) in fields {
                    let lower = key.to_ascii_lowercase();
                    for forbidden in [
                        "capability",
                        "handle",
                        "secret",
                        "training",
                        "teacher_config",
                        "private_key",
                    ] {
                        assert!(
                            !lower.contains(forbidden),
                            "authority field exported: {key}"
                        );
                    }
                    pending.push(child);
                }
            }
            serde_json::Value::Array(items) => pending.extend(items),
            _ => {}
        }
    }
}

pub(super) fn sample_report(seed: u8) -> Metrics {
    Metrics {
        index_build_steps: 1,
        index_build_monotonic_ns: 0,
        arms: vec![ArmMetrics {
            arm: String::from("schema-fixture"),
            seed: u64::from(seed),
            acquisition_entries: 4,
            acquisition_tasks: 4,
            surviving_generations: 5,
            held_out_tasks: 0,
            transfer_passed: 0,
            earlier_tasks: 0,
            regression_failed: 0,
            growth_entries_added: 4,
            growth_bytes_added: 0,
            retrieval_queries: 0,
            retrieval_examined: 0,
            max_examined: 0,
            retrieval_overhead_ns: 0,
            restart_identity_match: true,
            proposer_calls: 0,
            teacher_calls: 0,
            break_even_invocations: None,
            break_even_reason: String::from("no runtime comparison"),
        }],
        model_synthesis: String::from("no model synthesis measured"),
        executed_mechanics: String::from("schema-only fixture"),
        correspondence: String::from("no implementation proof inferred"),
        assumptions: String::from("synthetic evidence references are not proof attestations"),
    }
}
