use super::*;

pub(super) struct LoadMetrics {
    pub(super) cold_ns: u64,
    pub(super) warm_ns_samples: [u64; 5],
    pub(super) warm_ns_median: u64,
}

pub(super) struct RunResults {
    pub(super) training_records: Vec<Value>,
    pub(super) result_records: Vec<Value>,
    pub(super) load: LoadMetrics,
}

fn load_model(artifact: &ModelArtifact<'_>) -> Result<LoadMetrics, String> {
    let started = std::time::Instant::now();
    let (graph, snapshot) = reload_model(&artifact.graph_text, artifact.fit.snapshot.bytes())?;
    let cold_ns = elapsed_ns(started);
    if snapshot.id() != artifact.fit.snapshot.id() || graph.id() != artifact.fit.graph.id() {
        return Err(String::from(
            "cold-loaded model differs from trained graph/snapshot",
        ));
    }
    debug_assert_eq!(snapshot.id(), artifact.fit.snapshot.id());
    debug_assert_eq!(graph.id(), artifact.fit.graph.id());
    let mut warm_ns_samples = [0u64; 5];
    for elapsed in &mut warm_ns_samples {
        let started = std::time::Instant::now();
        let (graph, snapshot) = reload_model(&artifact.graph_text, artifact.fit.snapshot.bytes())?;
        if snapshot.id() != artifact.fit.snapshot.id() || graph.id() != artifact.fit.graph.id() {
            return Err(String::from(
                "warm-loaded model differs from trained graph/snapshot",
            ));
        }
        *elapsed = elapsed_ns(started);
    }
    let mut sorted = warm_ns_samples;
    sorted.sort_unstable();
    Ok(LoadMetrics {
        cold_ns,
        warm_ns_samples,
        warm_ns_median: sorted[2],
    })
}

fn held_out_records(
    tasks: &[TaskSpec],
    libraries: &[Library],
    artifact: &ModelArtifact<'_>,
) -> Result<Vec<Value>, String> {
    let mut records = Vec::with_capacity(120 * PAIRED_SEEDS.len() * 4);
    let session = noble_contracts::source::Session::without_test_hosts();
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
                records.push(completion_mode(EvaluationRun {
                    metadata: EvaluationMeta {
                        mode,
                        repetition,
                        seed,
                        spec,
                        library,
                    },
                    session: &session,
                    fit: artifact.fit,
                    model_id: &artifact.identity,
                    graph_text: &artifact.graph_text,
                    model_payload_bytes: artifact.model_payload_bytes,
                })?);
            }
        }
    }
    debug_assert_eq!(PAIRED_SEEDS.len(), 10);
    debug_assert_eq!(records.len(), 120 * PAIRED_SEEDS.len() * 4);
    Ok(records)
}

pub(super) fn run_results(
    tasks: &[TaskSpec],
    libraries: &[Library],
    artifact: &ModelArtifact<'_>,
) -> Result<RunResults, String> {
    debug_assert_eq!(artifact.fit.training_rows.rows.len(), 280 * 4);
    debug_assert_eq!(artifact.fit.validation_rows.rows.len(), 80 * 4);
    let load = load_model(artifact)?;
    let mut training_records = Vec::with_capacity(
        artifact
            .fit
            .training_rows
            .rows
            .len()
            .saturating_add(artifact.fit.validation_rows.rows.len()),
    );
    training_records.extend(
        artifact
            .fit
            .training_rows
            .rows
            .iter()
            .map(TrainingRow::json),
    );
    training_records.extend(
        artifact
            .fit
            .validation_rows
            .rows
            .iter()
            .map(TrainingRow::json),
    );
    let result_records = held_out_records(tasks, libraries, artifact)?;
    Ok(RunResults {
        training_records,
        result_records,
        load,
    })
}
