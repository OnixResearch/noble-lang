pub(super) struct Pending<T> {
    pub(super) member: super::manager::MemberRef,
    pub(super) callback: noble_kernel::async_tasks::Callback,
    pub(super) input: Option<T>,
    pub(super) failure: Option<noble_kernel::async_tasks::Failure>,
    pub(super) needs_retire: bool,
    pub(super) retirement_requested: bool,
    pub(super) native_stopped: bool,
    pub(super) pins_settled: bool,
    pub(super) cleaned: bool,
    pub(super) finalized: bool,
    pub(super) released: bool,
}

pub(super) struct ScopeRegistry<T> {
    pub(super) pending: Vec<Pending<T>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RegistryError {
    NotPending,
    WrongTable,
    RetirementNotRequested,
    AlreadyStopped,
    NativeNotStopped,
    AlreadyPinsSettled,
    PinsNotSettled,
    AlreadyCleaned,
    CleanupNotSettled,
    AlreadyFinalized,
    AlreadyReleased,
    Table(noble_kernel::async_tasks::Error),
}

impl<T> ScopeRegistry<T> {
    pub(super) fn shared() -> std::rc::Rc<std::cell::RefCell<Self>> {
        std::rc::Rc::new(std::cell::RefCell::new(Self {
            pending: Vec::new(),
        }))
    }

    pub(super) fn inherit(&mut self, inherited: Pending<T>) {
        if let Some(pending) = self
            .pending
            .iter_mut()
            .find(|pending| pending.member == inherited.member)
        {
            if pending.input.is_none() {
                pending.input = inherited.input;
            }
            if pending.failure.is_none() {
                pending.failure = inherited.failure;
            }
            pending.needs_retire |= inherited.needs_retire;
            pending.native_stopped |= inherited.native_stopped;
            pending.pins_settled |= inherited.pins_settled;
            pending.cleaned |= inherited.cleaned;
            pending.finalized |= inherited.finalized;
            pending.released |= inherited.released;
            return;
        }
        self.pending.push(inherited);
    }

    pub(super) fn request_retirement(
        &mut self,
        table_id: noble_kernel::async_tasks::TableId,
        table: &mut noble_kernel::async_tasks::Table,
    ) -> Result<(), RegistryError> {
        for pending in &mut self.pending {
            if pending.member.table != table_id {
                continue;
            }
            if !pending.needs_retire {
                continue;
            }
            if pending.retirement_requested {
                continue;
            }
            let failure = pending
                .failure
                .unwrap_or(noble_kernel::async_tasks::Failure::Internal);
            table
                .retire(pending.member.handle, failure)
                .map_err(RegistryError::Table)?;
            pending.retirement_requested = true;
        }
        Ok(())
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; index_of searches the runtime Vec-backed retirement registry, whose iteration is not const-evaluable."
    )]
    fn index_of(
        &self,
        table_id: noble_kernel::async_tasks::TableId,
        member: super::manager::MemberRef,
    ) -> Result<usize, RegistryError> {
        if member.table != table_id {
            return Err(RegistryError::WrongTable);
        }
        self.pending
            .iter()
            .position(|pending| pending.member == member)
            .ok_or(RegistryError::NotPending)
    }

    pub(super) fn native_stopped(
        &mut self,
        table_id: noble_kernel::async_tasks::TableId,
        table: &mut noble_kernel::async_tasks::Table,
        member: super::manager::MemberRef,
    ) -> Result<(), RegistryError> {
        let index = self.index_of(table_id, member)?;
        if self.pending[index].needs_retire && !self.pending[index].retirement_requested {
            return Err(RegistryError::RetirementNotRequested);
        }
        if self.pending[index].native_stopped {
            return Err(RegistryError::AlreadyStopped);
        }
        table
            .native_stopped(self.pending[index].callback)
            .map_err(RegistryError::Table)?;
        self.pending[index].native_stopped = true;
        Ok(())
    }

    pub(super) fn settle_pins(
        &mut self,
        table_id: noble_kernel::async_tasks::TableId,
        table: &mut noble_kernel::async_tasks::Table,
        member: super::manager::MemberRef,
        pins: u64,
    ) -> Result<(), RegistryError> {
        let index = self.index_of(table_id, member)?;
        if !self.pending[index].native_stopped {
            return Err(RegistryError::NativeNotStopped);
        }
        if self.pending[index].pins_settled {
            return Err(RegistryError::AlreadyPinsSettled);
        }
        let callback = self.pending[index].callback;
        // The table rejects a zero mask; after native stop, an empty snapshot is already settled.
        let is_empty_settlement = if pins == 0 {
            super::scope_support::is_settled_empty_snapshot(table, member, callback)
        } else {
            false
        };
        if !is_empty_settlement {
            table
                .settle_pins(callback, pins)
                .map_err(RegistryError::Table)?;
        }
        self.pending[index].pins_settled = true;
        Ok(())
    }

    pub(super) fn cleanup(
        &mut self,
        table_id: noble_kernel::async_tasks::TableId,
        table: &mut noble_kernel::async_tasks::Table,
        member: super::manager::MemberRef,
        obligations: noble_kernel::async_tasks::Obligations,
    ) -> Result<(), RegistryError> {
        let index = self.index_of(table_id, member)?;
        if !self.pending[index].pins_settled {
            return Err(RegistryError::PinsNotSettled);
        }
        if self.pending[index].cleaned {
            return Err(RegistryError::AlreadyCleaned);
        }
        table
            .cleanup(member.handle, obligations)
            .map_err(RegistryError::Table)?;
        self.pending[index].cleaned = true;
        self.pending[index].input = None;
        Ok(())
    }

    pub(super) fn finish(
        &mut self,
        table_id: noble_kernel::async_tasks::TableId,
        table: &mut noble_kernel::async_tasks::Table,
        member: super::manager::MemberRef,
    ) -> Result<(), RegistryError> {
        let index = self.index_of(table_id, member)?;
        if !self.pending[index].cleaned {
            return Err(RegistryError::CleanupNotSettled);
        }
        if self.pending[index].finalized {
            return Err(RegistryError::AlreadyFinalized);
        }
        table.finish(member.handle).map_err(RegistryError::Table)?;
        self.pending[index].finalized = true;
        Ok(())
    }

    pub(super) fn release(
        &mut self,
        table_id: noble_kernel::async_tasks::TableId,
        member: super::manager::MemberRef,
    ) -> Result<(), RegistryError> {
        let index = self.index_of(table_id, member)?;
        if !self.pending[index].finalized {
            return Err(RegistryError::CleanupNotSettled);
        }
        if self.pending[index].released {
            return Err(RegistryError::AlreadyReleased);
        }
        self.pending[index].released = true;
        Ok(())
    }
}
