use super::*;

pub(super) fn receipt_json(artifact: &ModelArtifact<'_>, results: &RunResults,
    selection: &Value) -> Value {
    let fit = artifact.fit;
    debug_assert_eq!(results.result_records.len(), 120 * PAIRED_SEEDS.len() * 4);
    debug_assert_eq!(results.training_records.len(), 280 * 4 + 80 * 4);
    json!({
        "schema": "tchunk-ranking-run/1", "measurement_performed": true,
        "criteria_blake3": hex(blake3::hash(FROZEN_CRITERIA.as_bytes()).as_bytes()),
        "corpus_blake3": hex(blake3::hash(FROZEN_CORPUS.as_bytes()).as_bytes()),
        "source_base": "272a41f472a46792430d3bd3e531a7c1fa295008",
        "source_commit": artifact.provenance.source_commit,
        "model_identity": hex(&artifact.identity),
        "task_count": 480, "train_tasks": 280, "validation_tasks": 80, "test_tasks": 120,
        "paired_repetitions": PAIRED_SEEDS.len(),
        "test_rows": results.result_records.len(),
        "training_rows": results.training_records.len(),
        "modes": ["primitive-only", "fixed-chunk-shortlex", "learned-ranking", "splitmix64-shuffled-control"],
        "model_load": {
            "cold_ns": results.load.cold_ns,
            "warm_ns_samples": results.load.warm_ns_samples,
            "warm_ns_median": results.load.warm_ns_median
        },
        "training_summary": selection,
        "training_labeling": {
            "teacher_assisted_training_labels": true,
            "teacher_calls": fit.training_rows.rows.len().saturating_sub(280)
                .saturating_add(fit.validation_rows.rows.len().saturating_sub(80)),
            "inference_student_only": true,
            "teacher_calls_in_test_inference": 0,
            "student_only_learned_inference_rows": 1200,
            "teacher_assisted_completion_rows": 0,
            "verification_cost": fit.training_rows.target_label_ns
                .saturating_add(fit.validation_rows.target_label_ns)
        },
        "scope": {
            "library_growth_comparison": "not measured; all ranking modes use the exact same frozen library",
            "selection_only": true, "proof_or_completeness_claim": false,
            "sparse_compute_claim": false,
            "compiled_select_claim": "selects already-computed values; does not skip branch computation"
        },
        "artifacts": {
            "model": "model.json", "training": "training.jsonl",
            "selection": "selection.json", "results": "results.jsonl"
        }
    })
}
