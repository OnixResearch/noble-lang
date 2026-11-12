//! Measured cost of one deep frame join.
//!
//! The review scenario from `KernelReview` finding 1: a stack of 128 values of
//! 510 nodes each, then repeated `swap`, inside a declared work limit. Before
//! the fix each join re-validated every retained entry through a size walk that
//! deep-copied every subtree, so one swap of two values cost about 1.7×10^7
//! allocations — work the request never charged and so could not bound. Forty
//! swaps then held the checker for minutes while charging 400 units.
//!
//! This is a resource bound, so it is stated as one: the scenario must finish
//! inside a wall-clock budget with a wide margin on both sides. Measured
//! pre-fix: 220 s. Measured post-fix: 0.26 s. The budget of 20 s leaves room
//! for a loaded machine while still failing the pre-fix checker.

use std::time::{Duration, Instant};

use noble_kernel::contracts::Definition;
use noble_kernel::untrusted::{Candidate, Expected, Limits, Node, NodeId, Request};

/// Depth that makes one entry 510 nodes.
const LIST_DEPTH: u32 = 509;
const ENTRY_COUNT: u32 = 128;
const SWAPS: u32 = 40;
/// Post-fix this check takes milliseconds; pre-fix it took 220 s.
const BUDGET: Duration = Duration::from_secs(20);

fn deep_list(depth: u32) -> noble_kernel::types::Ty {
    let mut ty = noble_kernel::types::Ty::I64;
    let mut index = 0;
    while index < depth {
        ty = noble_kernel::types::Ty::List(Box::new(ty));
        index += 1;
    }
    ty
}

#[test]
fn one_deep_join_costs_the_interface_not_the_whole_frame() {
    let env = match noble_kernel::contracts::environment() {
        Ok(env) => env,
        Err(defect) => panic!("table must validate: {defect:?}"),
    };
    let entry = deep_list(LIST_DEPTH);
    let mut stack: Vec<noble_kernel::types::Ty> = Vec::with_capacity(ENTRY_COUNT as usize);
    let mut pushed = 0;
    while pushed < ENTRY_COUNT {
        stack.push(entry.clone());
        pushed += 1;
    }
    let mut nodes = Vec::with_capacity(SWAPS as usize);
    let mut index = 0;
    while index < SWAPS {
        nodes.push(Node::Invocation {
            def: Definition(2),
            inst: noble_kernel::words::Inst {
                bindings: vec![
                    noble_kernel::words::Binding::Stack(Vec::new()),
                    noble_kernel::words::Binding::Value(entry.clone()),
                    noble_kernel::words::Binding::Value(entry.clone()),
                ],
            },
        });
        index += 1;
    }
    let candidate = Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision: noble_kernel::untrusted::SEMANTIC_REVISION,
        nodes,
        body: (0..SWAPS).map(NodeId).collect(),
    };
    let request = Request {
        input_bytes: 64,
        expected: Expected {
            stack_in: stack.clone(),
            stack_out: stack,
            allowed_effects: noble_kernel::types::EffSet::empty(),
        },
        limits: Limits {
            bytes: 1 << 20,
            nodes: 1_024,
            depth: 32,
            type_size: 510,
            stack_height: ENTRY_COUNT,
            work: 10_000,
            diagnostics: 64,
        },
    };
    let started = Instant::now();
    let outcome = noble_kernel::acceptance::check(&env, &request, &candidate);
    let measured = started.elapsed();
    assert!(
        matches!(outcome, noble_kernel::untrusted::Outcome::Accepted(_)),
        "the charged work covers this body, got {outcome:?}"
    );
    assert!(
        measured <= BUDGET,
        "one deep-stack check took {measured:?}, above the {BUDGET:?} budget"
    );
}
