//! D11 "Capture" and evidence applicability.
//!
//! Capturing snapshot values creates a **new subject**: params become exact
//! constants, the subject identity binds the old subject and the snapshot, and
//! the `GraphLogicalId` changes. Agreement evidence recorded for one
//! `(subject, GraphLogicalId, snapshot)` never transfers to another; it must
//! be checked again (#8, #10). This is a Phase B applicability check on run
//! records, not #8 witness binding (evidence *facts* are excluded by P5).

use crate::scheme::BLAKE3;
use noble_ngraph::check::CheckedGraph;
use noble_ngraph::diag::Diagnostic;
use noble_ngraph::ir::{Digest, Graph, Literal, Op, Provenance, SlotKind, Subject, SubjectKind};
use noble_ngraph::snapshot::ParamSnapshot;

/// Captures `s` into `model`: a new graph whose params are constants.
///
/// # Errors
/// The snapshot's `E-SNAPSHOT-*` refusal.
pub fn capture(model: &CheckedGraph, s: &ParamSnapshot) -> Result<Graph, Diagnostic> {
    s.check_use(model)?;
    let g = model.graph();
    let mut remap = vec![u32::MAX; g.slots.len()];
    let mut slots = Vec::new();
    for (k, slot) in g.slots.iter().enumerate() {
        if slot.kind == SlotKind::Input {
            remap[k] = slots.len() as u32;
            slots.push(slot.clone());
        }
    }
    let mut nodes = g.nodes.clone();
    for n in &mut nodes {
        n.op = match &n.op {
            Op::Input(k) => Op::Input(remap[*k as usize]),
            Op::Param(k) => Op::Const(Literal::F32(s.bits_of_slot(*k).unwrap_or(0))),
            other => other.clone(),
        };
    }
    let facts = g
        .facts
        .iter()
        .filter(|f| {
            !(f.provenance == Provenance::InputValidated
                && matches!(g.nodes[f.node.index()].op, Op::Param(_)))
        })
        .cloned()
        .collect();
    let subject = Subject::derive(
        &BLAKE3,
        SubjectKind::Captured,
        &[&g.subject.identity, &s.id()],
        g.subject.closure.clone(),
    );
    Ok(Graph {
        profile: g.profile,
        slots,
        nodes,
        outputs: g.outputs.clone(),
        facts,
        subject,
        input_shape: None,
        output_shape: g.output_shape.clone(),
    })
}

/// A recorded agreement result bound to its exact subject, graph and snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceRecord {
    /// What was observed (e.g. "L2/L3/L4 agree on 64 points").
    pub claim: String,
    /// Subject identity.
    pub subject: Digest,
    /// `GraphLogicalId`.
    pub graph: Digest,
    /// Snapshot, if the graph has params.
    pub snapshot: Option<Digest>,
}

/// Why a record does not apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotApplicable {
    /// Recorded for another subject.
    OtherSubject,
    /// Recorded for another `GraphLogicalId`.
    OtherGraph,
    /// Recorded for another snapshot.
    OtherSnapshot,
}

impl EvidenceRecord {
    /// A record for `g` (and `s`).
    #[must_use]
    pub fn new(claim: &str, g: &CheckedGraph, s: Option<&ParamSnapshot>) -> Self {
        Self {
            claim: claim.to_owned(),
            subject: g.graph().subject.identity,
            graph: g.id(),
            snapshot: s.map(ParamSnapshot::id),
        }
    }

    /// Exact applicability: same subject, graph and snapshot; nothing transfers.
    ///
    /// # Errors
    /// The first mismatch.
    pub fn applies(
        &self,
        g: &CheckedGraph,
        s: Option<&ParamSnapshot>,
    ) -> Result<(), NotApplicable> {
        if self.subject != g.graph().subject.identity {
            return Err(NotApplicable::OtherSubject);
        }
        if self.graph != g.id() {
            return Err(NotApplicable::OtherGraph);
        }
        if self.snapshot != s.map(ParamSnapshot::id) {
            return Err(NotApplicable::OtherSnapshot);
        }
        Ok(())
    }
}
