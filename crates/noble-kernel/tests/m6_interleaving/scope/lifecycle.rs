impl<T> super::Scope<T> {
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; index_of searches the runtime Vec-backed scope membership, whose iteration is not const-evaluable."
    )]
    fn index_of(&self, member: super::MemberRef) -> Result<usize, super::OperationError> {
        self.members
            .iter()
            .position(|child| child.member == member)
            .ok_or(super::OperationError::Scope(super::ScopeError::NotMember))
    }

    pub(crate) fn members(&self) -> Vec<super::MemberRef> {
        self.members.iter().map(|child| child.member).collect()
    }

    pub(crate) fn primary_failure(&self) -> Option<noble_kernel::async_tasks::Failure> {
        self.failure.get()
    }

    pub(crate) fn results(&self) -> Vec<super::ScopeResult> {
        let mut results = Vec::with_capacity(self.members.len());
        for child in &self.members {
            if child.delivered {
                if let Some(result) = child.result {
                    results.push(super::ScopeResult {
                        member: child.member,
                        outcome: result.outcome,
                        marker: result.marker,
                    });
                }
            }
        }
        results
    }

    pub(crate) fn complete(
        &mut self,
        table: &mut noble_kernel::async_tasks::Table,
        member: super::MemberRef,
        outcome: noble_kernel::async_tasks::Outcome,
        completion: noble_kernel::async_tasks::Completion,
        marker: u64,
    ) -> Result<noble_kernel::async_tasks::Decision, super::OperationError> {
        let index = self.index_of(member)?;
        let child = &mut self.members[index];
        let decision = table
            .complete(child.callback, outcome, completion)
            .map_err(super::OperationError::Table)?;
        if let Some(failure) = decision.record.failure {
            if self.failure.get().is_none() {
                self.failure.set(Some(failure));
            }
        }
        if child.cancelled {
            return Ok(decision);
        }
        if decision.accounting.completion_accepted
            && decision.record.state == noble_kernel::async_tasks::State::Ready
        {
            child.result = Some(super::ResultValue { outcome, marker });
        }
        Ok(decision)
    }

    pub(crate) fn deliver(
        &mut self,
        table: &mut noble_kernel::async_tasks::Table,
        member: super::MemberRef,
        context: noble_kernel::async_tasks::Context,
    ) -> Result<noble_kernel::async_tasks::Decision, super::OperationError> {
        let index = self.index_of(member)?;
        if self.members[index].delivered {
            return Err(super::OperationError::Scope(
                super::ScopeError::AlreadyDelivered,
            ));
        }
        if self.members[index].cancelled {
            return Err(super::OperationError::Scope(super::ScopeError::Cancelled));
        }
        let next = self
            .members
            .iter()
            .find(|child| {
                let is_pending_delivery = !child.delivered;
                let can_deliver = !child.cancelled;
                is_pending_delivery && can_deliver
            })
            .map(|child| child.member);
        if next != Some(member) {
            return Err(super::OperationError::Scope(super::ScopeError::OutOfOrder));
        }
        if self.members[index].result.is_none() {
            return Err(super::OperationError::Scope(super::ScopeError::NotReady));
        }
        let task = self.members[index]
            .task
            .take()
            .ok_or(super::OperationError::Scope(
                super::ScopeError::AlreadyDelivered,
            ))?;
        match table.deliver(task, context) {
            Ok(decision) => {
                self.members[index].delivered = true;
                Ok(decision)
            }
            Err(rejected) => {
                self.members[index].task = Some(rejected.input);
                Err(super::OperationError::Table(rejected.error))
            }
        }
    }

    pub(crate) fn cancel(
        &mut self,
        table: &mut noble_kernel::async_tasks::Table,
        member: super::MemberRef,
        context: noble_kernel::async_tasks::Context,
    ) -> Result<noble_kernel::async_tasks::Decision, super::OperationError> {
        let index = self.index_of(member)?;
        if self.members[index].delivered {
            return Err(super::OperationError::Scope(
                super::ScopeError::AlreadyDelivered,
            ));
        }
        if self.members[index].cancelled {
            return Err(super::OperationError::Scope(super::ScopeError::Cancelled));
        }
        let task = self.members[index]
            .task
            .take()
            .ok_or(super::OperationError::Scope(
                super::ScopeError::AlreadyDelivered,
            ))?;
        match table.cancel(task, context) {
            Ok(decision) => {
                self.members[index].cancelled = true;
                if self.failure.get().is_none() {
                    self.failure
                        .set(Some(noble_kernel::async_tasks::Failure::Cancelled));
                }
                Ok(decision)
            }
            Err(rejected) => {
                self.members[index].task = Some(rejected.input);
                Err(super::OperationError::Table(rejected.error))
            }
        }
    }

    pub(crate) fn retire(
        &mut self,
        table: &mut noble_kernel::async_tasks::Table,
        member: super::MemberRef,
        failure: noble_kernel::async_tasks::Failure,
    ) -> Result<noble_kernel::async_tasks::Decision, super::OperationError> {
        let index = self.index_of(member)?;
        let decision = table
            .retire(member.handle, failure)
            .map_err(super::OperationError::Table)?;
        self.members[index].retirement_requested = true;
        if self.failure.get().is_none() {
            self.failure.set(Some(failure));
        }
        Ok(decision)
    }

    pub(crate) fn native_stopped(
        &mut self,
        table: &mut noble_kernel::async_tasks::Table,
        member: super::MemberRef,
    ) -> Result<noble_kernel::async_tasks::Decision, super::OperationError> {
        let index = self.index_of(member)?;
        let decision = table
            .native_stopped(self.members[index].callback)
            .map_err(super::OperationError::Table)?;
        self.members[index].native_stopped = true;
        Ok(decision)
    }

    pub(crate) fn settle_pins(
        &mut self,
        table: &mut noble_kernel::async_tasks::Table,
        member: super::MemberRef,
        pins: u64,
    ) -> Result<(), super::OperationError> {
        let index = self.index_of(member)?;
        if !self.members[index].native_stopped {
            return Err(super::OperationError::Scope(super::ScopeError::Unsettled));
        }
        if self.members[index].pins_settled {
            return Err(super::OperationError::Scope(super::ScopeError::Unsettled));
        }
        let callback = self.members[index].callback;
        // The table rejects a zero mask; after native stop, an empty snapshot is already settled.
        let is_empty_settlement = if pins == 0 {
            super::super::scope_support::is_settled_empty_snapshot(table, member, callback)
        } else {
            false
        };
        if !is_empty_settlement {
            table
                .settle_pins(callback, pins)
                .map_err(super::OperationError::Table)?;
        }
        self.members[index].pins_settled = true;
        Ok(())
    }

    pub(crate) fn cleanup(
        &mut self,
        table: &mut noble_kernel::async_tasks::Table,
        member: super::MemberRef,
        obligations: noble_kernel::async_tasks::Obligations,
    ) -> Result<noble_kernel::async_tasks::Decision, super::OperationError> {
        let index = self.index_of(member)?;
        if !self.members[index].pins_settled {
            return Err(super::OperationError::Scope(super::ScopeError::Unsettled));
        }
        if self.members[index].cleaned {
            return Err(super::OperationError::Scope(super::ScopeError::Unsettled));
        }
        let decision = table
            .cleanup(member.handle, obligations)
            .map_err(super::OperationError::Table)?;
        self.members[index].input = None;
        self.members[index].cleaned = true;
        Ok(decision)
    }

    pub(crate) fn finish(
        &mut self,
        table: &mut noble_kernel::async_tasks::Table,
        member: super::MemberRef,
    ) -> Result<noble_kernel::async_tasks::Decision, super::OperationError> {
        let index = self.index_of(member)?;
        if !self.members[index].cleaned {
            return Err(super::OperationError::Scope(super::ScopeError::Unsettled));
        }
        if self.members[index].finalized {
            return Err(super::OperationError::Scope(super::ScopeError::Unsettled));
        }
        let decision = table
            .finish(member.handle)
            .map_err(super::OperationError::Table)?;
        self.members[index].finalized = true;
        Ok(decision)
    }

    pub(crate) fn release(&mut self, member: super::MemberRef) -> Result<(), super::ScopeError> {
        let index = self
            .members
            .iter()
            .position(|child| child.member == member)
            .ok_or(super::ScopeError::NotMember)?;
        if self.members[index].released {
            return Err(super::ScopeError::AlreadyReleased);
        }
        if !self.members[index].finalized {
            return Err(super::ScopeError::NotFinalized);
        }
        self.members[index].released = true;
        Ok(())
    }

    pub(crate) fn close(&mut self) -> Result<(), super::ScopeError> {
        if self.closed {
            return Ok(());
        }
        if !self.issued.is_empty() {
            return Err(super::ScopeError::Unsettled);
        }
        if self
            .members
            .iter()
            .all(|child| child.finalized && child.released)
        {
            self.closed = true;
            Ok(())
        } else {
            Err(super::ScopeError::Unsettled)
        }
    }
}
