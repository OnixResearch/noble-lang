pub const SENDER: noble_kernel::resources::Context = noble_kernel::resources::Context(10);
pub const RECEIVER: noble_kernel::resources::Context = noble_kernel::resources::Context(20);

pub fn accepted<T, E: core::fmt::Debug>(result: Result<T, E>) -> Result<T, String> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(format!("unexpected rejection: {error:?}")),
    }
}

pub fn rejected<T: core::fmt::Debug, E>(result: Result<T, E>) -> Result<E, String> {
    match result {
        Ok(value) => Err(format!("unexpected acceptance: {value:?}")),
        Err(error) => Ok(error),
    }
}

pub const fn limits() -> noble_kernel::resources::Limits {
    noble_kernel::resources::Limits {
        slots: 4,
        pins: 4,
        owners_per_context: 4,
        generations: u64::MAX,
        scopes: u64::MAX,
    }
}

pub fn table() -> Result<noble_kernel::resources::Table, String> {
    accepted(noble_kernel::resources::Table::new(
        noble_kernel::resources::TableId(1),
        limits(),
    ))
}

pub const fn required(
    context: noble_kernel::resources::Context,
) -> noble_kernel::resources::Requirement {
    noble_kernel::resources::Requirement {
        context,
        kind: noble_kernel::types::ResourceKind(7),
        rights: noble_kernel::resources::Rights(1),
    }
}

pub fn owner(
    completed: noble_kernel::resources::Completed,
) -> Result<noble_kernel::resources::Owner, String> {
    match completed.owner {
        Some(owner) => Ok(owner),
        None => Err("normal completion did not return its owner".to_string()),
    }
}

pub fn owner_from_scope(
    table: &mut noble_kernel::resources::Table,
    scope: noble_kernel::resources::Scope,
) -> Result<noble_kernel::resources::Owner, String> {
    owner(accepted(table.complete(
        scope,
        noble_kernel::resources::Completion::Success,
    ))?)
}

pub fn no_accounting(decision: noble_kernel::resources::Decision) {
    assert_eq!(
        decision.accounting(),
        noble_kernel::resources::Accounting {
            pins_acquired: 0,
            pins_released: 0,
            owners_returned: 0,
            owners_transferred: 0,
            local_releases: 0,
        }
    );
}

pub const fn invalid_callbacks(
    scope: noble_kernel::resources::Scope,
) -> [noble_kernel::resources::Scope; 6] {
    [
        noble_kernel::resources::Scope { serial: 0, ..scope },
        noble_kernel::resources::Scope {
            handle: noble_kernel::resources::Handle {
                context: crate::support::RECEIVER,
                ..scope.handle
            },
            ..scope
        },
        noble_kernel::resources::Scope {
            handle: noble_kernel::resources::Handle {
                generation: 0,
                ..scope.handle
            },
            ..scope
        },
        noble_kernel::resources::Scope {
            handle: noble_kernel::resources::Handle {
                kind: noble_kernel::types::ResourceKind(8),
                ..scope.handle
            },
            ..scope
        },
        noble_kernel::resources::Scope {
            handle: noble_kernel::resources::Handle {
                rights: noble_kernel::resources::Rights(0),
                ..scope.handle
            },
            ..scope
        },
        noble_kernel::resources::Scope {
            handle: noble_kernel::resources::Handle {
                slot: usize::MAX,
                ..scope.handle
            },
            ..scope
        },
    ]
}

pub const fn invalid_claims(
    handle: noble_kernel::resources::Handle,
) -> [(
    noble_kernel::resources::Handle,
    noble_kernel::resources::Error,
); 6] {
    [
        (
            noble_kernel::resources::Handle {
                slot: usize::MAX,
                ..handle
            },
            noble_kernel::resources::Error::InvalidHandle,
        ),
        (
            noble_kernel::resources::Handle {
                generation: 0,
                ..handle
            },
            noble_kernel::resources::Error::WrongGeneration,
        ),
        (
            noble_kernel::resources::Handle {
                kind: noble_kernel::types::ResourceKind(8),
                ..handle
            },
            noble_kernel::resources::Error::WrongKind,
        ),
        (
            noble_kernel::resources::Handle {
                context: crate::support::RECEIVER,
                ..handle
            },
            noble_kernel::resources::Error::WrongContext,
        ),
        (
            noble_kernel::resources::Handle {
                rights: noble_kernel::resources::Rights(3),
                ..handle
            },
            noble_kernel::resources::Error::WrongRights,
        ),
        (
            noble_kernel::resources::Handle {
                table: noble_kernel::resources::TableId(2),
                ..handle
            },
            noble_kernel::resources::Error::InvalidHandle,
        ),
    ]
}

pub const fn invalid_requirements() -> [(
    noble_kernel::resources::Requirement,
    noble_kernel::resources::Error,
); 3] {
    [
        (
            noble_kernel::resources::Requirement {
                context: crate::support::RECEIVER,
                ..crate::support::required(crate::support::SENDER)
            },
            noble_kernel::resources::Error::WrongContext,
        ),
        (
            noble_kernel::resources::Requirement {
                kind: noble_kernel::types::ResourceKind(8),
                ..crate::support::required(crate::support::SENDER)
            },
            noble_kernel::resources::Error::WrongKind,
        ),
        (
            noble_kernel::resources::Requirement {
                rights: noble_kernel::resources::Rights(2),
                ..crate::support::required(crate::support::SENDER)
            },
            noble_kernel::resources::Error::WrongRights,
        ),
    ]
}
