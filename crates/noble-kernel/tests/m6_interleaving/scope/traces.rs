#[test]
fn results_follow_attachment_order_for_both_completion_orders() -> Result<(), String> {
    for should_reverse in [false, true] {
        check_completion_order(should_reverse)?;
    }
    Ok(())
}

fn check_completion_order(should_reverse: bool) -> Result<(), String> {
    let mut table = super::support::table(2)?;
    let registry = super::registry::ScopeRegistry::shared();
    let mut scope = super::manager::Scope::new(
        super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        registry,
    );
    let left = super::scope_support::admit(&mut scope, &mut table, 1_u8)?;
    let right = super::scope_support::admit(&mut scope, &mut table, 2_u8)?;
    assert_eq!(table.observation().pending, 2);
    complete_children(should_reverse, &mut table, &mut scope, left, right)?;
    assert!(
        scope.results().is_empty(),
        "completion alone must not publish a scope result"
    );
    deliver_children(&mut table, &mut scope, left, right)?;
    settle_children(&mut table, &mut scope, left, right)?;
    Ok(())
}

fn complete_children(
    should_reverse: bool,
    table: &mut noble_kernel::async_tasks::Table,
    scope: &mut super::manager::Scope<u8>,
    left: super::manager::MemberRef,
    right: super::manager::MemberRef,
) -> Result<(), String> {
    if should_reverse {
        scope
            .complete(
                table,
                right,
                noble_kernel::async_tasks::Outcome::DomainError,
                super::support::completion(),
                22,
            )
            .map_err(|error| format!("{error:?}"))?;
        assert_eq!(
            scope.deliver(table, right, super::support::request().context),
            Err(super::manager::OperationError::Scope(
                super::manager::ScopeError::OutOfOrder
            ))
        );
        scope
            .complete(
                table,
                left,
                noble_kernel::async_tasks::Outcome::Success,
                super::support::completion(),
                11,
            )
            .map_err(|error| format!("{error:?}"))?;
    } else {
        scope
            .complete(
                table,
                left,
                noble_kernel::async_tasks::Outcome::Success,
                super::support::completion(),
                11,
            )
            .map_err(|error| format!("{error:?}"))?;
        scope
            .complete(
                table,
                right,
                noble_kernel::async_tasks::Outcome::DomainError,
                super::support::completion(),
                22,
            )
            .map_err(|error| format!("{error:?}"))?;
    }
    assert_eq!(table.observation().ready, 2);
    assert!(scope.results().is_empty());
    Ok(())
}

fn deliver_children(
    table: &mut noble_kernel::async_tasks::Table,
    scope: &mut super::manager::Scope<u8>,
    left: super::manager::MemberRef,
    right: super::manager::MemberRef,
) -> Result<noble_kernel::async_tasks::Observation, String> {
    scope
        .deliver(table, left, super::support::request().context)
        .map_err(|error| format!("{error:?}"))?;
    let after_left = table.observation();
    assert_eq!(after_left.delivered, 1);
    assert_eq!(after_left.ready, 1);
    assert_eq!(
        scope.deliver(table, left, super::support::request().context),
        Err(super::manager::OperationError::Scope(
            super::manager::ScopeError::AlreadyDelivered
        ))
    );
    scope
        .deliver(table, right, super::support::request().context)
        .map_err(|error| format!("{error:?}"))?;
    let after_delivery = table.observation();
    assert_eq!(after_delivery.delivered, 2);
    assert_eq!(
        scope.results(),
        vec![
            super::manager::ScopeResult {
                member: left,
                outcome: noble_kernel::async_tasks::Outcome::Success,
                marker: 11
            },
            super::manager::ScopeResult {
                member: right,
                outcome: noble_kernel::async_tasks::Outcome::DomainError,
                marker: 22
            },
        ]
    );
    Ok(after_delivery)
}

fn settle_children(
    table: &mut noble_kernel::async_tasks::Table,
    scope: &mut super::manager::Scope<u8>,
    left: super::manager::MemberRef,
    right: super::manager::MemberRef,
) -> Result<noble_kernel::async_tasks::Observation, String> {
    assert_eq!(scope.close(), Err(super::manager::ScopeError::Unsettled));
    super::scope_support::settle_delivered(table, scope, left)?;
    super::scope_support::settle_delivered(table, scope, right)?;
    scope.close().map_err(|error| format!("{error:?}"))?;
    let settled = table.observation();
    assert_eq!(settled.pending, 0);
    assert_eq!(settled.ready, 0);
    assert_eq!(settled.delivered, 2);
    assert_eq!(settled.retired, 0);
    assert_eq!(settled.outstanding_pins, 0);
    assert_eq!(
        settled.reserved,
        noble_kernel::async_tasks::Footprint::empty()
    );
    scope.close().map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        table.observation(),
        settled,
        "idempotent close has no table effects"
    );
    assert_eq!(
        scope.release(left),
        Err(super::manager::ScopeError::AlreadyReleased)
    );
    Ok(settled)
}

#[test]
fn refused_second_table_admission_preserves_first_child_and_caller_input() -> Result<(), String> {
    let mut table = super::support::table(1)?;
    let registry = super::registry::ScopeRegistry::shared();
    let mut scope = super::manager::Scope::new(
        super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        registry,
    );
    let first = super::scope_support::admit(&mut scope, &mut table, vec![1, 2, 3])?;
    let before = table.observation();
    let inputs = vec![7, 8, 9];
    let refused = scope.admit(&mut table, super::support::request(), inputs.clone());
    match refused {
        Err(super::manager::AdmissionError::Table(rejected)) => {
            assert_eq!(
                rejected.error,
                noble_kernel::async_tasks::Error::TaskCapacity
            );
            assert_eq!(rejected.input, inputs);
        }
        Err(super::manager::AdmissionError::Scope { error, input }) => {
            assert_eq!(input, inputs);
            return Err(format!("unexpected scope refusal: {error:?}"));
        }
        Ok(_) => return Err("second task admitted beyond table capacity".to_owned()),
    }
    assert_eq!(scope.members(), vec![first]);
    assert_eq!(table.observation(), before);
    scope
        .complete(
            &mut table,
            first,
            noble_kernel::async_tasks::Outcome::Success,
            super::support::completion(),
            1,
        )
        .map_err(|error| format!("{error:?}"))?;
    scope
        .deliver(&mut table, first, super::support::request().context)
        .map_err(|error| format!("{error:?}"))?;
    super::scope_support::settle_delivered(&mut table, &mut scope, first)?;
    scope.close().map_err(|error| format!("{error:?}"))?;
    assert_eq!(scope.results().len(), 1);
    assert_eq!(table.observation().delivered, 1);
    assert_eq!(table.observation().retired, 0);
    Ok(())
}

#[test]
fn oversized_completion_records_the_first_scope_failure() -> Result<(), String> {
    let mut table = super::support::table(1)?;
    let registry = super::registry::ScopeRegistry::shared();
    let mut scope = super::manager::Scope::new(
        super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        registry,
    );
    let member = super::scope_support::admit(&mut scope, &mut table, 1_u8)?;
    let mut completion = super::support::completion();
    completion.bytes = 17;
    let decision = scope
        .complete(
            &mut table,
            member,
            noble_kernel::async_tasks::Outcome::Success,
            completion,
            1,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        decision.record.failure,
        Some(noble_kernel::async_tasks::Failure::Budget)
    );
    assert_eq!(
        scope.primary_failure(),
        Some(noble_kernel::async_tasks::Failure::Budget)
    );
    Ok(())
}

#[test]
fn duplicate_completion_preserves_first_result_and_primary_failure() -> Result<(), String> {
    let mut table = super::support::table(2)?;
    let registry = super::registry::ScopeRegistry::shared();
    let mut scope = super::manager::Scope::new(
        super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        registry,
    );
    let cancelled = super::scope_support::admit(&mut scope, &mut table, 1_u8)?;
    let completed = super::scope_support::admit(&mut scope, &mut table, 2_u8)?;
    scope
        .cancel(&mut table, cancelled, super::support::request().context)
        .map_err(|error| format!("{error:?}"))?;
    let first = scope
        .complete(
            &mut table,
            completed,
            noble_kernel::async_tasks::Outcome::Success,
            super::support::completion(),
            22,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert!(first.accounting.completion_accepted);
    let failure = scope.primary_failure();
    let before_duplicate = table.observation();
    let duplicate = scope
        .complete(
            &mut table,
            completed,
            noble_kernel::async_tasks::Outcome::DomainError,
            super::support::completion(),
            99,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert!(!duplicate.accounting.completion_accepted);
    assert_eq!(table.observation(), before_duplicate);
    assert_eq!(scope.primary_failure(), failure);
    scope
        .deliver(&mut table, completed, super::support::request().context)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        scope.results(),
        vec![super::manager::ScopeResult {
            member: completed,
            outcome: noble_kernel::async_tasks::Outcome::Success,
            marker: 22,
        }]
    );
    Ok(())
}
