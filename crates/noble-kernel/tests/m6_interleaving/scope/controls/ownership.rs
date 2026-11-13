#[path = "ownership/cleanup.rs"]
mod cleanup;

fn settle_registry_input<T>(
    state: &mut super::super::registry::ScopeRegistry<T>,
    table: &mut noble_kernel::async_tasks::Table,
    member: super::super::manager::MemberRef,
) -> Result<(), String> {
    state
        .request_retirement(noble_kernel::async_tasks::TableId(3), table)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(table.observation().retiring, 1);
    assert_eq!(
        table
            .snapshot(member.handle.slot)
            .map(|record| record.failure),
        Some(Some(noble_kernel::async_tasks::Failure::Internal))
    );
    assert!(state.pending[0].retirement_requested);
    assert_eq!(
        state.settle_pins(noble_kernel::async_tasks::TableId(3), table, member, 3),
        Err(super::super::registry::RegistryError::NativeNotStopped)
    );
    state
        .native_stopped(noble_kernel::async_tasks::TableId(3), table, member)
        .map_err(|error| format!("{error:?}"))?;
    state
        .settle_pins(noble_kernel::async_tasks::TableId(3), table, member, 3)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        state.finish(noble_kernel::async_tasks::TableId(3), table, member),
        Err(super::super::registry::RegistryError::CleanupNotSettled)
    );
    state
        .cleanup(
            noble_kernel::async_tasks::TableId(3),
            table,
            member,
            noble_kernel::async_tasks::Obligations {
                inputs: 7,
                results: 0,
                buffers: 5,
            },
        )
        .map_err(|error| format!("{error:?}"))?;
    assert!(state.pending[0].input.is_none());
    state
        .finish(noble_kernel::async_tasks::TableId(3), table, member)
        .map_err(|error| format!("{error:?}"))?;
    state
        .release(noble_kernel::async_tasks::TableId(3), member)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        state.release(noble_kernel::async_tasks::TableId(3), member),
        Err(super::super::registry::RegistryError::AlreadyReleased)
    );
    assert_eq!(table.observation().retired, 1);
    assert_eq!(table.observation().outstanding_pins, 0);
    assert_eq!(
        table.observation().reserved,
        noble_kernel::async_tasks::Footprint::empty()
    );
    Ok(())
}

#[test]
fn dropped_admission_and_input_transfer_to_registry() -> Result<(), String> {
    let mut table = super::super::support::table(1)?;
    let registry = super::super::registry::ScopeRegistry::shared();
    let member;
    {
        let mut scope = super::super::manager::Scope::new(
            super::super::manager::ScopeId(115),
            noble_kernel::async_tasks::TableId(3),
            std::rc::Rc::clone(&registry),
        );
        let admission = scope
            .admit(
                &mut table,
                super::super::support::request(),
                String::from("caller-owned"),
            )
            .map_err(|failure| format!("{failure:?}"))?;
        member = admission.member();
        drop(admission);
        assert_eq!(registry.borrow().pending.len(), 1);
    }
    {
        let state = registry.borrow();
        assert_eq!(
            state.pending.len(),
            1,
            "handoff is deduplicated by exact membership"
        );
        assert_eq!(state.pending[0].member, member);
        assert_eq!(state.pending[0].input.as_deref(), Some("caller-owned"));
        assert_eq!(
            state.pending[0].failure,
            Some(noble_kernel::async_tasks::Failure::Internal)
        );
        assert!(state.pending[0].needs_retire);
        assert!(!state.pending[0].retirement_requested);
    }
    settle_registry_input(&mut registry.borrow_mut(), &mut table, member)?;
    Ok(())
}

#[test]
fn inherited_retirement_is_not_requested_twice() -> Result<(), String> {
    let mut table = super::super::support::table(1)?;
    let registry = super::super::registry::ScopeRegistry::shared();
    let member;
    {
        let mut scope = super::super::manager::Scope::new(
            super::super::manager::ScopeId(115),
            noble_kernel::async_tasks::TableId(3),
            std::rc::Rc::clone(&registry),
        );
        let mut request = super::super::support::request();
        request.pins = 0;
        let admission = scope
            .admit(&mut table, request, 9_u8)
            .map_err(|failure| format!("{failure:?}"))?;
        member = super::super::scope_support::attach(&mut scope, &table, admission)?;
        scope
            .retire(
                &mut table,
                member,
                noble_kernel::async_tasks::Failure::Deadline,
            )
            .map_err(|error| format!("{error:?}"))?;
    }
    {
        let state = registry.borrow();
        assert_eq!(state.pending.len(), 1);
        assert!(state.pending[0].needs_retire);
        assert!(state.pending[0].retirement_requested);
        assert_eq!(
            state.pending[0].failure,
            Some(noble_kernel::async_tasks::Failure::Deadline)
        );
    }
    let mut state = registry.borrow_mut();
    let before_duplicate_request = table.observation();
    state
        .request_retirement(noble_kernel::async_tasks::TableId(3), &mut table)
        .map_err(|error| format!("duplicate retirement request: {error:?}"))?;
    assert_eq!(table.observation(), before_duplicate_request);
    state
        .native_stopped(noble_kernel::async_tasks::TableId(3), &mut table, member)
        .map_err(|error| format!("{error:?}"))?;
    let before_empty_pin_settlement = table.observation();
    state
        .settle_pins(noble_kernel::async_tasks::TableId(3), &mut table, member, 0)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(table.observation(), before_empty_pin_settlement);
    state
        .cleanup(
            noble_kernel::async_tasks::TableId(3),
            &mut table,
            member,
            noble_kernel::async_tasks::Obligations {
                inputs: 7,
                results: 0,
                buffers: 5,
            },
        )
        .map_err(|error| format!("{error:?}"))?;
    state
        .finish(noble_kernel::async_tasks::TableId(3), &mut table, member)
        .map_err(|error| format!("{error:?}"))?;
    state
        .release(noble_kernel::async_tasks::TableId(3), member)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(table.observation().retired, 1);
    Ok(())
}

#[derive(Debug)]
pub(super) struct ScopeInputDrop(std::rc::Rc<std::cell::Cell<bool>>);

impl Drop for ScopeInputDrop {
    fn drop(&mut self) {
        self.0.set(true);
    }
}
