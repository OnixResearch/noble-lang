#[test]
fn all_exported_public_arms_support_exact_two_branch_match() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session,b"module ledger@1 [ variant Status Ready I64 public Failed Text public export Status export Status.Ready export Status.Failed ]")?;
    let accepted = session
        .prepare(
            b"5 ledger@1.Status.Ready [ drop 1 ] [ drop 0 ] ledger@1.Status.match",
            &[],
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(accepted.output(), &[noble_kernel::types::Ty::I64]);
    assert!(accepted.submission().is_some());
    assert!(session
        .prepare(
            b"5 ledger@1.Status.Ready [ drop 1 ] ledger@1.Status.match",
            &[],
            super::LIMITS
        )
        .is_err());
    assert!(session
        .prepare(
            b"5 ledger@1.Status.Ready [ drop true ] [ drop 0 ] ledger@1.Status.match",
            &[],
            super::LIMITS
        )
        .is_err());
    Ok(())
}

#[test]
fn result_source_definitions_instantiate_at_distinct_named_uses() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session, noble_contracts::source::RESULT_LIBRARY_SOURCE)?;
    let integers = session
        .resolve_type("result@1.Result<I64,Text>")
        .map_err(|e| format!("{e:?}"))?;
    let texts = session
        .resolve_type("result@1.Result<Text,I64>")
        .map_err(|e| format!("{e:?}"))?;
    assert_ne!(
        integers, texts,
        "ordered type arguments distinguish family instances"
    );
    let incremented = session
        .prepare(
            b"[ 1 + ] result@1.map_ok",
            core::slice::from_ref(&integers),
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(incremented.output(), &[integers]);
    let retained = session
        .prepare(
            b"[ ] result@1.map_ok",
            core::slice::from_ref(&texts),
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(retained.output(), &[texts]);
    Ok(())
}

#[test]
fn result_library_source_is_pinned_and_invalid_callbacks_reject() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    let forged = b"module result@1 [ variant Result<A,E> Ok A public Err E public ]";
    let refused = session.prepare(forged, &[], super::LIMITS);
    assert!(matches!(
        refused,
        Err(error) if error.stage() == noble_contracts::source::Stage::Link
    ));
    let session = super::commit(session, noble_contracts::source::RESULT_LIBRARY_SOURCE)?;
    let ty = session
        .resolve_type("result@1.Result<I64,Text>")
        .map_err(|e| format!("{e:?}"))?;
    assert!(session
        .prepare(
            b"[ + ] result@1.map_ok",
            core::slice::from_ref(&ty),
            super::LIMITS,
        )
        .is_err());
    assert!(session
        .prepare(b"[ drop drop ] result@1.map_ok", &[ty], super::LIMITS)
        .is_err());
    assert!(session
        .resolve_type("result@1.Result<Resource<test.counter>,Text>")
        .is_err());
    Ok(())
}

#[test]
fn signed_constructor_produces_concrete_result_for_checked_program() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session, noble_contracts::source::RESULT_LIBRARY_SOURCE)?;
    session = super::commit(
        session,
        b"module result_cases@1 [ signature make_ok forall<S:stack> [ S -- S result@1.Result<I64,Text> ! pure ] export make_ok def make_ok [ 2 result@1.Result.Ok ] ]",
    )?;
    let accepted = session
        .prepare(
            b"result_cases@1.make_ok [ 1 + ] result@1.map_ok",
            &[],
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        accepted.output(),
        &[session
            .resolve_type("result@1.Result<I64,Text>")
            .map_err(|e| format!("{e:?}"))?]
    );
    assert!(session
        .prepare(
            b"module bad_cases@1 [ signature make_ok forall<S:stack> [ S -- S result@1.Result<Bool,Text> ! pure ] def make_ok [ 2 result@1.Result.Ok ] ]",
            &[],
            super::LIMITS,
        )
        .is_err());
    Ok(())
}

#[test]
fn local_concrete_nominal_signature_preserves_identity_at_calls() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    session = super::commit(
        session,
        b"module calc@1 [ opaque Cursor I64 public export Cursor export Cursor.new export Cursor.into signature advance forall<S:stack> [ S Cursor -- S Cursor ! pure ] export advance def advance [ Cursor.into 1 + Cursor.new ] ]",
    )?;
    let accepted = session
        .prepare(
            b"41 calc@1.Cursor.new calc@1.advance calc@1.Cursor.into",
            &[],
            super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(accepted.output(), &[noble_kernel::types::Ty::I64]);
    assert!(accepted.submission().is_some());

    session = super::commit(
        session,
        b"module other@1 [ opaque Cursor I64 public export Cursor export Cursor.new ]",
    )?;
    assert!(matches!(
        session.prepare(
            b"41 other@1.Cursor.new calc@1.advance",
            &[],
            super::LIMITS
        ),
        Err(error) if error.stage() == noble_contracts::source::Stage::Check
    ));
    assert!(matches!(
        session.prepare(
            b"module private_calc@1 [ opaque Cursor I64 private signature expose forall<S:stack> [ S Cursor -- S Cursor ! pure ] export expose def expose [ ] ]",
            &[],
            super::LIMITS
        ),
        Err(error) if error.stage() == noble_contracts::source::Stage::Link
    ));
    Ok(())
}

#[test]
fn exported_concrete_nominal_signature_preserves_source_module_identity() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    session = super::commit(
        session,
        b"module numbers@1 [ opaque Exact I64 public export Exact export Exact.new export Exact.into ]",
    )?;
    session = super::commit(
        session,
        b"module consumers@1 [ signature retain forall<S:stack> [ S numbers@1.Exact -- S numbers@1.Exact ! pure ] export retain def retain [ ] ]",
    )?;
    let accepted = session
        .prepare(
            b"42 numbers@1.Exact.new consumers@1.retain numbers@1.Exact.into",
            &[],
            super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(accepted.output(), &[noble_kernel::types::Ty::I64]);
    session = super::commit(
        session,
        b"module other_numbers@1 [ opaque Exact I64 public export Exact export Exact.new ]",
    )?;
    assert!(matches!(
        session.prepare(
            b"42 other_numbers@1.Exact.new consumers@1.retain",
            &[],
            super::LIMITS,
        ),
        Err(error) if error.stage() == noble_contracts::source::Stage::Check
    ));
    Ok(())
}

#[test]
fn first_class_program_is_a_checked_result_payload() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session, noble_contracts::source::RESULT_LIBRARY_SOURCE)?;
    session = super::commit(
        session,
        b"module program_cases@1 [ signature make_program forall<S:stack> [ S -- S result@1.Result<Program<I64,I64,pure>,Text> ! pure ] export make_program def make_program [ [ 1 + ] result@1.Result.Ok ] ]",
    )?;
    let payload = session
        .resolve_type("result@1.Result<Program<I64,I64,pure>,Text>")
        .map_err(|e| format!("{e:?}"))?;
    let constructed = session
        .prepare(b"program_cases@1.make_program", &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(constructed.output(), &[payload]);
    let mapped = session
        .prepare(
            b"program_cases@1.make_program [ 2 swap run ] result@1.map_ok",
            &[],
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        mapped.output(),
        &[session
            .resolve_type("result@1.Result<I64,Text>")
            .map_err(|e| format!("{e:?}"))?]
    );
    Ok(())
}

#[test]
fn reflected_syntax_is_a_checked_error_payload() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session, noble_contracts::source::RESULT_LIBRARY_SOURCE)?;
    session = super::commit(
        session,
        b"module syntax_cases@1 [ signature make_syntax forall<S:stack> [ S -- S result@1.Result<I64,Syntax> ! pure ] export make_syntax def make_syntax [ [ 1 + ] reflect result@1.Result.Err ] ]",
    )?;
    let selected = session
        .prepare(
            b"syntax_cases@1.make_syntax [ drop 11 ] [ drop 22 ] result@1.Result.match",
            &[],
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(selected.output(), &[noble_kernel::types::Ty::I64]);
    Ok(())
}

#[test]
fn universal_source_signatures_reject_narrowed_value_and_effect() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    let narrowed_value = b"module bad@1 [ signature identity forall<S:stack,A:value> [ S A -- S A ! pure ] export identity def identity [ drop 0 ] ]";
    assert!(matches!(
        session.prepare(narrowed_value, &[], super::LIMITS),
        Err(error) if error.stage() == noble_contracts::source::Stage::Check
    ));
    let session = noble_contracts::source::ModuleSession::new(&[super::binding(super::Binding {
        version: 1,
        slot: 3,
        adapter: "version-A",
    })])
    .map_err(|e| format!("{e:?}"))?;
    let narrowed_effect = b"module ledger@1 [ require emit Text -- ! test.emit signature leak forall<S:stack,eps:effect> [ S -- S ! eps ] export leak def leak [ \"event\" emit ] ]";
    assert!(matches!(
        session.prepare(narrowed_effect, &[], super::LIMITS),
        Err(error) if error.stage() == noble_contracts::source::Stage::Check
    ));
    Ok(())
}

#[path = "cases/part2.rs"]
mod part2;

#[path = "cases/part3.rs"]
mod part3;

#[path = "cases/part4.rs"]
mod part4;

#[path = "cases/part5.rs"]
mod part5;

#[path = "cases/part6.rs"]
mod part6;
