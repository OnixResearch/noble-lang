use super::*;

pub(super) struct RunProvenance {
    pub(super) source_commit: String,
    pub(super) rustc_version: String,
    pub(super) cargo_version: String,
}

pub(super) struct ModelArtifact<'a> {
    pub(super) fit: &'a ModelFit,
    pub(super) provenance: &'a RunProvenance,
    pub(super) identity: [u8; chunks::REVISION_BYTES],
    pub(super) graph_text: String,
    pub(super) loss_text: String,
    pub(super) contexts: Vec<Value>,
    pub(super) state_bytes: Vec<u8>,
    pub(super) model_payload_bytes: u64,
}

pub(super) fn model_artifact<'a>(fit: &'a ModelFit, provenance: &'a RunProvenance) -> Result<ModelArtifact<'a>, String> {
    debug_assert_eq!(fit.graph.graph().input_slots().len(), 64);
    debug_assert_eq!(fit.graph.param_slots().len(), 65);
    let identity = model_identity(fit, &provenance.source_commit,
        &provenance.rustc_version, &provenance.cargo_version);
    let graph_text = noble_ngraph::format::graph::print(fit.graph.graph());
    let loss_text = noble_ngraph::format::graph::print(fit.loss_graph.graph());
    let contexts = contexts_json(&fit.training_rows.contexts);
    let contexts_bytes = serde_json::to_vec(&contexts).map_err(|error| error.to_string())?;
    let model_payload_bytes = u64::try_from(graph_text.len()).expect("in-memory graph size")
        .saturating_add(u64::try_from(fit.snapshot.bytes().len()).expect("in-memory snapshot size"))
        .saturating_add(u64::try_from(contexts_bytes.len()).expect("in-memory contexts size"))
        .saturating_add(u64::try_from(chunks::FEATURE_REVISION.len()).expect("static revision size"));
    Ok(ModelArtifact { fit, provenance, identity, graph_text, loss_text,
        contexts, state_bytes: optimizer_state_bytes(&fit.state), model_payload_bytes })
}

pub(super) fn model_json(artifact: &ModelArtifact<'_>) -> Result<Value, String> {
    let fit = artifact.fit;
    debug_assert_eq!(fit.snapshot.graph(), fit.graph.id());
    debug_assert!(!artifact.graph_text.is_empty());
    let graph_counts = score_graph_counts(&fit.graph)?;
    let loss_counts = loss_graph_counts(&fit.loss_graph)?;
    Ok(json!({
        "schema": "tchunk-ranking-model/1", "identity": hex(&artifact.identity),
        "source_commit": artifact.provenance.source_commit, "source_tree_clean": true,
        "toolchain": {"rustc": artifact.provenance.rustc_version,
            "cargo": artifact.provenance.cargo_version},
        "criteria_blake3": hex(blake3::hash(FROZEN_CRITERIA.as_bytes()).as_bytes()),
        "corpus_blake3": hex(blake3::hash(FROZEN_CORPUS.as_bytes()).as_bytes()),
        "feature_revision": chunks::FEATURE_REVISION,
        "graph_text": artifact.graph_text, "graph_id": hex(&fit.graph.id()),
        "graph_shape": graph_counts, "loss_graph_text": artifact.loss_text,
        "loss_graph_id": hex(&fit.loss_graph.id()), "loss_graph_shape": loss_counts,
        "parameter_snapshot": {
            "id": hex(&fit.snapshot.id()), "graph": hex(&fit.snapshot.graph()),
            "layout": hex(&fit.snapshot.layout()), "parameter_bytes": hex(fit.snapshot.bytes()),
            "parameter_byte_count": fit.snapshot.bytes().len()
        },
        "optimizer_state": {
            "digest": hex(blake3::hash(&artifact.state_bytes).as_bytes()),
            "provenance_digest": hex(&fit.provenance.optimizer_state),
            "bytes": hex(&artifact.state_bytes), "byte_count": artifact.state_bytes.len()
        },
        "training_provenance": {
            "subject": format!("{:?}", fit.provenance.subject),
            "graph": hex(&fit.provenance.graph), "loss_graph": hex(&fit.provenance.loss),
            "data": hex(&fit.provenance.data), "tools": fit.provenance.tools,
            "selected_seed": format!("0x{:016x}", fit.selected_seed),
            "steps": fit.provenance.steps, "optimizer": "adam/0",
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
            "training_elapsed_ns": seed.training_elapsed_ns, "steps": seed.steps,
            "optimizer_state_digest": hex(&seed.optimizer_state_digest),
            "optimizer_state_bytes": seed.optimizer_state_bytes,
            "parameter_bytes": seed.parameter_bytes,
            "provenance_data": hex(&seed.provenance_data)
        })).collect::<Vec<_>>(),
        "contexts": artifact.contexts,
        "runtime_payload_bytes": artifact.model_payload_bytes
    }))
}

pub(super) fn selection_json(artifact: &ModelArtifact<'_>) -> Value {
    let fit = artifact.fit;
    let optimizer_training_ns: u64 = fit.seeds.iter()
        .map(|seed| seed.training_elapsed_ns).fold(0, u64::saturating_add);
    debug_assert_eq!(fit.training_rows.rows.len(), 280 * 4);
    debug_assert_eq!(fit.validation_rows.rows.len(), 80 * 4);
    json!({
        "schema": "tchunk-ranking-selection/1",
        "selected_seed": format!("0x{:016x}", fit.selected_seed),
        "selected_validation_loss_bits": format!("0x{:08x}", fit.validation_loss.to_bits()),
        "training_point_count": fit.training_rows.rows.len(),
        "validation_point_count": fit.validation_rows.rows.len(),
        "training_task_count": 280, "validation_task_count": 80,
        "training_feature_collection_ns": fit.training_rows.feature_collection_ns,
        "training_teacher_label_ns": fit.training_rows.target_label_ns,
        "validation_feature_collection_ns": fit.validation_rows.feature_collection_ns,
        "validation_teacher_label_ns": fit.validation_rows.target_label_ns,
        "total_training_elapsed_ns": fit.training_elapsed_ns,
        "optimizer_training_elapsed_ns": optimizer_training_ns,
        "model_selection_elapsed_ns": fit.model_selection_elapsed_ns,
        "optimizer_step_count": fit.seeds.iter().map(|seed| seed.steps).sum::<u64>(),
        "train_scratch_peak_bytes": fit.train_scratch_peak_bytes,
        "train_evaluated_nodes": fit.train_evaluated_nodes,
        "training_memory": {
            "actual_peak_retained_bytes": null,
            "optimizer_state_bytes": artifact.state_bytes.len(),
            "parameter_bytes": fit.snapshot.bytes().len(),
            "scratch_peak_bytes": fit.train_scratch_peak_bytes,
            "unmeasured_reason": "process or allocator retained-memory peaks are not instrumented"
        },
        "training_cost_amortization_count": 0,
        "amortization_assumption": "none; offline training and model-selection costs stay separate from inference",
        "snapshot_selection": "lowest-validation-mean-loss; exact-f32-loss-ties-choose-lowest-seed",
        "seeds": fit.seeds.iter().map(|seed| json!({
            "seed": format!("0x{:016x}", seed.seed),
            "validation_loss_bits": format!("0x{:08x}", seed.validation_loss.to_bits()),
            "elapsed_ns": seed.training_elapsed_ns, "steps": seed.steps,
            "optimizer_state_bytes": seed.optimizer_state_bytes,
            "parameter_bytes": seed.parameter_bytes
        })).collect::<Vec<_>>()
    })
}
