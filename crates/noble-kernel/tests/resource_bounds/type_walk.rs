//! The declared work limit must bound the checking work, not only the charges.
//!
//! One frame holding many deep values re-validated every retained entry on
//! every join, and each size read deep-copied the subtree it measured, so a
//! candidate inside its declared limits could spend work quadratic in the type
//! depth per charged unit. These controls state the review scenario: a stack of
//! deep `List` values, then repeated `swap`. The measured cost of the same
//! scenario is bounded in `tests/type_walk_cost.rs`.

use noble_kernel::contracts::Definition;
use noble_kernel::untrusted::LimitKind;

use super::builders::{self, Seen};

/// Depth that makes one entry 510 nodes, just inside the local node cap.
const LIST_DEPTH: u32 = 509;

#[test]
fn deep_frame_entries_join_and_swap_within_the_charged_work() -> Result<(), String> {
    let env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    let entry = builders::deep_list(LIST_DEPTH);
    let entry_nodes = builders::deep_list_nodes(LIST_DEPTH);
    assert!(
        entry_nodes <= 512,
        "the control needs an entry inside the local node cap, got {entry_nodes} nodes"
    );
    let entry_count = 128u32;
    let swaps = 8u32;
    assert_eq!(swaps % 2, 0, "an even number of swaps restores the stack");
    let mut stack: Vec<noble_kernel::types::Ty> = Vec::with_capacity(entry_count as usize);
    let mut pushed = 0;
    while pushed < entry_count {
        stack.push(entry.clone());
        pushed += 1;
    }
    let mut nodes = Vec::with_capacity(swaps as usize);
    let mut body = Vec::with_capacity(swaps as usize);
    let mut index = 0;
    while index < swaps {
        // `swap`: `S a b -- S b a`, with both value variables bound to the
        // same deep value.
        nodes.push(builders::invocation(
            Definition(2),
            vec![
                builders::segment(Vec::new()),
                builders::value(entry.clone()),
                builders::value(entry.clone()),
            ],
        ));
        body.push(index);
        index += 1;
    }
    let program = builders::candidate(nodes, body);
    // Height admits every entry; one entry fits the declared type size; the
    // work limit is far above the charges this body can name.
    let request = builders::request(
        stack.clone(),
        stack,
        builders::limits(10_000, entry_count, entry_nodes),
    );
    let observed = builders::seen(&env, &request, &program)?;
    assert!(
        matches!(observed, Seen::Accepted),
        "the deep stack must join and swap, got {observed:?}"
    );
    Ok(())
}

#[test]
fn deep_frame_entries_refuse_a_height_the_join_would_exceed() -> Result<(), String> {
    let env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    let entry = builders::deep_list(LIST_DEPTH);
    let mut stack: Vec<noble_kernel::types::Ty> = Vec::with_capacity(2);
    stack.push(entry.clone());
    let program = builders::candidate(
        vec![builders::invocation(
            // `dup`: two results from one retained entry.
            Definition(0),
            vec![
                builders::segment(Vec::new()),
                builders::value(entry.clone()),
            ],
        )],
        vec![0],
    );
    let request = builders::request(
        stack,
        vec![entry.clone(), entry],
        builders::limits(10_000, 1, 512),
    );
    let observed = builders::seen(&env, &request, &program)?;
    assert!(
        matches!(observed, Seen::Exhausted(LimitKind::StackHeight)),
        "a join above the declared height must refuse as StackHeight, got {observed:?}"
    );
    Ok(())
}
