#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::numeric_units,
    reason = "Owner: noble-maintainers; this is the #108 certificate fixture and control harness: every control asserts one exact refusal and every pilot compares two executed endpoints, so terse fixtures, unwraps and long setup legs are the readable form."
)]
//! Guard-scoped rewrite certificates (`ngraph-cert-scoped/0`), issue #108.
//!
//! Two pilots whose premises matter (the pinned operand is an interface input, not a
//! constant, so the guard or input fact is load-bearing), the six named controls, a
//! weaker-relation control, the declared bounds, and a two-guard fixture that executes
//! both endpoints on the reference interpreter and the compiled machine and compares
//! full observations for inputs violating zero, one and two guard predicates.

use noble_ngraph::cert_scoped::{self, Derivation, Outcome};
use noble_ngraph::eval::Obs;
use noble_ngraph::ir::{
    Fact, Graph, GraphBuilder, Literal, Op, Provenance, ScalarType, SubProfile, Subject,
};
use noble_ngraph::sem::Value;

use super::differential::{observations, TestIds};

fn builder(subject: &str) -> GraphBuilder {
    GraphBuilder::new(SubProfile::I64, Subject::authored(&TestIds, subject))
}

fn exact(value: i64) -> Fact {
    Fact::Range {
        lo: Literal::I64(value),
        hi: Literal::I64(value),
    }
}

/// A guarded pilot: `x` is an interface input with no fact of its own, so the only thing
/// that pins it to `0` is the guard below the addition. Both operands of the folded node
/// stay live as outputs, so the rewritten endpoint is admitted.
pub(super) fn guarded_fold() -> Graph {
    let mut builder = builder("scoped-guarded");
    let x = builder.input(ScalarType::I64);
    let seven = builder.i64_const(7);
    let _guard = builder.guard(exact(0), x);
    let sum = builder.op(Op::I64Add(x, seven));
    let scaled = builder.op(Op::I64Mul(x, sum));
    builder.output(seven);
    builder.output(sum);
    builder.output(scaled);
    builder.finish()
}

/// The same graph with the guard removed: the fold below is what happens when the scope
/// is dropped rather than checked.
pub(super) fn guarded_fold_without_guard() -> Graph {
    let mut builder = builder("scoped-guarded-unscoped");
    let x = builder.input(ScalarType::I64);
    let seven = builder.i64_const(7);
    let sum = builder.op(Op::I64Add(x, seven));
    let scaled = builder.op(Op::I64Mul(x, sum));
    builder.output(seven);
    builder.output(sum);
    builder.output(scaled);
    builder.finish()
}

/// A preflight pilot: `x`'s checked input fact pins it to `3`, and the subtraction denotes
/// `6` only on that admitted input domain.
pub(super) fn preflight_fold() -> Graph {
    let mut builder = builder("scoped-preflight");
    let x = builder.input(ScalarType::I64);
    let nine = builder.i64_const(9);
    let difference = builder.op(Op::I64Sub(nine, x));
    let scaled = builder.op(Op::I64Mul(x, difference));
    builder.fact(x, exact(3), Provenance::InputValidated);
    builder.output(nine);
    builder.output(difference);
    builder.output(scaled);
    builder.finish()
}

/// The same graph without the input fact.
pub(super) fn preflight_fold_without_fact() -> Graph {
    let mut builder = builder("scoped-preflight-unscoped");
    let x = builder.input(ScalarType::I64);
    let nine = builder.i64_const(9);
    let difference = builder.op(Op::I64Sub(nine, x));
    let scaled = builder.op(Op::I64Mul(x, difference));
    builder.output(nine);
    builder.output(difference);
    builder.output(scaled);
    builder.finish()
}

/// Two guards in node order, both below the folded addition, and the addition's result
/// shared by an output and a later use.
pub(super) fn two_guards() -> Graph {
    let mut builder = builder("scoped-two-guards");
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let _first = builder.guard(exact(0), x);
    let _second = builder.guard(exact(7), y);
    let sum = builder.op(Op::I64Add(x, y));
    let scaled = builder.op(Op::I64Mul(x, sum));
    builder.output(sum);
    builder.output(scaled);
    builder.finish()
}

/// The future-guard fixture: the guard that pins `x` to `0` sits *after* the addition it
/// would justify, and a guard in between observes the addition's value. The fold is
/// therefore wrong for inputs where `x` is not `0`: the first failing guard changes.
pub(super) fn future_guard() -> Graph {
    let mut builder = builder("scoped-future-guard");
    let x = builder.input(ScalarType::I64);
    let one = builder.i64_const(1);
    let sum = builder.op(Op::I64Add(x, one));
    let _between = builder.guard(
        Fact::Range {
            lo: Literal::I64(0),
            hi: Literal::I64(2),
        },
        sum,
    );
    let _future = builder.guard(exact(0), x);
    let scaled = builder.op(Op::I64Mul(x, sum));
    builder.output(one);
    builder.output(scaled);
    builder.finish()
}

/// The future-guard graph with the addition replaced, in place, by the constant the
/// unsound scope would claim. Nothing else changes.
pub(super) fn future_guard_folded() -> Graph {
    let mut graph = future_guard();
    graph.nodes[2].op = Op::Const(Literal::I64(1));
    graph
}

/// An admitted graph with a single-valued *derived* fact: the subtraction really does
/// denote `6` there, but a derived fact is not an admitted input domain.
pub(super) fn derived_fact_fold() -> Graph {
    let mut builder = builder("scoped-derived-fact");
    let x = builder.input(ScalarType::I64);
    let nine = builder.i64_const(9);
    let difference = builder.op(Op::I64Sub(nine, x));
    let shifted = builder.op(Op::I64Add(difference, nine));
    let scaled = builder.op(Op::I64Mul(x, shifted));
    builder.fact(x, exact(3), Provenance::InputValidated);
    builder.fact(
        difference,
        exact(6),
        Provenance::Derived(String::from("i64.sub")),
    );
    builder.output(nine);
    builder.output(scaled);
    builder.finish()
}

fn admitted(graph: &Graph) {
    noble_ngraph::check::check(&TestIds, graph.clone())
        .unwrap_or_else(|diagnostic| panic!("the fixture must be admitted: {diagnostic:?}"));
}

fn both(graph: &Graph, inputs: &[Value]) -> (Obs, Obs) {
    let (compiled, _, reference, _) = observations(graph, inputs)
        .unwrap_or_else(|diagnostic| panic!("the endpoint must be admitted: {diagnostic:?}"));
    (reference, compiled)
}

fn inputs(values: &[i64]) -> Vec<Value> {
    values.iter().map(|&value| Value::I64(value)).collect()
}

/// Both endpoints are admitted, both evaluators agree on each endpoint, and the four
/// observations are equal under `Obs::obs_eq` (not merely their outputs).
fn assert_observations_agree(left: &Graph, right: &Graph, values: &[i64], context: &str) {
    let inputs = inputs(values);
    let (left_reference, left_compiled) = both(left, &inputs);
    let (right_reference, right_compiled) = both(right, &inputs);
    assert!(
        left_reference.obs_eq(&left_compiled),
        "{context}: the left endpoint's evaluators disagree: {left_reference:?} vs {left_compiled:?}"
    );
    assert!(
        right_reference.obs_eq(&right_compiled),
        "{context}: the right endpoint's evaluators disagree: {right_reference:?} vs {right_compiled:?}"
    );
    assert!(
        left_reference.obs_eq(&right_reference),
        "{context}: the reference observations differ: {left_reference:?} vs {right_reference:?}"
    );
    assert!(
        left_compiled.obs_eq(&right_compiled),
        "{context}: the compiled observations differ: {left_compiled:?} vs {right_compiled:?}"
    );
}

fn propose(graph: &Graph) -> (Derivation, Graph) {
    cert_scoped::propose(&TestIds, graph)
}

fn check(graph: &Graph, result: &Graph, derivation: &Derivation) -> Outcome {
    cert_scoped::check(&TestIds, graph, result, derivation)
}

#[path = "scoped/bounds.rs"]
mod bounds;
#[path = "scoped/endpoint_controls.rs"]
mod endpoint_controls;
#[path = "scoped/pilots.rs"]
mod pilots;
#[path = "scoped/scope_controls.rs"]
mod scope_controls;
