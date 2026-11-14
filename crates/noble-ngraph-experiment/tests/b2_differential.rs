#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::compound_assertion,
    tigerstyle::explicit_defaults,
    tigerstyle::function_length,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    tigerstyle::excessive_file_length,
    tigerstyle::bool_naming,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! B2: L2 (APFloat), B2-ORACLE (binary64 per op), L3 and L4 agree under
//! `≡obs` on the boundary vectors, F1–F3 and seeded random graphs. The full
//! ≥10,000-vector campaign runs in the `ngraph-phase-b` harness (release).

use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{f32_inputs, Control};
use noble_ngraph::ir::{
    AffineOperand, Fact, GraphBuilder, Literal, NodeId, Op, OutOfRange, Provenance, ScalarType,
    SubProfile, Subject,
};
use noble_ngraph::prng::SplitMix64;
use noble_ngraph::snapshot::ParamSnapshot;
use noble_ngraph::{check, Obs, Value};
use noble_ngraph_experiment::differential::{campaign, run_legs};
use noble_ngraph_experiment::fixtures::{self, F1_FIXED, RULES};
use noble_ngraph_experiment::oracle::{ap_add, ap_mul, ap_sub, f64_op};
use noble_ngraph_experiment::random::random_f32;
use noble_ngraph_experiment::scheme::BLAKE3;

mod shrink {
    //! Deterministic bounded shrinking for a binary32 expectation discrepancy
    //! (issue #38).
    //!
    //! The authority and the candidate are two binary32 evaluators for one
    //! operation. Given an operand pair that disagrees, the shrinker walks a fixed,
    //! documented sequence of simplifications and keeps a simplification only while
    //! the disagreement survives. The result is deterministic, bounded by an
    //! attempt budget, and replayable: re-running the candidate and the authority on
    //! the witness reproduces the same expected/candidate encodings.
    //!
    //! A witness shrunk here is *not* itself kernel evidence. A witness promoted to
    //! evidence must first become a retained expectation that the Lean reference
    //! checks; this lane only produces the minimized, replayable operand pair.

    /// Simplifications tried, in this order, for each operand. Every entry is a
    /// fixed bit pattern or a fixed mask, so two runs over the same start pair
    /// produce the same witness.
    const SIMPLIFICATIONS: [u32; 10] = [
        0x0000_0000, // +0
        0x8000_0000, // -0
        0x0000_0001, // smallest subnormal
        0x3f80_0000, // 1.0
        0x8000_0001, // smallest negative subnormal
        0x007f_ffff, // largest subnormal
        0x0080_0000, // smallest normal
        0x7f80_0000, // infinity
        0x7fc0_0000, // canonical NaN
        0x3f00_0000, // 0.5
    ];

    /// The documented simplicity order of one encoding: fewer set bits first, then
    /// the smaller pattern. A simplification is accepted only when it is strictly
    /// simpler under this order, which makes the walk monotone and terminating.
    const fn simplicity(bits: u32) -> (u32, u32) {
        (bits.count_ones(), bits)
    }

    /// Whether the pair is strictly simpler than `other` under `simplicity`.
    const fn simpler_than(pair: (u32, u32), other: (u32, u32)) -> bool {
        let (pa, pb) = (simplicity(pair.0), simplicity(pair.1));
        let (oa, ob) = (simplicity(other.0), simplicity(other.1));
        if pa.0 == oa.0 && pb.0 == ob.0 {
            pa.1 <= oa.1 && pb.1 <= ob.1 && (pa.1 < oa.1 || pb.1 < ob.1)
        } else {
            pa.0.saturating_add(pb.0) < oa.0.saturating_add(ob.0)
        }
    }

    /// How many leading simplifications the joint phase combines on both operands.
    const JOINT_OPERANDS: usize = 4;

    /// One minimized disagreement.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct Witness {
        /// The selected operation: `0` add, `1` subtract, `2` multiply.
        pub operation: u8,
        /// Operand `left` encoding that still disagrees, after shrinking.
        pub left: u32,
        /// Operand `right` encoding that still disagrees, after shrinking.
        pub right: u32,
        /// The authority's answer for the witness.
        pub expected: u32,
        /// The candidate's answer for the witness.
        pub candidate: u32,
        /// How many candidate simplifications were evaluated, within the budget.
        pub attempts: u32,
        /// Whether the shrinker stopped because the budget ran out with more
        /// simplifications pending.
        pub budget_exhausted: bool,
    }

    /// The documented bound on evaluated simplifications: ten simplifications per
    /// operand is 20 evaluations per pass and the joint phase adds at most 16, so
    /// 200 evaluations bound a walk that terminates within ten improving passes.
    pub const DEFAULT_ATTEMPTS: u32 = 200;

    /// Shrink a disagreement between `authority` and `candidate` for `operation`.
    ///
    /// Returns `None` when the starting pair already agrees: a shrinker must never
    /// invent a witness. The disagreement predicate is exactly "the two answers
    /// differ as encodings", so the disagreement class is preserved by construction.
    ///
    /// The result is a **local** minimum under the documented simplicity order and
    /// the documented simplification list: no single-slot or joint simplification in
    /// that list still disagrees. It is not a global minimum, and a pair that needs a
    /// simplification outside the list is reported unchanged rather than invented.
    #[must_use]
    pub fn minimize(
        operation: u8,
        authority: &dyn Fn(u8, u32, u32) -> u32,
        candidate: &dyn Fn(u8, u32, u32) -> u32,
        start: (u32, u32),
        budget_attempts: u32,
    ) -> Option<Witness> {
        let disagrees = |pair: (u32, u32)| {
            authority(operation, pair.0, pair.1) != candidate(operation, pair.0, pair.1)
        };
        if !disagrees(start) {
            return None;
        }
        let mut current = start;
        let mut attempts: u32 = 0;
        let mut is_improved = true;
        let mut is_exhausted = false;
        while is_improved && !is_exhausted {
            is_improved = false;
            for slot in 0..2u8 {
                for simplification in SIMPLIFICATIONS {
                    if attempts >= budget_attempts {
                        is_exhausted = true;
                        break;
                    }
                    attempts += 1;
                    let candidate_pair = if slot == 0 {
                        (simplification, current.1)
                    } else {
                        (current.0, simplification)
                    };
                    if simpler_than(candidate_pair, current) && disagrees(candidate_pair) {
                        current = candidate_pair;
                        is_improved = true;
                    }
                }
                if is_exhausted {
                    break;
                }
            }
            if !is_improved {
                // Some disagreements need both operands simplified at once: an
                // operand pair can stay agreeing while either slot alone changes.
                // The joint phase tries the documented cross product, still bounded
                // by the same attempt budget.
                for left in SIMPLIFICATIONS.into_iter().take(JOINT_OPERANDS) {
                    for right in SIMPLIFICATIONS.into_iter().take(JOINT_OPERANDS) {
                        if attempts >= budget_attempts {
                            is_exhausted = true;
                            break;
                        }
                        attempts += 1;
                        if simpler_than((left, right), current) && disagrees((left, right)) {
                            current = (left, right);
                            is_improved = true;
                        }
                    }
                    if is_exhausted {
                        break;
                    }
                }
            }
            if !is_improved {
                break;
            }
        }
        let (left, right) = current;
        Some(Witness {
            operation,
            left,
            right,
            expected: authority(operation, left, right),
            candidate: candidate(operation, left, right),
            attempts,
            budget_exhausted: is_exhausted,
        })
    }

    /// Replay a witness: `true` when the two evaluators still disagree on it with the
    /// recorded encodings. A witness that no longer replays is stale evidence and
    /// must not be retained.
    #[must_use]
    pub fn replays(
        witness: &Witness,
        authority: &dyn Fn(u8, u32, u32) -> u32,
        candidate: &dyn Fn(u8, u32, u32) -> u32,
    ) -> bool {
        let expected = authority(witness.operation, witness.left, witness.right);
        let observed = candidate(witness.operation, witness.left, witness.right);
        expected == witness.expected && observed == witness.candidate && expected != observed
    }
}

#[test]
fn boundary_vectors_agree_on_every_leg_and_match_expected_bits() {
    for v in fixtures::b2_vectors() {
        let c = check(&BLAKE3, v.graph.clone()).unwrap_or_else(|d| panic!("{}: {d}", v.name));
        let comp = Compiled::new(&BLAKE3, &c);
        let legs = run_legs(&comp, &f32_inputs(&v.inputs), None);
        assert!(legs.agree(), "{}: {legs:?}", v.name);
        let expected: Vec<Value> = v.expected.iter().map(|b| Value::F32(*b)).collect();
        assert!(
            legs.l3_reference.obs_eq(&Obs::Normal(expected)),
            "{}: {:?}",
            v.name,
            legs.l3_reference
        );
    }
}

#[test]
fn flush_to_zero_is_detected_by_the_subnormal_vector() {
    let r = fixtures::ftz_mul(f32::MIN_POSITIVE, 0.5);
    assert_ne!(
        r.to_bits(),
        0x0040_0000,
        "a simulated FTZ backend must disagree"
    );
}

#[test]
fn fused_multiply_add_would_be_detected() {
    let a = 1.0 + f32::from_bits(0x3980_0000);
    let c = -(1.0 + f32::from_bits(0x3a00_0000));
    assert_eq!((a * a + c).to_bits(), 0, "unfused host arithmetic");
    assert_eq!(
        a.mul_add(a, c),
        f32::from_bits(0x3380_0000),
        "fused gives 2^-24"
    );
}

#[test]
fn f1_f2_f3_agree_on_every_leg() {
    let f1 = check(&BLAKE3, fixtures::f1()).expect("F1 checks");
    let s1 = ParamSnapshot::new(&BLAKE3, &f1, &F1_FIXED).expect("snapshot");
    let c1 = Compiled::new(&BLAKE3, &f1);
    let mut rng = SplitMix64::new(14);
    for _ in 0..500 {
        let x = [random_f32(&mut rng), random_f32(&mut rng)];
        let legs = run_legs(&c1, &f32_inputs(&x), Some(&s1));
        assert!(legs.agree(), "F1 {x:?}: {legs:?}");
    }

    let m = [0.5f32, -1.25, 3.0, 0.75];
    let f2 = check(&BLAKE3, fixtures::f2()).expect("F2 checks");
    let s2 = ParamSnapshot::new(&BLAKE3, &f2, &m).expect("snapshot");
    let f2a = check(&BLAKE3, fixtures::f2_affine(m)).expect("F2 affine checks");
    let (c2, c2a) = (Compiled::new(&BLAKE3, &f2), Compiled::new(&BLAKE3, &f2a));
    for _ in 0..500 {
        let x = f32_inputs(&[random_f32(&mut rng), random_f32(&mut rng)]);
        let legs = run_legs(&c2, &x, Some(&s2));
        let legs_a = run_legs(&c2a, &x, None);
        assert!(legs.agree() && legs_a.agree());
        assert!(
            legs.l3_reference.obs_eq(&legs_a.l3_reference),
            "-0 bias fusion: {x:?}"
        );
    }
    // 1,000 host steps of the F2 transition: state feeds back; every leg agrees.
    let mut state = [1.0f32, -2.0];
    for _ in 0..1000 {
        let legs = run_legs(&c2, &f32_inputs(&state), Some(&s2));
        assert!(legs.agree());
        let Obs::Normal(o) = legs.l3_reference else {
            panic!()
        };
        let (Value::F32(a), Value::F32(b)) = (o[0], o[1]) else {
            panic!()
        };
        state = [f32::from_bits(a), f32::from_bits(b)];
    }

    let (a, b) = (2.5f32, -7.0f32);
    let lim = fixtures::INDEX_LIMIT;
    for below in RULES {
        for above in RULES {
            let g = check(&BLAKE3, fixtures::f3(below, above)).expect("F3 checks");
            let c = Compiled::new(&BLAKE3, &g);
            for i in [-lim, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, lim] {
                let legs = run_legs(&c, &f32_inputs(&[i, a, b]), None);
                assert!(legs.agree(), "F3 {below:?}/{above:?} i={i}: {legs:?}");
                let want = match i {
                    0.0 | 1.0 => Some(a.to_bits()),
                    2.0 => Some(b.to_bits()),
                    3.0 => Some(0x8000_0000),
                    x if x < 0.0 => match below {
                        noble_ngraph::ir::OutOfRange::Refuse => None,
                        noble_ngraph::ir::OutOfRange::Clamp => Some(a.to_bits()),
                        noble_ngraph::ir::OutOfRange::Zero => Some(0),
                    },
                    _ => match above {
                        noble_ngraph::ir::OutOfRange::Refuse => None,
                        noble_ngraph::ir::OutOfRange::Clamp => Some(0x8000_0000),
                        noble_ngraph::ir::OutOfRange::Zero => Some(0),
                    },
                };
                match want {
                    Some(bits) => {
                        assert_eq!(legs.l3_reference, Obs::Normal(vec![Value::F32(bits)]))
                    }
                    None => assert!(matches!(
                        legs.l3_reference,
                        Obs::GuardFailed {
                            node: NodeId(4),
                            fact: Fact::Range { .. }
                        }
                    )),
                }
            }
        }
    }
}

#[test]
fn per_operation_binary64_rounding_matches_apfloat_and_host() {
    // Checks the D3 double-rounding inference on random operand pairs
    // (the harness runs 10^6 pairs per operation).
    let mut rng = SplitMix64::new(0xd0b1e);
    for _ in 0..20_000 {
        let (x, y) = (
            random_f32(&mut rng).to_bits(),
            random_f32(&mut rng).to_bits(),
        );
        for (k, ap, host) in [
            (
                0u8,
                ap_add(x, y),
                (f32::from_bits(x) + f32::from_bits(y)).to_bits(),
            ),
            (
                1,
                ap_sub(x, y),
                (f32::from_bits(x) - f32::from_bits(y)).to_bits(),
            ),
            (
                2,
                ap_mul(x, y),
                (f32::from_bits(x) * f32::from_bits(y)).to_bits(),
            ),
        ] {
            let d = f64_op(k, x, y);
            let same = |p: u32, q: u32| {
                p == q || (f32::from_bits(p).is_nan() && f32::from_bits(q).is_nan())
            };
            assert!(
                same(ap, d) && same(ap, host),
                "op {k} {x:#x} {y:#x}: ap {ap:#x} f64 {d:#x} host {host:#x}"
            );
        }
    }
}

#[test]
fn seeded_random_graphs_agree_small() {
    for seed in [1u64, 2, 3] {
        let c = campaign(seed, 40, 1_000);
        assert!(c.checker_refusal.is_none(), "{c:?}");
        assert_eq!(c.disagreements, 0, "{:?}", c.first_disagreement);
        assert!(c.normal > 0 && c.refused > 0, "{c:?}");
    }
}

#[test]
fn compiled_matches_first_guard_in_canonical_order() {
    use noble_ngraph::ir::{GraphBuilder, Op, Provenance, ScalarType, SubProfile, Subject};
    // g1 (canonical 3) is in a deeper layer than g2 (canonical 4); both fail.
    let mut b = GraphBuilder::new(SubProfile::F32, Subject::authored(&BLAKE3, "guard order"));
    let x = b.input(ScalarType::F32);
    let y = b.op(Op::F32Add(x, x));
    let z = b.op(Op::F32Add(y, y));
    let g1 = b.guard(Fact::Finite, z);
    let g2 = b.guard(fixtures::range(0.0, 1.0), x);
    b.output(z);
    let _ = Provenance::InputValidated;
    let c = check(&BLAKE3, b.finish()).expect("checks");
    assert!(c.layer(g1) > c.layer(g2));
    let comp = Compiled::new(&BLAKE3, &c);
    let legs = run_legs(&comp, &f32_inputs(&[f32::MAX]), None);
    assert!(legs.agree(), "{legs:?}");
    assert_eq!(
        legs.l4_compiled,
        Obs::GuardFailed {
            node: g1,
            fact: Fact::Finite
        }
    );
    let r = comp.evaluate(&f32_inputs(&[0.5]), None, &mut Control::default());
    assert_eq!(r.obs, Obs::Normal(vec![Value::F32(2.0f32.to_bits())]));
}

// Issue #38: replay the retained kernel-checked binary32 expectations through the
// packed Rust graph reference. The Lean model is the authority for the
// expectation; this lane only shows that the Rust reference agrees with it, and
// it records which configuration the reference ran under.
const LEAN_RETAINED_EXPECTATIONS: &str =
    include_str!("../../../verification/binary32/proposals.json");

fn retained_expectations() -> Vec<(String, u32, u32, u32)> {
    let packet: serde_json::Value =
        serde_json::from_str(LEAN_RETAINED_EXPECTATIONS).expect("retained proposal packet parses");
    let proposals = packet["proposals"].as_array().expect("proposals array");
    proposals
        .iter()
        .map(|p| {
            let operation = p["operation"].as_str().expect("operation").to_owned();
            let bits =
                |key: &str| u32::try_from(p[key].as_u64().expect(key)).expect("u32 encoding");
            (operation, bits("left"), bits("right"), bits("expected"))
        })
        .collect()
}

#[test]
fn lean_retained_binary32_expectations_replay_through_the_graph_reference() {
    let proposals = retained_expectations();
    assert_eq!(proposals.len(), 26, "retained expectation count");
    for (operation, left, right, expected) in proposals {
        let a = f32::from_bits(left);
        let b = f32::from_bits(right);
        let observed = match operation.as_str() {
            "add" => noble_ngraph::sem::add(a, b),
            "sub" => noble_ngraph::sem::sub(a, b),
            "mul" => noble_ngraph::sem::mul(a, b),
            other => panic!("unsupported operation {other}"),
        };
        // NaN payloads are one observation class, so a NaN expectation is matched by
        // observation equality rather than by exact bytes; every other class is
        // matched by its exact encoding.
        let is_nan = |bits: u32| bits >> 23 & 0xFF == 0xFF && bits & 0x7F_FFFF != 0;
        assert!(
            noble_ngraph::sem::obs_eq(Value::F32(observed.to_bits()), Value::F32(expected)),
            "{operation}({left:#010x}, {right:#010x}) observed {:#010x} is not obs_eq to {expected:#010x}",
            observed.to_bits()
        );
        if !is_nan(expected) {
            assert_eq!(
                observed.to_bits(),
                expected,
                "{operation}({left:#010x}, {right:#010x}) observed {:#010x}",
                observed.to_bits()
            );
        }
    }
}

#[test]
fn lean_retained_binary32_expectations_replay_through_the_compiled_graph_legs() {
    use noble_ngraph::ir::{GraphBuilder, Op, ScalarType, SubProfile, Subject};

    let mut matched = 0usize;
    for (operation, left, right, expected) in retained_expectations() {
        let subject = format!("issue 38 retained {operation} {left:#010x} {right:#010x}");
        let mut b = GraphBuilder::new(SubProfile::F32, Subject::authored(&BLAKE3, &subject));
        let x = b.input(ScalarType::F32);
        let y = b.input(ScalarType::F32);
        let node = match operation.as_str() {
            "add" => b.op(Op::F32Add(x, y)),
            "sub" => b.op(Op::F32Sub(x, y)),
            "mul" => b.op(Op::F32Mul(x, y)),
            other => panic!("unsupported operation {other}"),
        };
        b.output(node);
        let checked = check(&BLAKE3, b.finish()).unwrap_or_else(|d| panic!("{subject}: {d}"));
        let comp = Compiled::new(&BLAKE3, &checked);
        let inputs = f32_inputs(&[f32::from_bits(left), f32::from_bits(right)]);
        let legs = run_legs(&comp, &inputs, None);
        assert!(legs.agree(), "{subject}: {legs:?}");
        let expected_observation = Obs::Normal(vec![Value::F32(expected)]);
        for (leg, observed) in [
            ("l2_apfloat", &legs.l2_apfloat),
            ("l3_reference", &legs.l3_reference),
            ("l4_compiled", &legs.l4_compiled),
        ] {
            let Obs::Normal(values) = observed else {
                panic!("{subject} {leg}: expected a normal observation, got {observed:?}");
            };
            let Some(Value::F32(bits)) = values.first() else {
                panic!("{subject} {leg}: expected one F32 value, got {values:?}");
            };
            let is_nan = expected >> 23 & 0xFF == 0xFF && expected & 0x7F_FFFF != 0;
            if is_nan {
                assert!(
                    f32::from_bits(*bits).is_nan(),
                    "{subject} {leg}: {bits:#010x} is not a NaN observation"
                );
            } else {
                assert_eq!(*bits, expected, "{subject} {leg}");
            }
        }
        assert!(
            legs.l3_reference.obs_eq(&expected_observation),
            "{subject}: L3"
        );
        assert!(
            legs.l4_compiled.obs_eq(&expected_observation),
            "{subject}: L4"
        );
        matched += 1;
    }
    assert_eq!(
        matched, 26,
        "retained expectations replayed through the graph legs"
    );
}

#[test]
fn flush_to_zero_disagreement_shrinks_to_a_deterministic_replayable_witness() {
    use shrink::{minimize, replays, DEFAULT_ATTEMPTS};

    let authority = |operation: u8, left: u32, right: u32| -> u32 {
        match operation {
            2 => (f32::from_bits(left) * f32::from_bits(right)).to_bits(),
            other => panic!("unsupported operation {other}"),
        }
    };
    let candidate = |operation: u8, left: u32, right: u32| -> u32 {
        match operation {
            2 => noble_ngraph_experiment::fixtures::ftz_mul(
                f32::from_bits(left),
                f32::from_bits(right),
            )
            .to_bits(),
            other => panic!("unsupported operation {other}"),
        }
    };
    // A pair whose product is a subnormal, so flush-to-zero disagrees.
    let start = (0x0080_0000, 0x3f00_0000);
    let witness = minimize(2, &authority, &candidate, start, DEFAULT_ATTEMPTS)
        .expect("the start pair disagrees under flush-to-zero");
    assert!(replays(&witness, &authority, &candidate), "{witness:?}");
    // Deterministic: a second walk over the same start pair yields the same witness.
    let again = minimize(2, &authority, &candidate, start, DEFAULT_ATTEMPTS)
        .expect("the start pair still disagrees");
    // One stable line lets the receipt runner record the exact witness the walk
    // produced instead of re-deriving it.
    println!(
        "shrink-witness operation={} left={:#010x} right={:#010x} expected={:#010x} candidate={:#010x} attempts={} exhausted={}",
        witness.operation,
        witness.left,
        witness.right,
        witness.expected,
        witness.candidate,
        witness.attempts,
        witness.budget_exhausted,
    );
    assert_eq!(again, witness);
    // The walk terminates inside the documented budget rather than exhausting it,
    // and the witness is a subnormal-versus-zero disagreement: the authority keeps
    // the subnormal product and the candidate flushes it.
    assert!(!witness.budget_exhausted, "{witness:?}");
    assert!(witness.attempts <= DEFAULT_ATTEMPTS);
    let expected_exponent = witness.expected >> 23 & 0xFF;
    assert_eq!(expected_exponent, 0, "{witness:?}");
    assert_ne!(witness.expected & 0x7F_FFFF, 0, "{witness:?}");
    assert_eq!(witness.candidate, 0x0000_0000, "{witness:?}");
}

/// The observation class of a binary32 encoding: the classes a retained expectation
/// distinguishes, so a shrink that moves a witness into another class is rejected
/// rather than reported as progress.
fn binary32_class(bits: u32) -> &'static str {
    let exponent = bits >> 23 & 0xFF;
    let mantissa = bits & 0x7F_FFFF;
    if exponent == 0xFF {
        if mantissa == 0 {
            "infinite"
        } else {
            "nan"
        }
    } else if exponent == 0 {
        if mantissa == 0 {
            "zero"
        } else {
            "subnormal"
        }
    } else {
        "normal"
    }
}

// Issue #38 open task: drive the shrinker from a retained-expectation discrepancy
// (kernel-checked model expectation versus tested implementation) instead of a bare
// operation discrepancy. Every retained expectation the candidate disagrees with is
// shrunk, so a future expectation failure becomes a minimized deterministic record
// that names the expectation it came from, rather than a bare failure.
#[test]
fn retained_expectation_discrepancies_shrink_and_preserve_their_classes() {
    use shrink::{minimize, replays, DEFAULT_ATTEMPTS};

    let authority = |operation: u8, left: u32, right: u32| -> u32 {
        match operation {
            2 => (f32::from_bits(left) * f32::from_bits(right)).to_bits(),
            other => panic!("unsupported operation {other}"),
        }
    };
    let candidate = |operation: u8, left: u32, right: u32| -> u32 {
        match operation {
            2 => noble_ngraph_experiment::fixtures::ftz_mul(
                f32::from_bits(left),
                f32::from_bits(right),
            )
            .to_bits(),
            other => panic!("unsupported operation {other}"),
        }
    };
    let operation_code = |operation: &str| -> u8 {
        match operation {
            "mul" => 2,
            other => panic!(
                "the expectation-driven lane selected {other}, which this walk does not carry"
            ),
        }
    };
    let is_nan = |bits: u32| bits >> 23 & 0xFF == 0xFF && bits & 0x7F_FFFF != 0;

    let mut disagreements = 0_usize;
    let mut carried = 0_usize;
    for (index, (operation, left, right, retained)) in
        retained_expectations().into_iter().enumerate()
    {
        // This walk carries multiplication only: the flush-to-zero candidate applies to
        // it, and the additive retained expectations have no such candidate here. Their
        // own replay lanes above cover them; skipping them is stated rather than a panic.
        if operation != "mul" {
            continue;
        }
        carried += 1;
        let operation_code = operation_code(&operation);
        let observed = candidate(operation_code, left, right);
        // A retained expectation the candidate still reproduces is not a discrepancy
        // and must not be shrunk into one.
        let agrees = if is_nan(retained) {
            noble_ngraph::sem::obs_eq(Value::F32(observed), Value::F32(retained))
        } else {
            observed == retained
        };
        if agrees {
            continue;
        }
        disagreements += 1;
        // The model expectation replays through the host operation (the first replay
        // test above), so the walk's authority and the retained expectation agree on
        // this pair: the discrepancy is exactly candidate-versus-expectation.
        assert_eq!(
            authority(operation_code, left, right),
            retained,
            "expectation {index} does not replay through the authority, so it is not a candidate discrepancy"
        );
        let witness = minimize(
            operation_code,
            &authority,
            &candidate,
            (left, right),
            DEFAULT_ATTEMPTS,
        )
        .expect("a retained-expectation discrepancy disagrees under the walk");
        assert!(replays(&witness, &authority, &candidate), "{witness:?}");
        let again = minimize(
            operation_code,
            &authority,
            &candidate,
            (left, right),
            DEFAULT_ATTEMPTS,
        )
        .expect("the retained pair still disagrees");
        assert_eq!(
            again, witness,
            "expectation {index}: the walk is deterministic"
        );
        assert!(
            !witness.budget_exhausted,
            "expectation {index}: {witness:?}"
        );
        // Class preservation: the minimized witness must still show the class the
        // retained expectation declared, so the shrink cannot silently convert a
        // subnormal rounding discrepancy into an unrelated class.
        assert_eq!(
            binary32_class(witness.expected),
            binary32_class(retained),
            "expectation {index}: the shrink changed the failure class"
        );
        println!(
            "expectation-shrink-witness expectation={index} operation={operation_code} retained={retained:#010x} class={} left={:#010x} right={:#010x} expected={:#010x} candidate={:#010x} attempts={} exhausted={}",
            binary32_class(retained),
            witness.left,
            witness.right,
            witness.expected,
            witness.candidate,
            witness.attempts,
            witness.budget_exhausted,
        );
    }
    println!("expectation-shrink-count={disagreements} carried={carried}");
    assert!(
        disagreements > 0,
        "the flush-to-zero candidate must disagree with at least one retained expectation"
    );
}

#[test]
fn agreeing_pairs_produce_no_witness() {
    use shrink::{minimize, DEFAULT_ATTEMPTS};
    let authority = |operation: u8, left: u32, right: u32| -> u32 {
        (operation as u32).wrapping_add(left).wrapping_add(right)
    };
    let candidate = authority;
    assert!(minimize(
        0,
        &authority,
        &candidate,
        (0x3f80_0000, 0x3f80_0000),
        DEFAULT_ATTEMPTS
    )
    .is_none());
}

/// B2 L1 coverage: a per-program mode record is honest only when every vector
/// is either an ordinary agreement or a recorded ordinary quota outcome, and at
/// least one ordinary comparison agreed. The committed phase-B-extract receipt
/// shows `random-1` agreeing on zero vectors, which this rule refuses.
#[test]
fn program_with_only_quota_outcomes_has_no_agreement() {
    use noble_ngraph_experiment::b2i64::l1_coverage;
    // The observed defect: 20 vectors, all refused at the acceptance stage.
    assert_eq!(l1_coverage(0, 20, 20), Err("no ordinary-path agreement"));
    assert_eq!(l1_coverage(1, 19, 20), Ok(()));
}

#[test]
fn every_l1_vector_must_be_agreement_or_recorded_quota() {
    use noble_ngraph_experiment::b2i64::l1_coverage;
    assert_eq!(l1_coverage(20, 0, 20), Ok(()));
    assert_eq!(
        l1_coverage(19, 0, 20),
        Err("vectors outside agreement plus recorded ordinary quota")
    );
    assert_eq!(l1_coverage(0, 19, 20), Err("no ordinary-path agreement"));
    assert_eq!(l1_coverage(0, 0, 0), Err("no ordinary-path agreement"));
    assert_eq!(
        l1_coverage(usize::MAX, 1, 0),
        Err("vectors outside agreement plus recorded ordinary quota")
    );
}

struct Issue125Cell {
    family: &'static str,
    fp_class: &'static str,
    graph: noble_ngraph::Graph,
    inputs: [f32; 6],
}

fn issue125_values(family: &str, fp_class: &str) -> [f32; 6] {
    let (special, secondary, third, safe) = match fp_class {
        "signed_zero" => (-0.0, -0.0, 0.0, -0.0),
        "nan" => (f32::from_bits(0x7fc0_0000), 0.0, 0.0, 1.0),
        "subnormal_normal_boundary" => (f32::from_bits(1), f32::MIN_POSITIVE, 0.0, 0.0),
        "overflow_infinity" => (f32::MAX, 0.0, 0.0, 1.0),
        "cancellation" => (1.0e20, -1.0e20, 3.0, 2.0e20),
        "rounding_tie" => (
            1.0,
            f32::from_bits(0x3380_0000),
            f32::from_bits(0x3380_0000),
            2.0,
        ),
        _ => unreachable!("matrix-v1 FP class"),
    };
    if matches!(family, "index_affine" | "guard_affine") && fp_class == "cancellation" {
        return [-1.0e20, secondary, third, 1.0, 1.0, 1.0e20];
    }
    if matches!(family, "index_affine" | "guard_affine") && fp_class == "rounding_tie" {
        return [special, secondary, -1.0, 1.0, 1.0, 1.0];
    }
    [special, secondary, third, 1.0, 1.0, safe]
}

fn issue125_cell(family: &'static str, fp_class: &'static str) -> Issue125Cell {
    let values = issue125_values(family, fp_class);
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, &format!("issue125/{family}/{fp_class}")),
    );
    let special = b.input(ScalarType::F32);
    let secondary = b.input(ScalarType::F32);
    let third = b.input(ScalarType::F32);
    let index = b.input(ScalarType::F32);
    let bit = b.input(ScalarType::F32);
    let safe = b.input(ScalarType::F32);
    b.fact(index, Fact::Integral, Provenance::InputValidated);
    b.fact(
        index,
        Fact::Range {
            lo: Literal::f32(0.0),
            hi: Literal::f32(1.0),
        },
        Provenance::InputValidated,
    );
    b.fact(bit, Fact::Bit, Provenance::InputValidated);
    let output = match family {
        "guard_select" => {
            let selected = b.op(Op::F32Select {
                bit,
                off: safe,
                on: special,
            });
            let output = issue125_post_select(&mut b, fp_class, selected, secondary, third);
            b.guard(Fact::Finite, safe);
            output
        }
        "select_index" => {
            let zero = b.f32_const(0.0);
            let selected = b.op(Op::F32Select {
                bit,
                off: zero,
                on: index,
            });
            let indexed = b.op(Op::F32Index {
                index: selected,
                table: vec![safe, special],
                below: OutOfRange::Refuse,
                above: OutOfRange::Refuse,
            });
            issue125_post_select(&mut b, fp_class, indexed, secondary, third)
        }
        "index_affine" => {
            let indexed = b.op(Op::F32Index {
                index,
                table: vec![safe, special],
                below: OutOfRange::Refuse,
                above: OutOfRange::Refuse,
            });
            issue125_affine(&mut b, fp_class, indexed, secondary, third, safe)
        }
        "guard_affine" => {
            let output = issue125_affine(&mut b, fp_class, special, secondary, third, safe);
            b.guard(Fact::Finite, safe);
            output
        }
        _ => unreachable!("matrix-v1 interaction family"),
    };
    b.output(output);
    Issue125Cell {
        family,
        fp_class,
        graph: b.finish(),
        inputs: values,
    }
}

fn issue125_input_slot(graph: &noble_ngraph::Graph, node: NodeId) -> Option<u32> {
    match &graph.nodes.get(node.index())?.op {
        Op::Input(slot) => Some(*slot),
        _ => None,
    }
}

fn issue125_input_value(
    graph: &noble_ngraph::Graph,
    node: NodeId,
    inputs: &[f32; 6],
) -> Option<f32> {
    match &graph.nodes.get(node.index())?.op {
        Op::Input(slot) => inputs.get(*slot as usize).copied(),
        Op::Const(Literal::F32(bits)) => Some(f32::from_bits(*bits)),
        _ => None,
    }
}

fn issue125_runtime_selected_branch(
    graph: &noble_ngraph::Graph,
    family: &str,
    inputs: &[f32; 6],
) -> Option<&'static str> {
    if !matches!(family, "guard_select" | "select_index") {
        return None;
    }
    let bit_node = graph.nodes.iter().find_map(|node| match &node.op {
        Op::F32Select { bit, .. } => Some(*bit),
        _ => None,
    })?;
    let bit_value = issue125_input_value(graph, bit_node, inputs)?;
    if bit_value == 1.0 {
        Some("on")
    } else if bit_value == 0.0 {
        Some("off")
    } else {
        Some("invalid-bit-input")
    }
}

fn issue125_runtime_table_index(
    graph: &noble_ngraph::Graph,
    family: &str,
    inputs: &[f32; 6],
) -> Option<u32> {
    if !matches!(family, "select_index" | "index_affine") {
        return None;
    }
    let (index_node, table) = graph.nodes.iter().find_map(|node| match &node.op {
        Op::F32Index { index, table, .. } => Some((*index, table)),
        _ => None,
    })?;
    let index_value = match &graph.nodes.get(index_node.index())?.op {
        Op::F32Select { off, on, .. } => {
            let branch = issue125_runtime_selected_branch(graph, "select_index", inputs)?;
            issue125_input_value(graph, if branch == "on" { *on } else { *off }, inputs)?
        }
        _ => issue125_input_value(graph, index_node, inputs)?,
    };
    let index = index_value as u32;
    (index_value >= 0.0 && index < table.len() as u32).then_some(index)
}

fn issue125_selected_branch_input_slot(graph: &noble_ngraph::Graph, family: &str) -> Option<u32> {
    if !matches!(family, "guard_select" | "select_index") {
        return None;
    }
    let bit_node = graph.nodes.iter().find_map(|node| match &node.op {
        Op::F32Select { bit, .. } => Some(*bit),
        _ => None,
    })?;
    issue125_input_slot(graph, bit_node)
}

fn issue125_index_input_slot(
    graph: &noble_ngraph::Graph,
    family: &str,
    inputs: &[f32; 6],
) -> Option<u32> {
    if !matches!(family, "select_index" | "index_affine") {
        return None;
    }
    let index_node = graph.nodes.iter().find_map(|node| match &node.op {
        Op::F32Index { index, .. } => Some(*index),
        _ => None,
    })?;
    let selected_node = match &graph.nodes.get(index_node.index())?.op {
        Op::F32Select { off, on, .. } => {
            let branch = issue125_runtime_selected_branch(graph, "select_index", inputs)?;
            if branch == "on" {
                *on
            } else {
                *off
            }
        }
        _ => index_node,
    };
    issue125_input_slot(graph, selected_node)
}

fn issue125_selected_table_node(
    graph: &noble_ngraph::Graph,
    family: &str,
    inputs: &[f32; 6],
) -> Option<NodeId> {
    let index = issue125_runtime_table_index(graph, family, inputs)?;
    graph.nodes.iter().find_map(|node| match &node.op {
        Op::F32Index { table, .. } => table.get(index as usize).copied(),
        _ => None,
    })
}

fn issue125_random_interaction_families(graph: &noble_ngraph::Graph) -> Vec<&'static str> {
    let mut families = Vec::new();
    let mut record = |family| {
        if !families.contains(&family) {
            families.push(family);
        }
    };
    for node in &graph.nodes {
        match &node.op {
            Op::F32Select { off, on, .. } => {
                if graph.nodes.iter().any(|candidate| {
                    matches!(&candidate.op,
                    Op::Guard { value, .. } if *value == *off || *value == *on)
                }) {
                    record("guard_select");
                }
            }
            Op::F32Index { index, .. } => {
                if matches!(&graph.nodes[index.index()].op, Op::F32Select { .. }) {
                    record("select_index");
                }
            }
            Op::F32Affine { terms, .. } => {
                for term in terms {
                    if !matches!(
                        &graph.nodes[match &term.operand {
                            AffineOperand::Node(value) => value.index(),
                            AffineOperand::One => continue,
                        }]
                        .op,
                        Op::F32Index { .. }
                    ) {
                        continue;
                    }
                    record("index_affine");
                }
                if graph.nodes.iter().any(|candidate| {
                    matches!(&candidate.op,
                    Op::Guard { value, .. } if terms.iter().any(|term|
                        matches!(&term.operand, AffineOperand::Node(input) if *input == *value)))
                }) {
                    record("guard_affine");
                }
            }
            _ => {}
        }
    }
    families
}

fn issue125_post_select(
    b: &mut GraphBuilder,
    fp_class: &str,
    selected: NodeId,
    secondary: NodeId,
    third: NodeId,
) -> NodeId {
    match fp_class {
        "overflow_infinity" => {
            let two = b.f32_const(2.0);
            b.op(Op::F32Mul(selected, two))
        }
        "cancellation" => b.op(Op::F32Add(selected, secondary)),
        "rounding_tie" => {
            let first = b.op(Op::F32Add(selected, secondary));
            b.op(Op::F32Add(first, third))
        }
        _ => selected,
    }
}

fn issue125_affine(
    b: &mut GraphBuilder,
    fp_class: &str,
    selected: NodeId,
    secondary: NodeId,
    third: NodeId,
    safe: NodeId,
) -> NodeId {
    let bias = if fp_class == "signed_zero" { -0.0 } else { 0.0 };
    let terms = match fp_class {
        "signed_zero" => vec![
            (1.0, AffineOperand::Node(safe)),
            (1.0, AffineOperand::Node(selected)),
        ],
        "nan" => vec![
            (1.0, AffineOperand::Node(safe)),
            (1.0, AffineOperand::Node(selected)),
        ],
        "subnormal_normal_boundary" => vec![
            (0.0, AffineOperand::Node(safe)),
            (1.0, AffineOperand::Node(selected)),
            (0.0, AffineOperand::Node(secondary)),
        ],
        "overflow_infinity" => vec![
            (0.0, AffineOperand::Node(safe)),
            (2.0, AffineOperand::Node(selected)),
        ],
        "cancellation" => vec![
            (1.0, AffineOperand::Node(safe)),
            (1.0, AffineOperand::Node(selected)),
            (1.0, AffineOperand::Node(third)),
        ],
        "rounding_tie" => vec![
            (0.0, AffineOperand::Node(safe)),
            (1.0, AffineOperand::Node(selected)),
            (1.0, AffineOperand::Node(secondary)),
            (1.0, AffineOperand::Node(third)),
        ],
        _ => unreachable!("matrix-v1 FP class"),
    };
    b.affine(bias, &terms)
}

fn issue125_expected(family: &str, fp_class: &str) -> Option<u32> {
    match fp_class {
        "signed_zero" => Some(0x8000_0000),
        "nan" => None,
        "subnormal_normal_boundary" => Some(1),
        "overflow_infinity" => Some(f32::INFINITY.to_bits()),
        "cancellation" if family == "index_affine" || family == "guard_affine" => {
            Some(3.0f32.to_bits())
        }
        "cancellation" => Some(0.0f32.to_bits()),
        "rounding_tie" if family == "index_affine" || family == "guard_affine" => {
            Some(0.0f32.to_bits())
        }
        "rounding_tie" => Some(1.0f32.to_bits()),
        _ => unreachable!("matrix-v1 FP class"),
    }
}

fn issue125_property(obs: &Obs, expected: Option<u32>) -> bool {
    let bits = issue125_output_bits(obs);
    match (expected, bits.first()) {
        (None, Some(bits)) => f32::from_bits(*bits).is_nan(),
        (Some(expected), Some(actual)) => *actual == expected,
        _ => false,
    }
}

fn issue125_output_bits(obs: &Obs) -> Vec<u32> {
    match obs {
        Obs::Normal(values) => values
            .iter()
            .filter_map(|value| match value {
                Value::F32(bits) => Some(*bits),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn issue125_observation_code(obs: &Obs) -> u32 {
    match obs {
        Obs::Normal(_) => issue125_output_bits(obs).first().copied().unwrap_or(0),
        Obs::GuardFailed { .. } => u32::MAX - 1,
        Obs::Refused(_) => u32::MAX - 2,
        Obs::Exhausted(_) => u32::MAX - 3,
        Obs::Cancelled => u32::MAX - 4,
    }
}

fn issue125_mismatch_class(
    legs: &noble_ngraph_experiment::differential::Legs,
) -> Option<&'static str> {
    if legs.agree() {
        return None;
    }
    match (&legs.l2_apfloat, &legs.l3_reference) {
        (Obs::Normal(_), Obs::Normal(_)) => Some("normal-value-mismatch"),
        (Obs::Normal(_), _) | (_, Obs::Normal(_)) => Some("normal-outcome-mismatch"),
        _ => Some("non-normal-observation-mismatch"),
    }
}

fn issue125_shrink_candidate_allowed(
    case: &Issue125Cell,
    inputs: &[f32; 6],
    legs: &noble_ngraph_experiment::differential::Legs,
    expected: Option<u32>,
    mismatch_class: &str,
) -> bool {
    let required_nodes: &[&str] = match case.family {
        "guard_select" => &["guard", "f32.select"],
        "select_index" => &["f32.select", "f32.index"],
        "index_affine" => &["f32.index", "f32.affine"],
        "guard_affine" => &["guard", "f32.affine"],
        _ => return false,
    };
    let graph_ops: Vec<&str> = case.graph.nodes.iter().map(|node| node.op.name()).collect();
    required_nodes.iter().all(|name| graph_ops.contains(name))
        && inputs[3].to_bits() == case.inputs[3].to_bits()
        && inputs[4].to_bits() == case.inputs[4].to_bits()
        && issue125_runtime_selected_branch(&case.graph, case.family, inputs)
            == issue125_runtime_selected_branch(&case.graph, case.family, &case.inputs)
        && issue125_runtime_table_index(&case.graph, case.family, inputs)
            == issue125_runtime_table_index(&case.graph, case.family, &case.inputs)
        && legs.l3_evaluated_nodes == case.graph.nodes.len() as u64
        && matches!(&legs.l3_reference, Obs::Normal(_) | Obs::GuardFailed { .. })
        && issue125_property(&legs.l2_apfloat, expected)
        && !issue125_property(&legs.l3_reference, expected)
        && issue125_mismatch_class(legs) == Some(mismatch_class)
}

fn issue125_shrink_inputs(
    case: &Issue125Cell,
    compiled: &Compiled,
    expected: Option<u32>,
    mismatch_class: &'static str,
) -> Option<serde_json::Value> {
    use shrink::{minimize, DEFAULT_ATTEMPTS};
    let run_pair = |left: u32, right: u32| {
        let mut inputs = case.inputs;
        inputs[0] = f32::from_bits(left);
        inputs[1] = f32::from_bits(right);
        run_legs(compiled, &f32_inputs(&inputs), None)
    };
    let authority = |_: u8, left: u32, right: u32| {
        let legs = run_pair(left, right);
        if issue125_property(&legs.l2_apfloat, expected) {
            issue125_observation_code(&legs.l2_apfloat)
        } else {
            0
        }
    };
    let candidate = |_: u8, left: u32, right: u32| {
        let mut inputs = case.inputs;
        inputs[0] = f32::from_bits(left);
        inputs[1] = f32::from_bits(right);
        let legs = run_pair(left, right);
        if issue125_shrink_candidate_allowed(case, &inputs, &legs, expected, mismatch_class) {
            issue125_observation_code(&legs.l3_reference)
        } else if issue125_property(&legs.l2_apfloat, expected) {
            issue125_observation_code(&legs.l2_apfloat)
        } else {
            0
        }
    };
    let witness = minimize(
        0,
        &authority,
        &candidate,
        (case.inputs[0].to_bits(), case.inputs[1].to_bits()),
        DEFAULT_ATTEMPTS,
    )?;
    let mut minimized_inputs = case.inputs;
    minimized_inputs[0] = f32::from_bits(witness.left);
    minimized_inputs[1] = f32::from_bits(witness.right);
    let legs = run_pair(witness.left, witness.right);
    if !issue125_shrink_candidate_allowed(case, &minimized_inputs, &legs, expected, mismatch_class)
    {
        return None;
    }
    let required = match case.family {
        "guard_select" => ["guard", "f32.select"],
        "select_index" => ["f32.select", "f32.index"],
        "index_affine" => ["f32.index", "f32.affine"],
        "guard_affine" => ["guard", "f32.affine"],
        _ => unreachable!("matrix-v1 interaction family"),
    };
    let graph_ops: Vec<&str> = case.graph.nodes.iter().map(|node| node.op.name()).collect();
    let target_feature_preserved = required.iter().all(|name| graph_ops.contains(name));
    Some(serde_json::json!({
        "input_bits": minimized_inputs.map(f32::to_bits),
        "original_input_bits": case.inputs.map(f32::to_bits),
        "graph_unchanged": true,
        "selected_branch_preserved": minimized_inputs[4].to_bits() == case.inputs[4].to_bits(),
        "selected_table_index_preserved": minimized_inputs[3].to_bits() == case.inputs[3].to_bits(),
        "target_feature_preserved": target_feature_preserved,
        "mutation_site": std::env::var("NOBLE_ISSUE125_MUTATION_ID").ok(),
        "snapshot": null,
        "limits": {
            "control": "Control::default()",
            "fuel": null,
            "cancellation": false,
            "scratch_instrumentation": false,
            "forward_scratch_override": null,
            "effective_forward_scratch": noble_ngraph::check::Budgets::default().forward_scratch
        },
        "mismatch_class": mismatch_class,
        "l2_after": format!("{:?}", legs.l2_apfloat),
        "l3_after": format!("{:?}", legs.l3_reference),
        "work": witness.attempts,
        "budget": DEFAULT_ATTEMPTS,
        "eligibility_rechecked_each_candidate": true,
        "expected_property_rechecked_each_candidate": true,
        "mismatch_class_rechecked_each_candidate": true,
        "termination_reason": if witness.budget_exhausted { "attempt-budget-exhausted" } else { "local-minimum-under-issue38-input-simplifications" },
        "global_minimum_claimed": false
    }))
}

#[test]
fn issue125_structured_interaction_matrix() {
    const MATRIX: &str =
        include_str!("../../../verification/differential-interactions/matrix-v1.json");
    let matrix_sha256 = blake3::hash(MATRIX.as_bytes()).to_hex().to_string();
    let families = [
        "guard_select",
        "select_index",
        "index_affine",
        "guard_affine",
    ];
    let classes = [
        "signed_zero",
        "nan",
        "subnormal_normal_boundary",
        "overflow_infinity",
        "cancellation",
        "rounding_tie",
    ];
    let mut cells = Vec::new();
    let mut all_reached = true;
    let mut shrunk = false;
    let mutation_id = std::env::var("NOBLE_ISSUE125_MUTATION_ID").ok();
    let mutation_target = std::env::var("NOBLE_ISSUE125_MUTATION_CELL").ok();
    for family in families {
        for fp_class in classes {
            let case = issue125_cell(family, fp_class);
            let checked = check(&BLAKE3, case.graph.clone()).unwrap_or_else(|diagnostic| {
                panic!("{} x {} refused: {diagnostic}", case.family, case.fp_class)
            });
            let compiled = Compiled::new(&BLAKE3, &checked);
            let inputs = f32_inputs(&case.inputs);
            let legs = run_legs(&compiled, &inputs, None);
            let output_bits = issue125_output_bits(&legs.l3_reference);
            let expected = issue125_expected(family, fp_class);
            let property_engaged = issue125_property(&legs.l3_reference, expected);
            let required_nodes: Vec<&str> = match family {
                "guard_select" => vec!["guard", "f32.select"],
                "select_index" => vec!["f32.select", "f32.index"],
                "index_affine" => vec!["f32.index", "f32.affine"],
                "guard_affine" => vec!["guard", "f32.affine"],
                _ => unreachable!("matrix-v1 interaction family"),
            };
            let graph_ops: Vec<&str> = case.graph.nodes.iter().map(|node| node.op.name()).collect();
            let ops_present = required_nodes.iter().all(|name| graph_ops.contains(name));
            let reached = matches!(legs.l3_reference, Obs::Normal(_))
                && legs.l3_evaluated_nodes == checked.node_count() as u64
                && ops_present
                && legs.agree()
                && property_engaged;
            all_reached &= reached;
            let mismatch_class = issue125_mismatch_class(&legs);
            let cell_name = format!("P2/{family}/{fp_class}");
            let minimized_witness = if !shrunk
                && mutation_target
                    .as_deref()
                    .is_none_or(|target| target == cell_name)
            {
                mismatch_class.and_then(|class| {
                    let witness = issue125_shrink_inputs(&case, &compiled, expected, class);
                    shrunk = witness.is_some();
                    witness
                })
            } else {
                None
            };
            let graph_text = noble_ngraph::format::graph::print(&case.graph);
            cells.push(serde_json::json!({
                "cell": cell_name,
                "interaction_family": family,
                "fp_class": fp_class,
                "selected": true,
                "reached": reached,
                "admission": "accepted",
                "graph_id": noble_ngraph::ir::hex(&checked.id()),
                "graph_text": graph_text,
                "graph_text_blake3": blake3::hash(graph_text.as_bytes()).to_hex().to_string(),
                "input_bits": case.inputs.map(f32::to_bits),
                "operator_nodes": graph_ops,
                "required_operator_nodes": required_nodes,
                "selected_branch": issue125_runtime_selected_branch(&case.graph, family, &case.inputs),
                "selected_branch_input_slot": issue125_selected_branch_input_slot(&case.graph, family),
                "selected_branch_input_bits": issue125_selected_branch_input_slot(&case.graph, family)
                    .map(|slot| case.inputs[slot as usize].to_bits()),
                "index_input_slot": issue125_index_input_slot(&case.graph, family, &case.inputs),
                "index_input_bits": issue125_index_input_slot(&case.graph, family, &case.inputs)
                    .map(|slot| case.inputs[slot as usize].to_bits()),
                "selected_table_index": issue125_runtime_table_index(&case.graph, family, &case.inputs),
                "selected_table_node": issue125_selected_table_node(&case.graph, family, &case.inputs)
                    .map(|node| node.0),
                "snapshot": null,
                "limits": {
                    "control": "Control::default()",
                    "fuel": null,
                    "cancellation": false,
                    "scratch_instrumentation": false,
                    "forward_scratch_override": null,
                    "effective_forward_scratch": noble_ngraph::check::Budgets::default().forward_scratch
                },
                "expected_output_bits": expected,
                "l3_nodes_evaluated": legs.l3_evaluated_nodes,
                "checked_node_count": checked.node_count(),
                "output_bits": output_bits,
                "property_engaged": issue125_property(&legs.l2_apfloat, expected),
                "property_failed": !issue125_property(&legs.l3_reference, expected) && mismatch_class.is_some(),
                "legs_agree": legs.agree(),
                "mismatch_class": mismatch_class,
                "minimized_witness": minimized_witness,
                "comparison_legs_reached": ["L2 APFloat", "binary64-per-operation", "L3 reference", "L4 compiled"],
                "l2": format!("{:?}", legs.l2_apfloat),
                "binary64": format!("{:?}", legs.b2_oracle_f64),
                "l3": format!("{:?}", legs.l3_reference),
                "l4": format!("{:?}", legs.l4_compiled),
                "l4_per_node_trace_available": false,
                "witness_kind": "L3 full-node evaluation count plus runtime-derived direct input branch/index and output-discriminating values; L4 matched output, no per-node trace"
            }));
        }
    }

    let structured_node_budget: u64 = cells
        .iter()
        .map(|cell| {
            cell["l3_nodes_evaluated"]
                .as_u64()
                .expect("structured node-work receipt")
        })
        .sum();
    let comparison_seed = 125_u64;
    let mut current_ops = 48_usize;
    let (current_random_graph, current_random_checked) = loop {
        let generated = noble_ngraph_experiment::random::graph(comparison_seed, current_ops);
        let checked = check(&BLAKE3, generated.graph.clone()).expect("current random graph checks");
        if checked.node_count() as u64 <= structured_node_budget || current_ops == 0 {
            break (generated, checked);
        }
        current_ops -= 1;
    };
    let current_node_count = current_random_checked.node_count() as u64;
    let current_vector_budget = (structured_node_budget / current_node_count).max(1);
    let current_potential_node_work = current_node_count * current_vector_budget;
    let current_campaign = campaign(comparison_seed, current_ops, current_vector_budget);
    let current_interactions = issue125_random_interaction_families(&current_random_graph.graph);
    let current_full_execution = current_campaign.full_normal_executions > 0;
    let current_reached_interactions: Vec<&str> = if current_full_execution {
        current_interactions.iter().copied().collect()
    } else {
        Vec::new()
    };
    let structured_failures: Vec<serde_json::Value> = cells.iter()
        .filter(|cell| cell["mismatch_class"] != serde_json::Value::Null)
        .map(|cell| serde_json::json!({ "cell": cell["cell"], "mismatch_class": cell["mismatch_class"] }))
        .collect();
    let structured_reached_count = cells.iter().filter(|cell| cell["reached"] == true).count();
    let structured_reached_cells: Vec<&str> = cells
        .iter()
        .filter(|cell| cell["reached"] == true)
        .filter_map(|cell| cell["cell"].as_str())
        .collect();
    let generator_comparison = serde_json::json!({
        "profiles_separate": true,
        "seed_budget": { "current_graph_seed": comparison_seed,
            "current_input_seed": current_campaign.input_seed,
            "structured_fixed_case_seed": comparison_seed, "graphs_per_arm": 1 },
        "compute_budget": { "unit": "L3 node evaluations", "cap": structured_node_budget,
            "structured_actual": structured_node_budget,
            "current_potential_max": current_potential_node_work,
            "current_actual": current_campaign.l3_evaluated_nodes,
            "current_unused": structured_node_budget.saturating_sub(current_campaign.l3_evaluated_nodes),
            "same_budget_cap": current_potential_node_work <= structured_node_budget,
            "note": "The random graph is indivisible; its full-execution node budget is capped at the structured arm's measured work, and input-fact refusals can leave that cap unused." },
        "current_generator": { "ops_requested": current_ops, "graph_id": noble_ngraph::ir::hex(&current_random_checked.id()),
            "graph_nodes": current_node_count, "vectors": current_campaign.vectors,
            "full_normal_executions": current_campaign.full_normal_executions,
            "full_normal_input_bits": current_campaign.full_normal_input_bits,
            "normal": current_campaign.normal, "guard_failed": current_campaign.guard_failed,
            "refused": current_campaign.refused, "disagreements": current_campaign.disagreements,
            "failure_classes": current_campaign.failure_classes,
            "first_disagreement": current_campaign.first_disagreement,
            "structural_interaction_pairs": current_interactions,
            "reached_interactions": current_reached_interactions,
            "reached_cells": [],
            "reached_cell_limitation": "No full-normal L3 execution; matrix-cell coverage is not inferred from structural tags.",
            "witness_kind": "connected operation nodes plus at least one normal, full-node L3 execution; no per-node or selected-branch trace" },
        "structured_extension": { "cells": cells.len(), "reached_cell_count": structured_reached_count,
            "reached_cells": structured_reached_cells,
            "actual_node_work": structured_node_budget, "distinct_attributed_failures": structured_failures,
            "witness_kind": "per-cell full-node L3 count, runtime-derived direct input branch/index, outputs and all-leg comparison" },
        "distinct_attributed_failures": { "current_generator_classes": current_campaign.failure_classes,
            "structured_cells": structured_failures }
    });

    let mut invalid = issue125_cell("guard_select", "nan");
    invalid.inputs[5] = f32::NAN;
    let invalid_checked = check(&BLAKE3, invalid.graph.clone()).expect("valid guard-failure graph");
    let invalid_legs = run_legs(
        &Compiled::new(&BLAKE3, &invalid_checked),
        &f32_inputs(&invalid.inputs),
        None,
    );
    let invalid_refused =
        matches!(invalid_legs.l3_reference, Obs::GuardFailed { .. }) && invalid_legs.agree();

    let mut invalid_index = issue125_cell("index_affine", "subnormal_normal_boundary");
    invalid_index.inputs[3] = f32::NAN;
    let invalid_index_checked = check(&BLAKE3, invalid_index.graph.clone())
        .expect("input-fact mutation keeps the graph admitted");
    let invalid_index_legs = run_legs(
        &Compiled::new(&BLAKE3, &invalid_index_checked),
        &f32_inputs(&invalid_index.inputs),
        None,
    );
    let invalid_index_refused = matches!(invalid_index_legs.l2_apfloat, Obs::Refused(_))
        && matches!(invalid_index_legs.b2_oracle_f64, Obs::Refused(_))
        && matches!(invalid_index_legs.l3_reference, Obs::Refused(_))
        && matches!(invalid_index_legs.l4_compiled, Obs::Refused(_))
        && invalid_index_legs.agree();
    let earlier_refusal_rejected_by_shrinker = !issue125_shrink_candidate_allowed(
        &invalid_index,
        &invalid_index.inputs,
        &invalid_index_legs,
        issue125_expected("index_affine", "subnormal_normal_boundary"),
        "normal-value-mismatch",
    );
    let target_failure_class = mutation_target
        .as_ref()
        .and_then(|target| {
            cells
                .iter()
                .find(|cell| cell["cell"].as_str() == Some(target.as_str()))
                .and_then(|cell| cell["mismatch_class"].as_str())
        })
        .unwrap_or("normal-value-mismatch");
    let substitute_class = if target_failure_class == "normal-value-mismatch" {
        "normal-outcome-mismatch"
    } else {
        "normal-value-mismatch"
    };
    let expected_value = noble_ngraph::sem::Value::f32(f32::from_bits(1));
    let substituted_l3 = if substitute_class == "normal-outcome-mismatch" {
        Obs::GuardFailed {
            node: noble_ngraph::ir::NodeId(0),
            fact: noble_ngraph::ir::Fact::Finite,
        }
    } else {
        Obs::Normal(vec![noble_ngraph::sem::Value::f32(2.0)])
    };
    let substituted_failure = noble_ngraph_experiment::differential::Legs {
        l2_apfloat: Obs::Normal(vec![expected_value]),
        b2_oracle_f64: Obs::Normal(vec![expected_value]),
        l3_reference: substituted_l3.clone(),
        l4_compiled: substituted_l3,
        l3_evaluated_nodes: 3,
    };
    let different_failure_case = issue125_cell("index_affine", "subnormal_normal_boundary");
    let different_failure_rejected_by_shrinker = !issue125_shrink_candidate_allowed(
        &different_failure_case,
        &different_failure_case.inputs,
        &substituted_failure,
        Some(1),
        target_failure_class,
    );
    let different_failure_control = serde_json::json!({
        "target_failure_class": target_failure_class,
        "substituted_failure_class": substitute_class,
        "property_still_engaged_on_l2": issue125_property(&substituted_failure.l2_apfloat, Some(1)),
        "different_failure_rejected_by_shrinker": different_failure_rejected_by_shrinker
    });

    let quota = issue125_cell("guard_select", "signed_zero");
    let quota_checked = check(&BLAKE3, quota.graph).expect("valid quota graph");
    let quota_inputs = f32_inputs(&quota.inputs);
    let mut reference_control = Control {
        fuel: Some(0),
        ..Control::default()
    };
    let reference_quota =
        noble_ngraph::eval::reference(&quota_checked, &quota_inputs, None, &mut reference_control)
            .obs;
    let mut compiled_control = Control {
        fuel: Some(0),
        ..Control::default()
    };
    let compiled_quota = Compiled::new(&BLAKE3, &quota_checked)
        .evaluate(&quota_inputs, None, &mut compiled_control)
        .obs;
    let exhausted = matches!(
        reference_quota,
        Obs::Exhausted(noble_ngraph::BudgetKind::Fuel)
    ) && matches!(
        compiled_quota,
        Obs::Exhausted(noble_ngraph::BudgetKind::Fuel)
    );
    let mut reference_boundary_control = Control {
        fuel: Some(1),
        ..Control::default()
    };
    let reference_boundary = noble_ngraph::eval::reference(
        &quota_checked,
        &quota_inputs,
        None,
        &mut reference_boundary_control,
    )
    .obs;
    let mut compiled_boundary_control = Control {
        fuel: Some(1),
        ..Control::default()
    };
    let compiled_boundary = Compiled::new(&BLAKE3, &quota_checked)
        .evaluate(&quota_inputs, None, &mut compiled_boundary_control)
        .obs;
    let exhausted_at_one = matches!(
        reference_boundary,
        Obs::Exhausted(noble_ngraph::BudgetKind::Fuel)
    ) && matches!(
        compiled_boundary,
        Obs::Exhausted(noble_ngraph::BudgetKind::Fuel)
    );

    let ftz = fixtures::b2_vectors()
        .into_iter()
        .find(|vector| vector.name.starts_with("subnormal: 2^-126"))
        .expect("retained #38 subnormal vector");
    let ftz_checked = check(&BLAKE3, ftz.graph).expect("retained #38 vector checks");
    let ftz_legs = run_legs(
        &Compiled::new(&BLAKE3, &ftz_checked),
        &f32_inputs(&ftz.inputs),
        None,
    );
    let weak_ftz = fixtures::ftz_mul(f32::MIN_POSITIVE, 0.5);
    let independent_oracle_control = ftz_legs.agree()
        && issue125_output_bits(&ftz_legs.l2_apfloat) == vec![ftz.expected[0]]
        && weak_ftz.to_bits() != ftz.expected[0];
    let accepted = cells.iter().filter(|cell| cell["reached"] == true).count();
    let executable = std::env::current_exe().expect("test executable path");
    let executable_hash = blake3::hash(&std::fs::read(&executable).expect("read test executable"))
        .to_hex()
        .to_string();
    let mutation_probe = mutation_id.as_ref().map(|id| {
        let target = mutation_target.as_deref().expect("mutation target cell");
        let cell = cells.iter().find(|cell| cell["cell"] == target)
            .expect("mutation target is a selected matrix cell");
        let expected_mismatch = cell["mismatch_class"].as_str();
        let l2_property = cell["property_engaged"] == true;
        let l3_property_failed = cell["property_failed"] == true;
        let execution_reached = cell["admission"] == "accepted"
            && cell["l3_nodes_evaluated"] == cell["checked_node_count"]
            && cell["required_operator_nodes"].as_array().is_some_and(|required| {
                let present = cell["operator_nodes"].as_array();
                required.iter().all(|name| present.is_some_and(|nodes|
                    nodes.iter().any(|node| node == name)))
            });
        let minimized = cell["minimized_witness"].clone();
        let shrink_work = minimized.get("work").cloned();
        let graph_text = cell["graph_text"].as_str().expect("retained graph text");
        let original_witness = serde_json::json!({
            "graph_text": graph_text,
            "input_bits": cell["input_bits"].clone()
        });
        let minimized_witness = serde_json::json!({
            "graph_text": graph_text,
            "input_bits": minimized["input_bits"].clone()
        });
        let original_witness_size_bytes = original_witness.to_string().len();
        let minimized_witness_size_bytes = minimized_witness.to_string().len();
        let failure_identity = expected_mismatch.map(|mismatch| format!("{id}:{target}:{mismatch}"));
        let outcome = if execution_reached && l2_property && l3_property_failed
            && expected_mismatch.is_some() && minimized.is_object()
            && minimized["mismatch_class"].as_str() == expected_mismatch
            && minimized["target_feature_preserved"] == true
            && minimized["selected_branch_preserved"] == true
            && minimized["selected_table_index_preserved"] == true
        { "killed" } else if execution_reached && expected_mismatch.is_none() {
            "executed-survived"
        } else if expected_mismatch.is_none() {
            "divergence-unreached"
        } else { "incomplete" };
        serde_json::json!({
            "mutation_id": id,
            "target_cell": target,
            "outcome": outcome,
            "failure_identity": failure_identity,
            "divergence_reached": execution_reached,
            "independent_l2_property_engaged": l2_property,
            "intended_l3_property_failed": l3_property_failed,
            "mismatch_class": expected_mismatch,
            "original": cell,
            "minimized": minimized,
            "witness_size_encoding": "compact UTF-8 JSON containing graph_text and six input-bit words",
            "original_witness_size_bytes": original_witness_size_bytes,
            "minimized_witness_size_bytes": minimized_witness_size_bytes,
            "shrink_work": shrink_work,
            "candidate_test_executable_blake3": executable_hash
        })
    });
    let receipt = serde_json::json!({
        "schema": "noble-differential-interactions-p2/v1",
        "profile": "P2-ngraph-f32",
        "matrix_sha256": matrix_sha256,
        "test_executable": executable,
        "test_executable_blake3": executable_hash,
        "selected": cells.len(),
        "generator_comparison": generator_comparison,
        "mutation_probe": mutation_probe,
        "reached": accepted,
        "unreached": cells.iter().filter(|cell| cell["reached"] != true).map(|cell| cell["cell"].clone()).collect::<Vec<_>>(),
        "accepted": cells.len(),
        "refused": 2,
        "refusal_controls": [
            { "variant": "Finite premise violated: safe input changed to NaN", "detected": invalid_refused,
                "comparison_legs_reached": ["L2 APFloat", "binary64-per-operation", "L3 reference", "L4 compiled"] },
            { "variant": "Integral premise violated: index input changed to NaN", "detected": invalid_index_refused,
                "earlier_refusal_rejected_by_shrinker": earlier_refusal_rejected_by_shrinker,
                "comparison_legs_reached": ["L2 APFloat", "binary64-per-operation", "L3 reference", "L4 compiled"] }
        ],
        "different_failure_control": different_failure_control,
        "exhausted": 2,
        "budget_controls": [
            { "fuel": 0, "l3": format!("{reference_quota:?}"), "l4": format!("{compiled_quota:?}"), "detected": exhausted,
                "comparison_legs_reached": ["L3 reference", "L4 compiled"] },
            { "fuel": 1, "l3": format!("{reference_boundary:?}"), "l4": format!("{compiled_boundary:?}"), "detected": exhausted_at_one,
                "comparison_legs_reached": ["L3 reference", "L4 compiled"] }
        ],
        "unsupported": 1,
        "unsupported_features": ["immutable parameter snapshots: this seeded path passes None to run_legs"],
        "oracle_weakness_control": { "fault": "flush subnormal result to zero", "fixture": ftz.name,
            "retained_expected_bits": ftz.expected[0], "weak_output_bits": weak_ftz.to_bits(),
            "independent_control_detected": independent_oracle_control },
        "all_refused_is_semantic_acceptance": false,
        "cells": cells
    });
    println!("ISSUE125_INTERACTION_RECEIPT={receipt}");
    assert_eq!(cells.len(), 24, "matrix-v1 P2 cell count");
    assert!(
        current_potential_node_work <= structured_node_budget,
        "current P2 generator exceeded the frozen node-work budget"
    );
    if mutation_id.is_none() {
        assert_eq!(
            accepted, 24,
            "all selected P2 cells must be accepted executions"
        );
        assert!(
            all_reached,
            "all-refused or unreached campaign cannot pass semantic acceptance"
        );
    } else {
        // An implementation mutant may change results outside its declared
        // target cell; mutation runs judge that target's retained witness,
        // while only the clean baseline is required to reach every cell.
        assert_eq!(
            mutation_probe
                .as_ref()
                .and_then(|probe| probe["outcome"].as_str()),
            Some("killed"),
            "semantic mutant must fail its selected property after reaching the divergence"
        );
    }
    assert!(
        invalid_refused,
        "one-premise guard refusal control must be observed by every leg"
    );
    assert!(
        invalid_index_refused,
        "one-premise index-fact refusal must be observed by every leg"
    );
    assert!(
        earlier_refusal_rejected_by_shrinker,
        "shrinker accepted an earlier input-fact refusal"
    );
    assert!(
        different_failure_rejected_by_shrinker,
        "shrinker accepted a different mismatch class for the retained failure"
    );
    assert!(
        exhausted && exhausted_at_one,
        "zero- and one-fuel controls must remain exhausted outcomes"
    );
    assert!(
        independent_oracle_control,
        "independent control must catch the deliberate FTZ oracle weakness"
    );
}

#[test]
#[ignore = "replay-only: consumes exact retained graph and input bytes written by the issue-125 runner"]
fn issue125_replay_retained_witness() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../verification/differential-interactions/replay.json");
    let retained: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("retained replay witness must exist"))
            .expect("retained witness JSON");
    let graph_text = retained["graph_text"].as_str().expect("graph bytes");
    let graph = noble_ngraph::format::graph::parse(&BLAKE3, graph_text)
        .unwrap_or_else(|diagnostic| panic!("retained graph refused: {diagnostic}"));
    let checked = check(&BLAKE3, graph).expect("retained graph checks");
    let graph_id = noble_ngraph::ir::hex(&checked.id());
    let executable = std::env::current_exe().expect("current test executable");
    let executable_hash =
        blake3::hash(&std::fs::read(&executable).expect("read current executable"))
            .to_hex()
            .to_string();
    assert_eq!(
        graph_id,
        retained["graph_id"].as_str().expect("graph identity")
    );
    assert_eq!(
        blake3::hash(graph_text.as_bytes()).to_hex().as_str(),
        retained["graph_text_blake3"]
            .as_str()
            .expect("graph byte digest")
    );
    assert!(retained["snapshot"].is_null(), "P2 matrix has no snapshot");
    let family = retained["interaction_family"]
        .as_str()
        .expect("interaction family");
    let fp_class = retained["fp_class"].as_str().expect("FP class");
    let expected = issue125_expected(family, fp_class);
    let expected_mismatch = retained["expected_mismatch_class"].as_str();
    let input_bits = |key: &str| -> [u32; 6] {
        let bits: Vec<u32> = retained[key]
            .as_array()
            .expect("retained input bits")
            .iter()
            .map(|value| {
                u32::try_from(value.as_u64().expect("u32 input bits")).expect("u32 input bits")
            })
            .collect();
        bits.try_into().expect("six retained F32 input bits")
    };
    let original_bits = input_bits("original_input_bits");
    let minimized_bits = input_bits("minimized_input_bits");
    assert_eq!(
        original_bits[3], minimized_bits[3],
        "shrinking changed the index input"
    );
    assert_eq!(
        original_bits[4], minimized_bits[4],
        "shrinking changed the select branch"
    );
    let original_values = original_bits.map(f32::from_bits);
    let minimized_values = minimized_bits.map(f32::from_bits);
    let original_branch =
        issue125_runtime_selected_branch(checked.graph(), family, &original_values);
    let minimized_branch =
        issue125_runtime_selected_branch(checked.graph(), family, &minimized_values);
    assert_eq!(
        original_branch, minimized_branch,
        "shrinking changed the selected branch"
    );
    assert_eq!(
        original_branch,
        retained["selected_branch"].as_str(),
        "retained branch witness changed"
    );
    let branch_slot = issue125_selected_branch_input_slot(checked.graph(), family);
    assert_eq!(
        branch_slot.map(u64::from),
        retained["selected_branch_input_slot"].as_u64(),
        "retained branch operand slot changed"
    );
    assert_eq!(
        branch_slot.map(|slot| original_values[slot as usize].to_bits()),
        retained["selected_branch_input_bits"]
            .as_u64()
            .map(|bits| bits as u32),
        "retained branch operand bits changed"
    );
    let original_index_slot = issue125_index_input_slot(checked.graph(), family, &original_values);
    let minimized_index_slot =
        issue125_index_input_slot(checked.graph(), family, &minimized_values);
    assert_eq!(
        original_index_slot, minimized_index_slot,
        "shrinking changed the index operand slot"
    );
    assert_eq!(
        original_index_slot.map(u64::from),
        retained["index_input_slot"].as_u64(),
        "retained index operand slot changed"
    );
    assert_eq!(
        original_index_slot.map(|slot| original_values[slot as usize].to_bits()),
        retained["index_input_bits"]
            .as_u64()
            .map(|bits| bits as u32),
        "retained index operand bits changed"
    );
    let original_index = issue125_runtime_table_index(checked.graph(), family, &original_values);
    let minimized_index = issue125_runtime_table_index(checked.graph(), family, &minimized_values);
    assert_eq!(
        original_index, minimized_index,
        "shrinking changed the selected table index"
    );
    assert_eq!(
        original_index.map(u64::from),
        retained["selected_table_index"].as_u64(),
        "retained table-index witness changed"
    );
    let original_table_node =
        issue125_selected_table_node(checked.graph(), family, &original_values);
    let minimized_table_node =
        issue125_selected_table_node(checked.graph(), family, &minimized_values);
    assert_eq!(
        original_table_node, minimized_table_node,
        "shrinking changed the table operand"
    );
    assert_eq!(
        original_table_node.map(|node| u64::from(node.0)),
        retained["selected_table_node"].as_u64(),
        "retained table operand changed"
    );
    assert_eq!(
        executable_hash,
        retained["candidate_test_executable_blake3"]
            .as_str()
            .expect("candidate binary identity"),
        "replay binary differs from the candidate receipt"
    );
    let historical_hash = retained["historical_test_executable_blake3"]
        .as_str()
        .expect("historical binary identity");
    assert_ne!(
        executable_hash, historical_hash,
        "historical baseline executable must be refused as evidence for changed bytes"
    );
    let replay_variant = |name: &str, key: &str| {
        let input_bits = input_bits(key);
        let inputs = input_bits.map(f32::from_bits);
        let legs = run_legs(
            &Compiled::new(&BLAKE3, &checked),
            &f32_inputs(&inputs),
            None,
        );
        let mismatch = issue125_mismatch_class(&legs);
        assert!(
            issue125_property(&legs.l2_apfloat, expected),
            "independent property no longer engaged for {name}"
        );
        assert!(
            !issue125_property(&legs.l3_reference, expected),
            "retained mutant no longer violates its intended property for {name}"
        );
        assert_eq!(
            mismatch, expected_mismatch,
            "retained failure identity changed for {name}"
        );
        assert!(
            !legs.agree(),
            "mutated retained failure unexpectedly agrees for {name}"
        );
        assert_eq!(
            legs.l3_evaluated_nodes,
            checked.node_count() as u64,
            "retained graph did not evaluate every node for {name}"
        );
        serde_json::json!({
            "variant": name,
            "input_bits": input_bits,
            "selected_branch": issue125_runtime_selected_branch(checked.graph(), family, &inputs),
            "selected_branch_input_slot": issue125_selected_branch_input_slot(checked.graph(), family),
            "selected_branch_input_bits": issue125_selected_branch_input_slot(checked.graph(), family)
                .map(|slot| inputs[slot as usize].to_bits()),
            "index_input_slot": issue125_index_input_slot(checked.graph(), family, &inputs),
            "index_input_bits": issue125_index_input_slot(checked.graph(), family, &inputs)
                .map(|slot| inputs[slot as usize].to_bits()),
            "selected_table_index": issue125_runtime_table_index(checked.graph(), family, &inputs),
            "selected_table_node": issue125_selected_table_node(checked.graph(), family, &inputs)
                .map(|node| node.0),
            "mismatch_class": mismatch,
            "failure_identity": retained["failure_identity"],
            "l2": format!("{:?}", legs.l2_apfloat),
            "l3": format!("{:?}", legs.l3_reference),
            "l4": format!("{:?}", legs.l4_compiled),
            "legs_agree": legs.agree(),
            "l3_evaluated_nodes": legs.l3_evaluated_nodes,
        })
    };
    let replayed = vec![
        replay_variant("original", "original_input_bits"),
        replay_variant("minimized", "minimized_input_bits"),
    ];
    let output = serde_json::json!({
        "replay_mode": retained["replay_mode"],
        "cell": retained["cell"],
        "graph_id": graph_id,
        "graph_text_blake3": blake3::hash(graph_text.as_bytes()).to_hex().to_string(),
        "replayed": replayed,
        "failure_identity": retained["failure_identity"],
        "candidate_test_executable_blake3": executable_hash,
        "historical_executable_refused": executable_hash != historical_hash,
        "corpus_regenerated": false,
        "fresh_process_replay": true
    });
    println!("ISSUE125_REPLAY={output}");
}
