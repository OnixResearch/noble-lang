//! Untimed source admission and reference observation.

use noble_ngraph::check;
use noble_ngraph::eval::{self, Control, Obs, ScratchCounters};
use noble_ngraph::ir::{hex, Graph};

use super::attempt::{self, RefusalInfo, ScratchObservation, Stage};
use super::attempt::{EXECUTED, EXECUTION_REFUSED, INPUT_REFUSED};

pub(super) struct Reference {
    pub(super) graph_logical_id: String,
    pub(super) graph_retained_bytes: u64,
    pub(super) outcome: &'static str,
    pub(super) observation: Option<Obs>,
    pub(super) evaluated_nodes: u64,
    pub(super) scratch: ScratchObservation,
    pub(super) refusal: Option<RefusalInfo>,
}

pub(super) fn observe(graph: &Graph) -> Reference {
    let graph_logical_id = hex(&graph.logical_id(&crate::common::IDS));
    let graph_retained_bytes = check::retained_bytes(graph);
    let checked = match check::check(&crate::common::IDS, graph.clone()) {
        Ok(value) => value,
        Err(diagnostic) => {
            return Reference {
                graph_logical_id,
                graph_retained_bytes,
                outcome: INPUT_REFUSED,
                observation: None,
                evaluated_nodes: 0,
                scratch: ScratchObservation::from("source_reference", &ScratchCounters::default()),
                refusal: Some(attempt::diagnostic_reason(
                    Stage::InputAdmission,
                    &diagnostic,
                )),
            };
        }
    };
    let mut counters = ScratchCounters::default();
    let mut control = Control {
        scratch: Some(&mut counters),
        ..Control::default()
    };
    let report = std::hint::black_box(eval::reference(&checked, &[], None, &mut control));
    let observation = report.obs;
    let outcome = if observation.outputs().is_some() {
        EXECUTED
    } else {
        EXECUTION_REFUSED
    };
    let refusal = if observation.outputs().is_some() {
        None
    } else {
        Some(attempt::observation_reason(&observation))
    };
    Reference {
        graph_logical_id,
        graph_retained_bytes,
        outcome,
        observation: Some(observation),
        evaluated_nodes: report.evaluated_nodes,
        scratch: ScratchObservation::from("source_reference", &counters),
        refusal,
    }
}
