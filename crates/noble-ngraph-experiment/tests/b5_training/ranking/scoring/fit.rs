use super::*;

struct TrainingInputs<'a> {
    model: &'a CheckedGraph,
    loss: &'a CheckedGraph,
    adam: Adam,
    train_points: &'a [Point],
    validation_rows: &'a [TrainingRow],
}

struct SeedTraining {
    trained: Trained,
    training_elapsed_ns: u64,
    scratch_peak_bytes: u64,
    evaluated_nodes: u64,
}

struct SeedRun {
    trained: Trained,
    summary: SeedSummary,
    scratch_peak_bytes: u64,
    evaluated_nodes: u64,
}

struct TrainingOutcome {
    best: Option<(u64, f32, Trained)>,
    summaries: Vec<SeedSummary>,
    scratch_peak_bytes: u64,
    evaluated_nodes: u64,
}

impl TrainingOutcome {
    fn include(&mut self, run: SeedRun) {
        self.scratch_peak_bytes = self.scratch_peak_bytes.max(run.scratch_peak_bytes);
        self.evaluated_nodes = self.evaluated_nodes.saturating_add(run.evaluated_nodes);
        let is_better = match &self.best {
            None => true,
            Some((best_seed, best_loss, _)) => {
                run.summary.validation_loss.total_cmp(best_loss).is_lt()
                    || (run.summary.validation_loss.to_bits() == best_loss.to_bits()
                        && run.summary.seed < *best_seed)
            }
        };
        if is_better {
            self.best = Some((run.summary.seed, run.summary.validation_loss, run.trained));
        }
        self.summaries.push(run.summary);
    }
}

fn has_frozen_adam_bits(adam: &Adam) -> bool {
    [
        adam.lr.to_bits(),
        adam.beta1.to_bits(),
        adam.beta2.to_bits(),
        adam.eps.to_bits(),
    ] == [
        0x3f947ae147ae147b,
        0x3feccccccccccccd,
        0x3feff7ced916872b,
        0x3e45798ee2308c3a,
    ]
}

fn checked_training_setup() -> Result<(CheckedGraph, CheckedGraph, Adam), String> {
    let model = score_graph();
    let loss = mse_graph();
    if model.param_slots().len() != 65 || model.graph().input_slots().len() != 64 {
        return Err(String::from(
            "scorer graph differs from the 64-input/65-parameter contract",
        ));
    }
    let adam = Adam::default();
    if !has_frozen_adam_bits(&adam) {
        return Err(String::from(
            "default adam/0 parameter bits differ from frozen criteria",
        ));
    }
    Ok((model, loss, adam))
}

fn training_contract(inputs: &TrainingInputs<'_>, seed: u64) -> Contract {
    Contract {
        params: inputs.model.param_slots().to_vec(),
        model: inputs.model.clone(),
        loss: inputs.loss.clone(),
        deriv: noble_ngraph::grad::DerivOptions::default(),
        adam: inputs.adam,
        seed,
        data: inputs.train_points.to_vec(),
    }
}

fn training_control<'a>(
    scratch: &'a mut noble_ngraph::eval::ScratchCounters,
    evaluated_nodes: &'a mut u64,
) -> TrainControl<'a> {
    TrainControl {
        scratch: Some(scratch),
        evaluated_nodes: Some(evaluated_nodes),
        batch_size: Some(TRAIN_BATCH_SIZE),
        ..TrainControl::default()
    }
}

fn train_with_counters(inputs: &TrainingInputs<'_>, seed: u64) -> Result<SeedTraining, String> {
    debug_assert!(TRAIN_SEEDS.contains(&seed));
    debug_assert_eq!(
        inputs.model.graph().input_slots().len(),
        chunks::FEATURE_LANES
    );
    let contract = training_contract(inputs, seed);
    let started = std::time::Instant::now();
    let mut scratch = noble_ngraph::eval::ScratchCounters::default();
    let mut evaluated_nodes = 0u64;
    let mut control = training_control(&mut scratch, &mut evaluated_nodes);
    let trained =
        train::train(&BLAKE3, &contract, None, TRAIN_STEPS, &mut control).map_err(|stopped| {
            format!(
                "training seed {seed:016x} stopped: {:?}",
                stopped.diagnostic
            )
        })?;
    Ok(SeedTraining {
        trained,
        training_elapsed_ns: elapsed_ns(started),
        scratch_peak_bytes: scratch.peak_bytes,
        evaluated_nodes,
    })
}

fn seed_summary(
    seed: u64,
    validation_loss: f32,
    training: &SeedTraining,
) -> Result<SeedSummary, String> {
    let state_bytes = optimizer_state_bytes(&training.trained.state);
    let optimizer_state_bytes = u64::try_from(state_bytes.len())
        .map_err(|error| format!("optimizer state byte count overflow: {error}"))?;
    let parameter_bytes = u64::try_from(training.trained.snapshot.bytes().len())
        .map_err(|error| format!("snapshot byte count overflow: {error}"))?;
    Ok(SeedSummary {
        seed,
        validation_loss,
        training_elapsed_ns: training.training_elapsed_ns,
        steps: training.trained.provenance.steps,
        optimizer_state_digest: *blake3::hash(&state_bytes).as_bytes(),
        optimizer_state_bytes,
        parameter_bytes,
        provenance_data: training.trained.provenance.data,
    })
}

fn train_one_seed(inputs: &TrainingInputs<'_>, seed: u64) -> Result<SeedRun, String> {
    let training = train_with_counters(inputs, seed)?;
    let validation_loss = validation_mean_loss(
        inputs.model,
        &training.trained.snapshot,
        inputs.validation_rows,
    )?;
    if !validation_loss.is_finite() {
        return Err(format!("validation loss is nonfinite for seed {seed:016x}"));
    }
    let summary = seed_summary(seed, validation_loss, &training)?;
    Ok(SeedRun {
        trained: training.trained,
        summary,
        scratch_peak_bytes: training.scratch_peak_bytes,
        evaluated_nodes: training.evaluated_nodes,
    })
}

fn run_all_seeds(inputs: &TrainingInputs<'_>) -> Result<(TrainingOutcome, u64), String> {
    let started = std::time::Instant::now();
    let mut outcome = TrainingOutcome {
        best: None,
        summaries: Vec::with_capacity(TRAIN_SEEDS.len()),
        scratch_peak_bytes: 0,
        evaluated_nodes: 0,
    };
    for seed in TRAIN_SEEDS {
        outcome.include(train_one_seed(inputs, seed)?);
    }
    Ok((outcome, elapsed_ns(started)))
}

pub(crate) fn train_and_select(
    session: &noble_contracts::source::Session,
    libraries: &[Library],
    tasks: &[TaskSpec],
) -> Result<ModelFit, String> {
    let training_started = std::time::Instant::now();
    let training_rows = dataset_for_split(session, libraries, tasks, "train")?;
    let validation_rows = dataset_for_split(session, libraries, tasks, "validation")?;
    assert_eq!(training_rows.rows.len(), 280 * 4);
    assert_eq!(validation_rows.rows.len(), 80 * 4);
    assert_equal_signature_features(&training_rows.rows)?;

    let (model, loss, adam) = checked_training_setup()?;
    let train_points: Vec<Point> = training_rows.rows.iter().map(TrainingRow::point).collect();
    let validation_points: Vec<Point> = validation_rows
        .rows
        .iter()
        .map(TrainingRow::point)
        .collect();
    let inputs = TrainingInputs {
        model: &model,
        loss: &loss,
        adam,
        train_points: &train_points,
        validation_rows: &validation_rows.rows,
    };
    let (outcome, model_selection_elapsed_ns) = run_all_seeds(&inputs)?;
    let (selected_seed, validation_loss, trained) = outcome
        .best
        .ok_or_else(|| String::from("no training seed produced a snapshot"))?;
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
        seeds: outcome.summaries,
        training_elapsed_ns: elapsed_ns(training_started),
        model_selection_elapsed_ns,
        training_rows,
        validation_rows,
        train_scratch_peak_bytes: outcome.scratch_peak_bytes,
        train_evaluated_nodes: outcome.evaluated_nodes,
    })
}

pub(crate) fn contexts_json(contexts: &[CandidateContext]) -> Vec<Value> {
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
