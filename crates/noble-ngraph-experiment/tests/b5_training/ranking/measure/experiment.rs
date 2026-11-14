use super::*;

#[test]
#[ignore = "runs the frozen 480-task training/evaluation experiment and writes retained evidence"]
pub(crate) fn evaluate_tchunk_ranking_experiment() -> Result<(), String> {
    let source_commit = command_text("git", &["rev-parse", "HEAD"])?;
    let dirty = command_text("git", &["status", "--porcelain"])?;
    if !dirty.is_empty() {
        return Err(format!(
            "measurement requires a clean committed tree; status was {dirty}"
        ));
    }
    let rustc_version = command_text("rustc", &["--version", "--verbose"])?;
    let cargo_version = command_text("cargo", &["--version"])?;
    let output_parent = std::env::temp_dir().join("noble-tchunk-ranking");
    std::fs::create_dir_all(&output_parent).map_err(|error| error.to_string())?;
    let output_dir = output_parent.join(format!(
        "run-{}",
        source_commit.get(..12).ok_or("short source commit")?
    ));
    let manifest = corpus_manifest();
    let frozen_corpus: Value = serde_json::from_str(FROZEN_CORPUS)
        .map_err(|error| format!("frozen corpus JSON is invalid: {error}"))?;
    if manifest != frozen_corpus {
        return Err(String::from(
            "generator differs from the committed frozen corpus",
        ));
    }
    let session = noble_contracts::source::Session::without_test_hosts();
    let tasks = corpus_tasks();
    let mut libraries = Vec::with_capacity(FAMILY_NAMES.len());
    for family in 0..FAMILY_NAMES.len() {
        libraries.push(family_library(&session, family)?);
    }

    let fit = fit_model(&session, &libraries, &tasks)?;
    let identity = model_identity(&fit, &source_commit, &rustc_version, &cargo_version);
    let graph_text = noble_ngraph::format::graph::print(fit.graph.graph());
    let loss_text = noble_ngraph::format::graph::print(fit.loss_graph.graph());
    let graph_counts = score_graph_counts(&fit.graph)?;
    let loss_counts = loss_graph_counts(&fit.loss_graph)?;
    let contexts = contexts_json(&fit.training_rows.contexts);
    let contexts_bytes = serde_json::to_vec(&contexts).map_err(|error| error.to_string())?;
    let model_payload_bytes = u64::try_from(graph_text.len())
        .unwrap_or(u64::MAX)
        .saturating_add(u64::try_from(fit.snapshot.bytes().len()).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(contexts_bytes.len()).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(chunks::FEATURE_REVISION.len()).unwrap_or(u64::MAX));
    let parameter_bytes = fit.snapshot.bytes().to_vec();

    let cold_started = std::time::Instant::now();
    let (loaded_graph, loaded_snapshot) = reload_model(&graph_text, &parameter_bytes)?;
    let cold_model_load_ns = elapsed_ns(cold_started);
    if loaded_snapshot.id() != fit.snapshot.id() || loaded_graph.id() != fit.graph.id() {
        return Err(String::from(
            "cold-loaded model differs from trained graph/snapshot",
        ));
    }
    let mut warm_model_load_samples = Vec::with_capacity(5);
    for _ in 0..5 {
        let warm_started = std::time::Instant::now();
        let (warm_graph, warm_snapshot) = reload_model(&graph_text, &parameter_bytes)?;
        if warm_snapshot.id() != fit.snapshot.id() || warm_graph.id() != fit.graph.id() {
            return Err(String::from(
                "warm-loaded model differs from trained graph/snapshot",
            ));
        }
        warm_model_load_samples.push(elapsed_ns(warm_started));
    }
    let mut sorted_warm_model_load_samples = warm_model_load_samples.clone();
    sorted_warm_model_load_samples.sort_unstable();
    let warm_model_load_median = sorted_warm_model_load_samples
        .get(sorted_warm_model_load_samples.len() / 2)
        .copied()
        .unwrap_or(0);

    let mut training_records = Vec::with_capacity(
        fit.training_rows
            .rows
            .len()
            .saturating_add(fit.validation_rows.rows.len()),
    );
    training_records.extend(fit.training_rows.rows.iter().map(TrainingRow::json));
    training_records.extend(fit.validation_rows.rows.iter().map(TrainingRow::json));

    let mut result_records = Vec::with_capacity(120 * PAIRED_SEEDS.len() * 4);
    let evaluation_session = noble_contracts::source::Session::without_test_hosts();
    for (repetition, seed) in PAIRED_SEEDS.iter().copied().enumerate() {
        for spec in tasks.iter().filter(|task| task.split() == "test") {
            let library = libraries
                .get(spec.family)
                .ok_or_else(|| format!("held-out library missing for {}", spec.id()))?;
            for mode in [
                EvaluationMode::PrimitiveOnly,
                EvaluationMode::FixedChunkShortlex,
                EvaluationMode::LearnedRanking,
                EvaluationMode::ShuffledControl,
            ] {
                result_records.push(run_completion_mode(EvaluationRun {
                    metadata: EvaluationMeta {
                        mode,
                        repetition,
                        seed,
                        spec,
                        library,
                    },
                    session: &evaluation_session,
                    fit: &fit,
                    model_id: &identity,
                    graph_text: &graph_text,
                    model_payload_bytes,
                })?);
            }
        }
    }
    std::fs::create_dir(&output_dir)
        .map_err(|error| format!("measurement output must be fresh: {error}"))?;
    let training_path = output_dir.join("training.jsonl");
    write_json_lines(&training_path, &training_records)?;
    let results_path = output_dir.join("results.jsonl");
    write_json_lines(&results_path, &result_records)?;

    let state_bytes = optimizer_state_bytes(&fit.state);
    let model = json!({
        "schema": "tchunk-ranking-model/1",
        "identity": hex(&identity),
        "source_commit": source_commit,
        "source_tree_clean": true,
        "toolchain": {
            "rustc": rustc_version,
            "cargo": cargo_version
        },
        "criteria_blake3": hex(blake3::hash(FROZEN_CRITERIA.as_bytes()).as_bytes()),
        "corpus_blake3": hex(blake3::hash(FROZEN_CORPUS.as_bytes()).as_bytes()),
        "feature_revision": chunks::FEATURE_REVISION,
        "graph_text": graph_text,
        "graph_id": hex(&fit.graph.id()),
        "graph_shape": graph_counts,
        "loss_graph_text": loss_text,
        "loss_graph_id": hex(&fit.loss_graph.id()),
        "loss_graph_shape": loss_counts,
        "parameter_snapshot": {
            "id": hex(&fit.snapshot.id()),
            "graph": hex(&fit.snapshot.graph()),
            "layout": hex(&fit.snapshot.layout()),
            "parameter_bytes": hex(&parameter_bytes),
            "parameter_byte_count": parameter_bytes.len()
        },
        "optimizer_state": {
            "digest": hex(blake3::hash(&state_bytes).as_bytes()),
            "provenance_digest": hex(&fit.provenance.optimizer_state),
            "bytes": hex(&state_bytes),
            "byte_count": state_bytes.len()
        },
        "training_provenance": {
            "subject": format!("{:?}", fit.provenance.subject),
            "graph": hex(&fit.provenance.graph),
            "loss_graph": hex(&fit.provenance.loss),
            "data": hex(&fit.provenance.data),
            "tools": fit.provenance.tools,
            "selected_seed": format!("0x{:016x}", fit.selected_seed),
            "steps": fit.provenance.steps,
            "optimizer": "adam/0",
            "derivative_policy": "grad::admit default exact-real; no boolean/index surrogate",
            "derivative_options": format!("{:?}", fit.provenance.deriv),
            "learning_rate_bits": format!("0x{:016x}", fit.provenance.optimizer.lr.to_bits()),
            "beta1_bits": format!("0x{:016x}", fit.provenance.optimizer.beta1.to_bits()),
            "beta2_bits": format!("0x{:016x}", fit.provenance.optimizer.beta2.to_bits()),
            "epsilon_bits": format!("0x{:016x}", fit.provenance.optimizer.eps.to_bits()),
            "validation_loss_bits": format!("0x{:08x}", fit.validation_loss.to_bits())
        },
        "selected_seed_summaries": fit.seeds.iter().map(|seed| json!({
            "seed": format!("0x{:016x}", seed.seed),
            "validation_loss_bits": format!("0x{:08x}", seed.validation_loss.to_bits()),
            "training_elapsed_ns": seed.training_elapsed_ns,
            "steps": seed.steps,
            "optimizer_state_digest": hex(&seed.optimizer_state_digest),
            "optimizer_state_bytes": seed.optimizer_state_bytes,
            "parameter_bytes": seed.parameter_bytes,
            "provenance_data": hex(&seed.provenance_data)
        })).collect::<Vec<_>>(),
        "contexts": contexts,
        "runtime_payload_bytes": model_payload_bytes
    });
    let model_path = output_dir.join("model.json");
    std::fs::write(
        model_path,
        serde_json::to_vec_pretty(&model).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let training_elapsed_ns = fit.training_elapsed_ns;
    let optimizer_training_ns: u64 = fit
        .seeds
        .iter()
        .map(|seed| seed.training_elapsed_ns)
        .fold(0, u64::saturating_add);
    let selection = json!({
        "schema": "tchunk-ranking-selection/1",
        "selected_seed": format!("0x{:016x}", fit.selected_seed),
        "selected_validation_loss_bits": format!("0x{:08x}", fit.validation_loss.to_bits()),
        "training_point_count": fit.training_rows.rows.len(),
        "validation_point_count": fit.validation_rows.rows.len(),
        "training_task_count": 280,
        "validation_task_count": 80,
        "training_feature_collection_ns": fit.training_rows.feature_collection_ns,
        "training_teacher_label_ns": fit.training_rows.target_label_ns,
        "validation_feature_collection_ns": fit.validation_rows.feature_collection_ns,
        "validation_teacher_label_ns": fit.validation_rows.target_label_ns,
        "total_training_elapsed_ns": training_elapsed_ns,
        "optimizer_training_elapsed_ns": optimizer_training_ns,
        "model_selection_elapsed_ns": fit.model_selection_elapsed_ns,
        "optimizer_step_count": fit.seeds.iter().map(|seed| seed.steps).sum::<u64>(),
        "train_scratch_peak_bytes": fit.train_scratch_peak_bytes,
        "train_evaluated_nodes": fit.train_evaluated_nodes,
        "training_memory": {
            "actual_peak_retained_bytes": null,
            "optimizer_state_bytes": state_bytes.len(),
            "parameter_bytes": parameter_bytes.len(),
            "scratch_peak_bytes": fit.train_scratch_peak_bytes,
            "unmeasured_reason": "process or allocator retained-memory peaks are not instrumented"
        },
        "training_cost_amortization_count": 0,
        "amortization_assumption": "none; offline training and model-selection costs stay separate from inference",
        "snapshot_selection": "lowest-validation-mean-loss; exact-f32-loss-ties-choose-lowest-seed",
        "seeds": fit.seeds.iter().map(|seed| json!({
            "seed": format!("0x{:016x}", seed.seed),
            "validation_loss_bits": format!("0x{:08x}", seed.validation_loss.to_bits()),
            "elapsed_ns": seed.training_elapsed_ns,
            "steps": seed.steps,
            "optimizer_state_bytes": seed.optimizer_state_bytes,
            "parameter_bytes": seed.parameter_bytes
        })).collect::<Vec<_>>()
    });
    let selection_path = output_dir.join("selection.json");
    std::fs::write(
        selection_path,
        serde_json::to_vec_pretty(&selection).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let receipt = json!({
        "schema": "tchunk-ranking-run/1",
        "measurement_performed": true,
        "criteria_blake3": hex(blake3::hash(FROZEN_CRITERIA.as_bytes()).as_bytes()),
        "corpus_blake3": hex(blake3::hash(FROZEN_CORPUS.as_bytes()).as_bytes()),
        "source_base": "272a41f472a46792430d3bd3e531a7c1fa295008",
        "source_commit": source_commit,
        "model_identity": hex(&identity),
        "task_count": 480,
        "train_tasks": 280,
        "validation_tasks": 80,
        "test_tasks": 120,
        "paired_repetitions": PAIRED_SEEDS.len(),
        "test_rows": result_records.len(),
        "training_rows": training_records.len(),
        "modes": ["primitive-only", "fixed-chunk-shortlex", "learned-ranking", "splitmix64-shuffled-control"],
        "model_load": {
            "cold_ns": cold_model_load_ns,
            "warm_ns_samples": warm_model_load_samples,
            "warm_ns_median": warm_model_load_median
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
            "verification_cost": fit.training_rows.target_label_ns.saturating_add(fit.validation_rows.target_label_ns)
        },
        "scope": {
            "library_growth_comparison": "not measured; all ranking modes use the exact same frozen library",
            "selection_only": true,
            "proof_or_completeness_claim": false,
            "sparse_compute_claim": false,
            "compiled_select_claim": "selects already-computed values; does not skip branch computation"
        },
        "artifacts": {
            "model": "model.json",
            "training": "training.jsonl",
            "selection": "selection.json",
            "results": "results.jsonl"
        }
    });
    std::fs::write(
        output_dir.join("receipt.json"),
        serde_json::to_vec_pretty(&receipt).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "tchunk ranking measurement retained: {}",
        output_dir.display()
    );
    Ok(())
}
