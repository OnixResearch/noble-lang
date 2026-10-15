//! Seeded random `F32` graphs and input vectors for the B2 differential legs
//! (seed recorded; `splitmix64/0`).

use crate::ir::{
    AffineOperand, Fact, GraphBuilder, NodeId, Op, Provenance, ScalarType, SubProfile, Subject,
};
use crate::prng::SplitMix64;
use crate::Graph;

/// Shape of a generated graph's inputs.
#[derive(Clone, Debug)]
pub struct RandomGraph {
    /// The graph.
    pub graph: Graph,
    /// Number of plain inputs (first slots).
    pub plain: usize,
    /// Index of the bit input slot.
    pub bit_slot: usize,
    /// Index of the index input slot.
    pub index_slot: usize,
    /// Largest table length used by any `index`.
    pub max_table: usize,
}

const COEFS: [f32; 12] = [
    1.0,
    -1.0,
    0.5,
    -0.5,
    2.0,
    0.25,
    -3.0,
    1.5,
    16_777_216.0,
    1e-3,
    -0.0,
    3.0e38,
];

fn exact_coef(rng: &mut SplitMix64) -> f32 {
    let k = rng.below(COEFS.len() as u64 + 4);
    if (k as usize) < COEFS.len() {
        COEFS[k as usize]
    } else {
        random_f32(rng)
    }
    .clamp(f32::MIN, f32::MAX)
}

/// One random binary32 input: specials, raw bit patterns and moderate values.
pub fn random_f32(rng: &mut SplitMix64) -> f32 {
    const SPECIAL: [u32; 16] = [
        0x0000_0000,
        0x8000_0000,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0000,
        0x0080_0000,
        0x8080_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
        0x3f80_0000,
        0xbf80_0000,
        0x3f00_0000,
        0x4b80_0000,
        0x0000_0001,
        0x8000_0001,
        0x007f_ffff,
    ];
    match rng.below(10) {
        0 | 1 => f32::from_bits(SPECIAL[rng.below(16) as usize]),
        2..=4 => f32::from_bits(rng.next_u64() as u32),
        _ => {
            let m = 1.0 + (rng.next_f64() as f32);
            let e = rng.below(41) as i32 - 20;
            let s = if rng.below(2) == 0 { 1.0 } else { -1.0 };
            s * m * 2f32.powi(e)
        }
    }
}

/// Generates a random `ngraph-f32/0` graph with about `ops` operation nodes.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn graph(seed: u64, ops: usize) -> RandomGraph {
    let mut rng = SplitMix64::new(seed);
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&format!("issue14/random seed={seed} ops={ops}")),
    );
    let plain = 3 + rng.below(3) as usize;
    let mut pool: Vec<NodeId> = (0..plain).map(|_| b.input(ScalarType::F32)).collect();
    let bit = b.input(ScalarType::F32);
    b.fact(bit, Fact::Bit, Provenance::InputValidated);
    let idx = b.input(ScalarType::F32);
    b.fact(idx, Fact::Integral, Provenance::InputValidated);
    b.fact(
        idx,
        crate::fixtures::range(-8.0, 12.0),
        Provenance::InputValidated,
    );
    let mut bits: Vec<NodeId> = vec![bit];
    let mut unit: Vec<NodeId> = vec![bit];
    let half = b.f32_const(0.5);
    unit.push(half);
    let mut max_table = 1;
    for _ in 0..ops {
        let pick =
            |rng: &mut SplitMix64, pool: &Vec<NodeId>| pool[rng.below(pool.len() as u64) as usize];
        let kind = rng.below(11);
        let n = match kind {
            0..=2 => {
                let (x, y) = (pick(&mut rng, &pool), pick(&mut rng, &pool));
                b.op(match kind {
                    0 => Op::F32Add(x, y),
                    1 => Op::F32Sub(x, y),
                    _ => Op::F32Mul(x, y),
                })
            }
            3 | 4 => {
                let k = 1 + rng.below(4) as usize;
                let mut terms = Vec::with_capacity(k);
                for _ in 0..k {
                    let c = exact_coef(&mut rng);
                    let o = if rng.below(6) == 0 {
                        AffineOperand::One
                    } else {
                        let x = pick(&mut rng, &pool);
                        AffineOperand::Node(x)
                    };
                    terms.push((c, o));
                }
                let bias = if rng.below(2) == 0 {
                    -0.0
                } else {
                    exact_coef(&mut rng)
                };
                b.affine(bias, &terms)
            }
            5 => {
                let x = pick(&mut rng, &pool);
                b.op(Op::F32Relu(x))
            }
            6 => {
                let (x, y) = (pick(&mut rng, &pool), pick(&mut rng, &pool));
                b.op(Op::F32Reglu { left: x, gate: y })
            }
            7 => {
                // step on a known-[0,1] value, or on a guarded random value.
                let s = if rng.below(4) == 0 {
                    let x = pick(&mut rng, &pool);
                    b.guard(crate::fixtures::range(0.0, 1.0), x);
                    x
                } else {
                    pick(&mut rng, &unit)
                };
                let r = b.op(Op::F32Step(s));
                bits.push(r);
                unit.push(r);
                r
            }
            8 => {
                let g = pick(&mut rng, &bits);
                let (x, y) = (pick(&mut rng, &pool), pick(&mut rng, &pool));
                b.op(Op::F32Select {
                    bit: g,
                    off: x,
                    on: y,
                })
            }
            9 => {
                let len = 1 + rng.below(6) as usize;
                max_table = max_table.max(len);
                let table: Vec<NodeId> = (0..len).map(|_| pick(&mut rng, &pool)).collect();
                let rules = crate::fixtures::RULES;
                b.op(Op::F32Index {
                    index: idx,
                    table,
                    below: rules[rng.below(3) as usize],
                    above: rules[rng.below(3) as usize],
                })
            }
            _ => {
                let v = exact_coef(&mut rng);
                b.f32_const(v)
            }
        };
        pool.push(n);
    }
    // Outputs: every unused value node (no dangling nodes), in canonical order.
    let total = b.len();
    let g = b.graph().clone();
    let mut consumed = vec![false; total];
    for node in &g.nodes {
        for o in node.op.operands() {
            consumed[o.index()] = true;
        }
    }
    for (i, node) in g.nodes.iter().enumerate() {
        if !consumed[i] && !node.op.is_guard() && !matches!(node.op, Op::Input(_)) {
            b.output(NodeId(i as u32));
        }
    }
    if b.graph().outputs.is_empty() {
        b.output(pool[pool.len() - 1]);
    }
    RandomGraph {
        graph: b.finish(),
        plain,
        bit_slot: plain,
        index_slot: plain + 1,
        max_table,
    }
}

/// One random input vector for `g`: plain inputs from [`random_f32`]; the bit
/// input is a bit 90% of the time; the index input is an in-fact integer 90%
/// of the time (otherwise fractional, NaN or out of its range fact).
pub fn inputs(g: &RandomGraph, rng: &mut SplitMix64) -> Vec<f32> {
    let mut v: Vec<f32> = (0..g.plain).map(|_| random_f32(rng)).collect();
    v.push(match rng.below(10) {
        0 => random_f32(rng),
        k => f32::from(u8::from(k % 2 == 0)),
    });
    v.push(match rng.below(20) {
        0 => 2.5,
        1 => f32::NAN,
        2 => 100.0,
        _ => (rng.below(21) as f32) - 8.0,
    });
    v
}
