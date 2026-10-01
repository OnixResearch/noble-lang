#[test]
fn forged_environment_contracts_and_bound_operations_fail_before_execution() -> Result<(), String> {
    let mut env = required!(super::env(), "bootstrap");
    let decl = super::variant(
        noble_kernel::types::NominalTypeId {
            module: 31,
            ordinal: 0,
        },
        noble_kernel::types::Ty::I64,
        noble_kernel::types::Ty::Text,
        [true, true],
    );
    let ops = declare!(env, decl.clone(), "variant");
    assert_eq!(
        env.kind(required!(ops.matcher, "match")),
        Some(noble_kernel::contracts::Behavior::NominalMatch(decl.id))
    );
    assert!(
        matches!(
            super::check(
                &env,
                super::super::support::request(vec![], vec![], &[]),
                super::super::support::candidate(vec![], vec![])
            ),
            noble_kernel::untrusted::Outcome::Accepted(_)
        ),
        "intact nominal contracts must admit an empty program"
    );
    required!(
        reject_forged_nominals(&env, ops),
        "nominal contract integrity"
    );
    required!(
        reject_forged_extra_type(&env, &decl),
        "nominal descriptor integrity"
    );
    required!(
        reject_forged_bound_operations(env),
        "bound operation integrity"
    );
    Ok(())
}

#[test]
fn scripted_clock_contract_rejects_forged_rows_and_preserves_owner_visibility() -> Result<(), String> {
    use noble_kernel::contracts::{BoundClockRegistration, ClockDecision, ClockPlan, NominalError, TEST_CLOCK};
    use noble_kernel::types::Ty;
    let env = required!(super::env(), "bootstrap");
    let registration = || BoundClockRegistration {
        adapter_identity: "clock-v1".to_owned(),
        adapter_slot: 5,
        owner: 31,
        input: vec![],
        output: vec![Ty::I64],
        effects: super::super::support::ids(&[TEST_CLOCK.0]),
    };
    let mut incorrect = registration();
    incorrect.effects = super::super::support::ids(&[0]);
    assert_eq!(env.clone().declare_bound_clock(incorrect).err(),Some(NominalError::InvalidRepresentation));
    let mut incorrect = registration();
    incorrect.input.push(Ty::Unit);
    assert_eq!(env.clone().declare_bound_clock(incorrect).err(),Some(NominalError::InvalidRepresentation));
    let mut incorrect = registration();
    incorrect.output.clear();
    assert_eq!(env.clone().declare_bound_clock(incorrect).err(),Some(NominalError::InvalidRepresentation));
    let (mut env, clock) = required!(env.declare_bound_clock(registration()),"valid scripted clock");
    assert_eq!(env.kind(clock),Some(noble_kernel::contracts::Behavior::BoundClock(5)));
    let request=super::super::support::request(vec![],vec![Ty::I64], &[TEST_CLOCK.0]);
    let candidate=super::single(clock,vec![super::super::support::segment(vec![])]);
    super::rejected(super::check(&env,request.clone(),candidate.clone()),
        noble_kernel::untrusted::Constraint::PrivateDefinition(clock));
    env.caller_module=Some(31);
    assert!(matches!(super::check(&env,request.clone(),candidate.clone()),
        noble_kernel::untrusted::Outcome::Accepted(_)));
    for mutation in 0..5 {
        let mut forged=env.clone();
        match mutation {
            0=>forged.bound_adapters[0].input.push(Ty::Unit),
            1=>forged.bound_adapters[0].output.clear(),
            2=>forged.bound_adapters[0].effects=super::super::support::ids(&[0]),
            3=>forged.defs[usize::try_from(clock.0).unwrap()].effects.clear(),
            _=>forged.bound_adapters[0].adapter_slot=6,
        }
        super::rejected(super::check(&forged,request.clone(),candidate.clone()),
            noble_kernel::untrusted::Constraint::InvalidContract);
    }
    let plan=ClockPlan {operation:"test.clock",adapter_identity:"clock-v1",
        input:&[],output:&[Ty::I64],effects:&[TEST_CLOCK],allowed:true,script:&[42]};
    assert_eq!(plan.decide("test.clock",0),Ok(ClockDecision::Value(42)));
    assert_eq!(plan.decide("test.clock",1),Ok(ClockDecision::ScriptExhausted));
    assert_eq!(plan.decide("test.emit",0),Ok(ClockDecision::UnexpectedOperation));
    let denied=ClockPlan {allowed:false,..plan};
    assert_eq!(denied.decide("test.clock",0),Ok(ClockDecision::Denied));
    Ok(())
}

fn reject_forged_nominals(
    env: &noble_kernel::contracts::Env,
    ops: noble_kernel::contracts::NominalOps,
) -> Result<(), String> {
    let matcher = required!(ops.matcher, "match");
    let matcher_index = required!(usize::try_from(matcher.0), "match index");
    let mut bad = (*env).clone();
    bad.defs[matcher_index].effects.clear();
    let outcome = super::check(
        &bad,
        super::super::support::request(vec![], vec![], &[]),
        super::super::support::candidate(vec![], vec![]),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::InvalidContract),
        "forged matcher effects must invalidate contract: {outcome:?}"
    );
    let mut bad = (*env).clone();
    bad.nominals[0].shape = noble_kernel::types::NominalShape::Variant(
        Box::new(noble_kernel::types::Ty::Bool),
        Box::new(noble_kernel::types::Ty::Text),
    );
    let outcome = super::check(
        &bad,
        super::super::support::request(vec![], vec![], &[]),
        super::super::support::candidate(vec![], vec![]),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::InvalidContract),
        "forged variant shape must invalidate contract: {outcome:?}"
    );
    let mut bad = (*env).clone();
    bad.kinds[matcher_index] = noble_kernel::contracts::Behavior::Named;
    let outcome = super::check(
        &bad,
        super::super::support::request(vec![], vec![], &[]),
        super::super::support::candidate(vec![], vec![]),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::InvalidContract),
        "forged matcher behavior must invalidate contract: {outcome:?}"
    );
    let mut bad = (*env).clone();
    bad.kinds[22] = noble_kernel::contracts::Behavior::Named;
    let outcome = super::check(
        &bad,
        super::super::support::request(vec![], vec![], &[]),
        super::super::support::candidate(vec![], vec![]),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::InvalidContract),
        "forged ambient behavior must invalidate contract: {outcome:?}"
    );
    Ok(())
}

fn reject_forged_extra_type(
    env: &noble_kernel::contracts::Env,
    decl: &noble_kernel::contracts::NominalDecl,
) -> Result<(), String> {
    let mut bad = (*env).clone();
    bad.defs.push(forged_type_scheme(decl));
    bad.kinds.push(noble_kernel::contracts::Behavior::Named);
    bad.deps.push(vec![]);
    bad.definition_owners.push(None);
    let outcome = super::check(
        &bad,
        super::super::support::request(vec![], vec![], &[]),
        super::super::support::candidate(vec![], vec![]),
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::InvalidType),
        "forged nominal descriptor must be InvalidType: {outcome:?}"
    );
    Ok(())
}

fn forged_type_scheme(decl: &noble_kernel::contracts::NominalDecl) -> noble_kernel::words::Scheme {
    noble_kernel::words::Scheme {
        var_kinds: vec![],
        stack_in: vec![noble_kernel::shapes::Pattern::Nominal(
            decl.id,
            Box::new(noble_kernel::types::NominalShape::Variant(
                Box::new(noble_kernel::types::Ty::Bool),
                Box::new(noble_kernel::types::Ty::Text),
            )),
        )],
        stack_out: vec![],
        effects: vec![],
    }
}

fn reject_forged_bound_operations(env: noble_kernel::contracts::Env) -> Result<(), String> {
    assert_eq!(
        env.clone()
            .declare_bound_emit(noble_kernel::contracts::BoundEmitRegistration {
                adapter_identity: "wrong-effect".to_owned(),
                adapter_slot: 3,
                owner: 31,
                input: vec![noble_kernel::types::Ty::Text],
                output: vec![],
                effects: super::super::support::ids(&[]),
            })
            .err(),
        Some(noble_kernel::contracts::NominalError::InvalidRepresentation)
    );
    assert_eq!(
        env.clone()
            .declare_bound_emit(noble_kernel::contracts::BoundEmitRegistration {
                adapter_identity: "wrong-input".to_owned(),
                adapter_slot: 3,
                owner: 31,
                input: vec![noble_kernel::types::Ty::I64],
                output: vec![],
                effects: super::super::support::ids(&[0]),
            })
            .err(),
        Some(noble_kernel::contracts::NominalError::InvalidRepresentation)
    );
    let (mut env, bound) = required!(
        env.declare_bound_emit(noble_kernel::contracts::BoundEmitRegistration {
            adapter_identity: "adapter-A".to_owned(),
            adapter_slot: 3,
            owner: 31,
            input: vec![super::super::support::text_ty()],
            output: vec![],
            effects: super::super::support::ids(&[0]),
        }),
        "bound adapter"
    );
    assert_eq!(
        env.kind(bound),
        Some(noble_kernel::contracts::Behavior::BoundEmit(3))
    );
    let request = super::super::support::request(vec![noble_kernel::types::Ty::Text], vec![], &[0]);
    let candidate = super::single(bound, vec![super::super::support::segment(vec![])]);
    super::rejected(
        super::check(&env, request.clone(), candidate.clone()),
        noble_kernel::untrusted::Constraint::PrivateDefinition(bound),
    );
    env.caller_module = Some(31);
    assert!(matches!(
        super::check(&env, request.clone(), candidate.clone()),
        noble_kernel::untrusted::Outcome::Accepted(_)
    ));
    required!(
        reject_corrupt_bound_rows(&env, bound, &request, &candidate),
        "forged bound adapter"
    );
    required!(reject_ambient_emit(&env), "ambient emit visibility");
    Ok(())
}

fn reject_corrupt_bound_rows(
    env: &noble_kernel::contracts::Env,
    bound: noble_kernel::contracts::Definition,
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
) -> Result<(), String> {
    let mut forged = (*env).clone();
    forged.bound_adapters[0].adapter_slot = 4;
    let outcome = super::check(&forged, (*request).clone(), (*candidate).clone());
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::InvalidContract),
        "forged adapter slot must invalidate contract: {outcome:?}"
    );
    let mut forged = (*env).clone();
    forged.bound_adapters[0].effects = super::super::support::ids(&[]);
    let outcome = super::check(&forged, (*request).clone(), (*candidate).clone());
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::InvalidContract),
        "forged adapter effects must invalidate contract: {outcome:?}"
    );
    let mut forged = (*env).clone();
    forged.defs[required!(usize::try_from(bound.0), "bound index")]
        .stack_out
        .push(noble_kernel::shapes::Pattern::Unit);
    let outcome = super::check(&forged, (*request).clone(), (*candidate).clone());
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::InvalidContract),
        "forged adapter output must invalidate contract: {outcome:?}"
    );
    Ok(())
}

fn reject_ambient_emit(env: &noble_kernel::contracts::Env) -> Result<(), String> {
    let ambient = noble_kernel::contracts::Definition(22);
    let request = super::super::support::request(
        vec![noble_kernel::types::Ty::Text],
        vec![super::super::support::unit_ty()],
        &[0],
    );
    let candidate = super::single(ambient, vec![super::super::support::segment(vec![])]);
    let mut legacy = (*env).clone();
    for caller in [Some(31), None] {
        legacy.caller_module = caller;
        let outcome = super::check(&legacy, request.clone(), candidate.clone());
        assert!(
            matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
                if problem.constraint == noble_kernel::untrusted::Constraint::PrivateDefinition(ambient)),
            "ambient emit must be private for {caller:?}: {outcome:?}"
        );
    }
    let mut pure = required!(noble_kernel::contracts::environment(), "legacy environment");
    pure.declared_modules = true;
    let outcome = super::check(&pure, request, candidate);
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::PrivateDefinition(ambient)),
        "ambient emit must be private without a declared adapter: {outcome:?}"
    );
    let unreachable = super::super::support::candidate(
        vec![super::super::support::invocation(
            ambient,
            vec![super::super::support::segment(vec![])],
        )],
        vec![],
    );
    let outcome = super::check(
        &pure,
        super::super::support::request(vec![], vec![], &[]),
        unreachable,
    );
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::PrivateDefinition(ambient)),
        "unreachable ambient emit must still be private: {outcome:?}"
    );
    Ok(())
}
