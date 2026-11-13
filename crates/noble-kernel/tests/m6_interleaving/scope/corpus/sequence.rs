#[derive(Clone, Copy)]
#[octet::sealed_enum]
enum CompletionAction {
    CompleteLeft,
    CompleteRight,
    TryOutOfOrderRight,
    DeliverLeft,
    TryDuplicateLeft,
    DeliverRight,
}

impl CompletionAction {
    const fn lean(self) -> Option<&'static str> {
        match self {
            Self::CompleteLeft => Some(".succeed left1"),
            Self::CompleteRight => Some(".succeed right2"),
            Self::TryOutOfOrderRight | Self::TryDuplicateLeft => None,
            Self::DeliverLeft => Some(".deliver left1"),
            Self::DeliverRight => Some(".deliver right2"),
        }
    }
}

#[derive(Default)]
pub(super) struct CompletionProgress {
    pub(super) model_steps: Vec<&'static str>,
    pub(super) accounting: Vec<noble_kernel::async_tasks::Obligations>,
    pub(super) after_first: Option<noble_kernel::async_tasks::Observation>,
    pub(super) out_of_order_refused: bool,
}

fn completion_schedule(reverse: bool) -> Vec<CompletionAction> {
    if reverse {
        vec![
            CompletionAction::CompleteRight,
            CompletionAction::TryOutOfOrderRight,
            CompletionAction::CompleteLeft,
            CompletionAction::DeliverLeft,
            CompletionAction::TryDuplicateLeft,
            CompletionAction::DeliverRight,
        ]
    } else {
        vec![
            CompletionAction::CompleteLeft,
            CompletionAction::CompleteRight,
            CompletionAction::DeliverLeft,
            CompletionAction::TryDuplicateLeft,
            CompletionAction::DeliverRight,
        ]
    }
}

struct CompletionContext<'a> {
    table: &'a mut noble_kernel::async_tasks::Table,
    scope: &'a mut super::super::super::manager::Scope<u8>,
    left: super::super::super::manager::MemberRef,
    right: super::super::super::manager::MemberRef,
    progress: &'a mut CompletionProgress,
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this dispatcher mutates the live M6 table, scope and runtime trace witness."
)]
fn apply_completion_action(
    action: CompletionAction,
    context: CompletionContext<'_>,
) -> Result<(), String> {
    let CompletionContext {
        table,
        scope,
        left,
        right,
        progress,
    } = context;
    if let Some(model_step) = action.lean() {
        progress.model_steps.push(model_step);
    }
    assert_eq!(scope.members().len(), 2);
    let result = match action {
        CompletionAction::CompleteLeft => complete_member(
            table,
            scope,
            left,
            noble_kernel::async_tasks::Outcome::Success,
            11,
        ),
        CompletionAction::CompleteRight => complete_member(
            table,
            scope,
            right,
            noble_kernel::async_tasks::Outcome::DomainError,
            22,
        ),
        CompletionAction::TryOutOfOrderRight => {
            refuse_out_of_order_right(table, scope, right, progress)
        }
        CompletionAction::DeliverLeft => deliver_left(table, scope, left, progress),
        CompletionAction::TryDuplicateLeft => refuse_duplicate_left(scope, table, left),
        CompletionAction::DeliverRight => deliver_right(table, scope, right, progress),
    };
    assert_eq!(scope.members().len(), 2);
    result
}

fn complete_member(
    table: &mut noble_kernel::async_tasks::Table,
    scope: &mut super::super::super::manager::Scope<u8>,
    member: super::super::super::manager::MemberRef,
    outcome: noble_kernel::async_tasks::Outcome,
    marker: u64,
) -> Result<(), String> {
    let decision = scope
        .complete(
            table,
            member,
            outcome,
            super::super::super::support::completion(),
            marker,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert!(decision.accounting.completion_accepted);
    assert_eq!(
        decision.record.state,
        noble_kernel::async_tasks::State::Ready
    );
    Ok(())
}

fn refuse_out_of_order_right(
    table: &mut noble_kernel::async_tasks::Table,
    scope: &mut super::super::super::manager::Scope<u8>,
    right: super::super::super::manager::MemberRef,
    progress: &mut CompletionProgress,
) -> Result<(), String> {
    match scope.deliver(
        table,
        right,
        super::super::super::support::request().context,
    ) {
        Err(super::super::super::manager::OperationError::Scope(
            super::super::super::manager::ScopeError::OutOfOrder,
        )) => {
            progress.out_of_order_refused = true;
            assert!(scope.results().is_empty());
            assert_eq!(table.observation().delivered, 0);
            Ok(())
        }
        Err(error) => Err(format!("wrong out-of-order refusal: {error:?}")),
        Ok(_) => Err("scope delivered the later child before the earlier result".to_owned()),
    }
}

fn deliver_left(
    table: &mut noble_kernel::async_tasks::Table,
    scope: &mut super::super::super::manager::Scope<u8>,
    left: super::super::super::manager::MemberRef,
    progress: &mut CompletionProgress,
) -> Result<(), String> {
    let decision = scope
        .deliver(table, left, super::super::super::support::request().context)
        .map_err(|error| format!("{error:?}"))?;
    progress.accounting.push(decision.accounting.delivered);
    progress.after_first = Some(table.observation());
    assert_eq!(scope.results().len(), 1);
    assert_eq!(progress.accounting.len(), 1);
    Ok(())
}

fn refuse_duplicate_left(
    scope: &mut super::super::super::manager::Scope<u8>,
    table: &mut noble_kernel::async_tasks::Table,
    left: super::super::super::manager::MemberRef,
) -> Result<(), String> {
    match scope.deliver(table, left, super::super::super::support::request().context) {
        Err(super::super::super::manager::OperationError::Scope(
            super::super::super::manager::ScopeError::AlreadyDelivered,
        )) => {
            assert_eq!(scope.results().len(), 1);
            Ok(())
        }
        Err(error) => Err(format!("wrong duplicate-delivery refusal: {error:?}")),
        Ok(_) => Err("scope accepted a repeated delivery".to_owned()),
    }
}

fn deliver_right(
    table: &mut noble_kernel::async_tasks::Table,
    scope: &mut super::super::super::manager::Scope<u8>,
    right: super::super::super::manager::MemberRef,
    progress: &mut CompletionProgress,
) -> Result<(), String> {
    let decision = scope
        .deliver(
            table,
            right,
            super::super::super::support::request().context,
        )
        .map_err(|error| format!("{error:?}"))?;
    progress.accounting.push(decision.accounting.delivered);
    assert_eq!(scope.results().len(), 2);
    assert_eq!(progress.accounting.len(), 2);
    Ok(())
}

pub(super) struct CompletionRun<'a> {
    pub(super) table: &'a mut noble_kernel::async_tasks::Table,
    pub(super) scope: &'a mut super::super::super::manager::Scope<u8>,
    pub(super) left: super::super::super::manager::MemberRef,
    pub(super) right: super::super::super::manager::MemberRef,
}

pub(super) fn run_completion_schedule(
    reverse: bool,
    run: CompletionRun<'_>,
) -> Result<CompletionProgress, String> {
    let CompletionRun {
        table,
        scope,
        left,
        right,
    } = run;
    let mut progress = CompletionProgress {
        model_steps: vec![
            ".admission left1",
            ".attach left1",
            ".admission right2",
            ".attach right2",
        ],
        accounting: Vec::with_capacity(2),
        ..CompletionProgress::default()
    };
    for action in completion_schedule(reverse) {
        apply_completion_action(
            action,
            CompletionContext {
                table: &mut *table,
                scope: &mut *scope,
                left,
                right,
                progress: &mut progress,
            },
        )?;
    }
    assert_eq!(progress.model_steps.len(), 8);
    assert_eq!(progress.accounting.len(), 2);
    Ok(progress)
}
