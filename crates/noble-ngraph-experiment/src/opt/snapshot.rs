//! Immutable candidate parameter/state snapshots and complete provenance binding.

use std::sync::Arc;

use super::candidate::Candidate;
use super::hash::digest;
use super::types::{
    classify_bits, Digest, Refusal, RefusalClass, StepDescriptor, TransitionOutput,
    NUMERIC_SEMANTICS,
};

/// Non-candidate identities and host toolchain context bound to every snapshot in a trial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotContext {
    /// Checked model graph identity.
    pub graph: Digest,
    /// Checked loss graph identity.
    pub loss: Digest,
    /// Derivative-policy identity.
    pub derivative: Digest,
    /// Ordered data-cursor identity.
    pub data_order: Digest,
    /// Trial seed.
    pub seed: u64,
    /// Exact host/compiler/tool revisions.
    pub tools: String,
}

/// Provenance for one root or published optimizer snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotProvenance {
    /// Exact admitted candidate artifact identity.
    pub optimizer_code: Digest,
    /// Prior snapshot identity, absent only for the initial snapshot.
    pub parent_snapshot: Option<Digest>,
    /// Parent parameter-byte identity.
    pub parent_parameters: Digest,
    /// Parent optimizer-state-byte identity.
    pub parent_state: Digest,
    /// Model graph identity.
    pub graph: Digest,
    /// Loss graph identity.
    pub loss: Digest,
    /// Derivative-policy identity.
    pub derivative: Digest,
    /// Ordered data identity.
    pub data_order: Digest,
    /// Trial seed.
    pub seed: u64,
    /// Fixed numeric contract.
    pub numeric_semantics: String,
    /// Host/compiler/tool revisions.
    pub tools: String,
    /// Descriptor of this transition, absent only for the initial snapshot.
    pub step: Option<StepDescriptor>,
}

/// Immutable parameter and state bytes with a source-bound identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptSnapshot {
    parameters: Arc<[u32]>,
    state: Arc<[u8]>,
    provenance: SnapshotProvenance,
    context: SnapshotContext,
    id: Digest,
}

impl OptSnapshot {
    /// Creates the initial snapshot for a candidate from exact parameter bits and `State0`.
    pub fn root(
        candidate: &Candidate,
        parameters: &[u32],
        state: &[u8],
        context: SnapshotContext,
    ) -> Result<Self, Refusal> {
        if parameters.len() != candidate.lanes() {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                "$.inputs.p",
                format!(
                    "expected {} lanes, observed {}",
                    candidate.lanes(),
                    parameters.len()
                ),
            ));
        }
        if !state.is_empty() {
            return Err(Refusal::new(
                RefusalClass::Exhausted,
                "$.state",
                format!("{} bytes exceeds the State0 limit 0", state.len()),
            ));
        }
        check_finite(parameters, "$.inputs.p")?;
        let provenance = SnapshotProvenance {
            optimizer_code: candidate.id(),
            parent_snapshot: None,
            parent_parameters: digest(b"root-parent-parameters", &[]),
            parent_state: digest(b"root-parent-state", &[]),
            graph: context.graph,
            loss: context.loss,
            derivative: context.derivative,
            data_order: context.data_order,
            seed: context.seed,
            numeric_semantics: NUMERIC_SEMANTICS.to_owned(),
            tools: context.tools.clone(),
            step: None,
        };
        let id = snapshot_id(&provenance, parameters, state);
        Ok(Self {
            parameters: Arc::from(parameters),
            state: Arc::from(state),
            provenance,
            context,
            id,
        })
    }

    /// Publishes a fresh child snapshot. The parent is never mutated or reused as an output buffer.
    pub fn publish(
        candidate: &Candidate,
        parent: &Self,
        output: &TransitionOutput,
        step: StepDescriptor,
    ) -> Result<Self, Refusal> {
        if candidate.id() != parent.provenance.optimizer_code {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                "$.optimizer_code",
                "candidate differs from the snapshot-bound optimizer code",
            ));
        }
        if step.version != 1 {
            return Err(Refusal::new(
                RefusalClass::Malformed,
                "$.inputs.step.version",
                step.version.to_string(),
            ));
        }
        let expected_step = parent
            .provenance
            .step
            .map_or(Some(0), |previous| previous.step_index.checked_add(1));
        if expected_step != Some(step.step_index) {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                "$.inputs.step.step_index",
                format!(
                    "observed {}, expected {:?} after the parent snapshot",
                    step.step_index, expected_step
                ),
            ));
        }
        if output.parameters.len() != candidate.lanes() {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                "$.outputs.parameters",
                format!(
                    "expected {} lanes, observed {}",
                    candidate.lanes(),
                    output.parameters.len()
                ),
            ));
        }
        if !output.state.is_empty() {
            return Err(Refusal::new(
                RefusalClass::Exhausted,
                "$.state",
                format!("{} bytes exceeds the State0 limit 0", output.state.len()),
            ));
        }
        check_finite(&output.parameters, "$.outputs.parameters")?;
        let parent_parameters = parameter_digest(parent.parameters());
        let parent_state = digest(b"state", &[parent.state_bytes()]);
        let provenance = SnapshotProvenance {
            optimizer_code: candidate.id(),
            parent_snapshot: Some(parent.id),
            parent_parameters,
            parent_state,
            graph: parent.context.graph,
            loss: parent.context.loss,
            derivative: parent.context.derivative,
            data_order: parent.context.data_order,
            seed: parent.context.seed,
            numeric_semantics: NUMERIC_SEMANTICS.to_owned(),
            tools: parent.context.tools.clone(),
            step: Some(step),
        };
        let id = snapshot_id(&provenance, &output.parameters, &output.state);
        Ok(Self {
            parameters: Arc::from(output.parameters.as_slice()),
            state: Arc::from(output.state.as_slice()),
            provenance,
            context: parent.context.clone(),
            id,
        })
    }

    /// Snapshot identity over provenance, canonical little-endian parameter bytes and state bytes.
    #[must_use]
    pub const fn id(&self) -> Digest {
        self.id
    }

    /// Exact parameter bits in serialized lane order.
    #[must_use]
    pub fn parameters(&self) -> &[u32] {
        &self.parameters
    }

    /// Canonical little-endian parameter bytes.
    #[must_use]
    pub fn parameter_bytes(&self) -> Vec<u8> {
        encode_params(&self.parameters)
    }

    /// Exact serialized optimizer state bytes (`State0` is empty).
    #[must_use]
    pub fn state_bytes(&self) -> &[u8] {
        &self.state
    }

    /// Snapshot provenance.
    #[must_use]
    pub const fn provenance(&self) -> &SnapshotProvenance {
        &self.provenance
    }

    /// Trial context bound to the snapshot.
    #[must_use]
    pub const fn context(&self) -> &SnapshotContext {
        &self.context
    }
}

fn check_finite(parameters: &[u32], base: &str) -> Result<(), Refusal> {
    for (lane, bits) in parameters.iter().copied().enumerate() {
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

fn encode_params(parameters: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(parameters.len().saturating_mul(4));
    for parameter in parameters {
        bytes.extend_from_slice(&parameter.to_le_bytes());
    }
    bytes
}

fn snapshot_id(provenance: &SnapshotProvenance, parameters: &[u32], state: &[u8]) -> Digest {
    let candidate = provenance.optimizer_code;
    let parent = provenance.parent_snapshot.unwrap_or([0; 32]);
    let parent_present = [u8::from(provenance.parent_snapshot.is_some())];
    let params_parent = provenance.parent_parameters;
    let state_parent = provenance.parent_state;
    let graph = provenance.graph;
    let loss = provenance.loss;
    let derivative = provenance.derivative;
    let data = provenance.data_order;
    let seed = provenance.seed.to_le_bytes();
    let semantics = provenance.numeric_semantics.as_bytes();
    let tools = provenance.tools.as_bytes();
    let step = provenance.step.map_or([0; 9], StepDescriptor::to_bytes);
    let step_present = [u8::from(provenance.step.is_some())];
    let mut params = [0u8; 64 * 4];
    for (index, parameter) in parameters.iter().enumerate() {
        let start = index * 4;
        params[start..start + 4].copy_from_slice(&parameter.to_le_bytes());
    }
    let params_len = parameters.len() * 4;
    digest(
        b"optimizer-snapshot",
        &[
            &candidate,
            &parent_present,
            &parent,
            &params_parent,
            &state_parent,
            &graph,
            &loss,
            &derivative,
            &data,
            &seed,
            semantics,
            tools,
            &step_present,
            &step,
            &params[..params_len],
            state,
        ],
    )
}

fn parameter_digest(parameters: &[u32]) -> Digest {
    let mut bytes = [0u8; 64 * 4];
    for (index, parameter) in parameters.iter().enumerate() {
        let start = index * 4;
        bytes[start..start + 4].copy_from_slice(&parameter.to_le_bytes());
    }
    digest(b"parameters", &[&bytes[..parameters.len() * 4]])
}
