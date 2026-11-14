//! B2 differential legs for `F32` graphs: L2 APFloat oracle, B2-ORACLE
//! binary64-per-operation leg, L3 reference interpreter, L4 compiled
//! evaluator. (L1 and the `I64` BigInt oracle belong to the extraction leg.)

use crate::oracle;
use crate::random;
use crate::scheme::BLAKE3;
use noble_ngraph::check::CheckedGraph;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::prng::SplitMix64;
use noble_ngraph::sem::Value;
use noble_ngraph::snapshot::ParamSnapshot;

/// Observations of every `F32` leg for one input vector.
#[derive(Clone, Debug)]
pub struct Legs {
    /// L2: APFloat oracle.
    pub l2_apfloat: Obs,
    /// B2-ORACLE: binary64 per operation, rounded once.
    pub b2_oracle_f64: Obs,
    /// L3: reference interpreter.
    pub l3_reference: Obs,
    /// L4: compiled layered evaluator.
    pub l4_compiled: Obs,
    /// L3 node work, counted by the reference interpreter itself.
    pub l3_evaluated_nodes: u64,
}

impl Legs {
    /// Whether all legs agree under `≡obs`.
    #[must_use]
    pub fn agree(&self) -> bool {
        self.l3_reference.obs_eq(&self.l2_apfloat)
            && self.l3_reference.obs_eq(&self.b2_oracle_f64)
            && self.l3_reference.obs_eq(&self.l4_compiled)
    }
}

/// Runs every `F32` leg.
#[must_use]
pub fn run_legs(c: &Compiled, inputs: &[Value], snapshot: Option<&ParamSnapshot>) -> Legs {
    let checked: &CheckedGraph = c.checked();
    let l3 = eval::reference(checked, inputs, snapshot, &mut Control::default());
    Legs {
        l2_apfloat: oracle::apfloat(checked, inputs, snapshot),
        b2_oracle_f64: oracle::f64_per_op(checked, inputs, snapshot),
        l3_reference: l3.obs,
        l4_compiled: c.evaluate(inputs, snapshot, &mut Control::default()).obs,
        l3_evaluated_nodes: l3.evaluated_nodes,
    }
}

/// Summary of one seeded random-graph campaign.
#[derive(Clone, Debug, Default)]
pub struct Campaign {
    /// Graph seed.
    pub seed: u64,
    /// Input-vector seed.
    pub input_seed: u64,
    /// Requested operation nodes.
    pub ops: usize,
    /// Checked node count.
    pub nodes: usize,
    /// Total L3 node work across attempted input vectors.
    pub l3_evaluated_nodes: u64,
    /// Layers.
    pub layers: u32,
    /// `GraphLogicalId` (hex).
    pub graph_id: String,
    /// Vectors run.
    pub vectors: u64,
    /// Normal vector evaluations that visited every checked node.
    pub full_normal_executions: u64,
    /// Input bits for the first full normal vector, if one was reached.
    pub full_normal_input_bits: Option<Vec<u32>>,
    /// `Normal` observations.
    pub normal: u64,
    /// `GuardFailed` observations.
    pub guard_failed: u64,
    /// `Refused` observations (input facts).
    pub refused: u64,
    /// Normal outputs containing NaN / ±0 / subnormal / ±∞ (coverage counters).
    pub nan_outputs: u64,
    /// Signed-zero outputs.
    pub zero_outputs: u64,
    /// Subnormal outputs.
    pub subnormal_outputs: u64,
    /// Infinite outputs.
    pub inf_outputs: u64,
    /// Disagreements (first 8 recorded).
    pub disagreements: u64,
    /// Distinct differential edges that disagreed during the campaign.
    pub failure_classes: Vec<&'static str>,
    /// First disagreement detail.
    pub first_disagreement: Option<String>,
    /// Whether the checker refused the generated graph (counted, not hidden).
    pub checker_refusal: Option<String>,
}

/// Runs `vectors` random input vectors through every leg of random graph `seed`.
#[must_use]
pub fn campaign(seed: u64, ops: usize, vectors: u64) -> Campaign {
    let rg = random::graph(seed, ops);
    let input_seed = seed ^ 0x5eed_0000_0000_0000;
    let mut out = Campaign {
        seed,
        input_seed,
        ops,
        ..Campaign::default()
    };
    let checked = match noble_ngraph::check::check(&BLAKE3, rg.graph.clone()) {
        Ok(c) => c,
        Err(d) => {
            out.checker_refusal = Some(d.to_string());
            return out;
        }
    };
    out.nodes = checked.node_count();
    out.layers = checked.layer_count();
    out.graph_id = noble_ngraph::ir::hex(&checked.id());
    let compiled = Compiled::new(&BLAKE3, &checked);
    let mut rng = SplitMix64::new(input_seed);
    for _ in 0..vectors {
        let inputs = eval::f32_inputs(&random::inputs(&rg, &mut rng));
        let legs = run_legs(&compiled, &inputs, None);
        out.vectors += 1;
        if legs.l3_evaluated_nodes == checked.node_count() as u64
            && matches!(&legs.l3_reference, Obs::Normal(_))
        {
            out.full_normal_executions += 1;
            if out.full_normal_input_bits.is_none() {
                out.full_normal_input_bits = Some(
                    inputs
                        .iter()
                        .filter_map(|value| match value {
                            Value::F32(bits) => Some(*bits),
                            _ => None,
                        })
                        .collect(),
                );
            }
        }
        match &legs.l3_reference {
            Obs::Normal(vs) => {
                out.normal += 1;
                for v in vs {
                    if let Value::F32(b) = v {
                        let f = f32::from_bits(*b);
                        out.nan_outputs += u64::from(f.is_nan());
                        out.zero_outputs += u64::from(f == 0.0);
                        out.subnormal_outputs += u64::from(f.is_subnormal());
                        out.inf_outputs += u64::from(f.is_infinite());
                    }
                }
            }
            Obs::GuardFailed { .. } => out.guard_failed += 1,
            Obs::Refused(_) => out.refused += 1,
            _ => {}
        }
        if !legs.agree() {
            out.disagreements += 1;
            for (class, agrees) in [
                ("L2-vs-L3", legs.l3_reference.obs_eq(&legs.l2_apfloat)),
                (
                    "B2-oracle-vs-L3",
                    legs.l3_reference.obs_eq(&legs.b2_oracle_f64),
                ),
                ("L3-vs-L4", legs.l3_reference.obs_eq(&legs.l4_compiled)),
            ] {
                if !agrees && !out.failure_classes.contains(&class) {
                    out.failure_classes.push(class);
                }
            }
            if out.first_disagreement.is_none() {
                out.first_disagreement = Some(format!("inputs={inputs:?} legs={legs:?}"));
            }
        }
        out.l3_evaluated_nodes += legs.l3_evaluated_nodes;
    }
    out
}
