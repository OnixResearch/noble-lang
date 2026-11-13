//! Frozen candidate/data artifacts and evaluator-only finite comparisons.

use std::sync::Arc;

use super::candidate::Candidate;
use super::hash::digest;
use super::types::{classify_bits, Digest, Refusal, RefusalClass, StepDescriptor, TransitionInput};

/// Frozen per-batch gradient and learning-rate row used by the host trial loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenRow {
    /// Gradient bits, in the declared parameter order.
    pub gradients: Vec<u32>,
    /// Learning-rate bits.
    pub eta: u32,
}

/// Exact candidate artifact stored outside proposer authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenCandidate {
    bytes: Arc<[u8]>,
    id: Digest,
}

impl FrozenCandidate {
    /// Freezes the exact admitted candidate bytes and identity.
    #[must_use]
    pub fn freeze(candidate: &Candidate) -> Self {
        Self {
            bytes: Arc::clone(candidate.bytes_arc()),
            id: candidate.id(),
        }
    }

    /// Loads stored bytes only when their externally recorded identity matches.
    pub fn from_parts(bytes: Arc<[u8]>, claimed_id: Digest) -> Result<Self, Refusal> {
        let actual = digest(b"candidate-code", &[&bytes]);
        if actual != claimed_id {
            return Err(Refusal::new(
                RefusalClass::Internal,
                "$.envelope",
                "frozen candidate bytes differ from their recorded identity",
            ));
        }
        Candidate::parse(&bytes)?;
        Ok(Self {
            bytes,
            id: claimed_id,
        })
    }

    /// Frozen identity.
    #[must_use]
    pub const fn id(&self) -> Digest {
        self.id
    }

    /// Frozen exact bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn verify(&self) -> Result<Candidate, Refusal> {
        let actual = digest(b"candidate-code", &[&self.bytes]);
        if actual != self.id {
            return Err(Refusal::new(
                RefusalClass::Internal,
                "$.envelope",
                "frozen candidate bytes differ from their recorded identity",
            ));
        }
        Candidate::parse(&self.bytes)
    }
}

/// Frozen ordered training rows, identity-bound to the root snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenDataset {
    rows: Arc<[FrozenRow]>,
    id: Digest,
}

impl FrozenDataset {
    /// Freezes row order, gradient bits and learning-rate bits.
    #[must_use]
    pub fn freeze(rows: Vec<FrozenRow>) -> Self {
        let id = dataset_id(&rows);
        Self {
            rows: Arc::from(rows),
            id,
        }
    }

    /// Identity of the complete ordered dataset.
    #[must_use]
    pub const fn id(&self) -> Digest {
        self.id
    }

    /// Number of frozen rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no row is available.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(crate) fn len_u64(&self) -> u64 {
        u64::try_from(self.rows.len()).unwrap_or(u64::MAX)
    }

    pub(crate) fn row(&self, index: u64) -> Option<&FrozenRow> {
        let index = usize::try_from(index).ok()?;
        self.rows.get(index)
    }

    pub(crate) fn verify(&self) -> Result<(), Refusal> {
        if dataset_id(&self.rows) != self.id {
            return Err(Refusal::new(
                RefusalClass::Internal,
                "$.data_order",
                "frozen data bytes differ from their recorded identity",
            ));
        }
        Ok(())
    }
}

/// One frozen explicit input row for independent reference comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationRow {
    /// Parameter bits.
    pub parameters: Vec<u32>,
    /// Gradient bits.
    pub gradients: Vec<u32>,
    /// Learning-rate bits.
    pub eta: u32,
    /// Host-owned metadata.
    pub step: StepDescriptor,
}

/// Frozen evaluation inputs, held only by the evaluator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenEvaluation {
    rows: Arc<[EvaluationRow]>,
    id: Digest,
}

impl FrozenEvaluation {
    /// Freezes input order and exact values for a finite comparison.
    #[must_use]
    pub fn freeze(rows: Vec<EvaluationRow>) -> Self {
        let id = evaluation_id(&rows);
        Self {
            rows: Arc::from(rows),
            id,
        }
    }

    /// Evaluator data identity.
    #[must_use]
    pub const fn id(&self) -> Digest {
        self.id
    }

    fn verify(&self) -> Result<(), Refusal> {
        if evaluation_id(&self.rows) != self.id {
            return Err(Refusal::new(
                RefusalClass::Internal,
                "$.evaluation",
                "frozen evaluation inputs differ from their recorded identity",
            ));
        }
        Ok(())
    }
}

/// Evaluator-owned result for one input row; the candidate supplies no score or verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationRowResult {
    /// Candidate transition result.
    pub candidate: Result<Vec<u32>, Refusal>,
    /// Direct `sem::{mul,sub}` reference result.
    pub reference: Result<Vec<u32>, Refusal>,
    /// Evaluator-computed exact agreement.
    pub agrees: bool,
}

/// Finite comparison result tied to exact frozen artifact identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evaluation {
    /// Exact candidate identity.
    pub candidate: Digest,
    /// Exact frozen evaluation-input identity.
    pub evaluation: Digest,
    /// Per-row observations in frozen order.
    pub rows: Vec<EvaluationRowResult>,
}

/// Evaluates only frozen candidate bytes against frozen explicit inputs.
///
/// The interpreter receives a transition's typed numeric inputs only. It has no callback, filesystem,
/// process, evaluator-secret or held-out-data handle; all correctness results are computed here from
/// the candidate output and a separate direct reference step.
pub fn evaluate_frozen(
    candidate: &FrozenCandidate,
    evaluation: &FrozenEvaluation,
) -> Result<Evaluation, Refusal> {
    let parsed = candidate.verify()?;
    evaluation.verify()?;
    let mut results = Vec::with_capacity(evaluation.rows.len());
    for row in evaluation.rows.iter() {
        let transition = TransitionInput {
            parameters: &row.parameters,
            gradients: &row.gradients,
            eta: row.eta,
            step: row.step,
            state: &[],
        };
        let candidate_result = parsed.evaluate(&transition).map(|output| output.parameters);
        let reference_result = reference_step(parsed.lanes(), row);
        let agrees = candidate_result == reference_result;
        results.push(EvaluationRowResult {
            candidate: candidate_result,
            reference: reference_result,
            agrees,
        });
    }
    Ok(Evaluation {
        candidate: candidate.id(),
        evaluation: evaluation.id,
        rows: results,
    })
}

fn reference_step(lanes: usize, row: &EvaluationRow) -> Result<Vec<u32>, Refusal> {
    if row.step.version != 1 {
        return Err(Refusal::new(
            RefusalClass::Malformed,
            "$.inputs.step.version",
            row.step.version.to_string(),
        ));
    }
    if row.parameters.len() != lanes {
        return Err(Refusal::new(
            RefusalClass::Unsupported,
            "$.inputs.p",
            format!("expected {lanes} lanes, observed {}", row.parameters.len()),
        ));
    }
    if row.gradients.len() != lanes {
        return Err(Refusal::new(
            RefusalClass::Unsupported,
            "$.inputs.g",
            format!("expected {lanes} lanes, observed {}", row.gradients.len()),
        ));
    }
    check_finite(&row.parameters, "$.inputs.p")?;
    check_finite(&row.gradients, "$.inputs.g")?;
    if let Some(kind) = classify_bits(row.eta) {
        return Err(Refusal::new(
            RefusalClass::Unsupported,
            "$.inputs.eta",
            format!("{kind:?} 0x{:08X}", row.eta),
        ));
    }
    let eta = f32::from_bits(row.eta);
    let mut outputs = Vec::with_capacity(lanes);
    for (lane, (parameter, gradient)) in row.parameters.iter().zip(&row.gradients).enumerate() {
        let product = noble_ngraph::sem::mul(eta, f32::from_bits(*gradient));
        let output = noble_ngraph::sem::sub(f32::from_bits(*parameter), product).to_bits();
        if classify_bits(output).is_some() {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                format!("$.outputs.parameters[{lane}]"),
                format!("nonfinite 0x{output:08X}"),
            ));
        }
        outputs.push(output);
    }
    Ok(outputs)
}

fn check_finite(values: &[u32], base: &str) -> Result<(), Refusal> {
    for (lane, bits) in values.iter().copied().enumerate() {
        if let Some(kind) = classify_bits(bits) {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                format!("{base}[{lane}]"),
                format!("{kind:?} 0x{bits:08X}"),
            ));
        }
    }
    Ok(())
}

fn dataset_id(rows: &[FrozenRow]) -> Digest {
    let mut encoded = Vec::new();
    append_len(&mut encoded, rows.len());
    for row in rows {
        append_len(&mut encoded, row.gradients.len());
        for bits in &row.gradients {
            encoded.extend_from_slice(&bits.to_le_bytes());
        }
        encoded.extend_from_slice(&row.eta.to_le_bytes());
    }
    digest(b"frozen-training-data", &[&encoded])
}

fn evaluation_id(rows: &[EvaluationRow]) -> Digest {
    let mut encoded = Vec::new();
    append_len(&mut encoded, rows.len());
    for row in rows {
        append_len(&mut encoded, row.parameters.len());
        for bits in &row.parameters {
            encoded.extend_from_slice(&bits.to_le_bytes());
        }
        append_len(&mut encoded, row.gradients.len());
        for bits in &row.gradients {
            encoded.extend_from_slice(&bits.to_le_bytes());
        }
        encoded.extend_from_slice(&row.eta.to_le_bytes());
        encoded.extend_from_slice(&row.step.to_bytes());
    }
    digest(b"frozen-evaluation-data", &[&encoded])
}

fn append_len(encoded: &mut Vec<u8>, length: usize) {
    encoded.extend_from_slice(&u64::try_from(length).unwrap_or(u64::MAX).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{evaluate_frozen, EvaluationRow, FrozenCandidate, FrozenEvaluation, FrozenRow};
    use crate::opt::{Candidate, RefusalClass, StepDescriptor};

    const SGD: &str = r#"{
      "format":"ngraph-opt/1","n":2,
      "inputs":["p","g","eta","step","state"],"state":"State0","effects":[],
      "budgets":{"nodes":16,"state_bytes":0,"work":128,"output_lanes":64},
      "nodes":[
        {"id":0,"op":"broadcast","operands":["$eta"],"type":"F32[n]"},
        {"id":1,"op":"Binary32.mul","operands":[0,"$g"],"type":"F32[n]"},
        {"id":2,"op":"Binary32.sub","operands":["$p",1],"type":"F32[n]"}
      ],"outputs":{"parameters":2,"state":"State0"}
    }"#;

    #[test]
    fn frozen_evaluation_uses_only_bound_inputs_and_evaluator_computed_results() {
        let candidate = Candidate::parse(SGD.as_bytes()).expect("admitted fixed program");
        let frozen = FrozenCandidate::freeze(&candidate);
        let evaluation = FrozenEvaluation::freeze(vec![EvaluationRow {
            parameters: vec![0x3f80_0000, 0x4000_0000],
            gradients: vec![0x3f80_0000, 0xc000_0000],
            eta: 0x3f00_0000,
            step: StepDescriptor::new(1, 0, 0).expect("descriptor"),
        }]);
        let result = evaluate_frozen(&frozen, &evaluation).expect("frozen run");
        assert_eq!(result.candidate, candidate.id());
        assert_eq!(result.evaluation, evaluation.id());
        assert_eq!(result.rows.len(), 1);
        assert!(result.rows[0].agrees);
        assert_eq!(result.rows[0].candidate, Ok(vec![0x3f00_0000, 0x4040_0000]));
    }

    #[test]
    fn tampered_candidate_or_frozen_evaluation_bytes_refuse_before_execution() {
        let candidate = Candidate::parse(SGD.as_bytes()).expect("admitted fixed program");
        let frozen = FrozenCandidate::freeze(&candidate);
        let mut changed = frozen.bytes.to_vec();
        changed[0] = b' ';
        let tampered_candidate = FrozenCandidate {
            bytes: Arc::from(changed),
            id: frozen.id,
        };
        let evaluation = FrozenEvaluation::freeze(vec![EvaluationRow {
            parameters: vec![0x3f80_0000, 0x4000_0000],
            gradients: vec![0x3f80_0000, 0xc000_0000],
            eta: 0x3f00_0000,
            step: StepDescriptor::new(1, 0, 0).expect("descriptor"),
        }]);
        let refusal =
            evaluate_frozen(&tampered_candidate, &evaluation).expect_err("tamper refused");
        assert_eq!(refusal.class, RefusalClass::Internal);
        assert_eq!(refusal.location, "$.envelope");

        let mut tampered_evaluation = evaluation.clone();
        let mut rows = tampered_evaluation.rows.to_vec();
        rows[0].eta = 0x3f80_0000;
        tampered_evaluation.rows = Arc::from(rows);
        let refusal = evaluate_frozen(&frozen, &tampered_evaluation).expect_err("tamper refused");
        assert_eq!(refusal.class, RefusalClass::Internal);
        assert_eq!(refusal.location, "$.evaluation");
    }

    #[test]
    fn candidate_cannot_read_held_out_data_secrets_or_write_its_verdict() {
        let attempts = [
            (
                SGD.replace(
                    "[\"p\",\"g\",\"eta\",\"step\",\"state\"]",
                    "[\"p\",\"g\",\"eta\",\"step\",\"state\",\"held_out\"]",
                ),
                RefusalClass::Unsupported,
                "$.inputs[5]",
            ),
            (
                SGD.replace("\"effects\":[]", "\"effects\":[\"read_evaluator_secret\"]"),
                RefusalClass::Unsupported,
                "$.effects[0]",
            ),
            (
                SGD.replace(
                    "\"format\":\"ngraph-opt/1\"",
                    "\"format\":\"ngraph-opt/1\",\"evaluator_secret\":\"hidden\"",
                ),
                RefusalClass::Malformed,
                "$.evaluator_secret",
            ),
            (
                SGD.replace(
                    "\"parameters\":2,\"state\":\"State0\"",
                    "\"parameters\":2,\"score\":1,\"state\":\"State0\"",
                ),
                RefusalClass::Malformed,
                "$.outputs.score",
            ),
        ];
        for (candidate, class, location) in attempts {
            let refusal = Candidate::parse(candidate.as_bytes()).expect_err("authority refused");
            assert_eq!(refusal.class, class);
            assert_eq!(refusal.location, location);
        }
    }

    #[test]
    fn frozen_training_rows_bind_order_and_exact_bits() {
        let dataset = super::FrozenDataset::freeze(vec![FrozenRow {
            gradients: vec![0x3f80_0000],
            eta: 0x3f00_0000,
        }]);
        assert!(!dataset.is_empty());
        assert_eq!(dataset.len(), 1);
        assert_eq!(dataset.row(0).map(|row| row.eta), Some(0x3f00_0000));
    }
}
