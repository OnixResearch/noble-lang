//! Phase C workloads (C1) and their straightforward host-native Rust
//! baselines. The native code computes the same operations in the same order
//! with plain `f32`/`i64` arithmetic (rustc never contracts `a * b + c`; the
//! default x86-64 target has no FMA instructions), so its outputs are
//! compared bitwise with the L3 reference before any timing.
//!
//! * W1 = `I1`, W2 = `I4` (host transition) — hand-built `ngraph-i64/0`
//!   graphs equal to the extraction's lowering; the extraction leg supplies
//!   the extracted graphs and the ordinary Wasm/V8 leg.
//! * W3 = `F32` ReGLU networks at three sizes up to the learned-gallery scale.
//! * W4 = 32 reads of one 10,000-entry index table.

use crate::fixtures::range;
use crate::ir::{
    Fact, GraphBuilder, NodeId, Op, OutOfRange, Provenance, ScalarType, SubProfile, Subject,
};
use crate::Graph;

/// A W3 ReGLU network shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReGluNet {
    /// Inputs per point.
    pub d_in: usize,
    /// Hidden ReGLU units.
    pub hidden: usize,
    /// Points evaluated by one graph (sharing the parameters).
    pub batch: usize,
}

/// The three W3 sizes: 76, 418 and 6,270 operation nodes; 45, 243, 243 params.
pub const W3_SIZES: [(&str, ReGluNet); 3] = [
    (
        "W3-S",
        ReGluNet {
            d_in: 4,
            hidden: 4,
            batch: 1,
        },
    ),
    (
        "W3-M",
        ReGluNet {
            d_in: 4,
            hidden: 22,
            batch: 1,
        },
    ),
    (
        "W3-L",
        ReGluNet {
            d_in: 4,
            hidden: 22,
            batch: 15,
        },
    ),
];

impl ReGluNet {
    /// Parameter count: per unit `2·(d_in+1)`, then `hidden + 1` output weights.
    #[must_use]
    pub const fn params(self) -> usize {
        self.hidden * 2 * (self.d_in + 1) + self.hidden + 1
    }

    /// Operation nodes (excluding input/param slots).
    #[must_use]
    pub const fn ops(self) -> usize {
        self.batch * (self.hidden * (4 * self.d_in + 1) + 2 * self.hidden)
    }

    /// Input slots.
    #[must_use]
    pub const fn inputs(self) -> usize {
        self.batch * self.d_in
    }

    /// The graph: inputs first (`batch × d_in`), then params. Each affine
    /// combination is the left fold `((b + w₀x₀) + w₁x₁) + …`; the output is
    /// `((b_o + v₀h₀) + v₁h₁) + …` with `hⱼ = reglu(leftⱼ, gateⱼ)`.
    #[must_use]
    pub fn graph(self, name: &str) -> Graph {
        let mut b = GraphBuilder::new(
            SubProfile::F32,
            Subject::authored(&format!("issue14/{name} {self:?}")),
        );
        let x: Vec<NodeId> = (0..self.inputs())
            .map(|_| b.input(ScalarType::F32))
            .collect();
        let p: Vec<NodeId> = (0..self.params()).map(|_| b.param()).collect();
        let unit = 2 * (self.d_in + 1);
        for pt in 0..self.batch {
            let xs = &x[pt * self.d_in..(pt + 1) * self.d_in];
            let mut hs = Vec::with_capacity(self.hidden);
            for j in 0..self.hidden {
                let q = &p[j * unit..(j + 1) * unit];
                let fold = |b: &mut GraphBuilder, w: &[NodeId], bias: NodeId| {
                    let mut acc = bias;
                    for (wk, xk) in w.iter().zip(xs) {
                        let m = b.op(Op::F32Mul(*wk, *xk));
                        acc = b.op(Op::F32Add(acc, m));
                    }
                    acc
                };
                let left = fold(&mut b, &q[..self.d_in], q[self.d_in]);
                let gate = fold(&mut b, &q[self.d_in + 1..unit - 1], q[unit - 1]);
                hs.push(b.op(Op::F32Reglu { left, gate }));
            }
            let o = &p[self.hidden * unit..];
            let mut acc = o[self.hidden];
            for (v, h) in o.iter().zip(&hs) {
                let m = b.op(Op::F32Mul(*v, *h));
                acc = b.op(Op::F32Add(acc, m));
            }
            b.output(acc);
        }
        b.finish()
    }

    /// Native forward: `out[pt]` for every point.
    pub fn native(self, params: &[f32], x: &[f32], out: &mut [f32]) {
        let unit = 2 * (self.d_in + 1);
        let o = &params[self.hidden * unit..];
        for (pt, y) in out.iter_mut().enumerate().take(self.batch) {
            let xs = &x[pt * self.d_in..(pt + 1) * self.d_in];
            let mut acc = o[self.hidden];
            for j in 0..self.hidden {
                let q = &params[j * unit..(j + 1) * unit];
                let mut left = q[self.d_in];
                for k in 0..self.d_in {
                    left += q[k] * xs[k];
                }
                let mut gate = q[unit - 1];
                for k in 0..self.d_in {
                    gate += q[self.d_in + 1 + k] * xs[k];
                }
                let h = crate::sem::reglu(left, gate);
                acc += o[j] * h;
            }
            *y = acc;
        }
    }
}

/// Sum-of-squares loss over `n` outputs: `Σ rn((yᵢ − tᵢ)²)` as a left fold.
#[must_use]
pub fn sse_loss(n: usize) -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&format!("issue14/SSE loss n={n}")),
    );
    let y: Vec<NodeId> = (0..n).map(|_| b.input(ScalarType::F32)).collect();
    let t: Vec<NodeId> = (0..n).map(|_| b.input(ScalarType::F32)).collect();
    let mut acc = None;
    for (yi, ti) in y.iter().zip(&t) {
        let d = b.op(Op::F32Sub(*yi, *ti));
        let s = b.op(Op::F32Mul(d, d));
        acc = Some(match acc {
            None => s,
            Some(a) => b.op(Op::F32Add(a, s)),
        });
    }
    if let Some(a) = acc {
        b.output(a);
    }
    b.finish()
}

/// Native training state for the W3 baseline (handwired-style: in-place
/// parameter update, binary64 Adam moments, no snapshot publication).
#[derive(Clone, Debug)]
pub struct NativeTrainer {
    /// Shape.
    pub net: ReGluNet,
    /// Parameters (updated in place).
    pub params: Vec<f32>,
    m: Vec<f64>,
    v: Vec<f64>,
    b1t: f64,
    b2t: f64,
    grad: Vec<f32>,
}

impl NativeTrainer {
    /// A trainer from initial params.
    #[must_use]
    pub fn new(net: ReGluNet, params: Vec<f32>) -> Self {
        let n = params.len();
        Self {
            net,
            params,
            m: vec![0.0; n],
            v: vec![0.0; n],
            b1t: 1.0,
            b2t: 1.0,
            grad: vec![0.0; n],
        }
    }

    /// One step on `points` (each `d_in` inputs) with targets: SSE loss,
    /// manual FP32 backward, `adam/0` hyperparameters. Returns the loss.
    pub fn step(&mut self, x: &[f32], t: &[f32], a: &crate::train::Adam) -> f32 {
        let net = self.net;
        let unit = 2 * (net.d_in + 1);
        let g = &mut self.grad;
        g.iter_mut().for_each(|v| *v = 0.0);
        let p = &self.params;
        let ob = net.hidden * unit;
        let mut loss = 0.0f32;
        let mut left = vec![0f32; net.hidden];
        let mut gate = vec![0f32; net.hidden];
        for (pt, tv) in t.iter().enumerate() {
            let xs = &x[pt * net.d_in..(pt + 1) * net.d_in];
            let mut y = p[ob + net.hidden];
            for j in 0..net.hidden {
                let q = &p[j * unit..(j + 1) * unit];
                let mut l = q[net.d_in];
                let mut gg = q[unit - 1];
                for k in 0..net.d_in {
                    l += q[k] * xs[k];
                }
                for k in 0..net.d_in {
                    gg += q[net.d_in + 1 + k] * xs[k];
                }
                left[j] = l;
                gate[j] = gg;
                y += p[ob + j] * (l * if gg > 0.0 { gg } else { 0.0 });
            }
            let d = y - tv;
            loss += d * d;
            let gy = d + d;
            g[ob + net.hidden] += gy;
            for j in 0..net.hidden {
                let r = if gate[j] > 0.0 { gate[j] } else { 0.0 };
                g[ob + j] += gy * (left[j] * r);
                let gh = gy * p[ob + j];
                let gl = gh * r;
                let gg = if gate[j] > 0.0 { gh * left[j] } else { 0.0 };
                let base = j * unit;
                g[base + net.d_in] += gl;
                g[base + unit - 1] += gg;
                for k in 0..net.d_in {
                    g[base + k] += gl * xs[k];
                    g[base + net.d_in + 1 + k] += gg * xs[k];
                }
            }
        }
        self.b1t *= a.beta1;
        self.b2t *= a.beta2;
        for k in 0..self.params.len() {
            let gk = f64::from(self.grad[k]);
            self.m[k] = a.beta1 * self.m[k] + (1.0 - a.beta1) * gk;
            self.v[k] = a.beta2 * self.v[k] + (1.0 - a.beta2) * (gk * gk);
            let mhat = self.m[k] / (1.0 - self.b1t);
            let vhat = self.v[k] / (1.0 - self.b2t);
            #[allow(clippy::cast_possible_truncation)]
            {
                self.params[k] =
                    (f64::from(self.params[k]) - a.lr * mhat / (vhat.sqrt() + a.eps)) as f32;
            }
        }
        loss
    }
}

/// W4 table length.
pub const W4_TABLE: usize = 10_000;
/// W4 reads per evaluation.
pub const W4_READS: usize = 32;

/// W4 table entry `k`: an exact binary32 value.
#[must_use]
pub fn w4_entry(k: usize) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let v = (k % 977) as f32 * 0.25 - 100.0;
    v
}

/// W4: `W4_READS` index inputs (input-validated integral, `[0, 9999]`), each
/// reading the same 10,000 constant candidates under `refuse`, summed by one
/// bias-free `affine`.
#[must_use]
pub fn w4_graph() -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored("issue14/W4 10000-entry index table"),
    );
    #[allow(clippy::cast_precision_loss)]
    let hi = (W4_TABLE - 1) as f32;
    let idx: Vec<NodeId> = (0..W4_READS)
        .map(|_| {
            let i = b.input(ScalarType::F32);
            b.fact(i, Fact::Integral, Provenance::InputValidated);
            b.fact(i, range(0.0, hi), Provenance::InputValidated);
            i
        })
        .collect();
    let table: Vec<NodeId> = (0..W4_TABLE).map(|k| b.f32_const(w4_entry(k))).collect();
    let reads: Vec<NodeId> = idx
        .iter()
        .map(|i| {
            b.op(Op::F32Index {
                index: *i,
                table: table.clone(),
                below: OutOfRange::Refuse,
                above: OutOfRange::Refuse,
            })
        })
        .collect();
    let terms: Vec<(f32, crate::ir::AffineOperand)> = reads
        .iter()
        .map(|r| (1.0, crate::ir::AffineOperand::Node(*r)))
        .collect();
    let s = b.affine(-0.0, &terms);
    b.output(s);
    b.finish()
}

/// Native W4: integral and range checks (refusal = `None`), reads, sum from `-0`.
#[must_use]
pub fn w4_native(table: &[f32], idx: &[f32]) -> Option<f32> {
    let mut acc = -0.0f32;
    for &i in idx {
        if !(i.is_finite() && i.trunc() == i && i >= 0.0) {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let k = i as usize;
        acc += 1.0 * *table.get(k)?;
    }
    Some(acc)
}

/// W1 = `I1` `(x y) → y·y + x·3 − 7`, hand-built as the extraction lowers it.
#[must_use]
pub fn w1_graph() -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored("issue14/W1 I1 hand-built"),
    );
    let x = b.input(ScalarType::I64);
    let y = b.input(ScalarType::I64);
    let yy = b.op(Op::I64Mul(y, y));
    let three = b.i64_const(3);
    let x3 = b.op(Op::I64Mul(x, three));
    let s = b.op(Op::I64Add(yy, x3));
    let seven = b.i64_const(7);
    let r = b.op(Op::I64Sub(s, seven));
    b.output(r);
    b.finish()
}

/// Native W1.
#[must_use]
pub const fn w1_native(x: i64, y: i64) -> i64 {
    y.wrapping_mul(y)
        .wrapping_add(x.wrapping_mul(3))
        .wrapping_sub(7)
}

/// W2 = `I4` transition `(s i k) → (s·k+i, s·k+i)`, hand-built.
#[must_use]
pub fn w2_graph() -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored("issue14/W2 I4 hand-built"),
    );
    let s = b.input(ScalarType::I64);
    let i = b.input(ScalarType::I64);
    let k = b.input(ScalarType::I64);
    let m = b.op(Op::I64Mul(s, k));
    let a = b.op(Op::I64Add(m, i));
    b.output(a);
    b.output(a);
    b.finish()
}

/// Native W2: `steps` host steps from `s0`.
#[must_use]
pub fn w2_native(s0: i64, i: i64, k: i64, steps: u64) -> i64 {
    let mut s = s0;
    for _ in 0..steps {
        // One host step; `black_box` keeps LLVM from collapsing the
        // recurrence (it otherwise removes the 10^5-step loop).
        s = std::hint::black_box(s.wrapping_mul(k).wrapping_add(i));
    }
    s
}
