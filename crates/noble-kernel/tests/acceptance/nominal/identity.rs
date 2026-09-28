#[test]
fn distinct_opaque_owners_and_exact_representation_are_checked() -> Result<(), String> {
    let mut env = required!(super::env(), "bootstrap");
    let id = noble_kernel::types::NominalTypeId {
        module: 17,
        ordinal: 0,
    };
    let other = noble_kernel::types::NominalTypeId {
        module: 17,
        ordinal: 1,
    };
    let first = super::opaque(id, noble_kernel::types::Ty::I64, false);
    let ops = declare!(env, first.clone(), "opaque declaration");
    let second = super::opaque(other, noble_kernel::types::Ty::I64, true);
    let other_ops = declare!(env, second.clone(), "distinct declaration");
    let first_ty = super::declared_type(&first);
    let second_ty = super::declared_type(&second);
    assert_ne!(first_ty, second_ty);
    assert_ne!(
        first_ty,
        noble_kernel::types::Ty::Nominal(
            noble_kernel::types::NominalTypeId {
                module: 18,
                ordinal: 0
            },
            Box::new(first.shape.clone())
        )
    );
    assert_eq!(first_ty.clone(), first_ty);
    assert_eq!(first_ty.size(), Some(2));
    let mut too_small = super::super::support::request(vec![], vec![], &[]);
    too_small.limits.type_size = 1;
    assert!(matches!(
        super::check(
            &env,
            too_small,
            super::super::support::candidate(vec![], vec![])
        ),
        noble_kernel::untrusted::Outcome::Exhausted(noble_kernel::untrusted::LimitKind::TypeSize)
    ));
    required!(
        check_opaque_operations(env, &first, &second, ops, other_ops),
        "opaque operation visibility"
    );
    Ok(())
}

#[test]
fn payload_conversion_counts_type_nodes_not_postorder_frames() -> Result<(), String> {
    let env = required!(super::env(), "bootstrap");
    let (env, operations) = required!(
        env.declare_nominal(super::opaque(
            noble_kernel::types::NominalTypeId {
                module: 31,
                ordinal: 0
            },
            nested_list(400),
            false,
        )),
        "bounded 401-node nominal payload"
    );
    assert!(operations.new.is_some());
    assert!(operations.into.is_some());

    assert_eq!(
        env.declare_nominal(super::opaque(
            noble_kernel::types::NominalTypeId {
                module: 31,
                ordinal: 1
            },
            nested_list(512),
            false,
        ))
        .err(),
        Some(noble_kernel::contracts::NominalError::InvalidRepresentation)
    );
    Ok(())
}

fn nested_list(depth: usize) -> noble_kernel::types::Ty {
    let mut payload = noble_kernel::types::Ty::I64;
    let mut index = 0;
    while index < depth {
        payload = noble_kernel::types::Ty::List(Box::new(payload));
        index += 1;
    }
    payload
}

fn check_opaque_operations(
    mut env: noble_kernel::contracts::Env,
    first: &noble_kernel::contracts::NominalDecl,
    second: &noble_kernel::contracts::NominalDecl,
    ops: noble_kernel::contracts::NominalOps,
    other_ops: noble_kernel::contracts::NominalOps,
) -> Result<(), String> {
    let id = first.id;
    let first_ty = super::declared_type(first);
    let second_ty = super::declared_type(second);
    let constructor = super::single(
        required!(ops.new, "new"),
        vec![super::super::support::segment(vec![])],
    );
    let request = super::super::support::request(
        vec![super::super::support::i64_ty()],
        vec![first_ty.clone()],
        &[],
    );
    super::rejected(
        super::check(&env, request.clone(), constructor.clone()),
        noble_kernel::untrusted::Constraint::PrivateDefinition(required!(ops.new, "new")),
    );
    env.caller_module = Some(id.module);
    assert!(matches!(
        super::check(&env, request.clone(), constructor.clone()),
        noble_kernel::untrusted::Outcome::Accepted(_)
    ));

    required!(
        rejects_fake_shapes(&env, id, &constructor),
        "opaque representation"
    );
    let wrong_identity =
        super::super::support::request(vec![noble_kernel::types::Ty::I64], vec![second_ty], &[]);
    super::rejected(
        super::check(&env, wrong_identity, constructor.clone()),
        noble_kernel::untrusted::Constraint::StackJoin,
    );
    let source =
        super::super::support::request(vec![first_ty], vec![noble_kernel::types::Ty::I64], &[]);
    assert!(matches!(
        super::check(
            &env,
            source,
            super::single(
                required!(ops.into, "into"),
                vec![super::super::support::segment(vec![])]
            )
        ),
        noble_kernel::untrusted::Outcome::Accepted(_)
    ));
    env.caller_module = None;
    assert!(matches!(
        super::check(
            &env,
            super::super::support::request(
                vec![noble_kernel::types::Ty::I64],
                vec![super::declared_type(second)],
                &[]
            ),
            super::single(
                required!(other_ops.new, "new"),
                vec![super::super::support::segment(vec![])]
            )
        ),
        noble_kernel::untrusted::Outcome::Accepted(_)
    ));
    Ok(())
}

fn rejects_fake_shapes(
    env: &noble_kernel::contracts::Env,
    id: noble_kernel::types::NominalTypeId,
    constructor: &noble_kernel::untrusted::Candidate,
) -> Result<(), String> {
    let wrong_shape = noble_kernel::types::Ty::Nominal(
        id,
        Box::new(noble_kernel::types::NominalShape::Opaque(Box::new(
            super::super::support::bool_ty(),
        ))),
    );
    let mismatch = super::super::support::request(
        vec![noble_kernel::types::Ty::I64],
        vec![wrong_shape.clone()],
        &[],
    );
    let outcome = super::check(env, mismatch, (*constructor).clone());
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::InvalidType),
        "forged nominal shape must be InvalidType, got {outcome:?}"
    );
    let outcome = super::check(
        env,
        super::super::support::request(
            vec![noble_kernel::types::Ty::I64],
            vec![noble_kernel::types::Ty::I64],
            &[],
        ),
        super::single(
            noble_kernel::contracts::Definition(0),
            vec![
                super::super::support::segment(vec![]),
                super::super::support::value(wrong_shape),
            ],
        ),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::InvalidType),
        "forged binding shape must be InvalidType, got {outcome:?}"
    );
    Ok(())
}

#[test]
fn first_declaration_failure_precedes_later_fault() -> Result<(), String> {
    let mut env = required!(super::env(), "bootstrap");
    for ordinal in 0..2 {
        let decl = super::opaque(
            noble_kernel::types::NominalTypeId {
                module: 79,
                ordinal,
            },
            noble_kernel::types::Ty::I64,
            true,
        );
        declare!(env, decl, "ordered nominal");
    }
    let mut request = super::super::support::request(vec![], vec![], &[]);
    request.limits.type_size = 0;
    let candidate = super::super::support::candidate(vec![], vec![]);
    let invalid = noble_kernel::types::NominalShape::Opaque(Box::new(
        noble_kernel::types::Ty::Resource(noble_kernel::types::ResourceKind(999)),
    ));

    let mut invalid_first = env.clone();
    invalid_first.nominals[0].shape = invalid.clone();
    let outcome = super::check(&invalid_first, request.clone(), candidate.clone());
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::InvalidType),
        "first invalid declaration must precede the later size limit: {outcome:?}"
    );

    env.nominals[1].shape = invalid;
    let outcome = super::check(&env, request, candidate);
    assert!(
        matches!(
            outcome,
            noble_kernel::untrusted::Outcome::Exhausted(
                noble_kernel::untrusted::LimitKind::TypeSize
            )
        ),
        "first oversized declaration must precede the later invalid one"
    );
    Ok(())
}
