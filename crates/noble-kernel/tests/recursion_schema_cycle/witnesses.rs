//! Witness cycles, kind mismatches, and resolution-work boundary controls.

#[test]
fn cyclic_witness_self_reference_rejects_invalid() -> Result<(), String> {
    // `dup` with its value variable bound to itself.
    let env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    let program = crate::builders::candidate(
        vec![crate::builders::invocation(
            noble_kernel::contracts::Definition(0),
            vec![
                crate::builders::segment(vec![]),
                crate::builders::reference(1),
            ],
        )],
        vec![0],
    );
    let observed =
        crate::builders::seen(&env, &crate::builders::request(vec![], vec![]), &program)?;
    let crate::builders::Seen::Bad(diagnostic) = observed else {
        return Err(format!(
            "a self-referential witness must reject as invalid, got {observed:?}"
        ));
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::CyclicWitness
    );
    assert!(!diagnostic.provenance_available);
    Ok(())
}

#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; let-else requires the mutually referential witness to reject and the assertion identifies CyclicWitness; repeating either condition would not improve coverage."
)]
fn cyclic_witness_mutual_references_reject_invalid() -> Result<(), String> {
    // `swap` with its two value variables bound to each other.
    let env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    let program = crate::builders::candidate(
        vec![crate::builders::invocation(
            noble_kernel::contracts::Definition(2),
            vec![
                crate::builders::segment(vec![]),
                crate::builders::reference(2),
                crate::builders::reference(1),
            ],
        )],
        vec![0],
    );
    let observed =
        crate::builders::seen(&env, &crate::builders::request(vec![], vec![]), &program)?;
    let crate::builders::Seen::Bad(diagnostic) = observed else {
        return Err(format!(
            "mutually referential witnesses must reject as invalid, got {observed:?}"
        ));
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::CyclicWitness
    );
    Ok(())
}

#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; acceptance is required by let-else against the exact resolved Bool/Bool request; a failure returns the observed outcome instead of a duplicate assertion."
)]
fn resolvable_witness_chains_accept_and_resolve() -> Result<(), String> {
    // `swap` with its first value bound by reference to the second: the
    // chain resolves and the interface is exactly the resolved one.
    let env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    let program = crate::builders::candidate(
        vec![
            crate::builders::lit_node(noble_kernel::untrusted::Lit::Bool(true), vec![]),
            crate::builders::lit_node(
                noble_kernel::untrusted::Lit::Bool(false),
                vec![noble_kernel::types::Ty::Bool],
            ),
            crate::builders::invocation(
                noble_kernel::contracts::Definition(2),
                vec![
                    crate::builders::segment(vec![]),
                    crate::builders::reference(2),
                    crate::builders::value(noble_kernel::types::Ty::Bool),
                ],
            ),
        ],
        vec![0, 1, 2],
    );
    let observed = crate::builders::seen(
        &env,
        &crate::builders::request(
            vec![],
            vec![noble_kernel::types::Ty::Bool, noble_kernel::types::Ty::Bool],
        ),
        &program,
    )?;
    let crate::builders::Seen::Accepted = observed else {
        return Err(format!(
            "a resolvable reference chain must accept, got {observed:?}"
        ));
    };
    // The swap of two identical bools leaves the resolved stack intact.
    Ok(())
}

#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the control asserts the exact hop charge at two limits; the setup is a linear scenario with no further invariants to assert."
)]
fn resolvable_chain_moves_the_accepting_work_limit_by_its_hops() -> Result<(), String> {
    // `pair` whose first value variable names the second through variable 2:
    // one reference hop. Resolution spends one unit per hop, and the spend is
    // charged to the request, so the same interface stated with a direct
    // binding and with a reference must differ by exactly one work unit at the
    // smallest accepting limit. (The bootstrap table has three variables per
    // word, so one hop is the longest chain a built-in word can state.)
    let env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    let direct = crate::builders::candidate(
        vec![
            crate::builders::lit_node(noble_kernel::untrusted::Lit::Bool(true), vec![]),
            crate::builders::lit_node(
                noble_kernel::untrusted::Lit::Bool(false),
                vec![noble_kernel::types::Ty::Bool],
            ),
            crate::builders::invocation(
                noble_kernel::contracts::Definition(13),
                vec![
                    crate::builders::segment(vec![]),
                    crate::builders::value(noble_kernel::types::Ty::Bool),
                    crate::builders::value(noble_kernel::types::Ty::Bool),
                ],
            ),
        ],
        vec![0, 1, 2],
    );
    let referenced = crate::builders::candidate(
        vec![
            crate::builders::lit_node(noble_kernel::untrusted::Lit::Bool(true), vec![]),
            crate::builders::lit_node(
                noble_kernel::untrusted::Lit::Bool(false),
                vec![noble_kernel::types::Ty::Bool],
            ),
            crate::builders::invocation(
                noble_kernel::contracts::Definition(13),
                vec![
                    crate::builders::segment(vec![]),
                    crate::builders::reference(2),
                    crate::builders::value(noble_kernel::types::Ty::Bool),
                ],
            ),
        ],
        vec![0, 1, 2],
    );
    let expected = vec![noble_kernel::types::Ty::Pair(
        Box::new(noble_kernel::types::Ty::Bool),
        Box::new(noble_kernel::types::Ty::Bool),
    )];
    let direct_min = first_accepting(&env, &direct, &expected)?;
    let chain_min = first_accepting(&env, &referenced, &expected)?;
    assert_eq!(
        chain_min,
        direct_min.saturating_add(1),
        "one hop must cost one unit: direct accepted at {direct_min}, referenced at {chain_min}"
    );
    Ok(())
}

/// The smallest work limit that accepts this candidate.
fn first_accepting(
    env: &noble_kernel::contracts::Env,
    candidate: &noble_kernel::untrusted::Candidate,
    expected: &[noble_kernel::types::Ty],
) -> Result<u32, String> {
    let mut work = 0;
    while work <= 200 {
        let mut request = crate::builders::request(vec![], expected.to_vec());
        request.limits.work = work;
        match crate::builders::seen(env, &request, candidate)? {
            crate::builders::Seen::Accepted => return Ok(work),
            crate::builders::Seen::Exhausted(noble_kernel::untrusted::LimitKind::Work) => {}
            other => {
                return Err(format!(
                    "work limit {work} must accept or exhaust for work, got {other:?}"
                ))
            }
        }
        work += 1;
    }
    Err("no work limit up to 200 accepted the candidate".to_string())
}

#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; let-else requires rejection of a stack-to-value reference and the assertion requires InstantiationKind, preserving the observable negative-control contract."
)]
fn kind_crossed_reference_rejects_as_instantiation_kind() -> Result<(), String> {
    // A stack variable bound by reference to a value variable: the chain
    // resolves, but the terminal binding's kind does not match the
    // referring variable's declared kind.
    let env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    let program = crate::builders::candidate(
        vec![crate::builders::invocation(
            noble_kernel::contracts::Definition(0),
            vec![
                crate::builders::reference(1),
                crate::builders::value(noble_kernel::types::Ty::Bool),
            ],
        )],
        vec![0],
    );
    let observed =
        crate::builders::seen(&env, &crate::builders::request(vec![], vec![]), &program)?;
    let crate::builders::Seen::Bad(diagnostic) = observed else {
        return Err(format!(
            "a kind-crossed reference must reject as invalid, got {observed:?}"
        ));
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::InstantiationKind
    );
    Ok(())
}
