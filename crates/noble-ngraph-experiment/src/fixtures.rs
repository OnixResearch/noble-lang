//! Phase B `F32` fixtures, authored directly in `ngraph-ir/0` (approval P2,
//! option B0(b)): F1–F3 of plan B1 and the B2 boundary vectors.

use crate::scheme::BLAKE3;
use noble_ngraph::ir::{
    AffineOperand, Fact, GraphBuilder, Literal, NodeId, Op, OutOfRange, Provenance, ScalarType,
    SubProfile, Subject,
};
use noble_ngraph::Graph;

/// Number of F1 parameters (3 hidden units × (left 2+1, gate 2+1) + output 3+1).
pub const F1_PARAMS: usize = 22;

/// Fixed F1 parameters (exact binary32 values), in param-slot order.
pub const F1_FIXED: [f32; F1_PARAMS] = [
    // unit 0: left w0, w1, b; gate w0, w1, b
    0.5, -0.25, 0.125, 1.0, 0.5, 0.25, //
    // unit 1
    -0.75, 0.5, 0.0, -0.5, 1.0, 0.5, //
    // unit 2
    0.25, 0.25, -0.5, 0.75, -0.25, 0.125, //
    // output v0, v1, v2, b
    1.0, -0.5, 0.75, 0.0625,
];

fn affine_param(
    b: &mut GraphBuilder,
    w0: NodeId,
    x0: NodeId,
    w1: NodeId,
    x1: NodeId,
    bias: NodeId,
) -> NodeId {
    // rn(rn(b + rn(w0 x0)) + rn(w1 x1)): the affine order with a param bias.
    let p0 = b.op(Op::F32Mul(w0, x0));
    let s0 = b.op(Op::F32Add(bias, p0));
    let p1 = b.op(Op::F32Mul(w1, x1));
    b.op(Op::F32Add(s0, p1))
}

/// F1: a 2→3→1 ReGLU network whose 22 weights are `param` slots. Inputs
/// `(x0, x1)`; output `y`. Parameters are combined with `mul`/`add` (affine
/// coefficients are constants by D2, so they cannot be trainable).
#[must_use]
pub fn f1() -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "issue14/F1 2-3-1 ReGLU"),
    );
    let x0 = b.input(ScalarType::F32);
    let x1 = b.input(ScalarType::F32);
    let p: Vec<NodeId> = (0..F1_PARAMS).map(|_| b.param()).collect();
    let mut hidden = Vec::new();
    for j in 0..3 {
        let q = &p[j * 6..j * 6 + 6];
        let left = affine_param(&mut b, q[0], x0, q[1], x1, q[2]);
        let gate = affine_param(&mut b, q[3], x0, q[4], x1, q[5]);
        hidden.push(b.op(Op::F32Reglu { left, gate }));
    }
    let o = &p[18..22];
    let mut acc = o[3];
    for (j, h) in hidden.iter().enumerate() {
        let t = b.op(Op::F32Mul(o[j], *h));
        acc = b.op(Op::F32Add(acc, t));
    }
    b.output(acc);
    b.finish()
}

/// MSE loss graph `(y, t) → rn(d × d)` with `d = rn(y − t)` per point; the
/// host averages in binary64 in fixed point order (see [`noble_ngraph::train`]).
#[must_use]
pub fn mse_loss() -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "issue14/MSE point loss"),
    );
    let y = b.input(ScalarType::F32);
    let t = b.input(ScalarType::F32);
    let d = b.op(Op::F32Sub(y, t));
    let l = b.op(Op::F32Mul(d, d));
    b.output(l);
    b.finish()
}

/// F2: the transition `(e, v; m₀..m₃) → (rn(rn(m₀e)+rn(m₁v)), rn(rn(m₂e)+rn(m₃v)))`
/// with `m` as params.
#[must_use]
pub fn f2() -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "issue14/F2 transition"),
    );
    let e = b.input(ScalarType::F32);
    let v = b.input(ScalarType::F32);
    let m: Vec<NodeId> = (0..4).map(|_| b.param()).collect();
    let a0 = b.op(Op::F32Mul(m[0], e));
    let a1 = b.op(Op::F32Mul(m[1], v));
    let o0 = b.op(Op::F32Add(a0, a1));
    let c0 = b.op(Op::F32Mul(m[2], e));
    let c1 = b.op(Op::F32Mul(m[3], v));
    let o1 = b.op(Op::F32Add(c0, c1));
    b.output(o0);
    b.output(o1);
    b.finish()
}

/// F2 with constant `m` as `affine(-0; (m₀,e),(m₁,v))` nodes: must equal
/// [`f2`] bitwise because `-0` is an exact additive identity.
#[must_use]
pub fn f2_affine(m: [f32; 4]) -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "issue14/F2 affine"),
    );
    let e = b.input(ScalarType::F32);
    let v = b.input(ScalarType::F32);
    let o0 = b.affine(
        -0.0,
        &[
            (m[0], AffineOperand::Node(e)),
            (m[1], AffineOperand::Node(v)),
        ],
    );
    let o1 = b.affine(
        -0.0,
        &[
            (m[2], AffineOperand::Node(e)),
            (m[3], AffineOperand::Node(v)),
        ],
    );
    b.output(o0);
    b.output(o1);
    b.finish()
}

/// Range fact `[lo, hi]` on `F32`.
#[must_use]
pub fn range(lo: f32, hi: f32) -> Fact {
    Fact::Range {
        lo: Literal::f32(lo),
        hi: Literal::f32(hi),
    }
}

/// Largest index magnitude allowed by D3 (`|i| < 2^24`).
pub const INDEX_LIMIT: f32 = 16_777_215.0;

/// F3: `index(i, [a, a, b, -0], below, above)` with input-validated
/// `integral` and `range[-(2^24−1), 2^24−1]` facts on `i`. Inputs `(i, a, b)`.
#[must_use]
pub fn f3(below: OutOfRange, above: OutOfRange) -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(
            &BLAKE3,
            &format!("issue14/F3 index {}/{}", below.name(), above.name()),
        ),
    );
    let i = b.input(ScalarType::F32);
    let a = b.input(ScalarType::F32);
    let bb = b.input(ScalarType::F32);
    let nz = b.f32_const(-0.0);
    b.fact(i, Fact::Integral, Provenance::InputValidated);
    b.fact(
        i,
        range(-INDEX_LIMIT, INDEX_LIMIT),
        Provenance::InputValidated,
    );
    let r = b.op(Op::F32Index {
        index: i,
        table: vec![a, a, bb, nz],
        below,
        above,
    });
    b.output(r);
    b.finish()
}

/// Every `(below, above)` rule pair.
pub const RULES: [OutOfRange; 3] = [OutOfRange::Refuse, OutOfRange::Clamp, OutOfRange::Zero];

/// A named B2 vector: graph, `F32` inputs, expected outputs (bits; NaN is a class).
#[derive(Clone, Debug)]
pub struct Vector {
    /// Name.
    pub name: &'static str,
    /// Graph.
    pub graph: Graph,
    /// Inputs.
    pub inputs: Vec<f32>,
    /// Expected output bits.
    pub expected: Vec<u32>,
}

fn unary(
    name: &'static str,
    build: impl FnOnce(&mut GraphBuilder, &[NodeId]) -> NodeId,
    inputs: Vec<f32>,
    expected: f32,
) -> Vector {
    let mut b = GraphBuilder::new(SubProfile::F32, Subject::authored(&BLAKE3, name));
    let xs: Vec<NodeId> = inputs.iter().map(|_| b.input(ScalarType::F32)).collect();
    let r = build(&mut b, &xs);
    b.output(r);
    Vector {
        name,
        graph: b.finish(),
        inputs,
        expected: vec![expected.to_bits()],
    }
}

/// The B2 `F32` boundary vectors (reassociation, FMA, signed zero, nonfinite,
/// subnormal, repeated reads). Index boundary cases live in [`f3`].
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn b2_vectors() -> Vec<Vector> {
    let two24 = 16_777_216.0f32;
    let one_p12 = 1.0 + f32::from_bits(0x3980_0000); // 1 + 2^-12
    let one_p11 = 1.0 + f32::from_bits(0x3a00_0000); // 1 + 2^-11
    vec![
        unary(
            "reassociation: affine(-0; 2^24·x, 1·y, 1·z) = 2^24 (not 2^24+2)",
            |b, x| {
                b.affine(
                    -0.0,
                    &[
                        (1.0, AffineOperand::Node(x[0])),
                        (1.0, AffineOperand::Node(x[1])),
                        (1.0, AffineOperand::Node(x[2])),
                    ],
                )
            },
            vec![two24, 1.0, 1.0],
            two24,
        ),
        unary(
            "reassociation (constant-one operands): affine(-0; 2^24·1, 1·1, 1·1) = 2^24",
            |b, _| {
                b.affine(
                    -0.0,
                    &[
                        (two24, AffineOperand::One),
                        (1.0, AffineOperand::One),
                        (1.0, AffineOperand::One),
                    ],
                )
            },
            vec![],
            two24,
        ),
        unary(
            "FMA detection: rn(rn(a·b)+c) = +0 with a=b=1+2^-12, c=-(1+2^-11) (fused: 2^-24)",
            |b, x| {
                let p = b.op(Op::F32Mul(x[0], x[1]));
                b.op(Op::F32Add(p, x[2]))
            },
            vec![one_p12, one_p12, -one_p11],
            0.0,
        ),
        unary(
            "FMA detection in affine: affine(c; a·b) = +0",
            |b, x| b.affine(-one_p11, &[(one_p12, AffineOperand::Node(x[0]))]),
            vec![one_p12],
            0.0,
        ),
        unary(
            "signed zero: lone -0 term in a bias-free sum stays -0",
            |b, x| b.affine(-0.0, &[(1.0, AffineOperand::Node(x[0]))]),
            vec![-0.0],
            -0.0,
        ),
        unary(
            "signed zero: relu(-0) = +0",
            |b, x| b.op(Op::F32Relu(x[0])),
            vec![-0.0],
            0.0,
        ),
        unary(
            "signed zero: reglu(-1, 0) = -0",
            |b, x| {
                b.op(Op::F32Reglu {
                    left: x[0],
                    gate: x[1],
                })
            },
            vec![-1.0, 0.0],
            -0.0,
        ),
        unary(
            "signed zero: select returns a -0 operand bitwise",
            |b, x| {
                b.fact(x[0], Fact::Bit, Provenance::InputValidated);
                b.op(Op::F32Select {
                    bit: x[0],
                    off: x[1],
                    on: x[2],
                })
            },
            vec![1.0, 0.0, -0.0],
            -0.0,
        ),
        unary(
            "nonfinite: 2^127·2 = +inf",
            |b, x| b.op(Op::F32Mul(x[0], x[1])),
            vec![f32::from_bits(0x7f00_0000), 2.0],
            f32::INFINITY,
        ),
        unary(
            "nonfinite: reglu(inf, 0) = NaN (class)",
            |b, x| {
                b.op(Op::F32Reglu {
                    left: x[0],
                    gate: x[1],
                })
            },
            vec![f32::INFINITY, 0.0],
            f32::NAN,
        ),
        unary(
            "nonfinite: a NaN candidate passes through index",
            |b, x| {
                b.fact(x[0], Fact::Integral, Provenance::InputValidated);
                b.fact(x[0], range(0.0, 1.0), Provenance::InputValidated);
                b.op(Op::F32Index {
                    index: x[0],
                    table: vec![x[1], x[2]],
                    below: OutOfRange::Refuse,
                    above: OutOfRange::Refuse,
                })
            },
            vec![1.0, 2.0, f32::NAN],
            f32::NAN,
        ),
        unary(
            "subnormal: 2^-126 · 0.5 = 2^-127 (no flush to zero)",
            |b, x| b.op(Op::F32Mul(x[0], x[1])),
            vec![f32::MIN_POSITIVE, 0.5],
            f32::from_bits(0x0040_0000),
        ),
        unary(
            "subnormal: 2^-149 + 2^-149 = 2^-148",
            |b, x| b.op(Op::F32Add(x[0], x[1])),
            vec![f32::from_bits(1), f32::from_bits(1)],
            f32::from_bits(2),
        ),
        unary(
            "repeated reads: index(1,[a,a,b]) + index(0,[a,a,b]) = 2a",
            |b, x| {
                for k in [x[0], x[1]] {
                    b.fact(k, Fact::Integral, Provenance::InputValidated);
                    b.fact(k, range(0.0, 2.0), Provenance::InputValidated);
                }
                let r0 = b.op(Op::F32Index {
                    index: x[0],
                    table: vec![x[2], x[2], x[3]],
                    below: OutOfRange::Refuse,
                    above: OutOfRange::Refuse,
                });
                let r1 = b.op(Op::F32Index {
                    index: x[1],
                    table: vec![x[2], x[2], x[3]],
                    below: OutOfRange::Refuse,
                    above: OutOfRange::Refuse,
                });
                b.op(Op::F32Add(r0, r1))
            },
            vec![1.0, 0.0, 1.5, 7.0],
            3.0,
        ),
    ]
}

/// A simulated flush-to-zero evaluator result for `rn(x × y)`: used only to
/// show the subnormal vector detects a nonconforming FTZ backend.
#[must_use]
pub fn ftz_mul(x: f32, y: f32) -> f32 {
    let r = x * y;
    if r != 0.0 && r.abs() < f32::MIN_POSITIVE {
        0.0f32.copysign(r)
    } else {
        r
    }
}

/// Identity "loss" `y → y`, for analytic gradient checks.
#[must_use]
pub fn identity_loss() -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "issue14/identity loss"),
    );
    let y = b.input(ScalarType::F32);
    b.output(y);
    b.finish()
}
