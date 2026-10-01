use noble_kernel::contracts::{GenericVariantDecl, NominalError};
use noble_kernel::types::{NominalShape, NominalTypeId, ResourceKind, Ty};
use noble_kernel::untrusted::{Constraint, Outcome};

fn family() -> GenericVariantDecl {
    GenericVariantDecl {
        id: NominalTypeId {
            module: 72,
            ordinal: 0,
        },
        payload_params: [0, 1],
        exported: true,
        public: [true, true],
    }
}

fn ready() -> Result<
    (
        noble_kernel::contracts::Env,
        noble_kernel::contracts::NominalOps,
    ),
    String,
> {
    let env = noble_kernel::contracts::environment().map_err(|error| format!("{error:?}"))?;
    env.declare_generic_variant(family())
        .map_err(|error| format!("{error:?}"))
}

fn instance(env: &noble_kernel::contracts::Env, first: Ty, second: Ty) -> Result<Ty, String> {
    env.generic_instance(family().id, [first, second])
        .ok_or_else(|| "failed to build generic instance".to_string())
}

fn accepted(
    env: &noble_kernel::contracts::Env,
    input: Vec<Ty>,
    output: Vec<Ty>,
    candidate: noble_kernel::untrusted::Candidate,
    effects: &[u32],
) -> Result<(), String> {
    let request = crate::support::request(input, output, effects);
    match noble_kernel::acceptance::check(env, &request, &candidate) {
        Outcome::Accepted(_) => Ok(()),
        result => Err(format!("expected accepted generic operation: {result:?}")),
    }
}

fn reject_type(env: &noble_kernel::contracts::Env, ty: Ty) -> Result<(), String> {
    let result = noble_kernel::acceptance::check(
        env,
        &crate::support::request(vec![ty], vec![], &[]),
        &crate::support::candidate(vec![], vec![]),
    );
    if matches!(result, Outcome::Invalid(ref problem) if problem.constraint == Constraint::InvalidType)
    {
        Ok(())
    } else {
        Err(format!("forged generic type was not rejected: {result:?}"))
    }
}

#[test]
fn generic_family_has_distinct_checked_ordered_instances_and_fresh_operations() -> Result<(), String>
{
    let (env, ops) = ready()?;
    let left = ops.left.ok_or("missing generic left constructor")?;
    let right = ops.right.ok_or("missing generic right constructor")?;
    let first = instance(&env, Ty::Bool, Ty::Text)?;
    let second = instance(&env, Ty::I64, Ty::Text)?;
    assert_ne!(first, second);
    let candidate = crate::support::candidate(
        vec![
            crate::support::invocation(
                left,
                vec![
                    crate::support::segment(vec![Ty::I64]),
                    crate::support::value(Ty::Bool),
                    crate::support::value(Ty::Text),
                ],
            ),
            crate::support::lit_node(
                noble_kernel::untrusted::Lit::Text,
                vec![Ty::I64, first.clone()],
            ),
            crate::support::invocation(
                right,
                vec![
                    crate::support::segment(vec![Ty::I64, first.clone()]),
                    crate::support::value(Ty::I64),
                    crate::support::value(Ty::Text),
                ],
            ),
        ],
        vec![0, 1, 2],
    );
    accepted(
        &env,
        vec![Ty::I64, Ty::Bool],
        vec![Ty::I64, first, second.clone()],
        candidate,
        &[],
    )?;
    let third = instance(&env, Ty::Bool, Ty::I64)?;
    accepted(
        &env,
        vec![Ty::I64],
        vec![second.clone(), third],
        crate::support::candidate(
            vec![
                crate::support::invocation(
                    left,
                    vec![
                        crate::support::segment(vec![]),
                        crate::support::value(Ty::I64),
                        crate::support::value(Ty::Text),
                    ],
                ),
                crate::support::lit_node(
                    noble_kernel::untrusted::Lit::Bool(true),
                    vec![second.clone()],
                ),
                crate::support::invocation(
                    left,
                    vec![
                        crate::support::segment(vec![second]),
                        crate::support::value(Ty::Bool),
                        crate::support::value(Ty::I64),
                    ],
                ),
            ],
            vec![0, 1, 2],
        ),
        &[],
    )
}

#[test]
fn declaration_controls_payload_order_not_family_spelling() -> Result<(), String> {
    let (env, _) = ready()?;
    let swapped = GenericVariantDecl {
        id: NominalTypeId {
            module: 72,
            ordinal: 1,
        },
        payload_params: [1, 0],
        exported: true,
        public: [true, true],
    };
    let (env, ops) = env
        .declare_generic_variant(swapped.clone())
        .map_err(|error| format!("{error:?}"))?;
    let ty = instance(&env, Ty::I64, Ty::Text)?;
    let reversed = env
        .generic_instance(swapped.id, [Ty::I64, Ty::Text])
        .ok_or("missing swapped instance")?;
    assert_ne!(ty, reversed);
    assert!(matches!(
        &reversed,
        Ty::GenericNominal(_, _, shape)
            if **shape == NominalShape::Variant(Box::new(Ty::Text), Box::new(Ty::I64))
    ));
    accepted(
        &env,
        vec![Ty::Text],
        vec![reversed],
        crate::support::candidate(
            vec![crate::support::invocation(
                ops.left.ok_or("missing swapped constructor")?,
                vec![
                    crate::support::segment(vec![]),
                    crate::support::value(Ty::I64),
                    crate::support::value(Ty::Text),
                ],
            )],
            vec![0],
        ),
        &[],
    )
}

#[test]
fn generic_constructor_visibility_preserves_module_boundary() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|error| format!("{error:?}"))?;
    let mut decl = family();
    decl.public = [true, false];
    let (mut env, ops) = env
        .declare_generic_variant(decl)
        .map_err(|error| format!("{error:?}"))?;
    let right = ops.right.ok_or("missing right constructor")?;
    let ty = instance(&env, Ty::I64, Ty::Text)?;
    let candidate = crate::support::candidate(
        vec![crate::support::invocation(
            right,
            vec![
                crate::support::segment(vec![]),
                crate::support::value(Ty::I64),
                crate::support::value(Ty::Text),
            ],
        )],
        vec![0],
    );
    let request = crate::support::request(vec![Ty::Text], vec![ty.clone()], &[]);
    let result = noble_kernel::acceptance::check(&env, &request, &candidate);
    assert!(
        matches!(result, Outcome::Invalid(ref problem)
            if problem.constraint == Constraint::PrivateDefinition(right)),
        "private generic right constructor leaked: {result:?}"
    );
    env.caller_module = Some(family().id.module);
    accepted(&env, vec![Ty::Text], vec![ty], candidate, &[])
}

#[test]
fn valid_program_and_syntax_payloads_are_data_even_with_resource_interfaces() -> Result<(), String>
{
    let (env, ops) = ready()?;
    let program = Ty::program(
        vec![Ty::Resource(ResourceKind(0))],
        vec![Ty::Unit],
        crate::support::ids(&[0]),
    );
    let family_type = env
        .generic_instance(family().id, [program.clone(), Ty::Text])
        .ok_or("valid effectful program interface was rejected as generic Data")?;
    assert!(family_type.is_data());
    accepted(
        &env,
        vec![program.clone()],
        vec![family_type],
        crate::support::candidate(
            vec![crate::support::invocation(
                ops.left.ok_or("missing generic left constructor")?,
                vec![
                    crate::support::segment(vec![]),
                    crate::support::value(program.clone()),
                    crate::support::value(Ty::Text),
                ],
            )],
            vec![0],
        ),
        &[],
    )?;
    let syntax_type = env
        .generic_instance(family().id, [Ty::Syntax, Ty::Text])
        .ok_or("Syntax was rejected as generic Data")?;
    accepted(
        &env,
        vec![Ty::Syntax],
        vec![syntax_type],
        crate::support::candidate(
            vec![crate::support::invocation(
                ops.left.ok_or("missing generic left constructor")?,
                vec![
                    crate::support::segment(vec![]),
                    crate::support::value(Ty::Syntax),
                    crate::support::value(Ty::Text),
                ],
            )],
            vec![0],
        ),
        &[],
    )?;
    let invalid_effect = Ty::program(vec![], vec![], crate::support::ids(&[1]));
    assert!(env
        .generic_instance(family().id, [invalid_effect.clone(), Ty::Text])
        .is_none());
    reject_type(
        &env,
        Ty::GenericNominal(
            family().id,
            Box::new([invalid_effect.clone(), Ty::Text]),
            Box::new(NominalShape::Variant(
                Box::new(invalid_effect),
                Box::new(Ty::Text),
            )),
        ),
    )?;
    let unknown_resource_interface = Ty::program(
        vec![Ty::Resource(ResourceKind(42))],
        vec![Ty::Unit],
        crate::support::ids(&[]),
    );
    assert!(env
        .generic_instance(family().id, [unknown_resource_interface, Ty::Text])
        .is_none());
    Ok(())
}

#[test]
fn existing_monomorphic_nominal_supports_valid_program_and_syntax_payloads() -> Result<(), String> {
    let (env, _) = ready()?;
    let program = Ty::program(
        vec![Ty::Resource(ResourceKind(0))],
        vec![Ty::Unit],
        crate::support::ids(&[0]),
    );
    let decl = noble_kernel::contracts::NominalDecl {
        id: NominalTypeId {
            module: 72,
            ordinal: 4,
        },
        shape: NominalShape::Variant(Box::new(program.clone()), Box::new(Ty::Syntax)),
        exported: true,
        public: [true, true],
    };
    let (env, ops) = env
        .declare_nominal(decl.clone())
        .map_err(|error| format!("valid legacy payload rejected: {error:?}"))?;
    let nominal = Ty::Nominal(decl.id, Box::new(decl.shape));
    accepted(
        &env,
        vec![program.clone()],
        vec![nominal.clone()],
        crate::support::candidate(
            vec![crate::support::invocation(
                ops.left.ok_or("missing monomorphic left constructor")?,
                vec![crate::support::segment(vec![])],
            )],
            vec![0],
        ),
        &[],
    )?;
    accepted(
        &env,
        vec![Ty::Syntax],
        vec![nominal],
        crate::support::candidate(
            vec![crate::support::invocation(
                ops.right.ok_or("missing monomorphic right constructor")?,
                vec![crate::support::segment(vec![])],
            )],
            vec![0],
        ),
        &[],
    )?;
    let nested = env
        .generic_instance(family().id, [Ty::I64, Ty::Text])
        .ok_or("missing nested generic type")?;
    let nested_program = Ty::program(vec![nested], vec![Ty::Bool], crate::support::ids(&[]));
    let nested_decl = noble_kernel::contracts::NominalDecl {
        id: NominalTypeId {
            module: 72,
            ordinal: 6,
        },
        shape: NominalShape::Opaque(Box::new(nested_program.clone())),
        exported: true,
        public: [true, true],
    };
    let (env, nested_ops) = env
        .declare_nominal(nested_decl.clone())
        .map_err(|error| format!("nested generic program rejected: {error:?}"))?;
    accepted(
        &env,
        vec![nested_program],
        vec![Ty::Nominal(nested_decl.id, Box::new(nested_decl.shape))],
        crate::support::candidate(
            vec![crate::support::invocation(
                nested_ops.new.ok_or("missing nested constructor")?,
                vec![crate::support::segment(vec![])],
            )],
            vec![0],
        ),
        &[],
    )?;
    let invalid = noble_kernel::contracts::NominalDecl {
        id: NominalTypeId {
            module: 72,
            ordinal: 5,
        },
        shape: NominalShape::Opaque(Box::new(Ty::program(
            vec![],
            vec![],
            crate::support::ids(&[1]),
        ))),
        exported: true,
        public: [true, true],
    };
    assert!(matches!(
        env.declare_nominal(invalid),
        Err(NominalError::InvalidRepresentation)
    ));
    Ok(())
}

#[test]
fn malformed_families_forged_shapes_and_resource_arguments_fail_closed() -> Result<(), String> {
    let (env, _) = ready()?;
    assert!(matches!(
        env.clone().declare_generic_variant(family()),
        Err(NominalError::DuplicateIdentity)
    ));
    assert!(matches!(
        env.clone()
            .declare_nominal(noble_kernel::contracts::NominalDecl {
                id: family().id,
                shape: NominalShape::Opaque(Box::new(Ty::I64)),
                exported: true,
                public: [true, true],
            }),
        Err(NominalError::DuplicateIdentity)
    ));
    let mut invalid = family();
    invalid.id.ordinal = 1;
    invalid.payload_params = [0, 0];
    assert!(matches!(
        env.clone().declare_generic_variant(invalid),
        Err(NominalError::InvalidRepresentation)
    ));
    let mut forged_env = env.clone();
    forged_env.generic_variants[0].payload_params = [0, 0];
    let result = noble_kernel::acceptance::check(
        &forged_env,
        &crate::support::request(vec![], vec![], &[]),
        &crate::support::candidate(vec![], vec![]),
    );
    assert!(
        matches!(result, Outcome::Invalid(ref problem) if problem.constraint == Constraint::InvalidType),
        "malformed family declaration was accepted: {result:?}"
    );
    let original = instance(&env, Ty::I64, Ty::Text)?;
    let Ty::GenericNominal(id, args, _) = original else {
        return Err("instance is not generic".to_string());
    };
    let forged_shape = Ty::GenericNominal(
        id,
        args.clone(),
        Box::new(NominalShape::Variant(Box::new(Ty::Text), Box::new(Ty::I64))),
    );
    assert!(!env.valid_generic_instance(&forged_shape));
    reject_type(&env, forged_shape)?;
    let forged_arguments = Ty::GenericNominal(
        id,
        Box::new([Ty::Text, Ty::I64]),
        Box::new(NominalShape::Variant(Box::new(Ty::I64), Box::new(Ty::Text))),
    );
    assert!(!env.valid_generic_instance(&forged_arguments));
    reject_type(&env, forged_arguments)?;
    let unknown_id = NominalTypeId {
        module: id.module,
        ordinal: id
            .ordinal
            .checked_add(1)
            .ok_or_else(|| "generic family ordinal has no successor".to_string())?,
    };
    if env.generic_variant(unknown_id).is_some() {
        return Err("successor generic family ordinal is registered, not unknown".to_string());
    }
    let unknown = Ty::GenericNominal(
        unknown_id,
        args,
        Box::new(NominalShape::Variant(Box::new(Ty::I64), Box::new(Ty::Text))),
    );
    reject_type(&env, unknown)?;
    let resource = Ty::Resource(ResourceKind(0));
    assert!(env
        .generic_instance(id, [resource.clone(), Ty::Text])
        .is_none());
    let nested_resource = Ty::List(Box::new(Ty::Pair(
        Box::new(Ty::I64),
        Box::new(resource.clone()),
    )));
    assert!(env
        .generic_instance(id, [Ty::I64, nested_resource.clone()])
        .is_none());
    let nested_forgery = Ty::GenericNominal(
        id,
        Box::new([Ty::I64, nested_resource.clone()]),
        Box::new(NominalShape::Variant(
            Box::new(Ty::I64),
            Box::new(nested_resource),
        )),
    );
    reject_type(&env, nested_forgery)?;
    let forged_resource = Ty::GenericNominal(
        id,
        Box::new([resource.clone(), Ty::Text]),
        Box::new(NominalShape::Variant(
            Box::new(resource),
            Box::new(Ty::Text),
        )),
    );
    reject_type(&env, forged_resource)?;
    Ok(())
}

#[test]
fn generic_declarations_reject_duplicate_ids_invalid_payload_and_nominal_collisions()
    -> Result<(), String>
{
    let (env, _) = ready()?;
    let request = crate::support::request(vec![], vec![], &[]);
    let empty = crate::support::candidate(vec![], vec![]);
    assert!(matches!(
        noble_kernel::acceptance::check(&env, &request, &empty),
        Outcome::Accepted(_)
    ));
    let mut duplicate = env.clone();
    duplicate.generic_variants.push(family());
    let mut invalid_payload = env.clone();
    invalid_payload.generic_variants[0].payload_params = [0, 0];
    let mut collision = env.clone();
    collision.nominals.push(noble_kernel::contracts::NominalDecl {
        id: family().id,
        shape: NominalShape::Opaque(Box::new(Ty::I64)),
        exported: true,
        public: [true, true],
    });
    let malformed_body = crate::support::candidate(vec![], vec![42]);
    for (name, forged) in [
        ("duplicate generic identity", duplicate),
        ("invalid payload parameters", invalid_payload),
        ("nominal/generic identity collision", collision),
    ] {
        let outcome = noble_kernel::acceptance::check(&forged, &request, &malformed_body);
        assert!(
            matches!(&outcome, Outcome::Invalid(problem) if problem.constraint == Constraint::InvalidType),
            "{name} must reject before a malformed candidate body: {outcome:?}"
        );
    }
    Ok(())
}

#[test]
fn generic_matcher_joins_both_callback_effect_bounds() -> Result<(), String> {
    let (env, ops) = ready()?;
    let matcher = ops.matcher.ok_or("missing generic matcher")?;
    let mut forged_env = env.clone();
    let slot = usize::try_from(matcher.0).map_err(|error| format!("{error:?}"))?;
    forged_env.defs[slot].effects.pop();
    let result = noble_kernel::acceptance::check(
        &forged_env,
        &crate::support::request(vec![], vec![], &[]),
        &crate::support::candidate(vec![], vec![]),
    );
    assert!(
        matches!(result, Outcome::Invalid(ref problem) if problem.constraint == Constraint::InvalidContract),
        "forged matcher contract omitted an effect: {result:?}"
    );
    let ty = instance(&env, Ty::I64, Ty::Text)?;
    for (left_effects, right_effects) in
        [(&[][..], &[][..]), (&[0][..], &[][..]), (&[][..], &[0][..])]
    {
        let input = vec![
            ty.clone(),
            Ty::program(
                vec![Ty::I64],
                vec![Ty::Bool],
                crate::support::ids(left_effects),
            ),
            Ty::program(
                vec![Ty::Text],
                vec![Ty::Bool],
                crate::support::ids(right_effects),
            ),
        ];
        let candidate = crate::support::candidate(
            vec![crate::support::invocation(
                matcher,
                vec![
                    crate::support::segment(vec![]),
                    crate::support::value(Ty::I64),
                    crate::support::value(Ty::Text),
                    crate::support::segment(vec![Ty::Bool]),
                    crate::support::effect_binding(left_effects),
                    crate::support::effect_binding(right_effects),
                ],
            )],
            vec![0],
        );
        let allowed = if left_effects.is_empty() && right_effects.is_empty() {
            &[][..]
        } else {
            &[0][..]
        };
        accepted(
            &env,
            input.clone(),
            vec![Ty::Bool],
            candidate.clone(),
            allowed,
        )?;
        if allowed.is_empty() {
            continue;
        }
        let result = noble_kernel::acceptance::check(
            &env,
            &crate::support::request(input, vec![Ty::Bool], &[]),
            &candidate,
        );
        assert!(
            matches!(result, Outcome::Invalid(ref problem)
                if problem.constraint == Constraint::EffectInclusion(noble_kernel::types::EffId(0))),
            "matcher omitted an arm effect: {result:?}"
        );
    }
    Ok(())
}
