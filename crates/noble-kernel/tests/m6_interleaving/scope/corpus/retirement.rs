#[test]
fn late_completion_keeps_failure_and_hides_result_until_scope_settles() -> Result<(), String> {
    for cancel_before_sibling_completion in [false, true] {
        exercise_cancellation_order(cancel_before_sibling_completion)?;
    }
    Ok(())
}

fn exercise_cancellation_order(cancel_before_sibling_completion: bool) -> Result<(), String> {
    let mut table = super::support::table(2)?;
    let registry = super::registry::ScopeRegistry::shared();
    let mut scope = super::manager::Scope::new(
        super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        registry,
    );
    let completed = super::scope_support::admit(&mut scope, &mut table, 1_u8)?;
    let cancelled = super::scope_support::admit(&mut scope, &mut table, 2_u8)?;
    assert_eq!(scope.members().len(), 2);
    if !cancel_before_sibling_completion {
        complete_sibling(&mut table, &mut scope, completed)?;
    }
    scope
        .cancel(&mut table, cancelled, super::support::request().context)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        scope.primary_failure(),
        Some(noble_kernel::async_tasks::Failure::Cancelled)
    );
    if cancel_before_sibling_completion {
        complete_sibling(&mut table, &mut scope, completed)?;
    }
    check_late_cancelled_completion(&mut table, &mut scope, cancelled)?;
    settle_cancelled_scope(&mut table, &mut scope, completed, cancelled)
}

fn complete_sibling(
    table: &mut noble_kernel::async_tasks::Table,
    scope: &mut super::manager::Scope<u8>,
    completed: super::manager::MemberRef,
) -> Result<(), String> {
    scope
        .complete(
            table,
            completed,
            noble_kernel::async_tasks::Outcome::Success,
            super::support::completion(),
            41,
        )
        .map_err(|error| format!("{error:?}"))?;
    Ok(())
}

fn check_late_cancelled_completion(
    table: &mut noble_kernel::async_tasks::Table,
    scope: &mut super::manager::Scope<u8>,
    cancelled: super::manager::MemberRef,
) -> Result<(), String> {
    assert_eq!(
        scope.primary_failure(),
        Some(noble_kernel::async_tasks::Failure::Cancelled)
    );
    let late = scope
        .complete(
            table,
            cancelled,
            noble_kernel::async_tasks::Outcome::DomainError,
            super::support::completion(),
            99,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        late.record.state,
        noble_kernel::async_tasks::State::Retiring
    );
    assert_eq!(
        late.record.failure,
        Some(noble_kernel::async_tasks::Failure::Cancelled)
    );
    assert!(scope.results().is_empty());
    assert_eq!(
        scope.deliver(table, cancelled, super::support::request().context),
        Err(super::manager::OperationError::Scope(
            super::manager::ScopeError::Cancelled
        ))
    );
    Ok(())
}

fn settle_cancelled_scope(
    table: &mut noble_kernel::async_tasks::Table,
    scope: &mut super::manager::Scope<u8>,
    completed: super::manager::MemberRef,
    cancelled: super::manager::MemberRef,
) -> Result<(), String> {
    scope
        .deliver(table, completed, super::support::request().context)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        scope.results(),
        vec![super::manager::ScopeResult {
            member: completed,
            outcome: noble_kernel::async_tasks::Outcome::Success,
            marker: 41
        }]
    );
    super::scope_support::settle_delivered(table, scope, completed)?;
    scope
        .native_stopped(table, cancelled)
        .map_err(|error| format!("{error:?}"))?;
    scope
        .settle_pins(table, cancelled, 3)
        .map_err(|error| format!("{error:?}"))?;
    scope
        .cleanup(
            table,
            cancelled,
            noble_kernel::async_tasks::Obligations {
                inputs: 5,
                results: 3,
                buffers: 7,
            },
        )
        .map_err(|error| format!("{error:?}"))?;
    scope
        .finish(table, cancelled)
        .map_err(|error| format!("{error:?}"))?;
    scope
        .release(cancelled)
        .map_err(|error| format!("{error:?}"))?;
    scope.close().map_err(|error| format!("{error:?}"))?;
    assert_eq!(table.observation().delivered, 1);
    assert_eq!(table.observation().retired, 1);
    assert_eq!(table.observation().outstanding_pins, 0);
    Ok(())
}
