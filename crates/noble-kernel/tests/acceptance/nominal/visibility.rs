#[test]
fn exported_opaque_hides_private_representation_but_public_operations_cannot_expose_it(
) -> Result<(), String> {
    let mut env = required!(super::env(), "bootstrap");
    let hidden_id = noble_kernel::types::NominalTypeId {
        module: 39,
        ordinal: 0,
    };
    let mut hidden = super::opaque(hidden_id, noble_kernel::types::Ty::I64, false);
    hidden.exported = false;
    declare!(env, hidden.clone(), "private inner");
    let wrapper = super::opaque(
        noble_kernel::types::NominalTypeId {
            module: 39,
            ordinal: 1,
        },
        super::declared_type(&hidden),
        true,
    );
    let ops = declare!(env, wrapper.clone(), "exported wrapper");
    let wrapper_ty = super::declared_type(&wrapper);
    assert!(matches!(
        super::check(
            &env,
            super::super::support::request(vec![wrapper_ty.clone()], vec![wrapper_ty.clone()], &[]),
            super::super::support::candidate(vec![], vec![])
        ),
        noble_kernel::untrusted::Outcome::Accepted(_)
    ));
    let outcome = super::check(
        &env,
        super::super::support::request(vec![noble_kernel::types::Ty::I64], vec![wrapper_ty], &[]),
        super::single(
            required!(ops.new, "new"),
            vec![super::super::support::segment(vec![])],
        ),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::PrivateDefinition(required!(ops.new, "new"))),
        "public constructor exposed private representation: {outcome:?}"
    );
    let outcome = super::check(
        &env,
        super::super::support::request(vec![super::declared_type(&hidden)], vec![], &[]),
        super::super::support::candidate(vec![], vec![]),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::InvalidType),
        "private nominal leaked into external request: {outcome:?}"
    );
    required!(
        reject_variant_exposure(env, &hidden),
        "private variant payload"
    );
    Ok(())
}

fn reject_variant_exposure(
    mut env: noble_kernel::contracts::Env,
    hidden: &noble_kernel::contracts::NominalDecl,
) -> Result<(), String> {
    let variant = private_payload_variant(hidden);
    let variant_ops = declare!(env, variant.clone(), "mixed payload");
    let left = required!(variant_ops.left, "public left constructor");
    assert!(matches!(
        super::check(
            &env,
            super::super::support::request(
                vec![noble_kernel::types::Ty::I64],
                vec![super::declared_type(&variant)],
                &[],
            ),
            super::single(left, vec![super::super::support::segment(vec![])])
        ),
        noble_kernel::untrusted::Outcome::Accepted(_)
    ));
    let matcher = required!(variant_ops.matcher, "match");
    let outcome = super::check(
        &env,
        super::super::support::request(
            vec![super::declared_type(&variant)],
            vec![noble_kernel::types::Ty::I64],
            &[],
        ),
        private_payload_match(matcher),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::PrivateDefinition(matcher)),
        "variant matcher exposed private payload: {outcome:?}"
    );
    Ok(())
}

fn private_payload_variant(
    hidden: &noble_kernel::contracts::NominalDecl,
) -> noble_kernel::contracts::NominalDecl {
    super::variant(
        noble_kernel::types::NominalTypeId {
            module: 39,
            ordinal: 2,
        },
        noble_kernel::types::Ty::I64,
        super::declared_type(hidden),
        [true, true],
    )
}

fn private_payload_match(
    matcher: noble_kernel::contracts::Definition,
) -> noble_kernel::untrusted::Candidate {
    super::single(
        matcher,
        vec![
            super::super::support::segment(vec![]),
            super::super::support::segment(vec![noble_kernel::types::Ty::I64]),
            super::super::support::effect_binding(&[]),
            super::super::support::effect_binding(&[]),
        ],
    )
}

#[test]
fn forward_schema_dependencies_keep_source_ordinals_without_recursive_aliasing(
) -> Result<(), String> {
    let mut env = required!(super::env(), "bootstrap");
    let dependency = super::opaque(
        noble_kernel::types::NominalTypeId {
            module: 93,
            ordinal: 2,
        },
        noble_kernel::types::Ty::I64,
        true,
    );
    let owner = super::opaque(
        noble_kernel::types::NominalTypeId {
            module: 93,
            ordinal: 0,
        },
        noble_kernel::types::Ty::Pair(
            Box::new(super::declared_type(&dependency)),
            Box::new(noble_kernel::types::Ty::Text),
        ),
        true,
    );
    assert_eq!(
        env.clone().declare_nominal(owner.clone()).err(),
        Some(noble_kernel::contracts::NominalError::InvalidRepresentation)
    );
    declare!(env, dependency.clone(), "registered dependency");
    let ops = declare!(env, owner.clone(), "forward source ordinal");
    assert_eq!(
        env.clone().declare_nominal(owner.clone()).err(),
        Some(noble_kernel::contracts::NominalError::DuplicateIdentity)
    );
    assert!(matches!(
        super::check(
            &env,
            super::super::support::request(
                vec![noble_kernel::types::Ty::Pair(
                    Box::new(super::declared_type(&dependency)),
                    Box::new(noble_kernel::types::Ty::Text)
                )],
                vec![super::declared_type(&owner)],
                &[],
            ),
            super::single(
                required!(ops.new, "new"),
                vec![super::super::support::segment(vec![])]
            )
        ),
        noble_kernel::untrusted::Outcome::Accepted(_)
    ));
    Ok(())
}
