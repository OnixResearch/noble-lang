#[test]
fn public_constructor_cannot_expose_unexported_nominal_payload() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[], super::super::retained())
        .map_err(|e| format!("{e:?}"))?;
    let hidden=b"module ledger@1 [ opaque Secret I64 private opaque Wrapper Secret public export Wrapper export Wrapper.new ]";
    assert!(
        matches!(&session.prepare(hidden,&[],super::super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Link)
    );
    let hidden_arm=b"module ledger@1 [ opaque Secret I64 private variant Status Ready Secret public Failed Text public export Status export Status.Ready export Status.Failed ]";
    assert!(
        matches!(&session.prepare(hidden_arm,&[],super::super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Link)
    );
    Ok(())
}

#[test]
fn alias_rebinding_retains_each_checked_adapter_slot() -> Result<(), String> {
    let mut session = noble_contracts::source::ModuleSession::new(
        &[
            super::super::binding(super::super::Binding {
                version: 1,
                slot: 3,
                adapter: "version-A",
            }),
            super::super::binding(super::super::Binding {
                version: 2,
                slot: 7,
                adapter: "version-B",
            }),
        ],
        super::super::retained(),
    )
    .map_err(|e| format!("{e:?}"))?;
    session = super::super::commit(session,b"module ledger@1 [ require emit Text -- ! test.emit export handle def handle [ \"notice\" emit ] ]")?;
    session = super::super::commit(session,b"module ledger@2 [ require emit Text -- ! test.emit export handle def handle [ \"notice\" emit ] ]")?;
    session = super::super::commit(session, b"import ledger@1 as account")?;
    let retained = session
        .prepare(b"account.handle", &[], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    session = super::super::commit(session, b"import ledger@2 as account")?;
    let rebound = session
        .prepare(b"account.handle", &[], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    let selected =
        |prepared: &noble_contracts::source::ModulePrepared| -> Result<Vec<u32>, String> {
            let submission = prepared.submission().ok_or("checked submission missing")?;
            Ok(submission
                .definitions
                .iter()
                .flat_map(|definition| &definition.body.candidate.nodes)
                .filter_map(|node| {
                    if let noble_kernel::untrusted::Node::Invocation { def, .. } = node {
                        if let Some(noble_kernel::contracts::Behavior::BoundEmit(slot)) =
                            submission.environment.kind(*def)
                        {
                            return Some(slot);
                        }
                    }
                    None
                })
                .collect::<Vec<u32>>())
        };
    assert_eq!(selected(&retained)?, vec![3]);
    assert_eq!(selected(&rebound)?, vec![7]);
    Ok(())
}

#[test]
fn largest_module_version_survives_direct_and_imported_resolution() -> Result<(), String> {
    let mut session = noble_contracts::source::ModuleSession::new(&[], super::super::retained())
        .map_err(|e| format!("{e:?}"))?;
    session = super::super::commit(
        session,
        b"module ledger@4294967295 [ export get def get [ 17 ] ]",
    )?;
    let direct = session
        .prepare(b"ledger@4294967295.get", &[], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(direct.output(), &[noble_kernel::types::Ty::I64]);
    session = super::super::commit(session, b"import ledger@4294967295 as account")?;
    let imported = session
        .prepare(b"account.get", &[], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(imported.output(), direct.output());
    Ok(())
}

#[test]
fn scoped_generic_types_resolve_forward_ordinals_without_cycles() -> Result<(), String> {
    let mut session = noble_contracts::source::ModuleSession::new(&[], super::super::retained())
        .map_err(|e| format!("{e:?}"))?;
    session = super::super::commit(session,b"module ledger@1 [ opaque Composite Pair<Base,Sum<Bool,List<I64>>> private opaque Base I64 private export Composite ]")?;
    let composite = session
        .resolve_type("ledger@1.Composite")
        .map_err(|e| format!("{e:?}"))?;
    let prepared = session
        .prepare(b"", &[composite], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        prepared.kind(),
        noble_contracts::source::ModuleKind::Expression
    );
    assert!(prepared.submission().is_some());
    assert!(
        matches!(&session.prepare(b"module ledger@2 [ opaque Self Self private ]",&[],super::super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Resolve)
    );
    assert!(session
        .prepare(
            b"module ledger@2 [ opaque Bad Pair<I64, Bool> private ]",
            &[],
            super::super::LIMITS
        )
        .is_err());
    Ok(())
}

#[test]
fn adapter_selection_changes_linked_definition_not_module_schema() -> Result<(), String> {
    let source=b"module ledger@1 [ opaque Id I64 private export Id require emit Text -- ! test.emit export handle def handle [ \"event\" emit ] ]";
    let mut first = noble_contracts::source::ModuleSession::new(
        &[super::super::binding(super::super::Binding {
            version: 1,
            slot: 3,
            adapter: "version-A",
        })],
        super::super::retained(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut second = noble_contracts::source::ModuleSession::new(
        &[super::super::binding(super::super::Binding {
            version: 1,
            slot: 3,
            adapter: "version-B",
        })],
        super::super::retained(),
    )
    .map_err(|e| format!("{e:?}"))?;
    first = super::super::commit(first, source)?;
    second = super::super::commit(second, source)?;
    let first_type = first
        .resolve_type("ledger@1.Id")
        .map_err(|e| format!("{e:?}"))?;
    let second_type = second
        .resolve_type("ledger@1.Id")
        .map_err(|e| format!("{e:?}"))?;
    match (first_type, second_type) {
        (noble_kernel::types::Ty::Nominal(_, left), noble_kernel::types::Ty::Nominal(_, right)) => {
            assert_eq!(left, right)
        }
        _ => return Err("missing checked nominal schemas".into()),
    }
    let first = first
        .prepare(b"ledger@1.handle", &[], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    let second = second
        .prepare(b"ledger@1.handle", &[], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    let first = first.submission().ok_or("missing first linked program")?;
    let second = second.submission().ok_or("missing second linked program")?;
    assert_ne!(
        first
            .definitions
            .first()
            .ok_or("missing first checked body")?
            .identity,
        second
            .definitions
            .first()
            .ok_or("missing second checked body")?
            .identity
    );
    Ok(())
}

#[test]
fn local_definitions_resolve_forward_without_accepting_cycles() -> Result<(), String> {
    let mut session = noble_contracts::source::ModuleSession::new(&[], super::super::retained())
        .map_err(|e| format!("{e:?}"))?;
    session = super::super::commit(
        session,
        b"module ledger@1 [ export first def first [ second ] def second [ 2 ] ]",
    )?;
    let prepared = session
        .prepare(b"ledger@1.first", &[], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(prepared.output(), &[noble_kernel::types::Ty::I64]);
    let cycle = b"module ledger@2 [ export first def first [ second ] def second [ first ] ]";
    assert!(
        matches!(&session.prepare(cycle,&[],super::super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Resolve)
    );
    Ok(())
}

#[test]
fn aliases_do_not_silently_shadow_qualified_definitions() -> Result<(), String> {
    let mut session = noble_contracts::source::ModuleSession::new(&[], super::super::retained())
        .map_err(|e| format!("{e:?}"))?;
    session = super::super::commit(session, b"def account.get [ 1 ]")?;
    session = super::super::commit(session, b"module ledger@1 [ export get def get [ 2 ] ]")?;
    assert!(
        matches!(&session.prepare(b"import ledger@1 as account",&[],super::super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Resolve)
    );
    let mut other = noble_contracts::source::ModuleSession::new(&[], super::super::retained())
        .map_err(|e| format!("{e:?}"))?;
    other = super::super::commit(other, b"module ledger@1 [ export get def get [ 2 ] ]")?;
    other = super::super::commit(other, b"import ledger@1 as account")?;
    assert!(
        matches!(&other.prepare(b"def account.get [ 1 ]",&[],super::super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Resolve)
    );
    Ok(())
}

#[test]
fn full_bounded_module_source_is_counted_once() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[], super::super::retained())
        .map_err(|e| format!("{e:?}"))?;
    let mut source = String::from("module ledger@1 [ export get def get [ 1 # ");
    let closing = "\n ] ]";
    let padding = usize::try_from(super::super::LIMITS.bytes)
        .map_err(|_| "source byte ceiling does not fit usize".to_string())?
        .checked_sub(source.len())
        .and_then(|remaining| remaining.checked_sub(closing.len()))
        .ok_or_else(|| {
            "module fixture prefix and closing exceed source byte ceiling".to_string()
        })?;
    source.extend(std::iter::repeat_n('x', padding));
    source.push_str(closing);
    assert_eq!(source.len(), 65_536);
    let prepared = session
        .prepare(source.as_bytes(), &[], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(prepared.kind(), noble_contracts::source::ModuleKind::Module);
    source.push(' ');
    assert!(
        matches!(&session.prepare(source.as_bytes(),&[],super::super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Parse)
    );
    Ok(())
}
