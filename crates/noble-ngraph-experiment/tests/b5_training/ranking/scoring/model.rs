use super::*;

pub(crate) const TRAIN_SEEDS: [u64; 3] = [0x74031a7e20261029, 0x74031a7e2026102a, 0x74031a7e2026102b];
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
    let inputs: Vec<_> = (0..chunks::FEATURE_LANES)
        .map(|_| builder.input(ScalarType::F32))
        .collect();
    let products: Vec<_> = inputs
        .into_iter()
        .map(|input| {
            let parameter = builder.param();
            builder.op(noble_ngraph::ir::Op::F32Mul(parameter, input))
        })
        .collect();
    let mut total = products[0];
    for product in products.iter().skip(1) {
        total = builder.op(noble_ngraph::ir::Op::F32Add(total, *product));
    }
    let bias = builder.param();
    let output = builder.op(noble_ngraph::ir::Op::F32Add(total, bias));
    builder.output(output);
    check(&BLAKE3, builder.finish()).expect("fixed scorer graph admits")
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

pub(crate) fn assert_equal_signature_features(rows: &[TrainingRow]) -> Result<(), String> {
    for ordinal in 1..=3 {
        let selected: Vec<&TrainingRow> = rows
            .iter()
            .filter(|row| row.family == FAMILY_NAMES[4] && row.ordinal == ordinal)
            .collect();
        let Some(first) = selected.first() else {
            return Err(String::from("equal-signature training rows are missing"));
        };
        let expected_bits: Vec<u32> = first.features.iter().map(|value| value.to_bits()).collect();
        let mut labels = std::collections::BTreeSet::new();
        for row in selected {
            let actual_bits: Vec<u32> = row.features.iter().map(|value| value.to_bits()).collect();
            if actual_bits != expected_bits {
                return Err(format!(
                    "fixture leakage changed equal-signature features for ordinal {ordinal}"
                ));
            }
            labels.insert(row.target.to_bits());
        }
        if labels.len() != 2 {
            return Err(format!(
                "equal-signature ordinal {ordinal} lacks both behavioral labels"
            ));
        }
    }
    Ok(())
}
