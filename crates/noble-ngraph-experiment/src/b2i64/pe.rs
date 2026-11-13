//! Issue #32 partial-evaluation leg: bounded static-input specialization of
//! checked `ngraph-i64/0` graphs.
//!
//! For one subject and one static map the leg compares five observations on
//! every input vector:
//!
//! 1. L3 of the original under the reconstructed (merged) input vector;
//! 2. L4 of the original under the same vector;
//! 3. L3 of the checked residual under the dynamic input vector;
//! 4. L4 of the residual under the same vector;
//! 5. L3 and L4 of the graph re-admitted from the residual's emitted source
//!    under the same vector.
//!
//! Agreement is `≡obs`. This is finite differential evidence for the
//! implementation and the residual Program; it is not a proof of the model
//! `specialize_sound`, of Rust/model correspondence or of Program refinement,
//! and it makes no claim that the two sides consume equal budget or fuel.

use noble_ngraph::check::{Budgets, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::sem::Value;
use noble_ngraph::specialize::{self, Residual};

use super::Lowered;
use crate::scheme::BLAKE3;

/// One specialized subject with both evaluators and both accepted graphs.
pub struct Specialized {
    /// The checked residual and its derivation record.
    pub residual: Residual,
    /// L4 of the residual.
    pub compiled: Compiled,
    /// The emitted `let`-based source.
    pub source: String,
    /// The graph re-admitted from `source` by the ordinary frontend and kernel.
    pub readmitted: CheckedGraph,
    /// L4 of the re-admitted graph.
    pub readmitted_compiled: Compiled,
}

fn outputs(obs: Obs) -> Result<Vec<Value>, String> {
    match obs {
        Obs::Normal(values) => Ok(values),
        other => Err(format!("non-normal observation {other:?}")),
    }
}

/// L3 outputs and the number of graph nodes it evaluated.
///
/// # Errors
/// A non-normal observation or a graph-node evaluation failure.
pub fn observe_l3(checked: &CheckedGraph, inputs: &[Value]) -> Result<(Vec<Value>, u64), String> {
    let mut control = Control::default();
    let report = eval::reference(checked, inputs, None, &mut control);
    let nodes = report.evaluated_nodes;
    outputs(report.obs).map(|values| (values, nodes))
}

/// L4 outputs and the number of graph nodes it evaluated.
///
/// # Errors
/// A non-normal observation.
pub fn observe_l4(compiled: &Compiled, inputs: &[Value]) -> Result<(Vec<Value>, u64), String> {
    let report = compiled.evaluate(inputs, None, &mut Control::default());
    let nodes = report.evaluated_nodes;
    outputs(report.obs).map(|values| (values, nodes))
}

impl Lowered {
    /// Specializes this subject's checked graph, emits the residual source and
    /// re-admits it on the ordinary path.
    ///
    /// # Errors
    /// Any refusal of specialization, emission or independent re-admission,
    /// rendered.
    pub fn specialized(&self, static_inputs: &[Option<i64>]) -> Result<Specialized, String> {
        let budgets = Budgets::default();
        let residual = specialize::specialize(
            &BLAKE3,
            &self.extraction.checked,
            &specialize::Request {
                static_inputs,
                budgets: &budgets,
            },
        )
        .map_err(|d| d.to_string())?;
        let compiled = Compiled::new(&BLAKE3, &residual.checked);
        let source = specialize::emit_source(&residual.checked).map_err(|d| d.to_string())?;
        let types = specialize::interface_types(&residual.checked).map_err(|d| d.to_string())?;
        let readmitted =
            specialize::readmit(&BLAKE3, &source, &types, &budgets).map_err(|d| d.to_string())?;
        let readmitted_compiled = Compiled::new(&BLAKE3, &readmitted);
        Ok(Specialized {
            residual,
            compiled,
            source,
            readmitted,
            readmitted_compiled,
        })
    }
}

impl Specialized {
    /// L3 of the residual.
    ///
    /// # Errors
    /// A non-normal observation.
    pub fn l3(&self, dynamic: &[Value]) -> Result<(Vec<Value>, u64), String> {
        observe_l3(&self.residual.checked, dynamic)
    }

    /// L4 of the residual.
    ///
    /// # Errors
    /// A non-normal observation.
    pub fn l4(&self, dynamic: &[Value]) -> Result<(Vec<Value>, u64), String> {
        observe_l4(&self.compiled, dynamic)
    }

    /// L3 of the re-admitted graph.
    ///
    /// # Errors
    /// A non-normal observation.
    pub fn readmitted_l3(&self, dynamic: &[Value]) -> Result<(Vec<Value>, u64), String> {
        observe_l3(&self.readmitted, dynamic)
    }

    /// L4 of the re-admitted graph.
    ///
    /// # Errors
    /// A non-normal observation.
    pub fn readmitted_l4(&self, dynamic: &[Value]) -> Result<(Vec<Value>, u64), String> {
        observe_l4(&self.readmitted_compiled, dynamic)
    }

    /// The original input vector a dynamic vector stands for.
    #[must_use]
    pub fn merge(&self, dynamic: &[i64]) -> Option<Vec<Value>> {
        self.residual.merge(dynamic)
    }
}

/// Per-leg node-visit totals and agreement of one differential row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Vectors compared.
    pub vectors: u64,
    /// L3 node visits over the original under merged inputs.
    pub l3_original: u64,
    /// L4 node visits over the original.
    pub l4_original: u64,
    /// L3 node visits over the residual.
    pub l3_residual: u64,
    /// L4 node visits over the residual.
    pub l4_residual: u64,
    /// L3 node visits over the re-admitted graph.
    pub l3_readmitted: u64,
    /// L4 node visits over the re-admitted graph.
    pub l4_readmitted: u64,
    /// Retained bytes of the original's compiled graph.
    pub retained_original: u64,
    /// Retained bytes of the residual's compiled graph.
    pub retained_residual: u64,
    /// Retained bytes of the re-admitted graph's compiled form.
    pub retained_readmitted: u64,
    /// Whether every pair agreed on every vector.
    pub agree: bool,
    /// The first disagreement, if any.
    pub first_mismatch: Option<String>,
}

/// Runs the differential over `dynamic_vectors` and returns the counts.
///
/// # Errors
/// A refusal or non-normal observation, rendered.
pub fn differential(
    lowered: &Lowered,
    static_inputs: &[Option<i64>],
    dynamic_vectors: &[Vec<i64>],
) -> Result<Counts, String> {
    let specialized = lowered.specialized(static_inputs)?;
    let mut counts = Counts {
        retained_original: lowered.compiled.retained_bytes(),
        retained_residual: specialized.compiled.retained_bytes(),
        retained_readmitted: specialized.readmitted_compiled.retained_bytes(),
        agree: true,
        ..Counts::default()
    };
    for dynamic in dynamic_vectors {
        counts.vectors = counts.vectors.saturating_add(1);
        let merged = specialized
            .merge(dynamic)
            .ok_or_else(|| format!("merge refused {dynamic:?}"))?;
        let dyn_values: Vec<Value> = dynamic.iter().map(|v| Value::I64(*v)).collect();
        let (original_l3, l3_original) = observe_l3(&lowered.extraction.checked, &merged)?;
        let (original_l4, l4_original) = observe_l4(&lowered.compiled, &merged)?;
        let (residual_l3, l3_residual) = specialized.l3(&dyn_values)?;
        let (residual_l4, l4_residual) = specialized.l4(&dyn_values)?;
        let (readmitted_l3, l3_readmitted) = specialized.readmitted_l3(&dyn_values)?;
        let (readmitted_l4, l4_readmitted) = specialized.readmitted_l4(&dyn_values)?;
        counts.l3_original = counts.l3_original.saturating_add(l3_original);
        counts.l4_original = counts.l4_original.saturating_add(l4_original);
        counts.l3_residual = counts.l3_residual.saturating_add(l3_residual);
        counts.l4_residual = counts.l4_residual.saturating_add(l4_residual);
        counts.l3_readmitted = counts.l3_readmitted.saturating_add(l3_readmitted);
        counts.l4_readmitted = counts.l4_readmitted.saturating_add(l4_readmitted);
        for (leg, observed) in [
            (
                "l3 original vs l4 original",
                noble_ngraph::sem::obs_eq_all(&original_l3, &original_l4),
            ),
            (
                "l3 residual vs l3 original",
                noble_ngraph::sem::obs_eq_all(&residual_l3, &original_l3),
            ),
            (
                "l4 residual vs l4 original",
                noble_ngraph::sem::obs_eq_all(&residual_l4, &original_l4),
            ),
            (
                "l3 readmitted vs l3 residual",
                noble_ngraph::sem::obs_eq_all(&readmitted_l3, &residual_l3),
            ),
            (
                "l4 readmitted vs l4 residual",
                noble_ngraph::sem::obs_eq_all(&readmitted_l4, &residual_l4),
            ),
            (
                "l3 readmitted vs l4 readmitted",
                noble_ngraph::sem::obs_eq_all(&readmitted_l3, &readmitted_l4),
            ),
        ] {
            if !observed && counts.agree {
                counts.agree = false;
                counts.first_mismatch = Some(format!(
                    "{leg} on {dynamic:?}: original {original_l3:?}/{original_l4:?}, residual {residual_l3:?}/{residual_l4:?}, readmitted {readmitted_l3:?}/{readmitted_l4:?}"
                ));
            }
        }
    }
    Ok(counts)
}
