//! Seeded B4 gradient campaigns (G1 binary64 reverse vs binary64 central
//! differences; G2 FP32 reference backward vs G1) on F1 with the MSE loss.

use crate::fixtures;
use crate::scheme::BLAKE3;
use noble_ngraph::check::{check, CheckedGraph};
use noble_ngraph::grad::{self, DerivOptions};
use noble_ngraph::ir::Op;
use noble_ngraph::prng::SplitMix64;

/// Whether the binary64 forward values at `slots` are farther than `margin`
/// from every ReLU/ReGLU corner, `step` threshold and integer index
/// boundary (B4 sampling; harness-only, so it uses the `std` float functions
/// the `no_std` library does not have).
#[must_use]
pub fn away_from_corners(model: &CheckedGraph, slots: &[f64], margin: f64) -> bool {
    let v: Vec<f64> = grad::forward(model, slots);
    model.graph().nodes.iter().all(|n| match &n.op {
        Op::F32Relu(x) => v[x.index()].abs() > margin,
        Op::F32Reglu { gate, .. } => v[gate.index()].abs() > margin,
        Op::F32Step(s) => (v[s.index()] - 0.5).abs() > margin,
        Op::F32Index { index, .. } => {
            let x = v[index.index()];
            (x - x.round()).abs() < f64::EPSILON || (x - x.floor()).min(x.ceil() - x) > margin
        }
        _ => true,
    })
}

/// Result of a gradient campaign.
#[derive(Clone, Debug, Default)]
pub struct GradStats {
    /// Seed.
    pub seed: u64,
    /// Sample points requested.
    pub requested: u64,
    /// Points used (away from corners).
    pub used: u64,
    /// Points skipped as within the corner margin.
    pub skipped_near_corner: u64,
    /// Composed critical-path depth.
    pub depth: u32,
    /// G1 slot checks.
    pub g1_checks: u64,
    /// G1 failures.
    pub g1_failures: u64,
    /// Largest `|g1 − cd| / tol`.
    pub g1_worst_ratio: f64,
    /// G2 slot checks.
    pub g2_checks: u64,
    /// G2 failures.
    pub g2_failures: u64,
    /// Largest `|g2 − g1| / bound`.
    pub g2_worst_ratio: f64,
    /// G2 checks that also meet the plain relative bound `4·depth·2^-24·|g1|`.
    pub g2_plain_relative_ok: u64,
    /// First failure detail.
    pub first_failure: Option<String>,
}

/// `h = 2^-20·max(1,|p|)`.
#[must_use]
pub fn step_size(p: f64) -> f64 {
    2f64.powi(-20) * p.abs().max(1.0)
}

/// `tol = 1e-6·max(1,|∂|)`.
#[must_use]
pub fn g1_tolerance(d: f64) -> f64 {
    1e-6 * d.abs().max(1.0)
}

fn models() -> (CheckedGraph, CheckedGraph) {
    let m = check(&BLAKE3, fixtures::f1()).unwrap_or_else(|d| panic!("F1: {d}"));
    let l = check(&BLAKE3, fixtures::mse_loss()).unwrap_or_else(|d| panic!("MSE: {d}"));
    (m, l)
}

/// Runs `samples` random points of F1+MSE.
#[must_use]
pub fn gradient_campaign(seed: u64, samples: u64) -> GradStats {
    let (model, loss) = models();
    let opts = DerivOptions::default();
    let mut rng = SplitMix64::new(seed);
    let depth = grad::depth(&model, &loss);
    let mut st = GradStats {
        seed,
        requested: samples,
        depth,
        ..GradStats::default()
    };
    let nslots = model.graph().slots.len();
    let bound_k = 4.0 * f64::from(depth) * 2f64.powi(-24);
    for _ in 0..samples {
        #[allow(clippy::cast_possible_truncation)]
        let mut s32: Vec<f32> = (0..nslots)
            .map(|_| (rng.next_f64() * 2.0 - 1.0) as f32)
            .collect();
        #[allow(clippy::cast_possible_truncation)]
        {
            s32[0] = (rng.next_f64() * 4.0 - 2.0) as f32;
            s32[1] = (rng.next_f64() * 4.0 - 2.0) as f32;
        }
        #[allow(clippy::cast_possible_truncation)]
        let t32 = [(rng.next_f64() * 2.0 - 1.0) as f32];
        let s64: Vec<f64> = s32.iter().map(|v| f64::from(*v)).collect();
        let t64 = [f64::from(t32[0])];
        let pmax = s64.iter().fold(1.0f64, |a, v| a.max(v.abs()));
        let margin = 64.0 * 4.0 * step_size(pmax);
        if !away_from_corners(&model, &s64, margin) {
            st.skipped_near_corner += 1;
            continue;
        }
        st.used += 1;
        let (_, g1) = grad::objective::<f64>(&model, &loss, &s64, &t64, &opts);
        let (_, g2) = grad::objective::<f32>(&model, &loss, &s32, &t32, &opts);
        let mag = grad::magnitude(&model, &loss, &s64, &t64, &opts);
        for k in 0..nslots {
            let h = step_size(s64[k]);
            let cd = grad::central_difference(&model, &loss, &s64, &t64, k, h);
            let tol = g1_tolerance(g1[k]);
            let r1 = (g1[k] - cd).abs() / tol;
            st.g1_checks += 1;
            st.g1_worst_ratio = st.g1_worst_ratio.max(r1);
            if r1 > 1.0 {
                st.g1_failures += 1;
                st.first_failure
                    .get_or_insert_with(|| format!("G1 slot {k}: g1={} cd={cd} tol={tol}", g1[k]));
            }
            let bound = bound_k * mag[k].max(f64::MIN_POSITIVE);
            let e2 = (f64::from(g2[k]) - g1[k]).abs();
            let r2 = e2 / bound;
            st.g2_checks += 1;
            st.g2_worst_ratio = st.g2_worst_ratio.max(r2);
            st.g2_plain_relative_ok += u64::from(e2 <= bound_k * g1[k].abs());
            if r2 > 1.0 {
                st.g2_failures += 1;
                st.first_failure.get_or_insert_with(|| {
                    format!("G2 slot {k}: g2={} g1={} bound={bound}", g2[k], g1[k])
                });
            }
        }
    }
    st
}

/// B5 training seed (recorded).
pub const B5_SEED: u64 = 0x1414_0005;
/// B5 data seed (recorded).
pub const B5_DATA_SEED: u64 = 0x1414_da7a;
/// B5 step count.
pub const B5_STEPS: u64 = 500;

/// The fixed 64-point target set: `x ∈ [-1,1)²` from `splitmix64/0(data_seed)`
/// (named RNE conversion to binary32), targets from F1 with [`fixtures::F1_FIXED`]
/// evaluated by the L3 reference.
#[must_use]
pub fn b5_data(data_seed: u64) -> Vec<noble_ngraph::train::Point> {
    let (model, _) = models();
    let teacher = noble_ngraph::snapshot::ParamSnapshot::new(&BLAKE3, &model, &fixtures::F1_FIXED)
        .unwrap_or_else(|d| panic!("teacher: {d}"));
    let mut rng = SplitMix64::new(data_seed);
    (0..64)
        .map(|_| {
            #[allow(clippy::cast_possible_truncation)]
            let x = [
                (rng.next_f64() * 2.0 - 1.0) as f32,
                (rng.next_f64() * 2.0 - 1.0) as f32,
            ];
            let r = noble_ngraph::eval::reference(
                &model,
                &noble_ngraph::eval::f32_inputs(&x),
                Some(&teacher),
                &noble_ngraph::eval::Control::default(),
            );
            let t = match r.obs.outputs() {
                Some([noble_ngraph::sem::Value::F32(b)]) => f32::from_bits(*b),
                _ => panic!("teacher evaluation: {:?}", r.obs),
            };
            noble_ngraph::train::Point {
                inputs: x.to_vec(),
                targets: vec![t],
            }
        })
        .collect()
}

/// The B5 training contract (F1, MSE, `adam/0`, default policy, recorded seeds).
#[must_use]
pub fn b5_contract(seed: u64, data_seed: u64) -> noble_ngraph::train::Contract {
    let (model, loss) = models();
    let params = model.graph().param_slots();
    noble_ngraph::train::Contract {
        model,
        loss,
        params,
        deriv: DerivOptions::default(),
        adam: noble_ngraph::train::Adam::default(),
        seed,
        data: b5_data(data_seed),
    }
}
