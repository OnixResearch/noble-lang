//! L2 independent `F32` oracles (approval P4).
//!
//! * [`apfloat`]: every rounding operation is performed by the pinned
//!   software binary32 implementation `rustc_apfloat =0.2.3+llvm-462a31f5a5ab`
//!   (pure Rust port of LLVM APFloat, `#![forbid(unsafe_code)]`), never by host
//!   floating point. Comparisons (`relu`, `step`, index bounds) use APFloat's
//!   own ordering. Guard facts reuse [`noble_ngraph::facts::holds`] (shared code).
//! * [`f64_per_op`]: the B2-ORACLE leg. Each `+`, `−`, `×` (and each affine
//!   product and partial sum) is computed in binary64 and rounded to binary32
//!   once, the analogue of handwired's per-operation `Math.fround`. Agreement
//!   checks the D3 double-rounding inference; it is not a proof.

use rustc_apfloat::ieee::Single;
use rustc_apfloat::{Float, Round};

use noble_ngraph::check::CheckedGraph;
use noble_ngraph::eval::{self, Obs};
use noble_ngraph::facts;
use noble_ngraph::ir::{AffineOperand, NodeId, Op, OutOfRange};
use noble_ngraph::sem::Value;
use noble_ngraph::snapshot::ParamSnapshot;

/// Oracle revision recorded in evidence.
pub const APFLOAT_ORACLE: &str =
    "rustc_apfloat =0.2.3+llvm-462a31f5a5ab (ieee::Single, NearestTiesToEven)";

const RNE: Round = Round::NearestTiesToEven;

fn s(bits: u32) -> Single {
    Single::from_bits(u128::from(bits))
}

fn b(x: Single) -> u32 {
    u32::try_from(x.to_bits()).unwrap_or(0x7fc0_0000)
}

/// APFloat `rn(a + b)` on bits.
#[must_use]
pub fn ap_add(x: u32, y: u32) -> u32 {
    b(s(x).add_r(s(y), RNE).value)
}

/// APFloat `rn(a − b)` on bits.
#[must_use]
pub fn ap_sub(x: u32, y: u32) -> u32 {
    b(s(x).sub_r(s(y), RNE).value)
}

/// APFloat `rn(a × b)` on bits.
#[must_use]
pub fn ap_mul(x: u32, y: u32) -> u32 {
    b(s(x).mul_r(s(y), RNE).value)
}

fn ap_gt_zero(x: Single) -> bool {
    !x.is_nan() && !x.is_zero() && !x.is_negative()
}

fn ap_relu(x: u32) -> u32 {
    let v = s(x);
    if v.is_nan() || ap_gt_zero(v) {
        x
    } else {
        0
    }
}

fn ap_step(x: u32) -> u32 {
    let half = Single::from_bits(0x3f00_0000);
    match s(x).partial_cmp(&half) {
        Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal) => 0x3f80_0000,
        _ => 0,
    }
}

fn ap_index(i: u32, len: usize, below: OutOfRange, above: OutOfRange) -> Result<Option<usize>, ()> {
    let v = s(i);
    if !v.is_finite() {
        return Err(());
    }
    let integral = v.round_to_integral(Round::TowardZero).value;
    if integral.partial_cmp(&v) != Some(std::cmp::Ordering::Equal) {
        return Err(());
    }
    let mut exact = false;
    let k = v.to_i128_r(64, Round::TowardZero, &mut exact).value;
    if k < 0 {
        return match below {
            OutOfRange::Refuse => Err(()),
            OutOfRange::Clamp => Ok(Some(0)),
            OutOfRange::Zero => Ok(None),
        };
    }
    if k >= len as i128 {
        return match above {
            OutOfRange::Refuse => Err(()),
            OutOfRange::Clamp => Ok(Some(len - 1)),
            OutOfRange::Zero => Ok(None),
        };
    }
    Ok(Some(usize::try_from(k).unwrap_or(0)))
}

/// Which rounding implementation a leg uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Arith {
    ApFloat,
    F64PerOp,
}

fn op2(a: Arith, kind: u8, x: u32, y: u32) -> u32 {
    match a {
        Arith::ApFloat => match kind {
            0 => ap_add(x, y),
            1 => ap_sub(x, y),
            _ => ap_mul(x, y),
        },
        Arith::F64PerOp => {
            let (fx, fy) = (f64::from(f32::from_bits(x)), f64::from(f32::from_bits(y)));
            let r = match kind {
                0 => fx + fy,
                1 => fx - fy,
                _ => fx * fy,
            };
            #[allow(clippy::cast_possible_truncation)]
            (r as f32).to_bits()
        }
    }
}

/// B2-ORACLE per-operation leg: `rn₃₂(x ∘₆₄ y)`.
#[must_use]
pub fn f64_op(kind: u8, x: u32, y: u32) -> u32 {
    op2(Arith::F64PerOp, kind, x, y)
}

fn run(
    a: Arith,
    checked: &CheckedGraph,
    inputs: &[Value],
    snapshot: Option<&ParamSnapshot>,
) -> Obs {
    let slots = match eval::preflight(checked, inputs, snapshot) {
        Ok(v) => v,
        Err(d) => return Obs::Refused(d),
    };
    let g = checked.graph();
    let mut vals: Vec<Value> = Vec::with_capacity(g.nodes.len());
    for (i, node) in g.nodes.iter().enumerate() {
        let get = |x: NodeId| vals[x.index()];
        let f = |x: NodeId| match get(x) {
            Value::F32(v) => v,
            _ => 0x7fc0_0000,
        };
        let v = match &node.op {
            Op::Input(k) | Op::Param(k) => slots[*k as usize],
            Op::Const(l) => Value::of_literal(*l),
            Op::F32Add(x, y) => Value::F32(op2(a, 0, f(*x), f(*y))),
            Op::F32Sub(x, y) => Value::F32(op2(a, 1, f(*x), f(*y))),
            Op::F32Mul(x, y) => Value::F32(op2(a, 2, f(*x), f(*y))),
            Op::F32Affine { bias, terms } => {
                let mut acc = *bias;
                for t in terms {
                    let xv = match t.operand {
                        AffineOperand::One => 0x3f80_0000,
                        AffineOperand::Node(x) => f(x),
                    };
                    let p = op2(a, 2, t.coefficient, xv);
                    acc = op2(a, 0, acc, p);
                }
                Value::F32(acc)
            }
            Op::F32Relu(x) => Value::F32(ap_relu(f(*x))),
            Op::F32Reglu { left, gate } => Value::F32(op2(a, 2, f(*left), ap_relu(f(*gate)))),
            Op::F32Step(x) => Value::F32(ap_step(f(*x))),
            Op::F32Select { bit, off, on } => Value::F32(if f(*bit) == 0x3f80_0000 {
                f(*on)
            } else {
                f(*off)
            }),
            Op::F32Index {
                index,
                table,
                below,
                above,
            } => match ap_index(f(*index), table.len(), *below, *above) {
                Ok(Some(k)) => get(table[k]),
                Ok(None) => Value::F32(0),
                Err(()) => {
                    return Obs::GuardFailed {
                        node: NodeId(i as u32),
                        fact: eval::index_refusal_fact(table.len()),
                    }
                }
            },
            Op::Guard { fact, value } => {
                let group = facts::group_values(fact, get);
                let shape = match &g.nodes[value.index()].op {
                    Op::F32Index { table, .. } => Some(table.len() as u32),
                    _ => None,
                };
                if !facts::holds(fact, get(*value), &group, shape) {
                    return Obs::GuardFailed {
                        node: NodeId(i as u32),
                        fact: fact.clone(),
                    };
                }
                Value::Bool(true)
            }
            // I64 graphs: the BigInt oracle belongs to the extraction leg.
            Op::I64Add(..) | Op::I64Sub(..) | Op::I64Mul(..) | Op::I64Eq(..) => {
                return Obs::Refused(noble_ngraph::diag::Diagnostic::new(
                    noble_ngraph::diag::Code::Profile,
                    noble_ngraph::diag::Site::Graph,
                    "the F32 oracle evaluates ngraph-f32/0 only",
                ))
            }
        };
        vals.push(v);
    }
    Obs::Normal(g.outputs.iter().map(|o| vals[o.index()]).collect())
}

/// L2 `F32` oracle: whole-graph evaluation with APFloat arithmetic.
#[must_use]
pub fn apfloat(checked: &CheckedGraph, inputs: &[Value], snapshot: Option<&ParamSnapshot>) -> Obs {
    run(Arith::ApFloat, checked, inputs, snapshot)
}

/// B2-ORACLE leg: whole-graph evaluation with binary64-then-round per operation.
#[must_use]
pub fn f64_per_op(
    checked: &CheckedGraph,
    inputs: &[Value],
    snapshot: Option<&ParamSnapshot>,
) -> Obs {
    run(Arith::F64PerOp, checked, inputs, snapshot)
}
