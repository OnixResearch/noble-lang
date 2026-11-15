#![allow(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the generated matrix checks every evidence tuple in exported JSON and capability refusal against the same retained snapshot"
)]

use super::super::super::procedures::support::{literal_record, revision};
use noble_contracts::source::procedures::{
    CapabilityMarker, ConsumerObligation, ConsumerPolicy, EvidenceClass, EvidenceOutcome,
    ProcedureLibrary, Producer, Promotion, RefusalKind, Status, MAX_EVIDENCE, MAX_GENERATIONS,
    MAX_MANIFEST_BYTES, MAX_METRICS_BYTES, MAX_RECORDS,
};
use noble_contracts::source::Session;
use std::string::String;
use std::vec;

#[path = "schema/support.rs"]
mod support;

#[test]
fn generated_libraries_preserve_evidence_distinctions_without_capabilities() -> Result<(), String> {
    for seed in [1_u8, 7, 23] {
        let namespace = Session::without_test_hosts();
        let mut library = ProcedureLibrary::new(revision(seed));
        let mut retired = literal_record(i64::from(seed) + 100, Producer::Chunk)
            .map_err(|error| format!("retired fixture: {error:?}"))?;
        retired.evidence = vec![
            support::fixture_evidence(&retired, 11, EvidenceClass::Lean, EvidenceOutcome::NotRun),
            support::fixture_evidence(&retired, 11, EvidenceClass::Test, EvidenceOutcome::Failed),
            support::fixture_evidence(&retired, 12, EvidenceClass::Review, EvidenceOutcome::Passed),
            support::fixture_evidence(
                &retired,
                13,
                EvidenceClass::Assumption,
                EvidenceOutcome::Unknown,
            ),
        ];
        let retired_id = library
            .admit(&namespace, None, retired)
            .map_err(|error| format!("retired admission: {error:?}"))?;

        let mut tested = literal_record(i64::from(seed) + 200, Producer::Synthesis)
            .map_err(|error| format!("tested fixture: {error:?}"))?;
        tested.evidence = vec![
            support::fixture_evidence(&tested, 11, EvidenceClass::Lean, EvidenceOutcome::Proved),
            support::fixture_evidence(&tested, 11, EvidenceClass::Test, EvidenceOutcome::Passed),
            support::fixture_evidence(
                &tested,
                14,
                EvidenceClass::AeneasLean,
                EvidenceOutcome::Unknown,
            ),
            support::fixture_evidence(
                &tested,
                15,
                EvidenceClass::Validation,
                EvidenceOutcome::Passed,
            ),
        ];
        tested.status = Status::Tested;
        tested.refresh_retrieval_signature();
        let tested_id = library
            .admit(&namespace, None, tested)
            .map_err(|error| format!("tested admission: {error:?}"))?;

        let mut promoted = literal_record(i64::from(seed) + 300, Producer::Chunk)
            .map_err(|error| format!("promoted fixture: {error:?}"))?;
        promoted.evidence = vec![
            support::fixture_evidence(&promoted, 16, EvidenceClass::Verus, EvidenceOutcome::Proved),
            support::fixture_evidence(&promoted, 16, EvidenceClass::Test, EvidenceOutcome::Passed),
        ];
        promoted.status = Status::Promoted;
        promoted.promotion = Some(Promotion {
            policy_revision: 3,
            obligations: vec![16],
        });
        promoted.refresh_retrieval_signature();
        // A synthetic policy exercises the schema branch; claim strings are not proof attestations.
        let policy = ConsumerPolicy {
            revision: 3,
            obligations: vec![ConsumerObligation {
                id: 16,
                allowed_classes: vec![EvidenceClass::Verus],
            }],
        };
        let promoted_id = library
            .admit(&namespace, Some(&policy), promoted)
            .map_err(|error| format!("synthetic promotion admission: {error:?}"))?;

        let mut replacement = literal_record(i64::from(seed) + 400, Producer::Chunk)
            .map_err(|error| format!("replacement fixture: {error:?}"))?;
        replacement.evidence = vec![support::fixture_evidence(
            &replacement,
            99,
            EvidenceClass::Validation,
            EvidenceOutcome::Passed,
        )];
        let replacement_id = library
            .replace(&namespace, Some(&policy), retired_id, replacement)
            .map_err(|error| format!("replacement admission: {error:?}"))?;
        assert_eq!(library.records().len(), 4);
        assert_eq!(library.generation_records().len(), 5);
        assert!(library.records().len() <= MAX_RECORDS);
        assert!(library.generation_records().len() <= MAX_GENERATIONS);
        assert!(library
            .records()
            .iter()
            .all(|record| record.evidence.len() <= MAX_EVIDENCE));

        let manifest_bytes = library
            .identity_manifest()
            .map_err(|error| format!("generated manifest: {error:?}"))?;
        assert!(manifest_bytes.len() <= MAX_MANIFEST_BYTES);
        let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| format!("generated manifest JSON: {error}"))?;
        assert_eq!(manifest["schema"], "noble-procedure-library-export/1");
        assert_eq!(manifest["library"]["generation"], 4);
        let entries = manifest["entries"]
            .as_array()
            .ok_or("entries not an array")?;
        assert_eq!(entries.len(), 4);
        support::assert_no_authority_fields(&manifest);
        for (id, state, expected) in [
            (
                retired_id,
                "superseded",
                vec![
                    (11, "lean", "not-run"),
                    (11, "test", "failed"),
                    (12, "review", "passed"),
                    (13, "assumption", "unknown"),
                ],
            ),
            (
                tested_id,
                "tested",
                vec![
                    (11, "lean", "proved"),
                    (11, "test", "passed"),
                    (14, "aeneas-lean", "unknown"),
                    (15, "validation", "passed"),
                ],
            ),
            (
                promoted_id,
                "promoted",
                vec![(16, "verus", "proved"), (16, "test", "passed")],
            ),
            (
                replacement_id,
                "candidate",
                vec![(99, "validation", "passed")],
            ),
        ] {
            let identity = support::hex_identity(&id);
            let entry = entries
                .iter()
                .find(|entry| entry["procedure_id"] == identity)
                .ok_or("admitted record absent from manifest")?;
            assert_eq!(entry["status"], state);
            let actual = entry["evidence"]
                .as_array()
                .ok_or("evidence not an array")?;
            assert_eq!(actual.len(), expected.len());
            for (item, (obligation, class, outcome)) in actual.iter().zip(expected) {
                assert_eq!(
                    item,
                    &serde_json::json!({
                        "obligation": obligation,
                        "class": class,
                        "outcome": outcome,
                        "toolchain": ["synthetic-schema-fixture"],
                        "claim": "unverified fixture reference; not a checked proof",
                        "subject_binding": identity,
                        "policy_revision": 3,
                    })
                );
            }
            assert_eq!(entry["promotion"].is_object(), state == "promoted");
            assert_eq!(entry["interface"]["effects"], serde_json::json!([]));
        }
        assert_eq!(
            manifest["retrieval_order"].as_array().map(Vec::len),
            Some(4)
        );

        let metrics_bytes = library
            .metrics_record(&support::sample_report(seed))
            .map_err(|error| format!("generated metrics: {error:?}"))?;
        assert!(metrics_bytes.len() <= MAX_METRICS_BYTES);
        let parsed_metrics: serde_json::Value = serde_json::from_slice(&metrics_bytes)
            .map_err(|error| format!("generated metrics JSON: {error}"))?;
        assert_eq!(
            parsed_metrics["schema"],
            "noble-procedure-library-metrics/1"
        );
        assert_eq!(
            parsed_metrics["generation"],
            manifest["library"]["generation"]
        );
        assert_eq!(parsed_metrics["index"]["entries"].as_u64(), Some(4));
        assert_eq!(parsed_metrics["teacher_calls"], 0);
        support::assert_no_authority_fields(&parsed_metrics);

        let before_storage = library
            .encode()
            .map_err(|error| format!("encode: {error:?}"))?;
        for marker in [
            CapabilityMarker::LiveCapability,
            CapabilityMarker::ServiceHandle,
            CapabilityMarker::Session,
        ] {
            let mut injected = literal_record(i64::from(seed) + 500, Producer::Chunk)
                .map_err(|error| format!("injection fixture: {error:?}"))?;
            let mut evidence = support::fixture_evidence(
                &injected,
                11,
                EvidenceClass::Test,
                EvidenceOutcome::Passed,
            );
            evidence.capability = Some(marker);
            injected.evidence.push(evidence);
            let refused = library
                .admit(&namespace, Some(&policy), injected)
                .expect_err("capability-bearing record must refuse before retention");
            assert_eq!(refused.kind, RefusalKind::LiveCapabilityInEvidence);
            assert_eq!(refused.location.field, "evidence.capability");
            assert_eq!(
                library
                    .identity_manifest()
                    .map_err(|error| format!("manifest after refusal: {error:?}"))?,
                manifest_bytes
            );
            assert_eq!(
                library
                    .encode()
                    .map_err(|error| format!("encode after refusal: {error:?}"))?,
                before_storage
            );
        }
    }
    Ok(())
}
