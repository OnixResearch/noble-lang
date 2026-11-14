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
//! Phase B B2 (default tier): L3 ≡obs L4 ≡obs L5 ≡obs L6 over extracted `I64` graphs —
//! the design's named vectors and seeded random Core programs.
//!
//! The full campaign (L1 real CLI `--opt off|on`, L2 Node BigInt oracle,
//! L5 independently checked belt, L6 bounded revision-3 net, ≥10,000 vectors per graph) is the
//! `ngraph-phase-b-extract` harness; its
//! receipt is `verification/ngraph/phase-b-extract.json`. The ignored test below
//! runs L2 against L3/L4/L5/L6 when `NOBLE_NODE` names a Node installation:
//! `NOBLE_NODE=… cargo test -p noble-ngraph-experiment --test extract_b2 -- --ignored`.

use noble_ngraph::prng::SplitMix64;
use noble_ngraph::sem::{obs_eq_all, Value};
use noble_ngraph_experiment::b2i64::{random, Lowered, Oracle};
use noble_ngraph_experiment::scheme::BLAKE3;

/// The L1 adapter's attribution and completion checks against fixture children (issue #75).
/// The module is test-only; `cfg(test)` states that scope, as Octet's test-context
/// rules read it from the attribute rather than from the harness-expanded `#[test]`.
#[cfg(test)]
#[path = "attribution/mod.rs"]
mod attribution;

/// Additional revision-3 boundary-shape and source-rebinding fixtures in this
/// existing test target (not a separate Cargo integration-test target).
#[cfg(test)]
#[path = "extract_b2/net.rs"]
mod graph_shapes;

/// Rust-produced input nets and results, serialized into kernel-checked Lean
/// theorems in the existing B2 integration-test target.
#[cfg(test)]
#[path = "extract_b2/net_i64_corpus.rs"]
mod proof;

fn i64s(v: &[i64]) -> Vec<Value> {
    v.iter().map(|x| Value::I64(*x)).collect()
}

#[test]
fn design_vectors_agree_on_l3_l4_l5_and_l6() {
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
        let inputs = i64s(v);
        let checked = &x.extraction.checked;
        let belt = noble_ngraph::belt::lower(&BLAKE3, checked, 8, 8)
            .expect("the extracted I64 graph lowers to a belt");
        let accepted = noble_ngraph::belt::check(&BLAKE3, checked, &belt.bytes)
            .expect("the independently checked belt matches the extracted graph");
        let l3 = x.l3(&inputs).expect("L3");
        let l4 = x.l4(&inputs).expect("L4");
        let l5 = noble_ngraph::belt::execute_checked(&BLAKE3, checked, accepted, &inputs)
            .expect("L5 belt");
        let l6 = x.l6(&inputs).expect("L6 net");
        assert_eq!(l3, i64s(expected), "{root}");
        assert_eq!(l4, i64s(expected), "{root}");
        assert_eq!(l5, i64s(expected), "{root}");
        assert_eq!(
            l6.obs,
            noble_ngraph::net::Obs::Normal(i64s(expected)),
            "{root}"
        );
    }
}

#[test]
fn seeded_random_programs_extract_and_agree_on_l3_l4_l5_and_l6() {
    let seed = 0x1404_b201;
    let mut rng = SplitMix64::new(seed);
    for i in 0..48 {
        let p = random::program(seed, i);
        let x = Lowered::new(&p.definitions, &p.root, p.inputs)
            .unwrap_or_else(|e| panic!("program {i} {p:?}: {e}"));
        let checked = &x.extraction.checked;
        let belt = noble_ngraph::belt::lower(&BLAKE3, checked, 8, 8)
            .unwrap_or_else(|e| panic!("program {i} {p:?} belt lowering: {e:?}"));
        let accepted = noble_ngraph::belt::check(&BLAKE3, checked, &belt.bytes)
            .unwrap_or_else(|e| panic!("program {i} {p:?} belt check: {e:?}"));
        for _ in 0..200 {
            let v: Vec<Value> = (0..p.inputs)
                .map(|_| Value::I64(random::value(&mut rng)))
                .collect();
            let (a, b) = (x.l3(&v).expect("L3"), x.l4(&v).expect("L4"));
            let c = noble_ngraph::belt::execute_checked(&BLAKE3, checked, accepted, &v)
                .unwrap_or_else(|e| panic!("program {i} {p:?} belt on {v:?}: {e:?}"));
            assert!(
                obs_eq_all(&a, &b),
                "program {i} {p:?} on {v:?}: {a:?} vs {b:?}"
            );
            assert!(
                obs_eq_all(&a, &c),
                "program {i} {p:?} on {v:?}: L3 {a:?} vs L5 belt {c:?}"
            );
            let net = x.l6(&v).expect("L6");
            assert!(
                matches!(&net.obs, noble_ngraph::net::Obs::Normal(values) if obs_eq_all(&a, values)),
                "program {i} {p:?} on {v:?}: L3 {a:?} vs L6 net {net:?}"
            );
        }
    }
}

#[test]
fn net_equality_captured_recipes_and_rebound_definitions_are_exact() {
    let equality = Lowered::new(&[], "=", 2).expect("equality extracts");
    for (row, expected) in [
        (i64s(&[i64::MIN, i64::MAX]), false),
        (i64s(&[i64::MAX, i64::MAX]), true),
    ] {
        let reference = equality.l3(&row).expect("L3");
        let compiled = equality.l4(&row).expect("L4");
        let net = equality.l6(&row).expect("L6");
        assert_eq!(reference, vec![Value::Bool(expected)]);
        assert_eq!(compiled, reference);
        assert_eq!(net.obs, noble_ngraph::net::Obs::Normal(reference));
    }

    let captured = Lowered::new(&["def sq [ dup * ]".to_owned()], "sq swap sq +", 2)
        .expect("captured recipe extracts");
    let captured_row = i64s(&[3, 4]);
    assert_eq!(
        captured.l6(&captured_row).expect("captured L6").obs,
        noble_ngraph::net::Obs::Normal(i64s(&[25]))
    );

    let old =
        Lowered::new(&["def step [ 1 + ]".to_owned()], "step", 1).expect("old definition extracts");
    let rebound = Lowered::new(
        &["def step [ 1 + ]".to_owned(), "def step [ 9 + ]".to_owned()],
        "step",
        1,
    )
    .expect("rebound definition extracts");
    assert_ne!(old.extraction.checked.id(), rebound.extraction.checked.id());
    let row = i64s(&[3]);
    assert_eq!(
        old.l6(&row).expect("saved old net").obs,
        noble_ngraph::net::Obs::Normal(i64s(&[4]))
    );
    assert_eq!(
        rebound.l6(&row).expect("new net").obs,
        noble_ngraph::net::Obs::Normal(i64s(&[12]))
    );
}

#[test]
#[ignore = "needs NOBLE_NODE (pinned Node) for the L2 BigInt oracle"]
fn l2_node_bigint_oracle_agrees_with_l3() {
    let node = std::env::var("NOBLE_NODE").expect("NOBLE_NODE");
    let oracle = Oracle {
        node: format!("{node}/bin/node").into(),
    };
    check_random_node_programs(&oracle);
    check_static_examples(&oracle);
}

fn check_random_node_programs(oracle: &Oracle) {
    let seed = 0x1404_b202;
    let mut rng = SplitMix64::new(seed);
    for i in 0..8 {
        let p = random::program(seed, i);
        let x = Lowered::new(&p.definitions, &p.root, p.inputs).expect("extracts");
        let checked = &x.extraction.checked;
        let belt = noble_ngraph::belt::lower(&BLAKE3, checked, 8, 8)
            .expect("the extracted graph lowers to a belt");
        let accepted = noble_ngraph::belt::check(&BLAKE3, checked, &belt.bytes)
            .expect("the belt checks against the extracted graph");
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
            let l5: Vec<String> =
                noble_ngraph::belt::execute_checked(&BLAKE3, checked, accepted, v)
                    .expect("the checked belt runs")
                    .into_iter()
                    .map(noble_ngraph_experiment::b2i64::cell)
                    .collect();
            assert_eq!(&l5, cells, "belt program {i} on {v:?}");
            let l6 = x.l6(v).expect("L6");
            let l6_cells: Vec<String> = match l6.obs {
                noble_ngraph::net::Obs::Normal(values) => values
                    .into_iter()
                    .map(noble_ngraph_experiment::b2i64::cell)
                    .collect(),
                other => panic!("net normalisation failed for random {i}: {other:?}"),
            };
            assert_eq!(&l6_cells, cells, "net on random {i} {v:?}");
        }
    }
}

fn check_static_examples(oracle: &Oracle) {
    type DifferentialFixture<'a> = (&'a [&'a str], &'a str, usize, &'a [&'a [i64]]);
    let fixtures: &[DifferentialFixture<'_>] = &[
        (&[], "=", 2, &[&[i64::MIN, i64::MAX], &[i64::MAX, i64::MAX]]),
        (&[], "drop", 1, &[&[91], &[i64::MIN]]),
        (
            &[],
            "swap [ * ] dip + dup",
            3,
            &[&[2, 3, 5], &[i64::MIN, 7, -1]],
        ),
        (
            &["def sq [ dup * ]"],
            "sq swap sq +",
            2,
            &[&[3, 4], &[i64::MIN, 1]],
        ),
    ];
    for (index, (definitions, root, arity, rows)) in fixtures.iter().enumerate() {
        let definitions: Vec<String> = definitions.iter().map(|item| (*item).to_owned()).collect();
        let lowered = Lowered::new(&definitions, root, *arity).expect("fixture extracts");
        let vectors: Vec<Vec<Value>> = rows.iter().map(|row| i64s(row)).collect();
        let oracle_values = oracle
            .evaluate(lowered.extraction.checked.graph(), &vectors)
            .expect("fixture BigInt oracle");
        for (row, expected) in vectors.iter().zip(oracle_values.iter()) {
            let reference = lowered.l3(row).expect("fixture L3");
            let compiled = lowered.l4(row).expect("fixture L4");
            let net = lowered.l6(row).expect("fixture L6");
            let net_values: Vec<String> = match net.obs {
                noble_ngraph::net::Obs::Normal(values) => values
                    .into_iter()
                    .map(noble_ngraph_experiment::b2i64::cell)
                    .collect(),
                other => panic!("fixture net refusal {other:?}"),
            };
            let reference_cells: Vec<String> = reference
                .iter()
                .map(|value| noble_ngraph_experiment::b2i64::cell(*value))
                .collect();
            assert_eq!(reference, compiled, "fixture {index} L3/L4 {row:?}");
            assert_eq!(&reference_cells, expected, "fixture {index} L2 {row:?}");
            assert_eq!(&net_values, expected, "fixture {index} L5 {row:?}");
        }
    }
}
