#[derive(Clone, Copy)]
#[octet::sealed_enum]
enum Action {
    CancelSibling,
    CompleteSibling,
    CompleteCancelled,
    DeliverSibling,
}

impl Action {
    const fn lean(self) -> &'static str {
        match self {
            Self::CancelSibling => ".cancel right2",
            Self::CompleteSibling => ".succeed left1",
            Self::CompleteCancelled => ".fail right2",
            Self::DeliverSibling => ".deliver left1",
        }
    }
}

fn action_order(should_cancel_before_sibling_completion: bool) -> [Action; 4] {
    if should_cancel_before_sibling_completion {
        [
            Action::CancelSibling,
            Action::CompleteSibling,
            Action::CompleteCancelled,
            Action::DeliverSibling,
        ]
    } else {
        [
            Action::CompleteSibling,
            Action::CancelSibling,
            Action::CompleteCancelled,
            Action::DeliverSibling,
        ]
    }
}

struct CancellationProgress {
    model_steps: Vec<&'static str>,
    after_late_completion: Option<noble_kernel::async_tasks::Observation>,
    delivered: Option<noble_kernel::async_tasks::Obligations>,
}

struct CancellationActionContext<'a> {
    table: &'a mut noble_kernel::async_tasks::Table,
    scope: &'a mut super::super::manager::Scope<u8>,
    completed: super::super::manager::MemberRef,
    cancelled: super::super::manager::MemberRef,
    progress: &'a mut CancellationProgress,
}

pub(super) fn trace(
    should_cancel_before_sibling_completion: bool,
) -> Result<super::CancellationWitness, String> {
    let mut table = super::super::support::table(2)?;
    let registry = super::super::registry::ScopeRegistry::shared();
    let mut scope = super::super::manager::Scope::new(
        super::super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        registry,
    );
    let completed = super::super::scope_support::admit(&mut scope, &mut table, 1_u8)?;
    let cancelled = super::super::scope_support::admit(&mut scope, &mut table, 2_u8)?;
    let actions = action_order(should_cancel_before_sibling_completion);
    let mut progress = CancellationProgress {
        model_steps: vec![
            ".admission left1",
            ".attach left1",
            ".admission right2",
            ".attach right2",
        ],
        after_late_completion: None,
        delivered: None,
    };
    run_actions(
        actions,
        &mut CancellationActionContext {
            table: &mut table,
            scope: &mut scope,
            completed,
            cancelled,
            progress: &mut progress,
        },
    )?;
    let model_step_count = progress.model_steps.len();
    let model_trace = format!("[{}]", progress.model_steps.join(", "));
    let cancelled_record = table
        .snapshot(cancelled.handle.slot)
        .ok_or("cancelled member lost its table record")?;
    let after_late_completion = progress
        .after_late_completion
        .ok_or("cancellation trace did not complete its cancelled member")?;
    let delivered = progress
        .delivered
        .ok_or("cancellation trace did not deliver its sibling")?;
    let results = scope.results();
    assert_eq!(results.len(), 1, "only the completed sibling is delivered");
    assert_eq!(results[0].member, completed);
    assert_eq!(
        results[0].outcome,
        noble_kernel::async_tasks::Outcome::Success
    );
    assert_eq!(results[0].marker, 41);
    super::super::scope_support::settle_delivered(&mut table, &mut scope, completed)?;
    settle_cancelled_member(&mut table, &mut scope, cancelled)?;
    let primary_failure = scope.primary_failure();
    scope.close().map_err(|error| format!("{error:?}"))?;
    let settled = table.observation();
    let witness = super::CancellationWitness {
        model_trace,
        results,
        primary_failure,
        cancelled_record,
        delivered,
        after_late_completion,
        settled,
    };
    assert_eq!(model_step_count, 8);
    validate_accounting(&witness);
    Ok(witness)
}

fn run_actions(
    actions: [Action; 4],
    context: &mut CancellationActionContext<'_>,
) -> Result<(), String> {
    for action in actions {
        context.progress.model_steps.push(action.lean());
        apply_action(action, context)?;
    }
    Ok(())
}

fn settle_cancelled_member(
    table: &mut noble_kernel::async_tasks::Table,
    scope: &mut super::super::manager::Scope<u8>,
    member: super::super::manager::MemberRef,
) -> Result<(), String> {
    scope
        .native_stopped(table, member)
        .map_err(|error| format!("{error:?}"))?;
    scope
        .settle_pins(table, member, 3)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(table.observation().outstanding_pins, 0);
    scope
        .cleanup(
            table,
            member,
            noble_kernel::async_tasks::Obligations {
                inputs: 5,
                results: 3,
                buffers: 7,
            },
        )
        .map_err(|error| format!("{error:?}"))?;
    scope
        .finish(table, member)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(table.observation().retired, 1);
    scope
        .release(member)
        .map_err(|error| format!("{error:?}"))?;
    Ok(())
}

fn validate_accounting(witness: &super::CancellationWitness) {
    assert_eq!(
        witness.primary_failure,
        Some(noble_kernel::async_tasks::Failure::Cancelled)
    );
    assert_eq!(
        witness.cancelled_record.state,
        noble_kernel::async_tasks::State::Retiring
    );
    assert_eq!(
        witness.cancelled_record.failure,
        Some(noble_kernel::async_tasks::Failure::Cancelled)
    );
    assert_eq!(witness.cancelled_record.pins, 3);
    assert_eq!(witness.after_late_completion.retiring, 1);
    assert_eq!(
        witness.delivered,
        noble_kernel::async_tasks::Obligations {
            inputs: 1,
            results: 3,
            buffers: 2,
        }
    );
    assert_eq!(witness.settled.delivered, 1);
    assert_eq!(witness.settled.retired, 1);
    assert_eq!(witness.settled.outstanding_pins, 0);
    assert_eq!(
        witness.settled.reserved,
        noble_kernel::async_tasks::Footprint::empty()
    );
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this dispatcher applies runtime M6 table transitions and mutates the cancellation trace witness."
)]
fn apply_action(action: Action, context: &mut CancellationActionContext<'_>) -> Result<(), String> {
    match action {
        Action::CancelSibling => revoke_child(context),
        Action::CompleteSibling => complete_sibling(context),
        Action::CompleteCancelled => complete_retiring_child(context),
        Action::DeliverSibling => deliver_sibling(context),
    }
}

fn revoke_child(context: &mut CancellationActionContext<'_>) -> Result<(), String> {
    let decision = context
        .scope
        .cancel(
            context.table,
            context.cancelled,
            super::super::support::request().context,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        decision.record.failure,
        Some(noble_kernel::async_tasks::Failure::Cancelled)
    );
    assert_eq!(
        context.scope.primary_failure(),
        Some(noble_kernel::async_tasks::Failure::Cancelled)
    );
    Ok(())
}

fn complete_sibling(context: &mut CancellationActionContext<'_>) -> Result<(), String> {
    let decision = context
        .scope
        .complete(
            context.table,
            context.completed,
            noble_kernel::async_tasks::Outcome::Success,
            super::super::support::completion(),
            41,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        decision.record.state,
        noble_kernel::async_tasks::State::Ready
    );
    assert!(context.scope.results().is_empty());
    Ok(())
}

fn complete_retiring_child(context: &mut CancellationActionContext<'_>) -> Result<(), String> {
    let decision = context
        .scope
        .complete(
            context.table,
            context.cancelled,
            noble_kernel::async_tasks::Outcome::DomainError,
            super::super::support::completion(),
            99,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        decision.record.state,
        noble_kernel::async_tasks::State::Retiring
    );
    assert_eq!(
        decision.record.failure,
        Some(noble_kernel::async_tasks::Failure::Cancelled)
    );
    context.progress.after_late_completion = Some(context.table.observation());
    Ok(())
}

fn deliver_sibling(context: &mut CancellationActionContext<'_>) -> Result<(), String> {
    let decision = context
        .scope
        .deliver(
            context.table,
            context.completed,
            super::super::support::request().context,
        )
        .map_err(|error| format!("{error:?}"))?;
    let accounting = decision.accounting.delivered;
    assert_eq!(accounting.inputs, 1);
    assert_eq!(accounting.results, 3);
    assert_eq!(accounting.buffers, 2);
    context.progress.delivered = Some(accounting);
    Ok(())
}
