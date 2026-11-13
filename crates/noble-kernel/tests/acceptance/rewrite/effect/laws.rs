#![expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the harness law compares interfaces retained by runtime kernel checks."
)]
//! Issue #64 effects-as-laws controls.
//!
//! The host derives the rewrite expectation from an independently accepted original. Effect sets
//! are upper bounds on effect identities, so kernel inclusion catches additions but cannot catch a
//! candidate that drops an effect identity; that second rule is harness-side equality.

const TEST_EMIT_WORD: noble_kernel::contracts::Definition = crate::TEST_EMIT;

fn pure_unit() -> noble_kernel::untrusted::Candidate {
    crate::support::candidate(
        vec![crate::support::lit_node(
            noble_kernel::untrusted::Lit::Unit,
            vec![],
        )],
        vec![0],
    )
}

fn emitted_unit() -> noble_kernel::untrusted::Candidate {
    crate::support::candidate(
        vec![
            crate::support::lit_node(noble_kernel::untrusted::Lit::Text, vec![]),
            crate::support::invocation(TEST_EMIT_WORD, vec![crate::support::segment(vec![])]),
        ],
        vec![0, 1],
    )
}

fn expected_from_original(
    original: &noble_kernel::untrusted::Checked,
    source_request: &noble_kernel::untrusted::Request,
) -> noble_kernel::untrusted::Request {
    noble_kernel::untrusted::Request {
        input_bytes: source_request.input_bytes,
        expected: noble_kernel::untrusted::Expected {
            stack_in: original.interface.stack_in.clone(),
            stack_out: original.interface.stack_out.clone(),
            allowed_effects: original.interface.effects.clone(),
        },
        limits: source_request.limits,
    }
}

fn accept_original(
    env: &noble_kernel::contracts::Env,
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
) -> Result<noble_kernel::untrusted::Checked, String> {
    let seen = crate::support::seen(env, request, candidate)?;
    if let crate::support::Seen::Accepted(checked) = seen {
        Ok(checked)
    } else {
        Err("original acceptance did not accept the candidate".to_string())
    }
}

fn effect_law_matches(
    original: &noble_kernel::untrusted::Checked,
    candidate: &noble_kernel::untrusted::Checked,
) -> Result<(), &'static str> {
    if original.interface.effects == candidate.interface.effects {
        Ok(())
    } else {
        Err("derived effect identities changed")
    }
}

#[test]
fn original_expectation_rejects_an_added_test_emit_effect() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|problem| format!("{problem:?}"))?;
    let source_request = crate::support::request(vec![], vec![crate::support::unit_ty()], &[]);
    let original = accept_original(&env, &source_request, &pure_unit())?;
    let request = expected_from_original(&original, &source_request);
    assert_eq!(request.expected.stack_out, original.interface.stack_out);
    assert_eq!(request.expected.allowed_effects, original.interface.effects);

    let crate::support::Seen::Bad(problem) = crate::support::seen(&env, &request, &emitted_unit())?
    else {
        return Err("the original effect-free expectation must reject test.emit".to_string());
    };
    assert_eq!(
        problem.constraint,
        noble_kernel::untrusted::Constraint::EffectInclusion(noble_kernel::contracts::TEST_EMIT)
    );
    Ok(())
}

#[test]
fn effect_equality_rejects_a_dropped_effect_with_the_same_unit_output() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|problem| format!("{problem:?}"))?;
    let source_request = crate::support::request(
        vec![],
        vec![crate::support::unit_ty()],
        &[noble_kernel::contracts::TEST_EMIT.0],
    );
    let original = accept_original(&env, &source_request, &emitted_unit())?;
    let request = expected_from_original(&original, &source_request);
    let crate::support::Seen::Accepted(candidate) =
        crate::support::seen(&env, &request, &pure_unit())?
    else {
        return Err("kernel inclusion allows the candidate's narrower effect bound".to_string());
    };
    assert_eq!(candidate.interface.stack_out, original.interface.stack_out);
    assert_ne!(candidate.interface.effects, original.interface.effects);
    assert_eq!(
        effect_law_matches(&original, &candidate),
        Err("derived effect identities changed"),
        "output-stack equality does not substitute for exact effect equality"
    );
    Ok(())
}

#[test]
fn changed_interface_is_refused_against_the_original() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|problem| format!("{problem:?}"))?;
    let source_request = crate::support::request(vec![], vec![crate::support::unit_ty()], &[]);
    let original = accept_original(&env, &source_request, &pure_unit())?;
    let request = expected_from_original(&original, &source_request);
    let candidate = crate::support::candidate(
        vec![crate::support::lit_node(
            noble_kernel::untrusted::Lit::Bool(true),
            vec![],
        )],
        vec![0],
    );
    let crate::support::Seen::Bad(problem) = crate::support::seen(&env, &request, &candidate)?
    else {
        return Err("the changed result stack must be refused".to_string());
    };
    assert_eq!(
        problem.constraint,
        noble_kernel::untrusted::Constraint::StackJoin
    );
    Ok(())
}

#[test]
fn exceeding_the_original_stack_height_is_exhausted() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|problem| format!("{problem:?}"))?;
    let mut source_request = crate::support::request(vec![], vec![crate::support::unit_ty()], &[]);
    source_request.limits.stack_height = 1;
    let original = accept_original(&env, &source_request, &pure_unit())?;
    let request = expected_from_original(&original, &source_request);
    let candidate = crate::support::candidate(
        vec![
            crate::support::lit_node(noble_kernel::untrusted::Lit::Unit, vec![]),
            crate::support::lit_node(noble_kernel::untrusted::Lit::Unit, vec![]),
        ],
        vec![0, 1],
    );
    let crate::support::Seen::Exhausted(limit) = crate::support::seen(&env, &request, &candidate)?
    else {
        return Err("the candidate must not exceed the original stack-height limit".to_string());
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::StackHeight);
    Ok(())
}

#[test]
fn candidate_output_claim_cannot_replace_the_original_expectation() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|problem| format!("{problem:?}"))?;
    let source_request = crate::support::request(vec![], vec![crate::support::unit_ty()], &[]);
    let original = accept_original(&env, &source_request, &pure_unit())?;
    let candidate = crate::support::candidate(
        vec![crate::support::lit_node(
            noble_kernel::untrusted::Lit::Bool(true),
            vec![],
        )],
        vec![0],
    );

    // This counterfactual shows that the candidate's own output shape is not an authority.
    let candidate_shaped = crate::support::request(vec![], vec![crate::support::bool_ty()], &[]);
    let crate::support::Seen::Accepted(_) =
        crate::support::seen(&env, &candidate_shaped, &candidate)?
    else {
        return Err("the deliberately self-shaped request should accept the fixture".to_string());
    };
    let original_shaped = expected_from_original(&original, &source_request);
    let crate::support::Seen::Bad(problem) =
        crate::support::seen(&env, &original_shaped, &candidate)?
    else {
        return Err("the original-derived interface must reject the candidate claim".to_string());
    };
    assert_eq!(
        problem.constraint,
        noble_kernel::untrusted::Constraint::StackJoin
    );
    Ok(())
}
