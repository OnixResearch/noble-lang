//! Acceptance controls for the fragment checker.
// Names the fragment acceptance contract this crate implements.
// r[impl VT-M2-01]

mod support;

const DUP: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(0);
const ADD: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(4);
const RUN: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(9);
const TEST_EMIT: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(21);

fn forty_two() -> (
    noble_kernel::untrusted::Request,
    noble_kernel::untrusted::Candidate,
) {
    let program = support::candidate(
        vec![
            support::lit_node(noble_kernel::untrusted::Lit::I64(41), vec![]),
            support::lit_node(
                noble_kernel::untrusted::Lit::I64(1),
                vec![support::i64_ty()],
            ),
            support::invocation(ADD, vec![support::segment(vec![])]),
            support::quote_node(
                vec![1, 2],
                vec![support::i64_ty()],
                vec![support::i64_ty()],
                vec![support::i64_ty()],
                &[],
            ),
            support::invocation(
                RUN,
                vec![
                    support::segment(vec![support::i64_ty()]),
                    support::segment(vec![support::i64_ty()]),
                    support::effect_binding(&[]),
                ],
            ),
        ],
        vec![0, 3, 4],
    );
    (
        support::request(vec![], vec![support::i64_ty()], &[]),
        program,
    )
}

#[test]
fn sequence_and_literals_accept_arithmetic() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let program = support::candidate(
        vec![
            support::lit_node(noble_kernel::untrusted::Lit::I64(41), vec![]),
            support::lit_node(
                noble_kernel::untrusted::Lit::I64(1),
                vec![support::i64_ty()],
            ),
            support::invocation(ADD, vec![support::segment(vec![])]),
        ],
        vec![0, 1, 2],
    );
    let support::Seen::Accepted(checked) = support::seen(
        &env,
        &support::request(vec![], vec![support::i64_ty()], &[]),
        &program,
    )?
    else {
        return Err("expected acceptance".to_string());
    };
    assert_eq!(checked.derivations.len(), 3);
    assert_eq!(checked.interface.stack_out, vec![support::i64_ty()]);
    Ok(())
}

#[test]
fn quotation_construction_checks_body_and_runs() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let (request, program) = forty_two();
    let support::Seen::Accepted(checked) = support::seen(&env, &request, &program)? else {
        return Err("expected acceptance".to_string());
    };
    assert_eq!(checked.derivations.len(), 5);
    Ok(())
}

#[test]
fn hidden_emit_rejects_against_empty_bound() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let program = support::candidate(
        vec![
            support::invocation(TEST_EMIT, vec![support::segment(vec![])]),
            support::quote_node(
                vec![0],
                vec![],
                vec![support::text_ty()],
                vec![support::unit_ty()],
                &[],
            ),
        ],
        vec![1],
    );
    let pushed = noble_kernel::types::Ty::program(
        vec![support::text_ty()],
        vec![support::unit_ty()],
        support::ids(&[]),
    );
    let support::Seen::Bad(diagnostic) = support::seen(
        &env,
        &support::request(vec![], vec![pushed], &[0]),
        &program,
    )?
    else {
        return Err("expected a rejection".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::EffectInclusion(noble_kernel::types::EffId(0))
    );
    Ok(())
}

#[test]
fn limits_boundaries_and_exhaustion() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let (base, program) = forty_two();

    let mut exact = base.clone();
    exact.limits.nodes = 5;
    let support::Seen::Accepted(_) = support::seen(&env, &exact, &program)? else {
        return Err("expected acceptance at the node boundary".to_string());
    };
    let mut over = base.clone();
    over.limits.nodes = 4;
    let support::Seen::Exhausted(limit) = support::seen(&env, &over, &program)? else {
        return Err("expected node exhaustion".to_string());
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::Nodes);

    let mut deep = base.clone();
    deep.limits.depth = 1;
    let support::Seen::Accepted(_) = support::seen(&env, &deep, &program)? else {
        return Err("expected acceptance at the depth boundary".to_string());
    };
    let mut shallow = base.clone();
    shallow.limits.depth = 0;
    let support::Seen::Exhausted(limit) = support::seen(&env, &shallow, &program)? else {
        return Err("expected depth exhaustion".to_string());
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::Depth);

    let single = support::candidate(
        vec![support::lit_node(
            noble_kernel::untrusted::Lit::Unit,
            vec![],
        )],
        vec![0],
    );
    let mut work = support::request(vec![], vec![support::unit_ty()], &[]);
    work.limits.work = 8;
    let support::Seen::Accepted(_) = support::seen(&env, &work, &single)? else {
        return Err("expected acceptance at the work boundary".to_string());
    };
    work.limits.work = 4;
    let support::Seen::Exhausted(limit) = support::seen(&env, &work, &single)? else {
        return Err("expected work exhaustion".to_string());
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::Work);
    Ok(())
}

#[test]
fn diagnostics_report_order_shape_and_truncation() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let program = support::candidate(
        vec![
            support::lit_node(noble_kernel::untrusted::Lit::I64(7), vec![]),
            support::lit_node(
                noble_kernel::untrusted::Lit::Bool(true),
                vec![support::i64_ty()],
            ),
        ],
        vec![0, 1],
    );
    let ordered = support::request(vec![], vec![support::bool_ty(), support::i64_ty()], &[]);
    let support::Seen::Bad(diagnostic) = support::seen(&env, &ordered, &program)? else {
        return Err("expected an order diagnostic".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::StackOrder
    );
    assert_eq!(
        diagnostic.expected,
        vec![support::bool_ty(), support::i64_ty()]
    );
    assert_eq!(
        diagnostic.actual,
        vec![support::i64_ty(), support::bool_ty()]
    );

    let mut tiny = ordered.clone();
    tiny.limits.diagnostics = 1;
    let support::Seen::Bad(diagnostic) = support::seen(&env, &tiny, &program)? else {
        return Err("expected a truncated diagnostic".to_string());
    };
    assert!(diagnostic.truncated);
    assert!(diagnostic.expected.len() + diagnostic.actual.len() <= 1);
    Ok(())
}

#[test]
fn resource_eligibility_rejects_duplication() -> Result<(), String> {
    let mut env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    assert_eq!(env.defs.len(), 22);
    let maker = noble_kernel::contracts::Definition(22);
    env.defs.push(noble_kernel::words::Scheme {
        var_kinds: vec![noble_kernel::words::VariableKind::Stack],
        stack_in: vec![noble_kernel::shapes::StackPart::Stack(
            noble_kernel::words::Variable(0),
        )],
        stack_out: vec![
            noble_kernel::shapes::StackPart::Stack(noble_kernel::words::Variable(0)),
            noble_kernel::shapes::StackPart::Pattern(noble_kernel::shapes::Pattern::Resource(
                noble_kernel::contracts::FIXTURE_RESOURCE,
            )),
        ],
        effects: vec![],
    });
    env.kinds.push(noble_kernel::contracts::Behavior::Named);
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let program = support::candidate(
        vec![
            support::invocation(maker, vec![support::segment(vec![])]),
            support::invocation(
                DUP,
                vec![support::segment(vec![]), support::value(resource.clone())],
            ),
        ],
        vec![0, 1],
    );
    let support::Seen::Bad(diagnostic) = support::seen(
        &env,
        &support::request(vec![], vec![resource.clone()], &[]),
        &program,
    )?
    else {
        return Err("expected an eligibility rejection".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::Eligibility(resource)
    );
    Ok(())
}
