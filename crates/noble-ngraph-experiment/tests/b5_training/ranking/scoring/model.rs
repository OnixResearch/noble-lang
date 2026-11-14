use super::*;

pub(crate) const TRAIN_SEEDS: [u64; 3] =
    [0x74031a7e20261029, 0x74031a7e2026102a, 0x74031a7e2026102b];
pub(crate) const PAIRED_SEEDS: [u64; 10] = [
    0x7403000000000001,
    0x7403000000000002,
    0x7403000000000003,
    0x7403000000000004,
    0x7403000000000005,
    0x7403000000000006,
    0x7403000000000007,
    0x7403000000000008,
    0x7403000000000009,
    0x740300000000000a,
];
pub(crate) const TRAIN_STEPS: u64 = 256;
pub(crate) const TRAIN_BATCH_SIZE: u32 = 32;

pub(crate) fn score_graph() -> CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "tchunk-ranking/score-v1"),
    );
    let products = weighted_terms(&mut builder);
    let mut total = products[0];
    for product in products.iter().skip(1) {
        total = builder.op(noble_ngraph::ir::Op::F32Add(total, *product));
    }
    let bias = builder.param();
    let output = builder.op(noble_ngraph::ir::Op::F32Add(total, bias));
    builder.output(output);
    check(&BLAKE3, builder.finish()).expect("fixed scorer graph admits")
}

fn weighted_terms(builder: &mut GraphBuilder) -> Vec<noble_ngraph::ir::NodeId> {
    let inputs: Vec<_> = (0..chunks::FEATURE_LANES)
        .map(|_| builder.input(ScalarType::F32))
        .collect();
    inputs
        .into_iter()
        .map(|input| {
            let parameter = builder.param();
            builder.op(noble_ngraph::ir::Op::F32Mul(parameter, input))
        })
        .collect()
}

pub(crate) fn mse_graph() -> CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "tchunk-ranking/loss-v1"),
    );
    let score = builder.input(ScalarType::F32);
    let target = builder.input(ScalarType::F32);
    let difference = builder.op(noble_ngraph::ir::Op::F32Sub(score, target));
    let loss = builder.op(noble_ngraph::ir::Op::F32Mul(difference, difference));
    builder.output(loss);
    check(&BLAKE3, builder.finish()).expect("fixed MSE loss graph admits")
}

pub(crate) fn numeric_output(
    model: &CheckedGraph,
    snapshot: &ParamSnapshot,
    inputs: &[f32],
) -> Result<f32, String> {
    let mut scratch = noble_ngraph::eval::ScratchCounters::default();
    let mut evaluated_nodes = 0u64;
    numeric_output_traced(model, snapshot, inputs, &mut scratch, &mut evaluated_nodes)
}

pub(crate) fn numeric_output_traced(
    model: &CheckedGraph,
    snapshot: &ParamSnapshot,
    inputs: &[f32],
    scratch: &mut noble_ngraph::eval::ScratchCounters,
    evaluated_nodes: &mut u64,
) -> Result<f32, String> {
    let observation = eval::reference(
        model,
        &eval::f32_inputs(inputs),
        Some(snapshot),
        &mut Control {
            scratch: Some(scratch),
            evaluated_nodes: Some(evaluated_nodes),
            ..Control::default()
        },
    )
    .obs;
    singleton_f32(observation)
}

fn singleton_f32(observation: Obs) -> Result<f32, String> {
    match observation {
        Obs::Normal(values) => match values.as_slice() {
            [noble_ngraph::sem::Value::F32(bits)] => Ok(f32::from_bits(*bits)),
            _ => Err(String::from("scorer returned a non-singleton F32 result")),
        },
        other => Err(format!("scorer evaluation refused: {other:?}")),
    }
}

pub(crate) fn validation_mean_loss(
    model: &CheckedGraph,
    snapshot: &ParamSnapshot,
    rows: &[TrainingRow],
) -> Result<f32, String> {
    let mut sum = 0.0f32;
    for row in rows {
        let score = numeric_output(model, snapshot, &row.features)?;
        let difference = score - row.target;
        sum += difference * difference;
    }
    let count = u32::try_from(rows.len()).map_err(|error| error.to_string())?;
    if count == 0 {
        return Err(String::from("validation set is empty"));
    }
    Ok(sum / count as f32)
}

fn checked_signature_labels<'a>(
    rows: impl Iterator<Item = &'a TrainingRow>,
    first: &TrainingRow,
    ordinal: u32,
) -> Result<std::collections::BTreeSet<u32>, String> {
    let mut labels = std::collections::BTreeSet::new();
    for row in rows {
        let is_matching = row
            .features
            .iter()
            .zip(&first.features)
            .all(|(actual, expected)| actual.to_bits() == expected.to_bits());
        if !is_matching {
            return Err(format!(
                "fixture leakage changed equal-signature features for ordinal {ordinal}"
            ));
        }
        labels.insert(row.target.to_bits());
    }
    Ok(labels)
}

fn check_equal_signature_ordinal(rows: &[TrainingRow], ordinal: u32) -> Result<(), String> {
    let selected = rows
        .iter()
        .filter(|row| row.family == FAMILY_NAMES[4] && row.ordinal == ordinal);
    let Some(first) = selected.clone().next() else {
        return Err(String::from("equal-signature training rows are missing"));
    };
    if checked_signature_labels(selected, first, ordinal)?.len() != 2 {
        return Err(format!(
            "equal-signature ordinal {ordinal} lacks both behavioral labels"
        ));
    }
    Ok(())
}

pub(crate) fn assert_equal_signature_features(rows: &[TrainingRow]) -> Result<(), String> {
    for ordinal in 1..=3 {
        check_equal_signature_ordinal(rows, ordinal)?;
    }
    Ok(())
}

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
    for value in state.m.iter().chain(&state.v) {
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    bytes.extend_from_slice(&state.b1t.to_bits().to_le_bytes());
    bytes.extend_from_slice(&state.b2t.to_bits().to_le_bytes());
    bytes.extend_from_slice(&state.t.to_le_bytes());
    bytes
}

pub(crate) struct BuildIdentity<'a> {
    pub(crate) source_commit: &'a str,
    pub(crate) rustc_version: &'a str,
    pub(crate) cargo_version: &'a str,
}

pub(crate) fn trained_identity(
    fit: &ModelFit,
    build: BuildIdentity<'_>,
) -> [u8; chunks::REVISION_BYTES] {
    let state_bytes = optimizer_state_bytes(&fit.state);
    let mut hasher = blake3::Hasher::new();
    hash_header(&mut hasher, &build);
    hash_snapshot_lineage(&mut hasher, fit);
    for seed in &fit.seeds {
        hash_seed_summary(&mut hasher, seed);
    }
    for context in &fit.training_rows.contexts {
        hash_training_context(&mut hasher, context);
    }
    hasher.update(blake3::hash(&state_bytes).as_bytes());
    *hasher.finalize().as_bytes()
}

fn hash_header(hasher: &mut blake3::Hasher, build: &BuildIdentity<'_>) {
    hasher.update(b"noble.tchunk-ranking-model/1\0");
    hasher.update(build.source_commit.as_bytes());
    hasher.update(build.rustc_version.as_bytes());
    hasher.update(build.cargo_version.as_bytes());
    hasher.update(blake3::hash(FROZEN_CRITERIA.as_bytes()).as_bytes());
    hasher.update(blake3::hash(FROZEN_CORPUS.as_bytes()).as_bytes());
}

fn hash_snapshot_lineage(hasher: &mut blake3::Hasher, fit: &ModelFit) {
    let graph_text = noble_ngraph::format::graph::print(fit.graph.graph());
    let loss_text = noble_ngraph::format::graph::print(fit.loss_graph.graph());
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
}

fn hash_seed_summary(hasher: &mut blake3::Hasher, seed: &SeedSummary) {
    hasher.update(&seed.seed.to_le_bytes());
    hasher.update(&seed.validation_loss.to_bits().to_le_bytes());
    hasher.update(&seed.provenance_data);
    hasher.update(&seed.optimizer_state_digest);
}

fn hash_training_context(hasher: &mut blake3::Hasher, context: &CandidateContext) {
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
