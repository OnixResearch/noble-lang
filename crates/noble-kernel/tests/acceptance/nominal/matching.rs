#[path = "matching/branches.rs"]
mod branches;

#[test]
fn variant_match_requires_both_typed_arms_and_all_public_external_visibility() -> Result<(), String>
{
    let mut env = required!(super::env(), "bootstrap");
    let private = super::variant(
        noble_kernel::types::NominalTypeId {
            module: 48,
            ordinal: 0,
        },
        noble_kernel::types::Ty::I64,
        noble_kernel::types::Ty::Text,
        [true, false],
    );
    let private_ops = declare!(env, private.clone(), "mixed variant");
    let ty = super::declared_type(&private);
    let matcher = required!(private_ops.matcher, "match");
    let request =
        super::super::support::request(vec![ty.clone()], vec![noble_kernel::types::Ty::I64], &[]);
    let complete = branches::match_candidate(matcher, ty.clone(), true, &[]);
    let outcome = super::check(&env, request.clone(), complete.clone());
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::PrivateDefinition(matcher)),
        "private variant matcher must be inaccessible externally: {outcome:?}"
    );
    env.caller_module = Some(48);
    assert!(matches!(
        super::check(&env, request.clone(), complete),
        noble_kernel::untrusted::Outcome::Accepted(_)
    ));
    let outcome = super::check(
        &env,
        request.clone(),
        branches::match_candidate(matcher, ty.clone(), false, &[]),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::StackJoin),
        "variant match without right arm must fail StackJoin: {outcome:?}"
    );
    let wrong_effect = branches::match_candidate(matcher, ty.clone(), true, &[0]);
    let outcome = super::check(&env, request.clone(), wrong_effect);
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::EffectInclusion(noble_kernel::types::EffId(0))),
        "undeclared match branch effect must be refused: {outcome:?}"
    );
    required!(
        check_public_variant(env, private, private_ops),
        "external visibility"
    );
    Ok(())
}

fn check_public_variant(
    mut env: noble_kernel::contracts::Env,
    private: noble_kernel::contracts::NominalDecl,
    private_ops: noble_kernel::contracts::NominalOps,
) -> Result<(), String> {
    let public = super::variant(
        noble_kernel::types::NominalTypeId {
            module: 48,
            ordinal: 1,
        },
        noble_kernel::types::Ty::I64,
        noble_kernel::types::Ty::Text,
        [true, true],
    );
    let public_ops = declare!(env, public.clone(), "public variant");
    env.caller_module = None;
    let request = super::super::support::request(
        vec![super::declared_type(&public)],
        vec![noble_kernel::types::Ty::I64],
        &[],
    );
    let accepted = super::check(
        &env,
        request,
        branches::match_candidate(
            required!(public_ops.matcher, "match"),
            super::declared_type(&public),
            true,
            &[],
        ),
    );
    match accepted {
        noble_kernel::untrusted::Outcome::Accepted(checked) => {
            assert_eq!(
                checked.interface.stack_in,
                vec![super::declared_type(&public)]
            );
            assert_eq!(
                checked.interface.stack_out,
                vec![noble_kernel::types::Ty::I64]
            );
            assert_eq!(
                checked.interface.effects,
                noble_kernel::types::EffSet::empty()
            );
        }
        other => return Err(format!("all-arm public match failed: {other:?}")),
    }
    required!(
        reject_public_order_and_private_right(&env, &private, private_ops, &public, public_ops),
        "public branch order"
    );
    Ok(())
}

fn reject_public_order_and_private_right(
    env: &noble_kernel::contracts::Env,
    private: &noble_kernel::contracts::NominalDecl,
    private_ops: noble_kernel::contracts::NominalOps,
    public: &noble_kernel::contracts::NominalDecl,
    public_ops: noble_kernel::contracts::NominalOps,
) -> Result<(), String> {
    let left_branch = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::I64],
        noble_kernel::types::EffSet::empty(),
    );
    let right_branch = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::Text],
        vec![noble_kernel::types::Ty::I64],
        noble_kernel::types::EffSet::empty(),
    );
    let outcome = super::check(
        env,
        super::super::support::request(
            vec![super::declared_type(public), right_branch, left_branch],
            vec![noble_kernel::types::Ty::I64],
            &[],
        ),
        super::single(
            required!(public_ops.matcher, "match"),
            vec![
                super::super::support::segment(vec![]),
                super::super::support::segment(vec![noble_kernel::types::Ty::I64]),
                super::super::support::effect_binding(&[]),
                super::super::support::effect_binding(&[]),
            ],
        ),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::StackOrder),
        "reversed variant branches must fail StackOrder: {outcome:?}"
    );
    let outcome = super::check(
        env,
        super::super::support::request(
            vec![noble_kernel::types::Ty::Text],
            vec![super::declared_type(private)],
            &[],
        ),
        super::single(
            required!(private_ops.right, "right"),
            vec![super::super::support::segment(vec![])],
        ),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::PrivateDefinition(required!(private_ops.right, "right"))),
        "private right constructor must be inaccessible externally: {outcome:?}"
    );
    Ok(())
}

#[test]
fn variant_match_derives_branch_effects_and_exact_ordered_result() -> Result<(), String> {
    let mut env = required!(super::env(), "bootstrap");
    let decl = super::variant(
        noble_kernel::types::NominalTypeId {
            module: 48,
            ordinal: 2,
        },
        noble_kernel::types::Ty::I64,
        noble_kernel::types::Ty::Text,
        [true, true],
    );
    let ops = declare!(env, decl.clone(), "variant");
    let (mut env, emit) = required!(
        env.declare_bound_emit(noble_kernel::contracts::BoundEmitRegistration {
            adapter_identity: "bound-test".to_owned(),
            adapter_slot: 4,
            owner: 48,
            input: vec![super::super::support::text_ty()],
            output: vec![],
            effects: super::super::support::ids(&[0]),
        }),
        "binding"
    );
    env.caller_module = Some(48);
    let ty = super::declared_type(&decl);
    let mut program =
        branches::match_candidate(required!(ops.matcher, "match"), ty.clone(), true, &[0]);
    if let noble_kernel::untrusted::Node::Quotation { body, .. } = &mut program.nodes[0] {
        *body = vec![
            noble_kernel::untrusted::NodeId(5),
            noble_kernel::untrusted::NodeId(6),
        ];
    } else {
        return Err("left arm is not a quotation".to_string());
    }
    program.nodes.push(super::super::support::lit_node(
        noble_kernel::untrusted::Lit::Text,
        vec![noble_kernel::types::Ty::I64],
    ));
    program.nodes.push(super::super::support::invocation(
        emit,
        vec![super::super::support::segment(vec![
            noble_kernel::types::Ty::I64,
        ])],
    ));
    let exact =
        super::super::support::request(vec![ty.clone()], vec![noble_kernel::types::Ty::I64], &[0]);
    match required!(
        super::super::support::seen(&env, &exact, &program),
        "typed match outcome"
    ) {
        super::super::support::Seen::Accepted(checked) => {
            assert_eq!(
                checked.interface.stack_out,
                vec![noble_kernel::types::Ty::I64]
            );
            assert_eq!(checked.interface.effects, super::super::support::ids(&[0]));
        }
        super::super::support::Seen::Bad(problem) => {
            return Err(format!("typed effect union failed: {problem:?}"));
        }
        super::super::support::Seen::Exhausted(limit) => {
            return Err(format!("typed match exhausted: {limit:?}"));
        }
    }
    reject_branch_effect_changes(&env, &decl, ty, program);
    Ok(())
}

fn reject_branch_effect_changes(
    env: &noble_kernel::contracts::Env,
    decl: &noble_kernel::contracts::NominalDecl,
    ty: noble_kernel::types::Ty,
    mut program: noble_kernel::untrusted::Candidate,
) {
    let outcome = super::check(
        env,
        super::super::support::request(vec![ty], vec![noble_kernel::types::Ty::I64], &[]),
        program.clone(),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::EffectInclusion(noble_kernel::types::EffId(0))),
        "missing request effect must be refused: {outcome:?}"
    );
    if let noble_kernel::untrusted::Node::Quotation { inst, .. } = &mut program.nodes[0] {
        inst.bindings[3] = super::super::support::effect_binding(&[]);
    }
    let outcome = super::check(
        env,
        super::super::support::request(
            vec![super::declared_type(decl)],
            vec![noble_kernel::types::Ty::I64],
            &[0],
        ),
        program,
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::EffectInclusion(noble_kernel::types::EffId(0))),
        "missing branch effect binding must be refused: {outcome:?}"
    );
}
