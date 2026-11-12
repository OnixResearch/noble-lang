//! A witness too deep to copy is refused, not overflowed.
//!
//! `node_of` returns an owned node, and copying one is structural: a witness
//! nested deeper than the process stack would abort inside the copy. The
//! control states a 100,000-deep witness and requires a typed refusal instead.

use noble_kernel::contracts::Definition;
use noble_kernel::untrusted::{Constraint, LimitKind};

use super::builders::{self, Seen};

/// Far past any stack a recursive copy would survive.
const DEPTH: u32 = 100_000;

#[test]
fn witness_nested_past_the_copy_bound_is_refused() -> Result<(), String> {
    let env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    let deep = builders::deep_list(DEPTH);
    // `dup` with its value variable bound to the deep witness: the node is
    // fetched and copied before its interface could be checked.
    let program = builders::candidate(
        vec![builders::invocation(
            Definition(0),
            vec![builders::segment(Vec::new()), builders::value(deep)],
        )],
        vec![0],
    );
    let request = builders::request(Vec::new(), Vec::new(), builders::limits(10_000, 16, 512));
    let observed = builders::seen(&env, &request, &program)?;
    let Seen::Bad(diagnostic) = observed else {
        return Err(format!(
            "a witness past the copy bound must be refused as invalid, got {observed:?}"
        ));
    };
    assert_eq!(diagnostic.constraint, Constraint::InvalidType);
    assert_eq!(diagnostic.node, Some(noble_kernel::untrusted::NodeId(0)));
    // The kernel copies type trees structurally, so it has no iterative
    // destructor to offer: release the measured value as one block instead of
    // recursing 100,000 frames in drop glue.
    core::mem::forget(program);
    Ok(())
}

#[test]
fn deep_witness_refusal_is_the_type_size_bound_not_a_panic() -> Result<(), String> {
    let env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    // Just past the local node cap, where the walk still runs to completion:
    // the same shape must refuse for an ordinary reason, not by crashing.
    let deep = builders::deep_list(600);
    let program = builders::candidate(
        vec![builders::invocation(
            Definition(0),
            vec![builders::segment(Vec::new()), builders::value(deep)],
        )],
        vec![0],
    );
    let request = builders::request(Vec::new(), Vec::new(), builders::limits(10_000, 16, 512));
    let observed = builders::seen(&env, &request, &program)?;
    let Seen::Bad(diagnostic) = observed else {
        return Err(format!(
            "a 600-node witness must be refused as invalid, got {observed:?}"
        ));
    };
    assert_eq!(diagnostic.constraint, Constraint::InvalidType);
    Ok(())
}
