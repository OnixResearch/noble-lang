//! Effect latency, list collection and retained-text behavior across the session lifecycle.

#[test]
fn effects_are_latent_until_execution_and_union_both_composed_bodies() -> Result<(), String> {
    let session = noble_contracts::source::Session::new();
    let dropped = super::prepare(&session, b"[ \"audit\" test.emit ] drop")?;
    let dropped = dropped.submission().ok_or("expression has no submission")?;
    assert!(dropped.request.expected.allowed_effects.is_empty());
    let effectful = super::prepare(
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
    let accepted = super::prepare(
        &session,
        b"20 nil [ 1 + ] swap cons [ ] [ drop run ] list.case",
    )?;
    assert_eq!(accepted.output(), &[noble_kernel::types::Ty::I64]);
    match session.prepare(b"[ 1 ] nil cons [ true ] swap cons", &[], super::limits()) {
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
    let accepted = super::prepare(&session, b"\"# // \\u{1F642} \\u{65}\\u{301}\"")?;
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
        match session.prepare(unbound, &[], super::limits()) {
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
    super::define(&mut session, b"def call [ run ]")?;
    super::define(&mut session, b"def // [ 1 ]")?;
    assert_eq!(
        super::prepare(&session, b"41 [ // + ] call")?.output(),
        &[noble_kernel::types::Ty::I64]
    );
    Ok(())
}
