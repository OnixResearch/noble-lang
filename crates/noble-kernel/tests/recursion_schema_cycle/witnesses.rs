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
    reason = "Owner: noble-maintainers; let-else requires resolved-chain acceptance and tight-budget exhaustion, then the assertion requires Work; long witness setup needs no additional assertions."
)]
fn resolvable_chain_at_the_walk_boundary_and_exhausted_below_it() -> Result<(), String> {
    // `pair` with a two-hop reference chain under its declared value
    // variable: the resolution walk charges one unit per hop inside `apply`
    // — before the node's fold charge — so a work limit below the hop count
    // fails closed at the walk, and an ample budget resolves and accepts.
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
    let request = crate::builders::request(
        vec![],
        vec![noble_kernel::types::Ty::Pair(
            Box::new(noble_kernel::types::Ty::Bool),
            Box::new(noble_kernel::types::Ty::Bool),
        )],
    );
    let observed = crate::builders::seen(&env, &request, &program)?;
    let crate::builders::Seen::Accepted = observed else {
        return Err(format!(
            "an ample budget resolves the chain and accepts, got {observed:?}"
        ));
    };
    let mut tight = request.clone();
    tight.limits.work = 1;
    let observed = crate::builders::seen(&env, &tight, &program)?;
    let crate::builders::Seen::Exhausted(limit) = observed else {
        return Err(format!(
            "a work limit below the chain's hops must fail closed, got {observed:?}"
        ));
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::Work);
    Ok(())
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
