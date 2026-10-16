//! B2 differential legs for `F32` graphs: L2 APFloat oracle, B2-ORACLE
//! binary64-per-operation leg, L3 reference interpreter, L4 compiled
//! evaluator. (L1 and the `I64` BigInt oracle belong to the extraction leg.)

use crate::check::CheckedGraph;
use crate::compiled::Compiled;
use crate::eval::{self, Control, Obs};
use crate::oracle;
use crate::prng::SplitMix64;
use crate::random;
use crate::sem::Value;
use crate::snapshot::ParamSnapshot;

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
    Legs {
        l2_apfloat: oracle::apfloat(checked, inputs, snapshot),
        b2_oracle_f64: oracle::f64_per_op(checked, inputs, snapshot),
        l3_reference: eval::reference(checked, inputs, snapshot, &Control::default()).obs,
        l4_compiled: c.evaluate(inputs, snapshot, &Control::default()).obs,
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
    /// Layers.
    pub layers: u32,
    /// `GraphLogicalId` (hex).
    pub graph_id: String,
    /// Vectors run.
    pub vectors: u64,
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
    let checked = match crate::check::check(rg.graph.clone()) {
        Ok(c) => c,
        Err(d) => {
            out.checker_refusal = Some(d.to_string());
            return out;
        }
    };
    out.nodes = checked.node_count();
    out.layers = checked.layer_count();
    out.graph_id = crate::ir::hex(&checked.id());
    let compiled = Compiled::new(&checked);
    let mut rng = SplitMix64::new(input_seed);
    for _ in 0..vectors {
        let inputs = eval::f32_inputs(&random::inputs(&rg, &mut rng));
        let legs = run_legs(&compiled, &inputs, None);
        out.vectors += 1;
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
            if out.first_disagreement.is_none() {
                out.first_disagreement = Some(format!("inputs={inputs:?} legs={legs:?}"));
            }
        }
    }
    out
}
