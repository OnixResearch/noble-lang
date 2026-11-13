//! Test-only host scope policy over the production M6 table. This module adds no kernel surface.

const MAX_CHILDREN: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ScopeId(pub u64);

/// A descriptive membership key; only the scope-owned `ScopedAdmission` can mint it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MemberRef {
    pub(super) scope: ScopeId,
    pub(super) table: noble_kernel::async_tasks::TableId,
    pub(super) handle: noble_kernel::async_tasks::Handle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScopeError {
    Closed,
    Full,
    UnattachedAdmission,
    ForeignScope,
    ForeignTable,
    StaleGeneration,
    FabricatedMember,
    AlreadyMember,
    NotMember,
    NotReady,
    OutOfOrder,
    Cancelled,
    AlreadyDelivered,
    NotFinalized,
    AlreadyReleased,
    Unsettled,
}

#[derive(Debug)]
pub(super) enum AdmissionError<T> {
    Scope { error: ScopeError, input: T },
    Table(noble_kernel::async_tasks::Rejected<T>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OperationError {
    Scope(ScopeError),
    Table(noble_kernel::async_tasks::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ScopeResult {
    pub(super) member: MemberRef,
    pub(super) outcome: noble_kernel::async_tasks::Outcome,
    pub(super) marker: u64,
}

#[derive(Clone, Copy)]
struct Issued {
    member: MemberRef,
    callback: noble_kernel::async_tasks::Callback,
}

#[derive(Clone, Copy)]
struct ResultValue {
    outcome: noble_kernel::async_tasks::Outcome,
    marker: u64,
}

struct Child<T> {
    member: MemberRef,
    callback: noble_kernel::async_tasks::Callback,
    task: Option<noble_kernel::async_tasks::Task>,
    input: Option<T>,
    result: Option<ResultValue>,
    cancelled: bool,
    delivered: bool,
    native_stopped: bool,
    pins_settled: bool,
    cleaned: bool,
    finalized: bool,
    released: bool,
    retirement_requested: bool,
}

/// Move-only output of this scope's own successful `Table::admit` call.
pub(super) struct ScopedAdmission<T> {
    member: MemberRef,
    callback: noble_kernel::async_tasks::Callback,
    task: Option<noble_kernel::async_tasks::Task>,
    input: Option<T>,
    registry: std::rc::Rc<std::cell::RefCell<super::registry::ScopeRegistry<T>>>,
    failure: std::rc::Rc<std::cell::Cell<Option<noble_kernel::async_tasks::Failure>>>,
}

impl<T> ScopedAdmission<T> {
    pub(super) const fn member(&self) -> MemberRef {
        self.member
    }
}

pub(super) struct Scope<T> {
    id: ScopeId,
    table_id: noble_kernel::async_tasks::TableId,
    members: Vec<Child<T>>,
    issued: Vec<Issued>,
    failure: std::rc::Rc<std::cell::Cell<Option<noble_kernel::async_tasks::Failure>>>,
    closed: bool,
    registry: std::rc::Rc<std::cell::RefCell<super::registry::ScopeRegistry<T>>>,
}

impl<T> Scope<T> {
    pub(super) fn new(
        id: ScopeId,
        table_id: noble_kernel::async_tasks::TableId,
        registry: std::rc::Rc<std::cell::RefCell<super::registry::ScopeRegistry<T>>>,
    ) -> Self {
        Self {
            id,
            table_id,
            members: Vec::with_capacity(MAX_CHILDREN),
            issued: Vec::with_capacity(MAX_CHILDREN),
            failure: std::rc::Rc::new(std::cell::Cell::new(None)),
            closed: false,
            registry,
        }
    }

    pub(super) fn admit(
        &mut self,
        table: &mut noble_kernel::async_tasks::Table,
        request: noble_kernel::async_tasks::Request,
        input: T,
    ) -> Result<ScopedAdmission<T>, AdmissionError<T>> {
        if self.closed {
            return Err(AdmissionError::Scope {
                error: ScopeError::Closed,
                input,
            });
        }
        if !self.issued.is_empty() {
            return Err(AdmissionError::Scope {
                error: ScopeError::UnattachedAdmission,
                input,
            });
        }
        if self.members.len().saturating_add(self.issued.len()) >= MAX_CHILDREN {
            return Err(AdmissionError::Scope {
                error: ScopeError::Full,
                input,
            });
        }
        let noble_kernel::async_tasks::Admitted {
            task,
            callback,
            inputs,
            ..
        } = table.admit(request, input).map_err(AdmissionError::Table)?;
        let handle = task.handle();
        let member = MemberRef {
            scope: self.id,
            table: handle.table,
            handle,
        };
        self.issued.push(Issued { member, callback });
        Ok(ScopedAdmission {
            member,
            callback,
            task: Some(task),
            input: Some(inputs),
            registry: std::rc::Rc::clone(&self.registry),
            failure: std::rc::Rc::clone(&self.failure),
        })
    }

    pub(super) fn attach(
        &mut self,
        table: &noble_kernel::async_tasks::Table,
        admission: &mut Option<ScopedAdmission<T>>,
    ) -> Result<MemberRef, ScopeError> {
        let Some(candidate) = admission.as_ref() else {
            return Err(ScopeError::FabricatedMember);
        };
        let error = if candidate.member.scope != self.id {
            Some(ScopeError::ForeignScope)
        } else if candidate.member.table != self.table_id
            || candidate.member.handle.table != self.table_id
        {
            Some(ScopeError::ForeignTable)
        } else if self.closed {
            Some(ScopeError::Closed)
        } else if let Some(snapshot) = table.snapshot(candidate.member.handle.slot) {
            if snapshot.handle != candidate.member.handle
                || snapshot.finalized
                || snapshot.state != noble_kernel::async_tasks::State::Pending
            {
                Some(ScopeError::StaleGeneration)
            } else if self
                .issued
                .iter()
                .all(|issued| issued.member != candidate.member)
            {
                Some(
                    if self
                        .members
                        .iter()
                        .any(|child| child.member == candidate.member)
                    {
                        ScopeError::AlreadyMember
                    } else {
                        ScopeError::FabricatedMember
                    },
                )
            } else if self.members.len() >= MAX_CHILDREN {
                Some(ScopeError::Full)
            } else {
                None
            }
        } else {
            Some(ScopeError::StaleGeneration)
        };
        if let Some(error) = error {
            return Err(error);
        }
        let Some(mut admission) = admission.take() else {
            return Err(ScopeError::FabricatedMember);
        };
        let position = self
            .issued
            .iter()
            .position(|issued| issued.member == admission.member);
        if let Some(position) = position {
            self.issued.remove(position);
        }
        let member = admission.member;
        self.members.push(Child {
            member,
            callback: admission.callback,
            task: admission.task.take(),
            input: admission.input.take(),
            result: None,
            cancelled: false,
            delivered: false,
            native_stopped: false,
            pins_settled: false,
            cleaned: false,
            finalized: false,
            released: false,
            retirement_requested: false,
        });
        Ok(member)
    }
}

#[path = "lifecycle.rs"]
mod lifecycle;

#[path = "controls/drop.rs"]
mod drop;
