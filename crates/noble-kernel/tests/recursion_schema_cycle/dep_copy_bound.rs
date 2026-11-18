//! Resource bound of the dependency preflight's edge handling (issue #138).
//!
//! The dependency walk read one entered definition's dependency list through a
//! `Vec` cloned out of `Env::deps` before its first work check, so a request
//! with a zero or small work allowance still copied the whole caller-supplied
//! list. The list is external environment data: neither its length nor the
//! definition count is fixed by the type, so that copy was input-sized work
//! the work meter never charged and therefore could not bound.
//!
//! This is a resource bound, so it is stated as one on the consumer-visible
//! `acceptance::check`: one environment carrying a single 2^22-edge dependency
//! list (16 MiB of definitions) is built once, then checked many times at work
//! allowances 0, 1 and 2. No allowance reaches past the third edge, so every
//! outcome must be `Exhausted(Work)` and the whole loop must finish inside a
//! wall-clock budget with a wide margin on both sides.
//!
//! Measured pre-fix: 47.76 s (the list was cloned once per check, so
//! 3840 input-sized copies). Measured post-fix: 0.016 s (the walk charges the
//! first edge and stops; no list is materialized). The budget of 250 ms sits
//! 191x below the pre-fix measurement and 15x above the post-fix one, so a
//! loaded machine does not flake it and the pre-fix checker cannot pass it.
//!
//! The sibling preflight walkers were audited for the same root cause and
//! none has it: the schema scan reads `Env::schemas` through one borrowed
//! index after charging each entry, the nominal budget charges the declared
//! `nominals`/`generic_variants`/`bound_adapters` count before scanning them,
//! `signed_self_check` borrows its candidate node and charges before it, and
//! the calculator clone budget measures lengths without cloning. Only the
//! dependency walk copied external data before its charge.

use std::time::{Duration, Instant};

use noble_kernel::contracts::{Behavior, Definition};
use noble_kernel::untrusted::{LimitKind, Limits, Outcome, Request};

/// Edges in the single dependency list: 2^23 entries, 32 MiB.
const EDGE_COUNT: usize = 1 << 23;
/// Zero- and near-zero-work checks sampled from the one environment.
const REPEATS: u32 = 1_280;
/// The allowances sampled: no edge is reachable under any of them.
const ALLOWANCES: [u32; 3] = [0, 1, 2];
/// Post-fix this loop takes milliseconds; pre-fix it took seconds.
const BUDGET: Duration = Duration::from_millis(250);

#[test]
fn unbudgeted_dependency_edges_are_not_materialized() {
    let mut env = match noble_kernel::contracts::environment() {
        Ok(env) => env,
        Err(defect) => panic!("table must validate: {defect:?}"),
    };
    env.defs.push(crate::builders::unit_like());
    env.kinds.push(Behavior::Named);
    env.deps.push(vec![Definition(0); EDGE_COUNT]);
    let candidate = crate::builders::empty_program();

    let started = Instant::now();
    let mut refusals = 0;
    let mut unexpected = Vec::new();
    for allowance in ALLOWANCES {
        let mut request: Request = crate::builders::request(vec![], vec![]);
        request.limits = Limits {
            work: allowance,
            ..request.limits
        };
        let mut repeat = 0;
        while repeat < REPEATS {
            match noble_kernel::acceptance::check(&env, &request, &candidate) {
                Outcome::Exhausted(LimitKind::Work) => refusals += 1,
                other => unexpected.push(format!("work {allowance}: {other:?}")),
            }
            repeat += 1;
        }
    }
    let measured = started.elapsed();
    println!("near-zero-work checks: {refusals} in {measured:?}");
    assert!(
        unexpected.is_empty(),
        "every allowance is below the first edge, got {unexpected:?}"
    );
    assert_eq!(
        refusals,
        u32::try_from(ALLOWANCES.len()).expect("allowance count fits") * REPEATS,
        "every sampled check refuses on the work meter"
    );
    assert!(
        measured <= BUDGET,
        "{} near-zero-work checks over a {EDGE_COUNT}-edge list took {measured:?}, above the {BUDGET:?} budget",
        refusals
    );
}
