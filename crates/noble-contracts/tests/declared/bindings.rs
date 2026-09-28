#[test]
fn declarations_are_inert_and_retained_slots_are_exact() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[super::binding(super::Binding {
            version: 1,
            slot: 4,
            adapter: "version-A",
        })])
        .map_err(|e| format!("{e:?}"))?;
    let module=b"module ledger@1 [ opaque UserId I64 private variant Status Ready I64 public Failed Text private require emit Text -- ! test.emit export UserId export Status export handle def handle [ \"audit\" emit ] ]";
    let prepared = session
        .prepare(module, &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(prepared.kind(), noble_contracts::source::ModuleKind::Module);
    assert!(prepared.submission().is_none());
    assert_eq!(
        prepared
            .linked_binding()
            .ok_or("unlinked module")?
            .adapter_identity,
        "version-A"
    );
    let (next, outcome) = session.commit(prepared);
    outcome.map_err(|error| format!("{error:?}"))?;
    session = next;
    assert!(matches!(
        session
            .resolve_type("ledger@1.UserId")
            .map_err(|e| format!("{e:?}"))?,
        noble_kernel::types::Ty::Nominal(_, _)
    ));
    let imported = session
        .prepare(b"import ledger@1 as account", &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(imported.kind(), noble_contracts::source::ModuleKind::Import);
    assert!(imported.submission().is_none());
    let (next, outcome) = session.commit(imported);
    outcome.map_err(|error| format!("{error:?}"))?;
    session = next;
    let prepared = session
        .prepare(b"account.handle", &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    let submission = prepared.submission().ok_or("missing checked candidate")?;
    assert!(prepared.output().is_empty());
    assert_eq!(
        submission.request.expected.allowed_effects.as_slice(),
        &[noble_kernel::types::EffId(0)]
    );
    assert_eq!(submission.environment.bound_adapters.len(), 1);
    assert_eq!(
        submission.environment.bound_adapters[0].adapter_identity,
        "version-A"
    );
    assert_eq!(submission.environment.bound_adapters[0].adapter_slot, 4);
    assert!(session
        .prepare(b"account.Status.match", &[], super::LIMITS)
        .is_err());
    assert!(session
        .prepare(b"account.UserId.new", &[], super::LIMITS)
        .is_err());
    Ok(())
}

#[test]
fn explicit_exported_conversions_preserve_nominal_identity() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session,b"module ledger@1 [ opaque UserId I64 public opaque OrderId I64 public opaque Hidden I64 public export UserId export UserId.new export UserId.into export OrderId export OrderId.new export OrderId.into export Hidden ]")?;
    let accepted = session
        .prepare(
            b"4 ledger@1.UserId.new ledger@1.UserId.into",
            &[],
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(accepted.output(), &[noble_kernel::types::Ty::I64]);
    let rejected = session.prepare(
        b"4 ledger@1.UserId.new ledger@1.OrderId.into",
        &[],
        super::LIMITS,
    );
    assert!(matches!(&rejected,Err(e) if e.stage()==noble_contracts::source::Stage::Check));
    assert!(session
        .prepare(b"4 ledger@1.Hidden.new", &[], super::LIMITS)
        .is_err());
    Ok(())
}
