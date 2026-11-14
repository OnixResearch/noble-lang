use super::*;

pub(crate) fn optimizer_state_bytes(state: &train::AdamState) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(
        state
            .m
            .len()
            .saturating_add(state.v.len())
            .saturating_mul(core::mem::size_of::<f64>())
            .saturating_add(2 * core::mem::size_of::<f64>())
            .saturating_add(core::mem::size_of::<u64>()),
    );
    for value in &state.m {
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    for value in &state.v {
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    bytes.extend_from_slice(&state.b1t.to_bits().to_le_bytes());
    bytes.extend_from_slice(&state.b2t.to_bits().to_le_bytes());
    bytes.extend_from_slice(&state.t.to_le_bytes());
    bytes
}

pub(crate) fn fit_model(
    session: &noble_contracts::source::Session,
    libraries: &[Library],
    tasks: &[TaskSpec],
) -> Result<ModelFit, String> {
    let training_started = std::time::Instant::now();
    let training_rows = collect_split(session, libraries, tasks, "train")?;
    let validation_rows = collect_split(session, libraries, tasks, "validation")?;
    assert_eq!(training_rows.rows.len(), 280 * 4);
    assert_eq!(validation_rows.rows.len(), 80 * 4);
    assert_equal_signature_features(&training_rows.rows)?;

    let model = score_graph();
    let loss = mse_graph();
    if model.param_slots().len() != 65 || model.graph().input_slots().len() != 64 {
        return Err(String::from(
            "scorer graph differs from the 64-input/65-parameter contract",
        ));
    }
    let adam = Adam::default();
    if adam.lr.to_bits() != 0x3f947ae147ae147b
        || adam.beta1.to_bits() != 0x3feccccccccccccd
        || adam.beta2.to_bits() != 0x3feff7ced916872b
        || adam.eps.to_bits() != 0x3e45798ee2308c3a
    {
        return Err(String::from(
            "default adam/0 parameter bits differ from frozen criteria",
        ));
    }
    let train_points: Vec<Point> = training_rows.rows.iter().map(TrainingRow::point).collect();
    let validation_points: Vec<Point> = validation_rows
        .rows
        .iter()
        .map(TrainingRow::point)
        .collect();
    let mut summaries = Vec::with_capacity(TRAIN_SEEDS.len());
    let mut best: Option<(u64, f32, Trained)> = None;
    let selection_started = std::time::Instant::now();
    let mut train_scratch_peak_bytes = 0u64;
    let mut train_evaluated_nodes = 0u64;
    for seed in TRAIN_SEEDS {
        let contract = Contract {
            params: model.param_slots().to_vec(),
            model: model.clone(),
            loss: loss.clone(),
            deriv: noble_ngraph::grad::DerivOptions::default(),
            adam,
            seed,
            data: train_points.clone(),
        };
        let started = std::time::Instant::now();
        let mut scratch = noble_ngraph::eval::ScratchCounters::default();
        let mut evaluated_nodes = 0u64;
        let mut control = TrainControl {
            scratch: Some(&mut scratch),
            evaluated_nodes: Some(&mut evaluated_nodes),
            batch_size: Some(TRAIN_BATCH_SIZE),
            ..TrainControl::default()
        };
        let trained = train::train(&BLAKE3, &contract, None, TRAIN_STEPS, &mut control).map_err(
            |stopped| {
                format!(
                    "training seed {seed:016x} stopped: {:?}",
                    stopped.diagnostic
                )
            },
        )?;
        let elapsed = elapsed_ns(started);
        train_scratch_peak_bytes = train_scratch_peak_bytes.max(scratch.peak_bytes);
        train_evaluated_nodes = train_evaluated_nodes.saturating_add(evaluated_nodes);
        let validation_loss =
            validation_mean_loss(&model, &trained.snapshot, &validation_rows.rows)?;
        if !validation_loss.is_finite() {
            return Err(format!("validation loss is nonfinite for seed {seed:016x}"));
        }
        let state_bytes = optimizer_state_bytes(&trained.state);
        let summary = SeedSummary {
            seed,
            validation_loss,
            training_elapsed_ns: elapsed,
            steps: trained.provenance.steps,
            optimizer_state_digest: *blake3::hash(&state_bytes).as_bytes(),
            optimizer_state_bytes: u64::try_from(state_bytes.len()).unwrap_or(u64::MAX),
            parameter_bytes: u64::try_from(trained.snapshot.bytes().len()).unwrap_or(u64::MAX),
            provenance_data: trained.provenance.data,
        };
        summaries.push(summary);
        let is_better = match &best {
            None => true,
            Some((best_seed, best_loss, _)) => {
                validation_loss.total_cmp(best_loss).is_lt()
                    || (validation_loss.to_bits() == best_loss.to_bits() && seed < *best_seed)
            }
        };
        if is_better {
            best = Some((seed, validation_loss, trained));
        }
    }
    let (selected_seed, validation_loss, trained) =
        best.ok_or_else(|| String::from("no training seed produced a snapshot"))?;
    let model_selection_elapsed_ns = elapsed_ns(selection_started);
    if train_points.len() != 1_120 || validation_points.len() != 320 {
        return Err(String::from(
            "split point counts changed from the frozen 280/80 task sets",
        ));
    }
    Ok(ModelFit {
        graph: model,
        loss_graph: loss,
        snapshot: trained.snapshot,
        provenance: trained.provenance,
        state: (*trained.state).clone(),
        selected_seed,
        validation_loss,
        seeds: summaries,
        training_elapsed_ns: elapsed_ns(training_started),
        model_selection_elapsed_ns,
        training_rows,
        validation_rows,
        train_scratch_peak_bytes,
        train_evaluated_nodes,
    })
}

pub(crate) fn contexts_json(contexts: &[RankingContext]) -> Vec<Value> {
    contexts
        .iter()
        .map(|context| {
            json!({
                "family": context.family,
                "library_identity": hex(&context.library_identity),
                "library_revision": hex(&context.library_revision),
                "candidate_set": hex(&context.candidate_set),
                "policy": hex(&context.policy),
                "editor_request": hex(&context.editor_request),
                "task_interfaces": context.task_interfaces.iter().map(|digest| hex(digest)).collect::<Vec<_>>(),
                "allowed_candidates": context.allowed_candidates.iter().map(|candidate| json!({
                    "ordinal": candidate.ordinal,
                    "digest": hex(&candidate.digest)
                })).collect::<Vec<_>>()
            })
        })
        .collect()
}

pub(crate) fn model_identity(
    fit: &ModelFit,
    source_commit: &str,
    rustc_version: &str,
    cargo_version: &str,
) -> [u8; chunks::REVISION_BYTES] {
    let graph_text = noble_ngraph::format::graph::print(fit.graph.graph());
    let loss_text = noble_ngraph::format::graph::print(fit.loss_graph.graph());
    let state_bytes = optimizer_state_bytes(&fit.state);
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"noble.tchunk-ranking-model/1\0");
    hasher.update(source_commit.as_bytes());
    hasher.update(rustc_version.as_bytes());
    hasher.update(cargo_version.as_bytes());
    hasher.update(blake3::hash(FROZEN_CRITERIA.as_bytes()).as_bytes());
    hasher.update(blake3::hash(FROZEN_CORPUS.as_bytes()).as_bytes());
    hasher.update(graph_text.as_bytes());
    hasher.update(loss_text.as_bytes());
    hasher.update(&fit.snapshot.id());
    hasher.update(fit.snapshot.bytes());
    hasher.update(&fit.provenance.parent);
    hasher.update(&fit.provenance.graph);
    hasher.update(&fit.provenance.loss);
    hasher.update(&fit.provenance.data);
    hasher.update(&fit.provenance.optimizer_state);
    hasher.update(fit.provenance.tools.as_bytes());
    hasher.update(&fit.selected_seed.to_le_bytes());
    hasher.update(&fit.validation_loss.to_bits().to_le_bytes());
    for seed in &fit.seeds {
        hasher.update(&seed.seed.to_le_bytes());
        hasher.update(&seed.validation_loss.to_bits().to_le_bytes());
        hasher.update(&seed.provenance_data);
        hasher.update(&seed.optimizer_state_digest);
    }
    for context in &fit.training_rows.contexts {
        hasher.update(context.family.as_bytes());
        hasher.update(&context.library_identity);
        hasher.update(&context.library_revision);
        hasher.update(&context.candidate_set);
        hasher.update(&context.policy);
        hasher.update(&context.editor_request);
        for interface in &context.task_interfaces {
            hasher.update(interface);
        }
        for candidate in &context.allowed_candidates {
            hasher.update(&candidate.ordinal.to_le_bytes());
            hasher.update(&candidate.digest);
        }
    }
    hasher.update(blake3::hash(&state_bytes).as_bytes());
    *hasher.finalize().as_bytes()
}

pub(crate) fn reload_model(
    graph_text: &str,
    parameter_bytes: &[u8],
) -> Result<(CheckedGraph, ParamSnapshot), String> {
    let graph = noble_ngraph::format::graph::parse(&BLAKE3, graph_text)
        .map_err(|diagnostic| format!("serialized graph parse refused: {diagnostic:?}"))?;
    let graph = check(&BLAKE3, graph)
        .map_err(|diagnostic| format!("serialized graph check refused: {diagnostic:?}"))?;
    let snapshot = ParamSnapshot::from_parts(
        &BLAKE3,
        graph.id(),
        layout_key(&BLAKE3, graph.graph()),
        graph.param_slots().to_vec(),
        parameter_bytes.to_vec(),
    );
    snapshot
        .check_use(&graph)
        .map_err(|diagnostic| format!("serialized snapshot refused: {diagnostic:?}"))?;
    Ok((graph, snapshot))
}
