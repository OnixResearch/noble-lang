#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::assertion_density,
    tigerstyle::ambient_env,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Phase B B2 (default tier): L3 ≡obs L4 over extracted `I64` graphs —
//! the design's named vectors and seeded random Core programs.
//!
//! The full campaign (L1 real CLI `--opt off|on`, L2 Node BigInt oracle,
//! ≥10,000 vectors per graph) is the `ngraph-phase-b-extract` harness; its
//! receipt is `verification/ngraph/phase-b-extract.json`. The ignored test below
//! runs L2 when `NOBLE_NODE` names a Node installation:
//! `NOBLE_NODE=… cargo test -p noble-ngraph-experiment --test extract_b2 -- --ignored`.

use noble_ngraph::prng::SplitMix64;
use noble_ngraph::sem::{obs_eq_all, Value};
use noble_ngraph_experiment::b2i64::{random, Lowered, Oracle};

/// The L1 adapter's attribution and completion checks against fixture children (issue #75).
/// The module is test-only; `cfg(test)` states that scope, as Octet's test-context
/// rules read it from the attribute rather than from the harness-expanded `#[test]`.
#[cfg(test)]
#[path = "attribution/mod.rs"]
mod attribution;

fn i64s(v: &[i64]) -> Vec<Value> {
    v.iter().map(|x| Value::I64(*x)).collect()
}

#[test]
fn design_vectors_agree_on_l3_and_l4() {
    type Case = (
        &'static [&'static str],
        &'static str,
        usize,
        &'static [i64],
        &'static [i64],
    );
    let cases: &[Case] = &[
        (&[], "-", 2, &[5, 3], &[2]),
        (&[], "5 3 -", 0, &[], &[2]),
        (&[], "+", 2, &[i64::MAX, 1], &[i64::MIN]),
        (&[], "9223372036854775807 1 +", 0, &[], &[i64::MIN]),
        (&[], "*", 2, &[i64::MIN, -1], &[i64::MIN]),
        (&[], "-9223372036854775808 -1 *", 0, &[], &[i64::MIN]),
        (&[], "swap [ * ] dip + dup", 3, &[2, 3, 5], &[13, 13]),
    ];
    for (defs, root, inputs, v, expected) in cases {
        let defs: Vec<String> = defs.iter().map(|d| (*d).to_owned()).collect();
        let x = Lowered::new(&defs, root, *inputs).expect("extracts");
        assert_eq!(x.l3(&i64s(v)).expect("L3"), i64s(expected), "{root}");
        assert_eq!(x.l4(&i64s(v)).expect("L4"), i64s(expected), "{root}");
    }
}

#[test]
fn seeded_random_programs_extract_and_agree_on_l3_and_l4() {
    let seed = 0x1404_b201;
    let mut rng = SplitMix64::new(seed);
    for i in 0..48 {
        let p = random::program(seed, i);
        let x = Lowered::new(&p.definitions, &p.root, p.inputs)
            .unwrap_or_else(|e| panic!("program {i} {p:?}: {e}"));
        for _ in 0..200 {
            let v: Vec<Value> = (0..p.inputs)
                .map(|_| Value::I64(random::value(&mut rng)))
                .collect();
            let (a, b) = (x.l3(&v).expect("L3"), x.l4(&v).expect("L4"));
            assert!(
                obs_eq_all(&a, &b),
                "program {i} {p:?} on {v:?}: {a:?} vs {b:?}"
            );
        }
    }
}

#[test]
#[ignore = "needs NOBLE_NODE (pinned Node) for the L2 BigInt oracle"]
fn l2_node_bigint_oracle_agrees_with_l3() {
    let node = std::env::var("NOBLE_NODE").expect("NOBLE_NODE");
    let oracle = Oracle {
        node: format!("{node}/bin/node").into(),
    };
    let seed = 0x1404_b202;
    let mut rng = SplitMix64::new(seed);
    for i in 0..8 {
        let p = random::program(seed, i);
        let x = Lowered::new(&p.definitions, &p.root, p.inputs).expect("extracts");
        let vectors: Vec<Vec<Value>> = (0..1000)
            .map(|_| {
                (0..p.inputs)
                    .map(|_| Value::I64(random::value(&mut rng)))
                    .collect()
            })
            .collect();
        let l2 = oracle
            .evaluate(x.extraction.checked.graph(), &vectors)
            .expect("oracle");
        for (v, cells) in vectors.iter().zip(&l2) {
            let l3: Vec<String> = x
                .l3(v)
                .expect("L3")
                .into_iter()
                .map(noble_ngraph_experiment::b2i64::cell)
                .collect();
            assert_eq!(&l3, cells, "program {i} on {v:?}");
        }
    }
}
