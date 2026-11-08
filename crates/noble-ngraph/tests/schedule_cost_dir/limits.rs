#![allow(
    tigerstyle::assertion_density,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; these controls build the real packed pair and change one input or the cap at a time, so owner-path imports, fixture arithmetic and direct assertions are the readable form."
)]
//! The caller's cap on validation work (issue #43: budget controls never count as success).
//!
//! `schedule::check_within` refuses inputs that declare more work than the cap as `Exhausted`,
//! before any declaration, table or body is read. Within the cap it returns exactly what
//! `schedule::check` returns.

use noble_ngraph::schedule::{self, I64Instruction, StreamRefusal};

#[test]
fn the_declared_work_validates() {
    let (accepted, layout, ins) = super::packed(super::interleaved());
    let work = schedule::declared_work(&accepted, &layout, &ins);
    assert_eq!(
        schedule::check_within(&accepted, &layout, &ins, work),
        Ok(())
    );
}

#[test]
fn one_unit_under_the_declared_work_is_exhausted() {
    let (accepted, layout, ins) = super::packed(super::interleaved());
    let work = schedule::declared_work(&accepted, &layout, &ins);
    assert_eq!(
        schedule::check_within(&accepted, &layout, &ins, work - 1),
        Err(StreamRefusal::Exhausted {
            work,
            cap: work - 1
        })
    );
}

#[test]
fn a_longer_stream_over_the_cap_is_exhausted_before_any_body() {
    let (accepted, layout, mut ins) = super::packed(super::interleaved());
    let honest = schedule::declared_work(&accepted, &layout, &ins);
    ins.push(ins[0]);
    assert_eq!(
        schedule::check(&accepted, &layout, &ins),
        Err(StreamRefusal::StreamShape)
    );
    assert_eq!(
        schedule::check_within(&accepted, &layout, &ins, honest),
        Err(StreamRefusal::Exhausted {
            work: honest + 1,
            cap: honest
        })
    );
}

#[test]
fn a_padded_declaration_counts_against_the_cap() {
    let (accepted, mut layout, ins) = super::packed(super::interleaved());
    let honest = schedule::declared_work(&accepted, &layout, &ins);
    let row = layout.consts[0];
    layout.consts.extend(core::iter::repeat_n(row, 1000));
    assert!(matches!(
        schedule::check(&accepted, &layout, &ins),
        Err(StreamRefusal::Declarations(_))
    ));
    assert_eq!(
        schedule::check_within(&accepted, &layout, &ins, honest),
        Err(StreamRefusal::Exhausted {
            work: honest + 1000,
            cap: honest
        })
    );
}

#[test]
fn a_generous_cap_keeps_the_refusal_class() {
    let (accepted, layout, mut ins) = super::packed(super::interleaved());
    let at = ins
        .iter()
        .position(|instruction| matches!(instruction, I64Instruction::Add(..)))
        .expect("a wrapping addition");
    if let I64Instruction::Add(d, a, b) = ins[at] {
        ins[at] = I64Instruction::Sub(d, a, b);
    }
    assert_eq!(
        schedule::check(&accepted, &layout, &ins),
        Err(StreamRefusal::BodyMismatch)
    );
    assert_eq!(
        schedule::check_within(&accepted, &layout, &ins, u64::MAX),
        Err(StreamRefusal::BodyMismatch)
    );
}
