impl<T> Drop for super::ScopedAdmission<T> {
    fn drop(&mut self) {
        if self.task.is_some() {
            self.registry
                .borrow_mut()
                .inherit(super::super::registry::Pending {
                    member: self.member,
                    callback: self.callback,
                    input: self.input.take(),
                    failure: Some(
                        self.failure
                            .get()
                            .unwrap_or(noble_kernel::async_tasks::Failure::Internal),
                    ),
                    needs_retire: true,
                    retirement_requested: false,
                    native_stopped: false,
                    pins_settled: false,
                    cleaned: false,
                    finalized: false,
                    released: false,
                });
        }
    }
}

impl<T> Drop for super::Scope<T> {
    fn drop(&mut self) {
        let failure = Some(
            self.failure
                .get()
                .unwrap_or(noble_kernel::async_tasks::Failure::Internal),
        );
        let mut registry = self.registry.borrow_mut();
        for child in &mut self.members {
            if !child.released {
                registry.inherit(super::super::registry::Pending {
                    member: child.member,
                    callback: child.callback,
                    input: child.input.take(),
                    failure,
                    needs_retire: !child.finalized && !child.delivered && !child.cancelled,
                    retirement_requested: child.retirement_requested || child.cancelled,
                    native_stopped: child.native_stopped,
                    pins_settled: child.pins_settled,
                    cleaned: child.cleaned,
                    finalized: child.finalized,
                    released: child.released,
                });
            }
        }
        for issued in &self.issued {
            registry.inherit(super::super::registry::Pending {
                member: issued.member,
                callback: issued.callback,
                input: None,
                failure,
                needs_retire: true,
                retirement_requested: false,
                native_stopped: false,
                pins_settled: false,
                cleaned: false,
                finalized: false,
                released: false,
            });
        }
    }
}
