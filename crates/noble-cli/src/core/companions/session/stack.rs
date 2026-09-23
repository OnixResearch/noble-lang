impl crate::core::companions::Driver {
    /// The live stack as the guest reports it. An empty push is a pure read:
    /// it adds no cell and executes no candidate.
    pub(in crate::core::companions) fn read_stack(
        &mut self,
    ) -> crate::core::companions::Attempt<crate::core::report::Value> {
        let reply = attempt!(attempt!(self.engine())
            .push("[]")
            .map_err(crate::core::companions::Refused::engine));
        let report = attempt!(crate::core::companions::reporting::parse_report(
            &reply.json
        ));
        if reply.outcome != "pushed" {
            return Err(crate::core::companions::Refused::new(
                "refused",
                "stack-read",
                std::string::String::from("the guest could not report its stack"),
            ));
        }
        Ok(report)
    }

    /// Remember every program handle the stack reported, with its exact type.
    pub(in crate::core::companions) fn record_programs(
        &mut self,
        report: &crate::core::report::Value,
    ) -> crate::core::companions::Attempt<()> {
        let entries = attempt!(crate::core::companions::reporting::values::slots(report));
        let mut at = 0;
        while at < entries.len() {
            let index = at;
            at = at.saturating_add(1);
            let (kind, handle) = &entries[index];
            if kind != "Program" {
                continue;
            }
            let Some(handle) = handle else { continue };
            let Some(ty) = self.stack.get(index) else {
                continue;
            };
            if self.program_type(*handle).is_none() {
                self.programs.push(crate::core::companions::ProgramSlot {
                    handle: *handle,
                    ty: ty.clone(),
                });
            }
        }
        Ok(())
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; runtime handle lookup clones a heap-owned type from retained Program metadata through non-const APIs."
    )]
    pub(in crate::core::companions) fn program_type(
        &self,
        handle: u64,
    ) -> Option<noble_kernel::types::Ty> {
        self.programs
            .iter()
            .find(|slot| slot.handle == handle)
            .map(|slot| slot.ty.clone())
    }

    /// Publish one typed cell; a missing handle is an error, never handle zero.
    pub(in crate::core::companions) fn push_cell(
        &mut self,
        value: &crate::workflow::encoding::Json,
        ty: noble_kernel::types::Ty,
    ) -> crate::core::companions::Attempt<u64> {
        let reply = attempt!(attempt!(self.engine())
            .push(
                &crate::core::companions::reporting::inputs::injection_frame(std::slice::from_ref(
                    value,
                )),
            )
            .map_err(crate::core::companions::Refused::engine));
        let report = attempt!(crate::core::companions::reporting::parse_report(
            &reply.json
        ));
        if reply.outcome != "pushed" {
            return Err(crate::core::companions::Refused::new(
                "refused",
                "injection-refused",
                std::string::String::from("the guest refused the injected companion"),
            ));
        }
        let handle = attempt!(report
            .member("stack")
            .and_then(crate::core::report::Value::items)
            .and_then(|entries| entries.last())
            .and_then(|entry| entry.member("handle"))
            .and_then(crate::core::report::Value::index)
            .ok_or_else(|| {
                crate::core::companions::Refused::script("the injected cell has no handle")
            }));
        self.stack.push(ty);
        Ok(handle)
    }

    /// Park the whole operand stack in an engine-owned frame. No values are
    /// reconstructed, so aggregates and companion handles retain identity.
    pub(in crate::core::companions) fn park(
        &mut self,
    ) -> crate::core::companions::Attempt<crate::core::companions::Parked> {
        let reply = attempt!(attempt!(self.engine())
            .park()
            .map_err(crate::core::companions::Refused::engine));
        let report = attempt!(crate::core::companions::reporting::parse_report(
            &reply.json
        ));
        if reply.outcome != "parked"
            || report
                .member("stack_length")
                .and_then(crate::core::report::Value::index)
                != Some(self.stack.len() as u64)
        {
            return Err(crate::core::companions::Refused::new(
                "internal-failure",
                "session-model",
                std::string::String::from("the live stack disagrees with the session model"),
            ));
        }
        Ok(crate::core::companions::Parked {
            types: std::mem::take(&mut self.stack),
        })
    }

    /// Drop every cell the session holds. The drop chain is compiled against
    /// the current stack, so its module runs on exactly the stack it consumes.
    pub(in crate::core::companions) fn clear_stack(
        &mut self,
    ) -> crate::core::companions::Attempt<()> {
        let depth = self.stack.len();
        if depth == 0 {
            return Ok(());
        }
        let mut source = std::string::String::new();
        let mut step = 0;
        while step < depth {
            if step > 0 {
                source.push(' ');
            }
            source.push_str("drop");
            step += 1;
        }
        let input = self.stack.clone();
        let report = attempt!(self.execute(source.as_bytes(), &input, &[]));
        match report
            .member("outcome")
            .and_then(crate::core::report::Value::text)
        {
            Some("normal") => Ok(()),
            other => Err(crate::core::companions::Refused::new(
                "refused",
                "session-drop",
                std::format!(
                    "the guest did not drop the parked stack: {}",
                    other.unwrap_or("no outcome")
                ),
            )),
        }
    }

    /// Restore only the retained engine frame, never caller-supplied values.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; frame restoration calls the live engine, parses its report and replaces heap-owned stack types through non-const APIs."
    )]
    fn settle(
        &mut self,
        parked: crate::core::companions::Parked,
    ) -> crate::core::companions::Attempt<()> {
        let reply = attempt!(attempt!(self.engine())
            .restore()
            .map_err(crate::core::companions::Refused::engine));
        let report = attempt!(crate::core::companions::reporting::parse_report(
            &reply.json
        ));
        if reply.outcome != "restored"
            || attempt!(crate::core::companions::reporting::values::slots(&report)).len()
                != parked.types.len()
        {
            return Err(crate::core::companions::Refused::script(
                "engine did not restore the parked frame",
            ));
        }
        self.stack = parked.types;
        Ok(())
    }

    /// Every ordinary refusal restores the original cells, including errors
    /// during intermediate compilation. A poisoned engine still fails closed.
    pub(in crate::core::companions) fn restore_frame<T>(
        &mut self,
        parked: crate::core::companions::Parked,
        outcome: crate::core::companions::Attempt<T>,
    ) -> crate::core::companions::Attempt<T> {
        attempt!(self.settle(parked));
        outcome
    }

    pub(in crate::core::companions) fn run_subject(
        &mut self,
        subject: &crate::core::companions::ProgramSlot,
        argument: i64,
    ) -> crate::core::companions::Attempt<crate::core::report::Value> {
        attempt!(self.clear_stack());
        self.execute(
            b"run",
            &[noble_kernel::types::Ty::I64, subject.ty.clone()],
            &[
                crate::core::companions::reporting::inputs::i64_injection(argument),
                crate::core::companions::reporting::inputs::program_injection(subject.handle),
            ],
        )
    }
}
