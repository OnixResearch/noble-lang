/// B-CHECK-06: eliminating `Sum<Resource<R>, I64>` exposes the resource
/// payload at its own type inside the branch program (the positive
/// control), while the sum itself stays non-`Data` and cannot be
/// duplicated (the negative control).
// r[verify VT-M3-01]
#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; accepts_resource_payload checks the positive control, then let-else and the eligibility assertion check resource-sum duplication rejects."
)]
fn sum_resource_payload_exposes_at_its_own_type() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let sum = noble_kernel::types::Ty::Sum(
        Box::new(resource.clone()),
        Box::new(noble_kernel::types::Ty::I64),
    );
    accepts_resource_payload(&env, &resource, &sum)?;

    // The sum stays non-Data: duplicating it violates eligibility.
    let duplicate = crate::support::candidate(
        vec![crate::support::invocation(
            crate::DUP,
            vec![
                crate::support::segment(vec![]),
                crate::support::value(sum.clone()),
            ],
        )],
        vec![0],
    );
    let crate::support::Seen::Bad(diagnostic) = crate::support::seen(
        &env,
        &crate::support::request(vec![sum.clone()], vec![sum.clone(), sum.clone()], &[]),
        &duplicate,
    )?
    else {
        return Err("the resource-bearing sum must not be duplicable".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::Eligibility(sum)
    );
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; let-else requires case acceptance and the assertion checks the exposed resource-typed branch interface; extra fixture assertions would duplicate this contract."
)]
fn accepts_resource_payload(
    env: &noble_kernel::contracts::Env,
    resource: &noble_kernel::types::Ty,
    sum: &noble_kernel::types::Ty,
) -> Result<(), String> {
    let case = noble_kernel::contracts::Definition(17);
    // The exposed branch consumes the resource payload directly.
    let payload_branch = noble_kernel::types::Ty::program(
        vec![resource.clone()],
        vec![noble_kernel::types::Ty::Unit],
        noble_kernel::types::EffSet::empty(),
    );
    let quiet_branch = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::Unit],
        noble_kernel::types::EffSet::empty(),
    );
    let program = crate::support::candidate(
        vec![crate::support::invocation(
            case,
            vec![
                crate::support::segment(vec![]),
                crate::support::value(resource.clone()),
                crate::support::value(noble_kernel::types::Ty::I64),
                crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
                crate::support::effect_binding(&[]),
                crate::support::effect_binding(&[]),
            ],
        )],
        vec![0],
    );
    let crate::support::Seen::Accepted(checked) = crate::support::seen(
        env,
        &crate::support::request(
            vec![sum.clone(), payload_branch, quiet_branch],
            vec![noble_kernel::types::Ty::Unit],
            &[],
        ),
        &program,
    )?
    else {
        return Err("case must accept the payload-exposure control".to_string());
    };
    // The exposed interface carries the payload branch verbatim: the
    // resource stands at its own type, never widened to `Data`.
    assert_eq!(
        checked.interface.stack_in.get(1),
        Some(&noble_kernel::types::Ty::program(
            vec![resource.clone()],
            vec![noble_kernel::types::Ty::Unit],
            noble_kernel::types::EffSet::empty(),
        ))
    );
    Ok(())
}

/// B-CHECK-06: no trusted refinements — the eliminator's witness must match
/// the eliminated structure exactly; a payload-order refinement
/// (`Sum<I64,Bool>` read as `Sum<Bool,I64>`) is a stack-join rejection,
/// not an identification.
// r[verify VT-M3-01]
#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; let-else requires rejection of the payload-swapped witness and the assertion checks StackJoin; no refinement is accepted implicitly."
)]
fn advertised_refinement_is_not_trusted() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let case = noble_kernel::contracts::Definition(17);
    let unit = noble_kernel::types::Ty::Unit;
    let (i64_ty, bool_ty) = (noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool);
    // The stack holds `Sum<I64, Bool>`; the witness advertises the
    // refinement `Sum<Bool, I64>` with branches swapped to match.
    let actual_sum =
        noble_kernel::types::Ty::Sum(Box::new(i64_ty.clone()), Box::new(bool_ty.clone()));
    let refined_left = noble_kernel::types::Ty::program(
        vec![bool_ty.clone()],
        vec![unit.clone()],
        noble_kernel::types::EffSet::empty(),
    );
    let refined_right = noble_kernel::types::Ty::program(
        vec![i64_ty.clone()],
        vec![unit.clone()],
        noble_kernel::types::EffSet::empty(),
    );
    let program = crate::support::candidate(
        vec![crate::support::invocation(
            case,
            vec![
                crate::support::segment(vec![]),
                crate::support::value(bool_ty.clone()),
                crate::support::value(i64_ty.clone()),
                crate::support::segment(vec![unit.clone()]),
                crate::support::effect_binding(&[]),
                crate::support::effect_binding(&[]),
            ],
        )],
        vec![0],
    );
    let crate::support::Seen::Bad(diagnostic) = crate::support::seen(
        &env,
        &crate::support::request(
            vec![actual_sum, refined_left, refined_right],
            vec![unit.clone()],
            &[],
        ),
        &program,
    )?
    else {
        return Err("the payload-swapped refinement must reject".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::StackJoin,
        "the checker identifies no refinement of the advertised sum"
    );
    Ok(())
}
