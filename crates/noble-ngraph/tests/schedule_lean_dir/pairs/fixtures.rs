//! The fixed graph pairs and the graphs their steps build (issue #56).

use noble_ngraph::ir::{Graph, GraphBuilder, NodeId, Op, ScalarType, SubProfile, Subject};

/// One graph-building step; operands name earlier steps.
#[derive(Clone, Copy)]
pub(super) enum Step {
    Input,
    Const(i64),
    Add(usize, usize),
    Sub(usize, usize),
    Mul(usize, usize),
    Eq(usize, usize),
}

/// A fixed pair: both graphs, their shared ordered output positions, and the declared universe.
pub(super) struct Fixture {
    pub(super) name: &'static str,
    pub(super) doc: &'static str,
    pub(super) left: &'static [Step],
    pub(super) right: &'static [Step],
    pub(super) outputs: &'static [usize],
    pub(super) universe: &'static [&'static [i64]],
}

const TWO_POW_63: i64 = i64::MIN;

pub(super) const FIXTURES: [Fixture; 7] = [
    Fixture {
        name: "sharedInput",
        doc: "a shared input read three times, `(x + x) + x` against `x · 3`: equivalent",
        left: &[Step::Input, Step::Add(0, 0), Step::Add(1, 0)],
        right: &[Step::Input, Step::Const(3), Step::Mul(0, 1)],
        outputs: &[2],
        universe: &[&[0], &[1], &[-1], &[i64::MAX], &[i64::MIN]],
    },
    Fixture {
        name: "subtractionOrder",
        doc: "`x - y` against `y - x`: refuted, first where `x ≠ y`",
        left: &[Step::Input, Step::Input, Step::Sub(0, 1)],
        right: &[Step::Input, Step::Input, Step::Sub(1, 0)],
        outputs: &[2],
        universe: &[&[0, 0], &[5, 5], &[3, 8], &[i64::MIN, 0]],
    },
    Fixture {
        name: "wrapOnlyEquivalent",
        doc: "`x · 2^63 · 2` against `x · 0 · 1`: equivalent only because arithmetic wraps at `2^64`",
        left: &[Step::Input, Step::Const(TWO_POW_63), Step::Mul(0, 1), Step::Const(2), Step::Mul(2, 3)],
        right: &[Step::Input, Step::Const(0), Step::Mul(0, 1), Step::Const(1), Step::Mul(2, 3)],
        outputs: &[4],
        universe: &[&[0], &[1], &[-1], &[i64::MAX], &[i64::MIN]],
    },
    Fixture {
        name: "wrapBoundary",
        doc: "`x · 2^63` against `x · 0`: agree on even inputs, refuted first at an odd one",
        left: &[Step::Input, Step::Const(TWO_POW_63), Step::Mul(0, 1)],
        right: &[Step::Input, Step::Const(0), Step::Mul(0, 1)],
        outputs: &[2],
        universe: &[&[0], &[2], &[i64::MIN], &[1]],
    },
    Fixture {
        name: "boolOutput",
        doc: "`x + 1 = 0` against `x + 0 = -1`, a `Bool` output: equivalent",
        left: &[Step::Input, Step::Const(1), Step::Add(0, 1), Step::Const(0), Step::Eq(2, 3)],
        right: &[Step::Input, Step::Const(-1), Step::Const(0), Step::Add(0, 2), Step::Eq(3, 1)],
        outputs: &[4],
        universe: &[&[0], &[-1], &[i64::MAX], &[i64::MIN]],
    },
    Fixture {
        name: "repeatedOutputs",
        doc: "ordered outputs `[x + y, x - y, x + y]` against `[y + x, y - x, y + x]`: the repeated sum agrees, and the difference refutes first where `x ≠ y`",
        left: &[Step::Input, Step::Input, Step::Add(0, 1), Step::Sub(0, 1)],
        right: &[Step::Input, Step::Input, Step::Add(1, 0), Step::Sub(1, 0)],
        outputs: &[2, 3, 2],
        universe: &[&[4, 4], &[i64::MAX, i64::MAX], &[1, 2]],
    },
    Fixture {
        name: "negation",
        doc: "`0 - x` against `x + 0`: equal only where `2x` wraps to zero, at `0` and `i64::MIN`; refuted first at a large negative input, whose shrink halves toward zero",
        left: &[Step::Input, Step::Const(0), Step::Sub(1, 0)],
        right: &[Step::Input, Step::Const(0), Step::Add(0, 1)],
        outputs: &[2],
        universe: &[&[0], &[i64::MIN], &[-1_000_000_007], &[1]],
    },
];

/// The graph a step list builds, with the fixture's ordered outputs.
pub(super) fn build(name: &str, steps: &[Step], outputs: &[usize]) -> Graph {
    let scheme = crate::common::IDS;
    let mut builder = GraphBuilder::new(SubProfile::I64, Subject::authored(&scheme, name));
    let mut ids: Vec<NodeId> = Vec::with_capacity(steps.len());
    for step in steps {
        #[expect(
            tigerstyle::fragile_exhaustive_enum_match,
            reason = "Owner: noble-maintainers; each step kind builds exactly one graph node, so a new step kind must say which node it builds."
        )]
        let id = match *step {
            Step::Input => builder.input(ScalarType::I64),
            Step::Const(value) => builder.i64_const(value),
            Step::Add(a, b) => builder.op(Op::I64Add(ids[a], ids[b])),
            Step::Sub(a, b) => builder.op(Op::I64Sub(ids[a], ids[b])),
            Step::Mul(a, b) => builder.op(Op::I64Mul(ids[a], ids[b])),
            Step::Eq(a, b) => builder.op(Op::I64Eq(ids[a], ids[b])),
        };
        ids.push(id);
    }
    for &output in outputs {
        builder.output(ids[output]);
    }
    builder.finish()
}
