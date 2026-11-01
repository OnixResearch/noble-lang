#![allow(
    tigerstyle::assertion_density,
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the guard fixtures assert one obligation per test against the real emitted stream, so dense assertions are the readable form."
)]
//! Guarded I64 graphs validated against their real emitted stream (issue #85).
//!
//! A guard writes no register by construction, so it must not be routed through the
//! destination-register lookup that register-writing bodies use. Before the fix this file's
//! first test failed at the audited commit `9bbb2bcb`: `schedule::check` refused every genuine
//! guarded graph with `BodyMismatch` because the destination lookup ran before the guard arm.
//!
//! Each control keeps that fix narrow: a guard body with another node identity, a guard that
//! declares a register, a displaced guard, and a stream missing its guard are all still refused.

use noble_ngraph::ir::{Fact, Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::layout;
use noble_ngraph::schedule::{self, I64Instruction, StreamRefusal};

/// A guarded I64 graph whose guard writes no register by construction.
fn guarded() -> Graph {
    let scheme = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "schedule-guard-repro"),
    );
    let input = builder.input(ScalarType::I64);
    let three = builder.i64_const(3);
    let sum = builder.op(Op::I64Add(input, three));
    builder.guard(Fact::Bit, sum);
    builder.output(sum);
    builder.finish()
}

fn guard_position(ins: &[I64Instruction]) -> usize {
    let is_guard =
        |instruction: &I64Instruction| matches!(instruction, I64Instruction::Guard { .. });
    ins.iter()
        .position(is_guard)
        .expect("one guard in every stream of this file")
}

#[test]
fn a_real_i64_guard_stream_is_accepted() {
    let (accepted, layout, ins) = super::packed(guarded());
    let is_guard =
        |instruction: &I64Instruction| matches!(instruction, I64Instruction::Guard { .. });
    assert_eq!(
        ins.iter()
            .filter(|instruction| is_guard(instruction))
            .count(),
        1
    );
    assert_eq!(
        super::declared(&layout),
        u32::try_from(ins.len()).expect("fits")
    );
    assert_eq!(
        schedule::check(&accepted, &layout, &ins),
        Ok(()),
        "guarded pair"
    );
    assert_eq!(
        layout::check(&accepted, &layout),
        Ok(()),
        "guard keeps its sentinel"
    );
}

#[test]
fn a_wrong_guard_identity_is_refused() {
    let (accepted, layout, mut ins) = super::packed(guarded());
    let at = guard_position(&ins);
    if let I64Instruction::Guard { node } = ins[at] {
        ins[at] = I64Instruction::Guard { node: node ^ 1 };
    }
    let refused = schedule::check(&accepted, &layout, &ins);
    assert_eq!(
        refused,
        Err(StreamRefusal::BodyMismatch),
        "another guard node"
    );
}

#[test]
fn a_guard_with_a_forged_register_is_refused() {
    let (accepted, layout, ins) = super::packed(guarded());
    let at = guard_position(&ins);
    let I64Instruction::Guard { node } = ins[at] else {
        unreachable!("the position matches a guard");
    };
    let mut forged = layout.clone();
    forged.reg[usize::try_from(node).expect("a node id fits in usize")] = 0;
    assert_eq!(
        layout::check(&accepted, &forged),
        Err(layout::Refusal::GuardRegister),
        "a guard cannot declare a register"
    );
    assert_eq!(
        schedule::check(&accepted, &forged, &ins),
        Err(StreamRefusal::Declarations(layout::Refusal::GuardRegister)),
    );
}

#[test]
fn a_displaced_or_dropped_guard_is_refused() {
    let (accepted, layout, ins) = super::packed(guarded());
    let at = guard_position(&ins);
    let mut displaced = ins.clone();
    displaced.swap(at, 0);
    assert_eq!(
        schedule::check(&accepted, &layout, &displaced),
        Err(StreamRefusal::BodyMismatch),
        "a guard body at another position"
    );
    let mut dropped = ins.clone();
    dropped.remove(at);
    assert_eq!(
        schedule::check(&accepted, &layout, &dropped),
        Err(StreamRefusal::StreamShape),
        "a stream missing its guard"
    );
}
