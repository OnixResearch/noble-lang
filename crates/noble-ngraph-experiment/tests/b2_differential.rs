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
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! B2: L2 (APFloat), B2-ORACLE (binary64 per op), L3 and L4 agree under
//! `≡obs` on the boundary vectors, F1–F3 and seeded random graphs. The full
//! ≥10,000-vector campaign runs in the `ngraph-phase-b` harness (release).

use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{f32_inputs, Control};
use noble_ngraph::ir::{Fact, NodeId};
use noble_ngraph::prng::SplitMix64;
use noble_ngraph::snapshot::ParamSnapshot;
use noble_ngraph::{check, Obs, Value};
use noble_ngraph_experiment::differential::{campaign, run_legs};
use noble_ngraph_experiment::fixtures::{self, F1_FIXED, RULES};
use noble_ngraph_experiment::oracle::{ap_add, ap_mul, ap_sub, f64_op};
use noble_ngraph_experiment::random::random_f32;
use noble_ngraph_experiment::scheme::BLAKE3;

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
    use noble_ngraph_experiment::shrink::{minimize, replays, Budget};

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
    let witness = minimize(2, &authority, &candidate, start, Budget::default_attempts())
        .expect("the start pair disagrees under flush-to-zero");
    assert!(replays(&witness, &authority, &candidate), "{witness:?}");
    // Deterministic: a second walk over the same start pair yields the same witness.
    let again = minimize(2, &authority, &candidate, start, Budget::default_attempts())
        .expect("the start pair still disagrees");
    assert_eq!(again, witness);
    // The walk terminates inside the documented budget rather than exhausting it,
    // and the witness is a subnormal-versus-zero disagreement: the authority keeps
    // the subnormal product and the candidate flushes it.
    assert!(!witness.budget_exhausted, "{witness:?}");
    assert!(witness.attempts <= Budget::default_attempts().attempts);
    let expected_exponent = witness.expected >> 23 & 0xFF;
    assert_eq!(expected_exponent, 0, "{witness:?}");
    assert_ne!(witness.expected & 0x7F_FFFF, 0, "{witness:?}");
    assert_eq!(witness.candidate, 0x0000_0000, "{witness:?}");
}

#[test]
fn agreeing_pairs_produce_no_witness() {
    use noble_ngraph_experiment::shrink::{minimize, Budget};
    let authority =
        |operation: u8, left: u32, right: u32| -> u32 { operation as u32 + left + right };
    let candidate = authority;
    assert!(minimize(
        0,
        &authority,
        &candidate,
        (0x3f80_0000, 0x3f80_0000),
        Budget::default_attempts()
    )
    .is_none());
}
