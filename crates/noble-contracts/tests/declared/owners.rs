//! Private-variant ownership rules for resource and variant fixtures.

#[test]
fn private_arm_is_owner_only_but_owner_eliminator_is_callable() -> Result<(), String> {
    let mut session = noble_contracts::source::ModuleSession::new(&[], super::retained())
        .map_err(|e| format!("{e:?}"))?;
    session = super::commit(session,b"module ledger@1 [ variant Status Ready I64 public Failed Text private export Status export Status.Ready export make export inspect def make [ \"err\" Status.Failed ] def inspect [ [ drop 1 ] [ drop 0 ] Status.match ] ]")?;
    let accepted = session
        .prepare(b"ledger@1.make ledger@1.inspect", &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(accepted.output(), &[noble_kernel::types::Ty::I64]);
    assert!(accepted.submission().is_some());
    assert!(session
        .prepare(b"ledger@1.Status.Failed", &[], super::LIMITS)
        .is_err());
    assert!(session
        .prepare(
            b"ledger@1.make [ drop 1 ] [ drop 0 ] ledger@1.Status.match",
            &[],
            super::LIMITS
        )
        .is_err());
    let missing_arm=b"module ledger@2 [ variant Status Ready I64 public Failed Text private export bad def bad [ 1 Status.Ready [ drop 1 ] Status.match ] ]";
    assert!(
        matches!(&session.prepare(missing_arm,&[],super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Check)
    );
    Ok(())
}
