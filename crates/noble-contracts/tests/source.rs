const fn limits() -> noble_contracts::Limits {
    noble_contracts::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        work: 2_000_000,
    }
}

fn prepare(
    session: &noble_contracts::source::Session,
    source: &[u8],
) -> Result<noble_contracts::source::Prepared, String> {
    session
        .prepare(source, &[], limits())
        .map_err(|error| format!("{error:?}"))
}

fn define(session: &mut noble_contracts::source::Session, source: &[u8]) -> Result<(), String> {
    let prepared = prepare(session, source)?;
    if !prepared.is_definition() || prepared.submission().is_some() {
        return Err(String::from(
            "declaration incorrectly exposes executable root",
        ));
    }
    session
        .commit(prepared)
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn named_uses_are_fresh_but_duplicated_programs_are_monomorphic() -> Result<(), String> {
    let mut session = noble_contracts::source::Session::new();
    define(&mut session, b"def copy [ dup ]")?;
    let accepted = prepare(&session, b"1 copy drop true copy drop")?;
    assert_eq!(
        accepted.output(),
        &[noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool]
    );
    let source = b"[ dup ] dup [ 1 swap run drop drop ] dip true swap run";
    match session.prepare(source, &[], limits()) {
        Err(error) => {
            assert!(matches!(
                error.stage(),
                noble_contracts::source::Stage::Check
            ));
            assert_eq!(
                error.diagnostic().kind,
                noble_contracts::DiagnosticKind::Invalid
            );
        }
        Ok(_) => {
            return Err(String::from(
                "dup independently polymorphized one Program value",
            ))
        }
    }
    Ok(())
}

#[test]
fn rebinding_preserves_saved_dependency_meaning() -> Result<(), String> {
    let mut session = noble_contracts::source::Session::new();
    define(&mut session, b"def value [ 1 ]")?;
    define(&mut session, b"def saved [ value ]")?;
    define(&mut session, b"def value [ true ]")?;
    let accepted = prepare(&session, b"saved value")?;
    assert_eq!(
        accepted.output(),
        &[noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool]
    );
    Ok(())
}

#[test]
fn canonical_definition_identity_ignores_names_and_formatting() -> Result<(), String> {
    let mut session = noble_contracts::source::Session::new();
    define(&mut session, b"def a [ 1 ]")?;
    define(&mut session, b"def b [1 # formatting is not semantics\n]")?;
    let prepared = prepare(&session, b"a b")?;
    let submission = prepared
        .submission()
        .ok_or("expression has no submission")?;
    match submission.definitions.as_slice() {
        [a, b] => assert_eq!(a.identity, b.identity),
        _ => return Err(String::from("missing concrete definition specializations")),
    }
    define(&mut session, b"def b [ true ]")?;
    let prepared = prepare(&session, b"a b")?;
    let submission = prepared
        .submission()
        .ok_or("expression has no submission")?;
    match submission.definitions.as_slice() {
        [a, b] => assert_ne!(a.identity, b.identity),
        _ => return Err(String::from("missing rebound definition specializations")),
    }
    assert_eq!(
        prepared.output(),
        &[noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool]
    );
    Ok(())
}

#[test]
fn stale_and_cross_namespace_declarations_cannot_commit() -> Result<(), String> {
    let mut session = noble_contracts::source::Session::new();
    let stale = prepare(&session, b"def stale [ 1 ]")?;
    define(&mut session, b"def current [ true ]")?;
    let generation = session.generation();
    match session.commit(stale) {
        Err(error) => assert!(matches!(
            error.stage(),
            noble_contracts::source::Stage::Acceptance
        )),
        Ok(()) => return Err(String::from("stale namespace preparation committed")),
    }
    assert_eq!(session.generation(), generation);
    assert!(session.prepare(b"stale", &[], limits()).is_err());
    assert_eq!(
        prepare(&session, b"current")?.output(),
        &[noble_kernel::types::Ty::Bool]
    );

    let mut other = noble_contracts::source::Session::new();
    define(&mut other, b"def current [ 2 ]")?;
    let foreign = prepare(&other, b"def inherited [ current ]")?;
    assert!(session.commit(foreign).is_err());
    assert!(session.prepare(b"inherited", &[], limits()).is_err());
    Ok(())
}

#[test]
fn generic_definitions_check_open_constraints_without_execution() -> Result<(), String> {
    let mut session = noble_contracts::source::Session::new();
    define(&mut session, b"def twice [ dup compose ]")?;
    assert_eq!(
        prepare(&session, b"20 [ 1 + ] twice run")?.output(),
        &[noble_kernel::types::Ty::I64]
    );
    let generation = session.generation();
    for invalid in [
        b"def inconsistent [ [ 1 ] [ [ ] [ ] if ] compose ]".as_slice(),
        b"def occurs [ dup run ]".as_slice(),
    ] {
        match session.prepare(invalid, &[], limits()) {
            Err(error) => {
                assert!(matches!(
                    error.stage(),
                    noble_contracts::source::Stage::Check
                ));
                assert_eq!(
                    error.diagnostic().kind,
                    noble_contracts::DiagnosticKind::Invalid
                );
            }
            Ok(_) => {
                return Err(String::from(
                    "invalid open generic constraints were accepted",
                ))
            }
        }
    }
    assert_eq!(session.generation(), generation);
    Ok(())
}

#[test]
fn conditional_branch_joins_reject_incompatible_outputs_with_located_shapes() -> Result<(), String>
{
    let session = noble_contracts::source::Session::new();
    for (source, expected_end, expected_shape, actual_shape) in [
        (
            b"true [ 1 ] [ \"x\" ] if".as_slice(),
            21,
            "Program<?stack -- ?stack Text>",
            "Program<?stack -- ?stack I64>",
        ),
        (
            b"true [ 1 ] [ 1 2 ] if".as_slice(),
            21,
            "Program<?stack -- ?stack I64 I64>",
            "Program<?stack I64 -- ?stack I64 I64>",
        ),
        (
            b"true [ 1 \"x\" ] [ \"x\" 1 ] if".as_slice(),
            27,
            "Program<?stack -- ?stack Text I64>",
            "Program<?stack -- ?stack I64 Text>",
        ),
    ] {
        let error = session
            .prepare(source, &[], limits())
            .expect_err("incompatible conditional outputs must not be accepted");
        assert_eq!(error.stage(), noble_contracts::source::Stage::Check);
        assert_eq!(
            error.diagnostic().kind,
            noble_contracts::DiagnosticKind::Invalid
        );
        let join = error
            .diagnostic()
            .join()
            .ok_or("branch rejection has no structured join shapes")?;
        assert_eq!(join.word, "if");
        assert_ne!(join.expected_stack, join.actual_stack);
        assert!(join.expected_stack.contains(expected_shape));
        assert!(join.actual_stack.contains(actual_shape));
        assert_eq!(
            error.diagnostic().span,
            noble_contracts::Span {
                start: expected_end - 2,
                end: expected_end,
            }
        );
    }
    assert_eq!(
        prepare(&session, b"true [ 1 ] [ 2 ] if")?.output(),
        &[noble_kernel::types::Ty::I64]
    );
    Ok(())
}

#[test]
fn typed_source_resource_input_rejects_dup_with_exact_eligibility() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let error = session
        .prepare(b"dup", core::slice::from_ref(&resource), limits())
        .expect_err("resource duplication must not produce a candidate");
    assert_eq!(error.stage(), noble_contracts::source::Stage::Acceptance);
    assert_eq!(
        error.diagnostic().kind,
        noble_contracts::DiagnosticKind::Invalid
    );
    let join = error
        .diagnostic()
        .join()
        .ok_or("resource rejection has no structured word diagnostic")?;
    assert_eq!(join.word, "dup");
    assert_eq!(join.expected_stack, "S Data");
    assert_eq!(join.actual_stack, format!("S {resource:?}"));
    assert_eq!(join.constraint, "eligibility:Data");
    assert_eq!(join.value_origin, None);
    assert_eq!(
        error.diagnostic().span,
        noble_contracts::Span { start: 0, end: 3 }
    );
    println!("DX-01 checked exact Resource<test.counter> source: {error:?}");
    Ok(())
}

#[test]
fn effects_are_latent_until_execution_and_union_both_composed_bodies() -> Result<(), String> {
    let session = noble_contracts::source::Session::new();
    let dropped = prepare(&session, b"[ \"audit\" test.emit ] drop")?;
    let dropped = dropped.submission().ok_or("expression has no submission")?;
    assert!(dropped.request.expected.allowed_effects.is_empty());
    let effectful = prepare(
        &session,
        b"[ \"first\" test.emit test.abort ] [ \"second\" test.emit ] compose run",
    )?;
    let effectful = effectful
        .submission()
        .ok_or("expression has no submission")?;
    assert_eq!(
        effectful.request.expected.allowed_effects.as_slice(),
        &[noble_kernel::types::EffId(0), noble_kernel::types::EffId(1)]
    );
    assert!(effectful.request.expected.stack_out.is_empty());
    Ok(())
}

#[test]
fn list_collection_does_not_generalize_program_elements() -> Result<(), String> {
    let session = noble_contracts::source::Session::new();
    let accepted = prepare(
        &session,
        b"20 nil [ 1 + ] swap cons [ ] [ drop run ] list.case",
    )?;
    assert_eq!(accepted.output(), &[noble_kernel::types::Ty::I64]);
    match session.prepare(b"[ 1 ] nil cons [ true ] swap cons", &[], limits()) {
        Err(error) => assert!(matches!(
            error.stage(),
            noble_contracts::source::Stage::Check
        )),
        Ok(_) => {
            return Err(String::from(
                "homogeneous program list erased incompatible outputs",
            ))
        }
    }
    Ok(())
}

#[test]
fn retained_text_is_decoded_without_normalization() -> Result<(), String> {
    let session = noble_contracts::source::Session::new();
    let accepted = prepare(&session, b"\"# // \\u{1F642} \\u{65}\\u{301}\"")?;
    let submission = accepted
        .submission()
        .ok_or("expression has no submission")?;
    let literal = submission
        .body
        .texts
        .first()
        .ok_or("missing decoded text")?;
    assert_eq!(literal.bytes, "# // \u{1f642} e\u{301}".as_bytes());
    assert_eq!(accepted.output(), &[noble_kernel::types::Ty::Text]);
    Ok(())
}

#[test]
fn unreserved_migration_spellings_only_work_after_explicit_binding() -> Result<(), String> {
    let mut session = noble_contracts::source::Session::without_test_hosts();
    for unbound in [
        b"call".as_slice(),
        b"reify".as_slice(),
        b"//".as_slice(),
        b"test.emit".as_slice(),
    ] {
        match session.prepare(unbound, &[], limits()) {
            Err(error) => assert!(matches!(
                error.stage(),
                noble_contracts::source::Stage::Resolve
            )),
            Ok(_) => {
                return Err(String::from(
                    "fresh namespace provided an undeclared alias or host",
                ))
            }
        }
    }
    define(&mut session, b"def call [ run ]")?;
    define(&mut session, b"def // [ 1 ]")?;
    assert_eq!(
        prepare(&session, b"41 [ // + ] call")?.output(),
        &[noble_kernel::types::Ty::I64]
    );
    Ok(())
}
