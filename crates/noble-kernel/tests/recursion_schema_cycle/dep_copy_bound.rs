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
//! `acceptance::check`: one environment carrying a single 2^23-edge dependency
//! list (32 MiB of definitions) is built once, then checked many times at work
//! allowances 0, 1 and 2. No allowance reaches past the third edge, so every
//! outcome must be `Exhausted(Work)` and the whole loop must finish inside a
//! wall-clock budget with a wide margin on both sides.
//!
//! Measured pre-fix: 47.76 s (the list was cloned once per check, so
//! 3840 input-sized copies). Measured post-fix: 0.013 s (the walk charges the
//! first edge and stops; no list is materialized). The budget of 750 ms sits
//! 64x below the pre-fix measurement and 58x above the post-fix one, so a
//! loaded machine does not flake it and the pre-fix checker cannot pass it.
//!
//! The sibling preflight walkers were audited for the same root cause and
//! none has it: the schema scan reads `Env::schemas` through one borrowed
//! index after charging each entry, the nominal budget charges the declared
//! `nominals`/`generic_variants`/`bound_adapters` count before scanning them,
//! `signed_self_check` borrows its candidate node and charges before it, and
//! the calculator clone budget measures lengths without cloning. Only the
//! dependency walk copied external data before its charge.

/// Edges in the single dependency list: 2^23 entries, 32 MiB.
const EDGE_COUNT: usize = 1 << 23;
/// Zero- and near-zero-work checks sampled from the one environment.
const REPEATS: u32 = 1_280;
/// The allowances sampled: no edge is reachable under any of them.
const ALLOWANCES: [u32; 3] = [0, 1, 2];
/// Post-fix this loop takes milliseconds; pre-fix it took seconds.
const BUDGET: std::time::Duration = std::time::Duration::from_millis(750);

#[test]
fn unbudgeted_dependency_edges_are_not_materialized() -> Result<(), String> {
    let mut env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    env.defs.push(crate::builders::unit_like());
    env.kinds.push(noble_kernel::contracts::Behavior::Named);
    env.deps
        .push(vec![noble_kernel::contracts::Definition(0); EDGE_COUNT]);
    let candidate = crate::builders::empty_program();
    let started = std::time::Instant::now();
    let mut refusals: u32 = 0;
    let mut examined: u32 = 0;
    let mut unexpected: Vec<String> = Vec::new();
    for allowance in ALLOWANCES {
        let mut request = crate::builders::request(vec![], vec![]);
        request.limits = noble_kernel::untrusted::Limits {
            work: allowance,
            ..request.limits
        };
        let mut repeat = 0;
        while repeat < REPEATS {
            match noble_kernel::acceptance::check(&env, &request, &candidate) {
                noble_kernel::untrusted::Outcome::Exhausted(
                    noble_kernel::untrusted::LimitKind::Work,
                ) => refusals += 1,
                other => unexpected.push(format!("work {allowance}: {other:?}")),
            }
            examined += 1;
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
        refusals, examined,
        "every sampled check refuses on the work meter"
    );
    assert!(
        measured <= BUDGET,
        "{examined} near-zero-work checks over a {EDGE_COUNT}-edge list took {measured:?}, above the {BUDGET:?} budget"
    );
    Ok(())
}
