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
