#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::fallible_int_fallback,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::ambiguous_params,
    tigerstyle::assertion_density,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! A test-only identity scheme: four SipHash-1-3 streams (std's
//! `DefaultHasher` with fixed zero keys), each prefixed with its index,
//! concatenated to 32 bytes. Deterministic for one toolchain; it keeps the
//! library's tests free of third-party crates. BLAKE3 identities are pinned
//! by the experiment's `identity_agreement` test.

#![allow(dead_code)]

pub mod rewrite_corpus;

use std::hash::{DefaultHasher, Hasher};

use noble_ngraph::ir::Digest;
use noble_ngraph::{IdentityHasher, IdentityScheme};

/// The test scheme.
pub struct TestScheme;

/// The scheme instance used by the library tests.
pub static IDS: TestScheme = TestScheme;

struct Streams([DefaultHasher; 4]);

impl IdentityHasher for Streams {
    fn update(&mut self, bytes: &[u8]) {
        for h in &mut self.0 {
            h.write(bytes);
        }
    }
    fn finish(self: Box<Self>) -> Digest {
        let mut out = [0u8; 32];
        for (i, h) in self.0.iter().enumerate() {
            out[i * 8..i * 8 + 8].copy_from_slice(&h.finish().to_le_bytes());
        }
        out
    }
}

impl IdentityScheme for TestScheme {
    fn name(&self) -> &'static str {
        "test-siphash13x4"
    }
    fn start(&self) -> Box<dyn IdentityHasher + '_> {
        let mut hs = [
            DefaultHasher::new(),
            DefaultHasher::new(),
            DefaultHasher::new(),
            DefaultHasher::new(),
        ];
        for (i, h) in hs.iter_mut().enumerate() {
            h.write_u8(u8::try_from(i).unwrap_or(0));
        }
        Box::new(Streams(hs))
    }
}

use noble_ngraph::ir::{GraphBuilder, Literal, Op, Provenance, ScalarType, SubProfile, Subject};
use noble_ngraph::train::{Adam, Contract, Point};
use noble_ngraph::{check, CheckedGraph, Graph};

/// `y = rn(rn(w·x) + b)`: input `x` (slot 0), params `w` (slot 1) and `b`
/// (slot 2). With `guarded`, a guard root requires `x ∈ [0, 4]`; with
/// `input_fact`, an input-validated fact requires the same.
pub fn linear(guarded: bool, input_fact: bool) -> Graph {
    let mut b = GraphBuilder::new(SubProfile::F32, Subject::authored(&IDS, "tests/linear"));
    let x = b.input(ScalarType::F32);
    let w = b.param();
    let bias = b.param();
    let wx = b.op(Op::F32Mul(w, x));
    let y = b.op(Op::F32Add(wx, bias));
    let range = noble_ngraph::ir::Fact::Range {
        lo: Literal::f32(0.0),
        hi: Literal::f32(4.0),
    };
    if guarded {
        b.guard(range.clone(), x);
    }
    if input_fact {
        b.fact(x, range, Provenance::InputValidated);
    }
    b.output(y);
    b.finish()
}

/// `(y, t) → rn(d·d)`, `d = rn(y − t)`.
pub fn mse() -> Graph {
    let mut b = GraphBuilder::new(SubProfile::F32, Subject::authored(&IDS, "tests/mse"));
    let y = b.input(ScalarType::F32);
    let t = b.input(ScalarType::F32);
    let d = b.op(Op::F32Sub(y, t));
    let l = b.op(Op::F32Mul(d, d));
    b.output(l);
    b.finish()
}

/// Points of `t = 2x + 0.5` (all exact in binary32).
pub fn points() -> Vec<Point> {
    [0.25f32, 0.5, 1.0, 2.0, 3.0]
        .iter()
        .map(|x| Point {
            inputs: vec![*x],
            targets: vec![2.0 * x + 0.5],
        })
        .collect()
}

/// Checks `g` under the test scheme.
pub fn checked(g: Graph) -> CheckedGraph {
    check(&IDS, g).expect("checks")
}

/// The training contract of `model` against `mse()` on `data`.
pub fn contract(model: CheckedGraph, data: Vec<Point>) -> Contract {
    Contract {
        params: model.param_slots().to_vec(),
        model,
        loss: checked(mse()),
        deriv: noble_ngraph::grad::DerivOptions::default(),
        adam: Adam::default(),
        seed: 0x5eed,
        data,
    }
}
