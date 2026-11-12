/// B-CHECK-05: a quotation's witness may state its result stack as a
/// reference to its input stack. The resolved witness is the one that
/// opens the child frame, so the empty body checks exactly as the direct
/// `[I64] -- [I64]` spelling does, never as an internal failure.
// r[verify VT-M3-01]
#[test]
fn quotation_result_stack_may_reference_its_input_stack() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let program_value = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::I64],
        noble_kernel::types::EffSet::empty(),
    );
    let request = crate::support::request(vec![], vec![program_value], &[]);
    let direct = crate::support::candidate(
        vec![crate::support::quote_node(
            vec![],
            vec![],
            vec![noble_kernel::types::Ty::I64],
            vec![noble_kernel::types::Ty::I64],
            &[],
        )],
        vec![0],
    );
    let referenced = crate::support::candidate(
        vec![noble_kernel::untrusted::Node::Quotation {
            body: vec![],
            inst: crate::support::inst(vec![
                crate::support::segment(vec![]),
                crate::support::segment(vec![noble_kernel::types::Ty::I64]),
                noble_kernel::words::Binding::Ref(noble_kernel::words::Variable(1)),
                crate::support::effect_binding(&[]),
            ]),
        }],
        vec![0],
    );
    let crate::support::Seen::Accepted(expected) = crate::support::seen(&env, &request, &direct)?
    else {
        return Err("the direct quotation witness must be accepted".to_string());
    };
    let crate::support::Seen::Accepted(checked) =
        crate::support::seen(&env, &request, &referenced)?
    else {
        return Err("a result stack referring to the input stack must be accepted".to_string());
    };
    assert_eq!(checked.interface.stack_in, expected.interface.stack_in);
    assert_eq!(
        checked.interface.stack_out, expected.interface.stack_out,
        "the reference resolves to exactly the direct witness's Program type"
    );
    Ok(())
}

/// B-CHECK-03: a duplicated first-class program value shares one
/// instantiated interface. The first `run` consumes the copy whose type is
/// `Program<[], [I64], {}>`; the second copy keeps that very interface, so
/// the second `run` cannot claim a fresh instantiation with another result
/// stack.
// r[verify VT-M3-01]
#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; let-else requires the second run's refusal, and three assertions pin its node, the expected instantiated interface and the retained copy; the fixture itself needs no redundant checks."
)]
fn duplicated_program_value_shares_one_interface() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    // The shared interface's input stack is empty, so `dup` leaves the two
    // copies adjacent and the first run can consume the top one.
    let program_value = noble_kernel::types::Ty::program(
        vec![],
        vec![noble_kernel::types::Ty::I64],
        noble_kernel::types::EffSet::empty(),
    );
    // `[ i64 7 ] : [] -- [I64]` derives the shared `Program<[], [I64], {}>`.
    let program_body = crate::support::lit_node(noble_kernel::untrusted::Lit::I64(7), vec![]);
    let quotation = crate::support::quote_node(
        vec![0],
        vec![],
        vec![],
        vec![noble_kernel::types::Ty::I64],
        &[],
    );
    let first_run = crate::support::invocation(
        crate::RUN,
        vec![
            crate::support::segment(vec![]),
            crate::support::segment(vec![noble_kernel::types::Ty::I64]),
            crate::support::effect_binding(&[]),
        ],
    );
    // The first run leaves `[I64]` above the retained copy; `drop` removes it
    // so the second use sees that copy alone.
    let discard_result = crate::support::invocation(
        crate::DROP,
        vec![
            crate::support::segment(vec![program_value.clone()]),
            crate::support::value(noble_kernel::types::Ty::I64),
        ],
    );
    let second_run = crate::support::invocation(
        crate::RUN,
        vec![
            crate::support::segment(vec![]),
            crate::support::segment(vec![noble_kernel::types::Ty::Bool]),
            crate::support::effect_binding(&[]),
        ],
    );
    let duplicate = crate::support::invocation(
        crate::DUP,
        vec![
            crate::support::segment(vec![]),
            crate::support::value(program_value.clone()),
        ],
    );
    let program = crate::support::candidate(
        vec![
            program_body,
            quotation,
            duplicate,
            first_run,
            discard_result,
            second_run,
        ],
        vec![1, 2, 3, 4, 5],
    );
    let crate::support::Seen::Bad(diagnostic) = crate::support::seen(
        &env,
        &crate::support::request(vec![], vec![noble_kernel::types::Ty::Bool], &[]),
        &program,
    )?
    else {
        return Err("the second use of the duplicated value must be refused".to_string());
    };
    assert_eq!(
        diagnostic.node,
        Some(noble_kernel::untrusted::NodeId(5)),
        "the first run succeeds and the second run is the failing node"
    );
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::StackJoin,
        "the second copy cannot claim a fresh interface"
    );
    assert_eq!(
        diagnostic.expected,
        vec![noble_kernel::types::Ty::program(
            vec![],
            vec![noble_kernel::types::Ty::Bool],
            noble_kernel::types::EffSet::empty(),
        )],
        "the refused witness claims a different result stack"
    );
    assert_eq!(
        diagnostic.actual,
        vec![program_value],
        "the retained copy keeps the interface the first run consumed"
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
