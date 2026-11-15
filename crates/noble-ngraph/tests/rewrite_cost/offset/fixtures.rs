//! Frozen input families and finite semantic comparison for the #32 experiment.

use noble_ngraph::check::{self, Budgets, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;
use noble_ngraph::specialize::{self, offset, Request};

use super::super::common::IDS;

#[derive(Clone, Copy)]
pub(super) enum Fixture {
    Add,
    Sub,
    Wrap,
    Shared,
    Nonprefix,
    Dead,
    Growth,
}

impl Fixture {
    #[expect(
        tigerstyle::fragile_exhaustive_enum_match,
        reason = "Owner: noble-maintainers; the seven frozen fixture names deliberately force review if this experiment adds a family."
    )]
    pub(super) const fn name(self) -> &'static str {
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

#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; the frozen seven-case match must force review if a new case is added."
)]
pub(super) fn graph(kind: Fixture) -> (CheckedGraph, Vec<Option<i64>>) {
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

pub(super) fn vectors(map: &[Option<i64>]) -> Vec<Vec<i64>> {
    let count = map.iter().filter(|value| value.is_none()).count();
    let mut rows = vec![0, 1, -1, i64::MAX, i64::MIN]
        .into_iter()
        .map(|value| vec![value; count])
        .collect::<Vec<_>>();
    rows.extend((0..64).map(|index| vector(0x32e0_f15e_u64 + index, count)));
    rows
}

pub(super) fn observed(checked: &CheckedGraph, values: &[Value]) -> (Vec<Value>, u64) {
    let report = eval::reference(checked, values, None, &mut Control::default());
    match report.obs {
        Obs::Normal(outputs) => (outputs, report.evaluated_nodes),
        other => panic!("accepted total graph must evaluate normally: {other:?}"),
    }
}

pub(super) fn compiled(compiled: &Compiled, values: &[Value]) -> (Vec<Value>, u64) {
    let report = compiled.evaluate(values, None, &mut Control::default());
    match report.obs {
        Obs::Normal(outputs) => (outputs, report.evaluated_nodes),
        other => panic!("accepted total compiled graph must evaluate normally: {other:?}"),
    }
}

pub(super) fn compare(kind: Fixture) -> (u64, u64, usize, usize) {
    let (original, map) = graph(kind);
    let budgets = Budgets::default();
    let comparison = offset::compare(
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
