//! Fixed #32 partial-static offset comparison: D3's baseline is always the control.
//! Timing is descriptive; semantic equality and growth refusals are asserted.

use std::time::Instant;

use noble_ngraph::check::{self, Budgets, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{GraphBuilder, Op, ScalarType, ShapeTree, SubProfile, Subject};
use noble_ngraph::sem::Value;
use noble_ngraph::specialize::{self, offset, Request};

use super::common::IDS;

#[derive(Clone, Copy)]
enum Fixture {
    Add,
    Sub,
    Wrap,
    Shared,
    Nonprefix,
    Dead,
    Growth,
}

impl Fixture {
    const fn name(self) -> &'static str {
        match self {
            Self::Add => "chained-add",
            Self::Sub => "chained-sub",
            Self::Wrap => "wrapping-boundary",
            Self::Shared => "shared-subexpression",
            Self::Nonprefix => "non-prefix-static",
            Self::Dead => "dead-total-forward",
            Self::Growth => "growth-cap",
        }
    }
}

fn graph(kind: Fixture) -> (CheckedGraph, Vec<Option<i64>>) {
    let mut builder = GraphBuilder::new(SubProfile::I64, Subject::authored(&IDS, kind.name()));
    let x = builder.input(ScalarType::I64);
    let mut map = vec![None];
    let output = match kind {
        Fixture::Add | Fixture::Sub | Fixture::Wrap | Fixture::Shared | Fixture::Dead => {
            let first_constant = builder.i64_const(match kind {
                Fixture::Wrap => i64::MAX,
                Fixture::Dead => 0,
                _ => 2,
            });
            let first = builder.op(if matches!(kind, Fixture::Sub) {
                Op::I64Sub(x, first_constant)
            } else {
                Op::I64Add(x, first_constant)
            });
            if matches!(kind, Fixture::Shared) {
                let twice = builder.op(Op::I64Add(first, first));
                builder.output(first);
                twice
            } else {
                let second_constant =
                    builder.i64_const(if matches!(kind, Fixture::Wrap) { 1 } else { 3 });
                builder.op(if matches!(kind, Fixture::Sub) {
                    Op::I64Sub(first, second_constant)
                } else {
                    Op::I64Add(first, second_constant)
                })
            }
        }
        Fixture::Nonprefix => {
            let y = builder.input(ScalarType::I64);
            let z = builder.input(ScalarType::I64);
            map = vec![None, Some(7), None];
            let sum = builder.op(Op::I64Add(x, y));
            let sum = builder.op(Op::I64Add(sum, z));
            let two = builder.i64_const(2);
            let sum = builder.op(Op::I64Add(sum, two));
            let three = builder.i64_const(3);
            builder.op(Op::I64Add(sum, three))
        }
        Fixture::Growth => {
            let y = builder.input(ScalarType::I64);
            map = vec![None, None];
            let two = builder.i64_const(2);
            let inner = builder.op(Op::I64Add(x, y));
            builder.op(Op::I64Sub(two, inner))
        }
    };
    builder.output(output);
    (
        check::check(&IDS, builder.finish()).expect("fixed graph accepted"),
        map,
    )
}

fn vector(seed: u64, count: usize) -> Vec<i64> {
    let mut state = seed;
    (0..count)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as i64
        })
        .collect()
}

fn vectors(map: &[Option<i64>]) -> Vec<Vec<i64>> {
    let count = map.iter().filter(|value| value.is_none()).count();
    let mut rows = vec![0, 1, -1, i64::MAX, i64::MIN]
        .into_iter()
        .map(|value| vec![value; count])
        .collect::<Vec<_>>();
    rows.extend((0..64).map(|index| vector(0x32e0_f15e_u64 + index, count)));
    rows
}

fn observed(checked: &CheckedGraph, values: &[Value]) -> (Vec<Value>, u64) {
    let report = eval::reference(checked, values, None, &mut Control::default());
    match report.obs {
        Obs::Normal(outputs) => (outputs, report.evaluated_nodes),
        other => panic!("accepted total graph must evaluate normally: {other:?}"),
    }
}

fn compiled(compiled: &Compiled, values: &[Value]) -> (Vec<Value>, u64) {
    let report = compiled.evaluate(values, None, &mut Control::default());
    match report.obs {
        Obs::Normal(outputs) => (outputs, report.evaluated_nodes),
        other => panic!("accepted total compiled graph must evaluate normally: {other:?}"),
    }
}

fn compare(kind: Fixture) -> (u64, u64, usize, usize) {
    let (original, map) = graph(kind);
    let budgets = Budgets::default();
    let comparison = offset::specialize(
        &IDS,
        &original,
        &Request {
            static_inputs: &map,
            budgets: &budgets,
        },
    )
    .expect("bounded offset comparison");
    let baseline = &comparison.baseline.checked;
    let normalized = &comparison.normalized;
    let baseline_compiled = Compiled::new(&IDS, baseline);
    let normalized_compiled = Compiled::new(&IDS, normalized);
    assert_ne!(baseline.id(), normalized.id(), "new residual identity");
    assert_eq!(
        baseline.graph().slots,
        normalized.graph().slots,
        "ordered dynamic input slots"
    );
    let source = specialize::emit_source(normalized).expect("emit new Program source");
    let readmitted = specialize::readmit(
        &IDS,
        &source,
        &specialize::interface_types(normalized).expect("interface"),
        noble_contracts::Limits::default(),
        &budgets,
    )
    .expect("ordinary independent admission");
    let mut work_baseline = 0;
    let mut work_normalized = 0;
    for dynamic in vectors(&map) {
        let inputs = dynamic.iter().copied().map(Value::I64).collect::<Vec<_>>();
        let original_inputs = comparison.baseline.merge(&inputs).expect("ordered merge");
        let expected = observed(&original, &original_inputs).0;
        let (baseline_result, baseline_nodes) = observed(baseline, &inputs);
        let (offset_result, offset_nodes) = observed(normalized, &inputs);
        assert_eq!(baseline_result, expected, "{} baseline", kind.name());
        assert_eq!(offset_result, expected, "{} offset", kind.name());
        assert_eq!(
            compiled(&baseline_compiled, &inputs).0,
            expected,
            "{} baseline L4",
            kind.name()
        );
        assert_eq!(
            compiled(&normalized_compiled, &inputs).0,
            expected,
            "{} offset L4",
            kind.name()
        );
        assert_eq!(
            observed(&readmitted, &inputs).0,
            expected,
            "{} emitted Program",
            kind.name()
        );
        work_baseline += baseline_nodes;
        work_normalized += offset_nodes;
    }
    (
        work_baseline,
        work_normalized,
        baseline.node_count(),
        normalized.node_count(),
    )
}

#[test]
fn offset_normalization_preserves_wrapping_order_and_shared_bindings() {
    for kind in [
        Fixture::Add,
        Fixture::Sub,
        Fixture::Wrap,
        Fixture::Shared,
        Fixture::Nonprefix,
        Fixture::Dead,
        Fixture::Growth,
    ] {
        let _ = compare(kind);
    }
    let (base_work, offset_work, base_nodes, offset_nodes) = compare(Fixture::Add);
    assert!(
        offset_work < base_work && offset_nodes < base_nodes,
        "(x+2)+3 becomes x+5"
    );
    let (shared, map) = graph(Fixture::Shared);
    let result = offset::specialize(
        &IDS,
        &shared,
        &Request {
            static_inputs: &map,
            budgets: &Budgets::default(),
        },
    )
    .expect("shared output");
    let emitted = specialize::emit_source(&result.normalized).expect("lexical sharing");
    assert!(
        emitted.contains("let v"),
        "the reused subexpression remains a lexical binding"
    );
    let inputs = [Value::I64(3)];
    let actual = observed(&result.normalized, &inputs).0;
    assert_eq!(
        actual,
        vec![Value::I64(5), Value::I64(10)],
        "the same shared node occurs twice"
    );
}

#[test]
fn offset_growth_cap_refuses_without_publishing_and_baseline_is_unchanged() {
    let (original, map) = graph(Fixture::Growth);
    let base = specialize::specialize(
        &IDS,
        &original,
        &Request {
            static_inputs: &map,
            budgets: &Budgets::default(),
        },
    )
    .expect("D3 baseline");
    let limit = base.checked.node_count() as u64;
    let mut budgets = Budgets {
        residual_nodes: limit,
        ..Budgets::default()
    };
    let refused = offset::specialize(
        &IDS,
        &original,
        &Request {
            static_inputs: &map,
            budgets: &budgets,
        },
    )
    .expect_err("an extra generated node charges the cap");
    assert_eq!(refused.code_string(), "E-BUDGET-RESIDUAL-NODES");
    budgets.residual_nodes = limit + 2;
    let accepted = offset::specialize(
        &IDS,
        &original,
        &Request {
            static_inputs: &map,
            budgets: &budgets,
        },
    )
    .expect("exact generated-node cap");
    assert_eq!(accepted.generated_nodes, limit + 2);
    assert_eq!(
        accepted.baseline.checked.id(),
        base.checked.id(),
        "D3 baseline is unchanged"
    );
    budgets.residual_nodes = limit + 1;
    assert_eq!(
        offset::specialize(
            &IDS,
            &original,
            &Request {
                static_inputs: &map,
                budgets: &budgets
            }
        )
        .expect_err("one below generated count")
        .code_string(),
        "E-BUDGET-RESIDUAL-NODES"
    );
    budgets.residual_nodes = limit - 1;
    assert_eq!(
        offset::specialize(
            &IDS,
            &original,
            &Request {
                static_inputs: &map,
                budgets: &budgets
            }
        )
        .expect_err("below baseline cap")
        .code_string(),
        "E-BUDGET-RESIDUAL-NODES"
    );
}

#[test]
fn offset_refuses_nested_partially_known_pairs_and_preserves_dynamic_bool_slots() {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&IDS, "offset-nested-pair"),
    );
    let x = builder.input(ScalarType::I64);
    let _y = builder.input(ScalarType::I64);
    builder.output(x);
    builder.shapes(
        Some(vec![ShapeTree::pair(
            ShapeTree::pair(ShapeTree::SCALAR, ShapeTree::UNIT).expect("bounded nested pair"),
            ShapeTree::SCALAR,
        )
        .expect("bounded nested pair")]),
        None,
    );
    let pair = check::check(&IDS, builder.finish()).expect("shape-bearing checked graph");
    let map = [Some(2), None];
    assert_eq!(
        offset::specialize(
            &IDS,
            &pair,
            &Request {
                static_inputs: &map,
                budgets: &Budgets::default()
            }
        )
        .expect_err("authored shape excluded")
        .code_string(),
        "E-AGGREGATE"
    );
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&IDS, "offset-dynamic-bool"),
    );
    let flag = builder.input(ScalarType::Bool);
    let number = builder.input(ScalarType::I64);
    let two = builder.i64_const(2);
    let three = builder.i64_const(3);
    let sum = builder.op(Op::I64Add(number, two));
    let sum = builder.op(Op::I64Add(sum, three));
    builder.output(flag);
    builder.output(sum);
    let graph = check::check(&IDS, builder.finish()).expect("mixed scalar slots");
    let result = offset::specialize(
        &IDS,
        &graph,
        &Request {
            static_inputs: &[None, None],
            budgets: &Budgets::default(),
        },
    )
    .expect("dynamic Bool remains ordered");
    assert_eq!(
        observed(&result.normalized, &[Value::Bool(true), Value::I64(4)]).0,
        vec![Value::Bool(true), Value::I64(9)]
    );
}

#[expect(
    tigerstyle::ambient_clock,
    reason = "Owner: noble-maintainers; the ignored, frozen #32 cost lane times only the explicitly named specialization, admission and replay scopes."
)]
fn clock() -> Instant {
    Instant::now()
}

#[test]
#[ignore = "descriptive #32 measurement; requires clean committed source and an external receipt destination"]
#[expect(
    tigerstyle::ambient_env,
    reason = "Owner: noble-maintainers; this ignored cost producer reads source identity and destination only at its evidence boundary."
)]
fn report_offset_costs() {
    let source_commit = std::env::var("NOBLE_SOURCE_COMMIT").expect("source commit");
    let source_tree = std::env::var("NOBLE_SOURCE_TREE").expect("source tree");
    let source_clean = std::env::var("NOBLE_SOURCE_CLEAN").expect("clean source assertion");
    assert_eq!(source_clean, "true");
    let output = std::env::var("NOBLE_PE_OFFSET_OUT").expect("external receipt destination");
    let mut records = Vec::new();
    for kind in [
        Fixture::Add,
        Fixture::Sub,
        Fixture::Wrap,
        Fixture::Shared,
        Fixture::Nonprefix,
        Fixture::Dead,
        Fixture::Growth,
    ] {
        let (original, map) = graph(kind);
        let budgets = Budgets::default();
        let request = Request {
            static_inputs: &map,
            budgets: &budgets,
        };
        let start = clock();
        let baseline = specialize::specialize(&IDS, &original, &request).expect("baseline");
        let baseline_specialization_ns = start.elapsed().as_nanos();
        let start = clock();
        let comparison = offset::specialize(&IDS, &original, &request).expect("offset");
        let offset_specialization_ns = start.elapsed().as_nanos();
        assert_eq!(baseline.checked.id(), comparison.baseline.checked.id());
        let mut emit_ns = [0u128; 2];
        let mut replay_first_ns = [0u128; 2];
        let mut replay_warm_ns = [0u128; 2];
        let mut l3_visits = [0u64; 2];
        let mut l4_visits = [0u64; 2];
        let mut graph_bytes = [0u64; 2];
        let mut compiled_bytes = [0u64; 2];
        let mut source_bytes = [0usize; 2];
        for (side, checked) in [&baseline.checked, &comparison.normalized]
            .into_iter()
            .enumerate()
        {
            graph_bytes[side] = check::retained_bytes(checked.graph());
            let packed = Compiled::new(&IDS, checked);
            compiled_bytes[side] = packed.retained_bytes();
            let start = clock();
            let source = specialize::emit_source(checked).expect("source");
            source_bytes[side] = source.len();
            let rechecked = specialize::readmit(
                &IDS,
                &source,
                &specialize::interface_types(checked).expect("types"),
                noble_contracts::Limits::default(),
                &budgets,
            )
            .expect("ordinary admission");
            emit_ns[side] = start.elapsed().as_nanos();
            let inputs = vectors(&map);
            let first = inputs[0]
                .iter()
                .copied()
                .map(Value::I64)
                .collect::<Vec<_>>();
            let start = clock();
            let first_result = compiled(&packed, &first);
            replay_first_ns[side] = start.elapsed().as_nanos();
            assert_eq!(first_result.0, observed(&rechecked, &first).0);
            for row in inputs {
                let dynamic = row.iter().copied().map(Value::I64).collect::<Vec<_>>();
                let merged = comparison.baseline.merge(&dynamic).expect("ordered merge");
                let expected = observed(&original, &merged).0;
                let (l3, visits) = observed(checked, &dynamic);
                let start = clock();
                let (l4, packed_visits) = compiled(&packed, &dynamic);
                replay_warm_ns[side] += start.elapsed().as_nanos();
                assert_eq!(l3, expected, "{} L3", kind.name());
                assert_eq!(l4, expected, "{} L4", kind.name());
                assert_eq!(
                    observed(&rechecked, &dynamic).0,
                    expected,
                    "{} readmission",
                    kind.name()
                );
                l3_visits[side] += visits;
                l4_visits[side] += packed_visits;
            }
        }
        records.push(format!(
            "    {{\"fixture\":\"{}\",\"vectors\":69,\"baseline_nodes\":{},\"offset_nodes\":{},\"baseline_l3_visits\":{},\"offset_l3_visits\":{},\"baseline_l4_visits\":{},\"offset_l4_visits\":{},\"baseline_specialize_ns\":{},\"offset_specialize_total_ns\":{},\"baseline_emit_readmit_ns\":{},\"offset_emit_readmit_ns\":{},\"baseline_first_replay_ns\":{},\"offset_first_replay_ns\":{},\"baseline_warm_replay_ns\":{},\"offset_warm_replay_ns\":{},\"baseline_graph_bytes\":{},\"offset_graph_bytes\":{},\"baseline_compiled_bytes\":{},\"offset_compiled_bytes\":{},\"baseline_source_bytes\":{},\"offset_source_bytes\":{}}}",
            kind.name(), baseline.checked.node_count(), comparison.normalized.node_count(),
            l3_visits[0], l3_visits[1], l4_visits[0], l4_visits[1], baseline_specialization_ns,
            offset_specialization_ns, emit_ns[0], emit_ns[1], replay_first_ns[0], replay_first_ns[1],
            replay_warm_ns[0], replay_warm_ns[1], graph_bytes[0], graph_bytes[1], compiled_bytes[0],
            compiled_bytes[1], source_bytes[0], source_bytes[1]));
    }
    let report = format!(
        "{{\n  \"schema\":\"noble-pe-offset-comparison/v1\",\n  \"issue\":32,\n  \"protocol_commit\":\"fbd67e7\",\n  \"source_commit\":\"{source_commit}\",\n  \"source_tree\":\"{source_tree}\",\n  \"clean_source\":true,\n  \"timings\":\"single shared-host debug samples, descriptive not matched-budget speedup\",\n  \"identity\":\"test-siphash13x4\",\n  \"rows\":[\n{}\n  ]\n}}\n",
        records.join(",\n"));
    std::fs::write(output, report).expect("write receipt outside source tree");
}
