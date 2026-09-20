/// B-CHECK-03: a duplicated first-class program value shares one
/// instantiated interface — the second use cannot claim a fresh
/// instantiation against the same empty stack.
// r[verify VT-M3-01]
#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; let-else requires rejection of the second run and the assertion checks StackJoin; the large shared-interface fixture needs no redundant checks."
)]
fn duplicated_program_value_shares_one_interface() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let program_value = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::I64],
        noble_kernel::types::EffSet::empty(),
    );
    // `[ B ] : R -- R Program<A,C,e>` with an empty body deriving
    // `[I64] -- [I64]`.
    let quotation = crate::support::quote_node(
        vec![],
        vec![],
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::I64],
        &[],
    );
    let program = crate::support::candidate(
        vec![
            quotation,
            crate::support::invocation(
                crate::DUP,
                vec![
                    crate::support::segment(vec![]),
                    crate::support::value(program_value.clone()),
                ],
            ),
            crate::support::invocation(
                crate::RUN,
                vec![
                    crate::support::segment(vec![]),
                    crate::support::segment(vec![noble_kernel::types::Ty::I64]),
                    crate::support::effect_binding(&[]),
                ],
            ),
            crate::support::invocation(
                crate::RUN,
                vec![
                    crate::support::segment(vec![]),
                    crate::support::segment(vec![noble_kernel::types::Ty::I64]),
                    crate::support::effect_binding(&[]),
                ],
            ),
        ],
        vec![0, 1, 2, 3],
    );
    let crate::support::Seen::Bad(diagnostic) = crate::support::seen(
        &env,
        &crate::support::request(
            vec![],
            vec![noble_kernel::types::Ty::I64, program_value.clone()],
            &[],
        ),
        &program,
    )?
    else {
        return Err("the duplicated program value must reject at its second run".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::StackJoin,
        "the second copy cannot re-instantiate against the same stack"
    );
    Ok(())
}

/// B-CHECK-06: the eliminators take the real union of the branch bounds —
// r[verify VT-M3-01]
// r[verify VT-M3-01]
#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; let-else requires rejection and the assertion checks the emitting branch's effect survives union; fixture construction is not an additional assertion obligation."
)]
fn eliminator_union_is_explicit_not_implicit() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let if_word = noble_kernel::contracts::Definition(18);
    let test_emit = noble_kernel::contracts::Definition(22);
    let unit = noble_kernel::types::Ty::Unit;
    // Branch one derives `[] -- [Unit]` carrying `test.emit` (latent `{0}`);
    // branch two derives the same interface quietly. The union bound the
    // `if` word derives is `{0}`, and the request allows nothing — a
    // checker that implicitly joined to the quiet branch's `{}` would
    // accept; the real union rejects.
    let emitting_value =
        noble_kernel::types::Ty::program(vec![], vec![unit.clone()], crate::support::ids(&[0]));
    let program = crate::support::candidate(
        vec![
            crate::support::lit_node(noble_kernel::untrusted::Lit::Bool(true), vec![]),
            crate::support::quote_node(
                vec![2, 3],
                vec![noble_kernel::types::Ty::Bool],
                vec![],
                vec![unit.clone()],
                &[0],
            ),
            crate::support::lit_node(noble_kernel::untrusted::Lit::Text, vec![]),
            crate::support::invocation(test_emit, vec![crate::support::segment(vec![])]),
            crate::support::quote_node(
                vec![5],
                vec![noble_kernel::types::Ty::Bool, emitting_value.clone()],
                vec![],
                vec![unit.clone()],
                &[],
            ),
            crate::support::lit_node(noble_kernel::untrusted::Lit::Unit, vec![]),
            crate::support::invocation(
                if_word,
                vec![
                    crate::support::segment(vec![]),
                    crate::support::segment(vec![unit.clone()]),
                    crate::support::effect_binding(&[0]),
                    crate::support::effect_binding(&[]),
                ],
            ),
        ],
        vec![0, 1, 4, 6],
    );
    let crate::support::Seen::Bad(diagnostic) = crate::support::seen(
        &env,
        &crate::support::request(vec![], vec![unit.clone()], &[]),
        &program,
    )?
    else {
        return Err("the union bound must reject against the empty allowance".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::EffectInclusion(noble_kernel::types::EffId(0)),
        "the quiet branch cannot implicitly absorb the emitting branch's bound"
    );
    Ok(())
}
