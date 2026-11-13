pub(super) fn attach<T>(
    scope: &mut super::manager::Scope<T>,
    table: &noble_kernel::async_tasks::Table,
    admission: super::manager::ScopedAdmission<T>,
) -> Result<super::manager::MemberRef, String> {
    let mut admission = Some(admission);
    scope
        .attach(table, &mut admission)
        .map_err(|failure| format!("{failure:?}"))
}

pub(super) fn admit<T: core::fmt::Debug>(
    scope: &mut super::manager::Scope<T>,
    table: &mut noble_kernel::async_tasks::Table,
    input: T,
) -> Result<super::manager::MemberRef, String> {
    let admission = scope
        .admit(table, super::support::request(), input)
        .map_err(|failure| format!("{failure:?}"))?;
    attach(scope, table, admission)
}

pub(super) fn settle_delivered<T>(
    table: &mut noble_kernel::async_tasks::Table,
    scope: &mut super::manager::Scope<T>,
    member: super::manager::MemberRef,
) -> Result<(), String> {
    assert!(
        scope.members().contains(&member),
        "settlement requires an attached member"
    );
    assert!(
        table.snapshot(member.handle.slot).is_some(),
        "settlement requires a retained table record"
    );
    scope
        .native_stopped(table, member)
        .map_err(|error| format!("{error:?}"))?;
    scope
        .settle_pins(table, member, 3)
        .map_err(|error| format!("{error:?}"))?;
    scope
        .cleanup(
            table,
            member,
            noble_kernel::async_tasks::Obligations {
                inputs: 4,
                results: 0,
                buffers: 5,
            },
        )
        .map_err(|error| format!("{error:?}"))?;
    scope
        .finish(table, member)
        .map_err(|error| format!("{error:?}"))?;
    scope.release(member).map_err(|error| format!("{error:?}"))
}

pub(super) fn is_settled_empty_snapshot(
    table: &noble_kernel::async_tasks::Table,
    member: super::manager::MemberRef,
    callback: noble_kernel::async_tasks::Callback,
) -> bool {
    let Some(record) = table.snapshot(member.handle.slot) else {
        return false;
    };
    record.handle == member.handle
        && record.native == callback.native
        && record.native_stopped
        && record.pins == 0
}
