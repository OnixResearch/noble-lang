#[test]
fn pin_ack_is_required_without_reserved_pins() -> Result<(), String> {
    let mut table = super::super::super::support::table(1)?;
    let mut scope = super::super::super::manager::Scope::new(
        super::super::super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        super::super::super::registry::ScopeRegistry::shared(),
    );
    let dropped = std::rc::Rc::new(std::cell::Cell::new(false));
    let mut request = super::super::super::support::request();
    request.pins = 0;
    let admission = scope
        .admit(
            &mut table,
            request,
            super::ScopeInputDrop(std::rc::Rc::clone(&dropped)),
        )
        .map_err(|failure| format!("{failure:?}"))?;
    let member = super::super::super::scope_support::attach(&mut scope, &table, admission)?;
    scope
        .complete(
            &mut table,
            member,
            noble_kernel::async_tasks::Outcome::Success,
            super::super::super::support::completion(),
            1,
        )
        .map_err(|error| format!("{error:?}"))?;
    scope
        .deliver(&mut table, member, request.context)
        .map_err(|error| format!("{error:?}"))?;
    scope
        .native_stopped(&mut table, member)
        .map_err(|error| format!("{error:?}"))?;
    let obligations = noble_kernel::async_tasks::Obligations {
        inputs: 4,
        results: 0,
        buffers: 5,
    };
    assert_eq!(
        scope.cleanup(&mut table, member, obligations),
        Err(super::super::super::manager::OperationError::Scope(
            super::super::super::manager::ScopeError::Unsettled
        ))
    );
    assert!(!dropped.get());
    assert_eq!(
        scope.finish(&mut table, member),
        Err(super::super::super::manager::OperationError::Scope(
            super::super::super::manager::ScopeError::Unsettled
        ))
    );
    let before_empty_settlement = table.observation();
    scope
        .settle_pins(&mut table, member, 0)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(table.observation(), before_empty_settlement);
    scope
        .cleanup(&mut table, member, obligations)
        .map_err(|error| format!("{error:?}"))?;
    assert!(dropped.get());
    scope
        .finish(&mut table, member)
        .map_err(|error| format!("{error:?}"))?;
    scope
        .release(member)
        .map_err(|error| format!("{error:?}"))?;
    scope.close().map_err(|error| format!("{error:?}"))?;
    Ok(())
}

#[test]
fn owned_input_is_released_after_ack() -> Result<(), String> {
    let mut table = super::super::super::support::table(1)?;
    let mut scope = super::super::super::manager::Scope::new(
        super::super::super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        super::super::super::registry::ScopeRegistry::shared(),
    );
    let dropped = std::rc::Rc::new(std::cell::Cell::new(false));
    let member = super::super::super::scope_support::admit(
        &mut scope,
        &mut table,
        super::ScopeInputDrop(std::rc::Rc::clone(&dropped)),
    )?;
    scope
        .complete(
            &mut table,
            member,
            noble_kernel::async_tasks::Outcome::Success,
            super::super::super::support::completion(),
            1,
        )
        .map_err(|error| format!("{error:?}"))?;
    scope
        .deliver(
            &mut table,
            member,
            super::super::super::support::request().context,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert!(!dropped.get());
    assert_eq!(
        scope.settle_pins(&mut table, member, 3),
        Err(super::super::super::manager::OperationError::Scope(
            super::super::super::manager::ScopeError::Unsettled
        ))
    );
    assert!(!dropped.get());
    scope
        .native_stopped(&mut table, member)
        .map_err(|error| format!("{error:?}"))?;
    let obligations = noble_kernel::async_tasks::Obligations {
        inputs: 4,
        results: 0,
        buffers: 5,
    };
    assert_refusal(&mut scope, &mut table, member, obligations, &dropped);
    scope
        .settle_pins(&mut table, member, 3)
        .map_err(|error| format!("{error:?}"))?;
    assert!(!dropped.get(), "input remains owned until cleanup succeeds");
    scope
        .cleanup(&mut table, member, obligations)
        .map_err(|error| format!("{error:?}"))?;
    assert!(dropped.get(), "cleanup must release the scope-owned input");
    scope
        .finish(&mut table, member)
        .map_err(|error| format!("{error:?}"))?;
    scope
        .release(member)
        .map_err(|error| format!("{error:?}"))?;
    scope.close().map_err(|error| format!("{error:?}"))?;
    Ok(())
}
fn assert_refusal<T>(
    scope: &mut super::super::super::manager::Scope<T>,
    table: &mut noble_kernel::async_tasks::Table,
    member: super::super::super::manager::MemberRef,
    obligations: noble_kernel::async_tasks::Obligations,
    dropped: &std::cell::Cell<bool>,
) {
    let before_refusal = table.observation();
    assert_eq!(
        table.cleanup(member.handle, obligations),
        Err(noble_kernel::async_tasks::Error::PinsOutstanding)
    );
    assert_eq!(table.observation(), before_refusal);
    assert!(
        !dropped.get(),
        "refused table cleanup must preserve the input"
    );
    assert_eq!(
        scope.cleanup(table, member, obligations),
        Err(super::super::super::manager::OperationError::Scope(
            super::super::super::manager::ScopeError::Unsettled
        ))
    );
    assert!(!dropped.get());
}
