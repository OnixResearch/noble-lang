//! Eligibility checks for polymorphic `Behavior::Named` definitions.
//!
//! Built-in `dup`, `drop`, and `quote` enforce `Data` on their fixed slots,
//! while user-defined `Behavior::Named` schemes may carry arbitrary rank-1
//! signatures in `Env`. Any value or stack variable used non-linearly
//! (duplicated, dropped, fabricated, or captured into a returned program)
//! must require `Data` at instantiation time; strictly linear polymorphic
//! schemes remain usable on resources.

use noble_kernel::contracts::{Behavior, Definition, Env, FIXTURE_RESOURCE};
use noble_kernel::shapes::{EffectSlot, Pattern};
use noble_kernel::types::{EffSet, Ty};
use noble_kernel::untrusted::{
    Candidate, Constraint, Node, NodeId, Outcome, CANDIDATE_FORMAT, SEMANTIC_REVISION,
};
use noble_kernel::words::{Binding, Inst, Scheme, Variable, VariableKind};

fn env_with_named(scheme: Scheme) -> Result<(Env, Definition), String> {
    let mut env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let def = Definition(
        u32::try_from(env.defs.len()).map_err(|_| "definition table overflow".to_string())?,
    );
    env.defs.push(scheme);
    env.kinds.push(Behavior::Named);
    env.deps.push(Vec::new());
    Ok((env, def))
}

fn pair_ty(left: Ty, right: Ty) -> Ty {
    Ty::Pair(Box::new(left), Box::new(right))
}

fn sum_ty(left: Ty, right: Ty) -> Ty {
    Ty::Sum(Box::new(left), Box::new(right))
}

fn resource() -> Ty {
    Ty::Resource(FIXTURE_RESOURCE)
}

fn stack_var(index: u32) -> Pattern {
    Pattern::StackVar(Variable(index))
}

fn value_var(index: u32) -> Pattern {
    Pattern::Var(Variable(index))
}

fn check_single(
    env: &Env,
    def: Definition,
    bindings: Vec<Binding>,
    stack_in: Vec<Ty>,
    stack_out: Vec<Ty>,
) -> Result<crate::support::Seen, String> {
    let program =
        crate::support::candidate(vec![crate::support::invocation(def, bindings)], vec![0]);
    crate::support::seen(
        env,
        &crate::support::request(stack_in, stack_out, &[]),
        &program,
    )
}

#[test]
fn named_duplication_scheme_rejects_resource_and_accepts_data() -> Result<(), String> {
    let scheme = Scheme {
        var_kinds: vec![VariableKind::Stack, VariableKind::Value],
        stack_in: vec![stack_var(0), value_var(1)],
        stack_out: vec![stack_var(0), value_var(1), value_var(1)],
        effects: vec![],
    };
    let (env, def) = env_with_named(scheme)?;
    let res = resource();
    let crate::support::Seen::Bad(diagnostic) = check_single(
        &env,
        def,
        vec![
            crate::support::segment(vec![]),
            crate::support::value(res.clone()),
        ],
        vec![res.clone()],
        vec![res.clone(), res.clone()],
    )?
    else {
        return Err("named S a -- S a a must reject Resource".to_string());
    };
    assert_eq!(diagnostic.constraint, Constraint::Eligibility(res));

    let crate::support::Seen::Accepted(_) = check_single(
        &env,
        def,
        vec![
            crate::support::segment(vec![]),
            crate::support::value(Ty::I64),
        ],
        vec![Ty::I64],
        vec![Ty::I64, Ty::I64],
    )?
    else {
        return Err("named S a -- S a a must accept I64".to_string());
    };
    Ok(())
}

#[test]
fn named_drop_and_projection_schemes_reject_resource_and_accept_data() -> Result<(), String> {
    let drop_scheme = Scheme {
        var_kinds: vec![VariableKind::Stack, VariableKind::Value],
        stack_in: vec![stack_var(0), value_var(1)],
        stack_out: vec![stack_var(0)],
        effects: vec![],
    };
    let (env, def) = env_with_named(drop_scheme)?;
    let res = resource();
    let crate::support::Seen::Bad(diagnostic) = check_single(
        &env,
        def,
        vec![
            crate::support::segment(vec![]),
            crate::support::value(res.clone()),
        ],
        vec![res.clone()],
        vec![],
    )?
    else {
        return Err("named S a -- S must reject Resource".to_string());
    };
    assert_eq!(diagnostic.constraint, Constraint::Eligibility(res.clone()));

    let proj_scheme = Scheme {
        var_kinds: vec![
            VariableKind::Stack,
            VariableKind::Value,
            VariableKind::Value,
        ],
        stack_in: vec![
            stack_var(0),
            Pattern::Pair(Box::new(value_var(1)), Box::new(value_var(2))),
        ],
        stack_out: vec![stack_var(0), value_var(1)],
        effects: vec![],
    };
    let (proj_env, proj_def) = env_with_named(proj_scheme)?;
    let pair_i64_res = pair_ty(Ty::I64, res.clone());
    let crate::support::Seen::Bad(proj_diag) = check_single(
        &proj_env,
        proj_def,
        vec![
            crate::support::segment(vec![]),
            crate::support::value(Ty::I64),
            crate::support::value(res.clone()),
        ],
        vec![pair_i64_res],
        vec![Ty::I64],
    )?
    else {
        return Err("project_first dropping Resource must reject".to_string());
    };
    assert_eq!(proj_diag.constraint, Constraint::Eligibility(res.clone()));

    let pair_res_i64 = pair_ty(res.clone(), Ty::I64);
    let crate::support::Seen::Accepted(_) = check_single(
        &proj_env,
        proj_def,
        vec![
            crate::support::segment(vec![]),
            crate::support::value(res.clone()),
            crate::support::value(Ty::I64),
        ],
        vec![pair_res_i64],
        vec![res],
    )?
    else {
        return Err("project_first keeping Resource and dropping I64 must accept".to_string());
    };
    Ok(())
}

#[test]
fn named_capture_fanout_fabricate_and_stack_dup_reject_resource() -> Result<(), String> {
    let res = resource();

    // Capture: S a -- S Program<R, R a, {}>
    let capture_scheme = Scheme {
        var_kinds: vec![
            VariableKind::Stack,
            VariableKind::Value,
            VariableKind::Stack,
        ],
        stack_in: vec![stack_var(0), value_var(1)],
        stack_out: vec![
            stack_var(0),
            Pattern::program(vec![stack_var(2)], vec![stack_var(2), value_var(1)], vec![]),
        ],
        effects: vec![],
    };
    let (env, def) = env_with_named(capture_scheme)?;
    let prog_ty = Ty::program(vec![], vec![res.clone()], EffSet::empty());
    let crate::support::Seen::Bad(diag) = check_single(
        &env,
        def,
        vec![
            crate::support::segment(vec![]),
            crate::support::value(res.clone()),
            crate::support::segment(vec![]),
        ],
        vec![res.clone()],
        vec![prog_ty],
    )?
    else {
        return Err("named capture scheme must reject Resource".to_string());
    };
    assert_eq!(diag.constraint, Constraint::Eligibility(res.clone()));

    // Fanout: S a Program<S a, S c, e> Program<S a, S d, e> -- S Pair<c, d> ! e
    let fanout_scheme = Scheme {
        var_kinds: vec![
            VariableKind::Stack,
            VariableKind::Value,
            VariableKind::Value,
            VariableKind::Value,
            VariableKind::Effect,
        ],
        stack_in: vec![
            stack_var(0),
            value_var(1),
            Pattern::program(
                vec![stack_var(0), value_var(1)],
                vec![stack_var(0), value_var(2)],
                vec![EffectSlot::Var(Variable(4))],
            ),
            Pattern::program(
                vec![stack_var(0), value_var(1)],
                vec![stack_var(0), value_var(3)],
                vec![EffectSlot::Var(Variable(4))],
            ),
        ],
        stack_out: vec![
            stack_var(0),
            Pattern::Pair(Box::new(value_var(2)), Box::new(value_var(3))),
        ],
        effects: vec![EffectSlot::Var(Variable(4))],
    };
    let (fenv, fdef) = env_with_named(fanout_scheme)?;
    let branch = Ty::program(vec![res.clone()], vec![Ty::I64], EffSet::empty());
    let crate::support::Seen::Bad(fdiag) = check_single(
        &fenv,
        fdef,
        vec![
            crate::support::segment(vec![]),
            crate::support::value(res.clone()),
            crate::support::value(Ty::I64),
            crate::support::value(Ty::I64),
            Binding::Effect(EffSet::empty()),
        ],
        vec![res.clone(), branch.clone(), branch],
        vec![pair_ty(Ty::I64, Ty::I64)],
    )?
    else {
        return Err("named fanout scheme must reject Resource".to_string());
    };
    assert_eq!(fdiag.constraint, Constraint::Eligibility(res.clone()));

    // Fabricate: S -- S a
    let fab_scheme = Scheme {
        var_kinds: vec![VariableKind::Stack, VariableKind::Value],
        stack_in: vec![stack_var(0)],
        stack_out: vec![stack_var(0), value_var(1)],
        effects: vec![],
    };
    let (fab_env, fab_def) = env_with_named(fab_scheme)?;
    let crate::support::Seen::Bad(fab_diag) = check_single(
        &fab_env,
        fab_def,
        vec![
            crate::support::segment(vec![]),
            crate::support::value(res.clone()),
        ],
        vec![],
        vec![res.clone()],
    )?
    else {
        return Err("named S -- S a must reject Resource".to_string());
    };
    assert_eq!(fab_diag.constraint, Constraint::Eligibility(res.clone()));

    // Stack-tail duplication: S -- S S
    let stack_dup_scheme = Scheme {
        var_kinds: vec![VariableKind::Stack],
        stack_in: vec![stack_var(0)],
        stack_out: vec![stack_var(0), stack_var(0)],
        effects: vec![],
    };
    let (senv, sdef) = env_with_named(stack_dup_scheme)?;
    let crate::support::Seen::Bad(sdiag) = check_single(
        &senv,
        sdef,
        vec![crate::support::segment(vec![res.clone()])],
        vec![res.clone()],
        vec![res.clone(), res.clone()],
    )?
    else {
        return Err("named S -- S S must reject stack containing Resource".to_string());
    };
    assert_eq!(sdiag.constraint, Constraint::Eligibility(res));
    Ok(())
}

#[test]
fn signed_recursive_duplication_scheme_rejects_resource() -> Result<(), String> {
    let scheme = Scheme {
        var_kinds: vec![VariableKind::Stack, VariableKind::Value],
        stack_in: vec![stack_var(0), value_var(1)],
        stack_out: vec![stack_var(0), value_var(1), value_var(1)],
        effects: vec![],
    };
    let (mut env, def) = env_with_named(scheme)?;
    let owner_module = 7_u64;
    env.caller_module = Some(owner_module);
    env.definition_owners = vec![None; env.defs.len()];
    env.definition_owners[def.0 as usize] = Some(owner_module);

    let res = resource();
    let owner_inst = Inst {
        bindings: vec![
            crate::support::segment(vec![]),
            crate::support::value(res.clone()),
        ],
    };
    let candidate = Candidate {
        format: CANDIDATE_FORMAT,
        revision: SEMANTIC_REVISION,
        nodes: vec![Node::SignedSelf {
            def,
            inst: owner_inst.clone(),
        }],
        body: vec![NodeId(0)],
    };
    let request = crate::support::request(vec![res.clone()], vec![res.clone(), res.clone()], &[]);
    let outcome = noble_kernel::acceptance::check_signed_structure(
        &env,
        &request,
        &candidate,
        def,
        &owner_inst,
    );
    let Outcome::Invalid(diag) = outcome else {
        return Err(format!(
            "signed self-recursive S a -- S a a must reject Resource, got {outcome:?}"
        ));
    };
    assert_eq!(diag.constraint, Constraint::Eligibility(res));
    Ok(())
}

#[test]
fn linear_polymorphic_named_schemes_accept_resource() -> Result<(), String> {
    let res = resource();

    // Identity: S a -- S a
    let id_scheme = Scheme {
        var_kinds: vec![VariableKind::Stack, VariableKind::Value],
        stack_in: vec![stack_var(0), value_var(1)],
        stack_out: vec![stack_var(0), value_var(1)],
        effects: vec![],
    };
    let (env, def) = env_with_named(id_scheme)?;
    let crate::support::Seen::Accepted(_) = check_single(
        &env,
        def,
        vec![
            crate::support::segment(vec![]),
            crate::support::value(res.clone()),
        ],
        vec![res.clone()],
        vec![res.clone()],
    )?
    else {
        return Err("linear S a -- S a must accept Resource".to_string());
    };

    // First: S Pair<a, b> Program<S a, S c, e> -- S Pair<c, b> ! e
    let first_scheme = Scheme {
        var_kinds: vec![
            VariableKind::Stack,
            VariableKind::Value,
            VariableKind::Value,
            VariableKind::Value,
            VariableKind::Effect,
        ],
        stack_in: vec![
            stack_var(0),
            Pattern::Pair(Box::new(value_var(1)), Box::new(value_var(2))),
            Pattern::program(
                vec![stack_var(0), value_var(1)],
                vec![stack_var(0), value_var(3)],
                vec![EffectSlot::Var(Variable(4))],
            ),
        ],
        stack_out: vec![
            stack_var(0),
            Pattern::Pair(Box::new(value_var(3)), Box::new(value_var(2))),
        ],
        effects: vec![EffectSlot::Var(Variable(4))],
    };
    let (fenv, fdef) = env_with_named(first_scheme)?;
    let step_prog = Ty::program(vec![res.clone()], vec![res.clone()], EffSet::empty());
    let crate::support::Seen::Accepted(_) = check_single(
        &fenv,
        fdef,
        vec![
            crate::support::segment(vec![]),
            crate::support::value(res.clone()),
            crate::support::value(res.clone()),
            crate::support::value(res.clone()),
            Binding::Effect(EffSet::empty()),
        ],
        vec![pair_ty(res.clone(), res.clone()), step_prog],
        vec![pair_ty(res.clone(), res.clone())],
    )?
    else {
        return Err("linear wire.first must accept Resource".to_string());
    };

    // Then/compose: S Program<A, B, e> Program<B, C, e> -- S Program<A, C, e>
    let then_scheme = Scheme {
        var_kinds: vec![
            VariableKind::Stack,
            VariableKind::Stack,
            VariableKind::Stack,
            VariableKind::Stack,
            VariableKind::Effect,
        ],
        stack_in: vec![
            stack_var(0),
            Pattern::program(
                vec![stack_var(1)],
                vec![stack_var(2)],
                vec![EffectSlot::Var(Variable(4))],
            ),
            Pattern::program(
                vec![stack_var(2)],
                vec![stack_var(3)],
                vec![EffectSlot::Var(Variable(4))],
            ),
        ],
        stack_out: vec![
            stack_var(0),
            Pattern::program(
                vec![stack_var(1)],
                vec![stack_var(3)],
                vec![EffectSlot::Var(Variable(4))],
            ),
        ],
        effects: vec![],
    };
    let (tenv, tdef) = env_with_named(then_scheme)?;
    let p_res = Ty::program(vec![res.clone()], vec![res.clone()], EffSet::empty());
    let crate::support::Seen::Accepted(_) = check_single(
        &tenv,
        tdef,
        vec![
            crate::support::segment(vec![]),
            crate::support::segment(vec![res.clone()]),
            crate::support::segment(vec![res.clone()]),
            crate::support::segment(vec![res.clone()]),
            Binding::Effect(EffSet::empty()),
        ],
        vec![p_res.clone(), p_res.clone()],
        vec![p_res],
    )?
    else {
        return Err("linear wire.then over Resource stack programs must accept".to_string());
    };

    // Phantom Sum branch: S a -- S Sum<a, b> with b = Resource
    let inl_scheme = Scheme {
        var_kinds: vec![
            VariableKind::Stack,
            VariableKind::Value,
            VariableKind::Value,
        ],
        stack_in: vec![stack_var(0), value_var(1)],
        stack_out: vec![
            stack_var(0),
            Pattern::Sum(Box::new(value_var(1)), Box::new(value_var(2))),
        ],
        effects: vec![],
    };
    let (ienv, idef) = env_with_named(inl_scheme)?;
    let crate::support::Seen::Accepted(_) = check_single(
        &ienv,
        idef,
        vec![
            crate::support::segment(vec![]),
            crate::support::value(Ty::I64),
            crate::support::value(res.clone()),
        ],
        vec![Ty::I64],
        vec![sum_ty(Ty::I64, res)],
    )?
    else {
        return Err("named inl with phantom Resource arm must accept".to_string());
    };
    Ok(())
}
