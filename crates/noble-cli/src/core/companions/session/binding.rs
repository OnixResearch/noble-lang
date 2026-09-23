impl crate::core::companions::Driver {
    /// Observe without changing the user's stack. If the subject is nested in
    /// a companion/list, a temporary Program slot is removed on every outcome.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; subject observation temporarily injects and removes live Program cells through non-const engine operations."
    )]
    pub(in crate::core::companions) fn observe_subject(
        &mut self,
        program: &crate::core::companions::ProgramSlot,
    ) -> crate::core::companions::Attempt<noble_contracts::companion::Subject> {
        let report = attempt!(self.read_stack());
        let entries = attempt!(crate::core::companions::reporting::values::slots(&report));
        let existing = entries
            .iter()
            .position(|(kind, handle)| kind == "Program" && *handle == Some(program.handle));
        let at = existing.unwrap_or(self.stack.len());
        if existing.is_none() {
            attempt!(self.push_cell(
                &crate::core::companions::reporting::inputs::program_injection(program.handle),
                program.ty.clone(),
            ));
        }
        let observed = self.observe_at(at, &program.ty);
        if existing.is_none() {
            let input = self.stack.clone();
            let report = attempt!(self.execute(b"drop", &input, &[]));
            if report
                .member("outcome")
                .and_then(crate::core::report::Value::text)
                != Some("normal")
            {
                return Err(crate::core::companions::Refused::script(
                    "could not remove the temporary observation slot",
                ));
            }
        }
        observed
    }

    /// Explicitly project one companion slot: the guest returns its live
    /// subject Program handle, and the slot value carries the retained
    /// contract index and identity the core issued.
    pub(in crate::core::companions) fn certified_at(
        &mut self,
        index: usize,
    ) -> crate::core::companions::Attempt<crate::core::companions::CertifiedSlot> {
        let report = attempt!(self.read_stack());
        let entries = attempt!(crate::core::companions::reporting::values::slots(&report));
        let entry = attempt!(entries.get(index).ok_or_else(|| {
            crate::core::companions::Refused::new(
                "malformed",
                "stack-index",
                std::format!("stack has no slot {index}"),
            )
        }));
        if entry.0 != "Certified" {
            return Err(crate::core::companions::Refused::new(
                "malformed",
                "stack-type",
                std::format!("slot {index} is a {}, not a companion", entry.0),
            ));
        }
        let raw = attempt!(report
            .member("stack")
            .and_then(crate::core::report::Value::items)
            .and_then(|items| items.get(index))
            .ok_or_else(|| {
                crate::core::companions::Refused::script(std::string::String::from(
                    "missing stack entry",
                ))
            }));
        let identity = raw
            .member("identity")
            .and_then(crate::core::report::Value::text)
            .unwrap_or_default()
            .to_owned();
        let contract_index = attempt!(crate::core::companions::reporting::nested_number(
            raw,
            ["contract", "index"]
        ));
        let evidence_index = attempt!(crate::core::companions::reporting::nested_number(
            raw,
            ["evidence", "index"]
        ));
        let handle = attempt!(entry.1.ok_or_else(|| {
            crate::core::companions::Refused::script(std::string::String::from(
                "companion slot has no handle",
            ))
        }));
        let handle =
            attempt!(
                u32::try_from(handle).map_err(|_| crate::core::companions::Refused::script(
                    "companion handle exceeds the engine handle domain"
                ))
            );
        let reply = attempt!(attempt!(self.engine())
            .project(handle)
            .map_err(crate::core::companions::Refused::engine));
        let projected = attempt!(crate::core::companions::reporting::parse_report(
            &reply.json
        ));
        if reply.outcome != "projected" {
            return Err(crate::core::companions::Refused::new(
                "refused",
                "projection-failed",
                std::string::String::from("the guest could not project the companion"),
            ));
        }
        let subject = attempt!(projected
            .member("subject")
            .and_then(crate::core::report::Value::index)
            .ok_or_else(|| {
                crate::core::companions::Refused::script(std::string::String::from(
                    "projection returned no subject",
                ))
            }));
        Ok(crate::core::companions::CertifiedSlot {
            subject,
            contract: noble_contracts::companion::ContractId(contract_index),
            evidence: noble_contracts::companion::EvidenceId(evidence_index),
            identity,
        })
    }

    /// Check the exact evidence carried by this value, not whichever evidence
    /// was most recently associated with its contract.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; re-observation of the current live subject and core applicability checks use non-const runtime APIs."
    )]
    pub(in crate::core::companions) fn bound_companion(
        &mut self,
        companion: &crate::core::companions::CertifiedSlot,
    ) -> crate::core::companions::Attempt<crate::core::companions::ProgramSlot> {
        attempt!(self
            .core
            .correspond(companion.evidence, companion.contract)
            .map_err(crate::core::companions::Refused::core));
        let program = crate::core::companions::ProgramSlot {
            handle: companion.subject,
            ty: attempt!(self.program_type(companion.subject).ok_or_else(|| {
                crate::core::companions::Refused::script("companion subject is not retained")
            })),
        };
        let observed = attempt!(self.observe_subject(&program));
        let bound = attempt!(self
            .core
            .bind(companion.evidence, &observed)
            .map_err(crate::core::companions::Refused::core));
        if companion.identity.parse::<u64>().ok() != Some(bound.identity.0) {
            return Err(crate::core::companions::Refused::core(
                noble_contracts::companion::Refusal::MismatchedSubject,
            ));
        }
        Ok(program)
    }

    /// The retained record of one admitted contract.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; retained-record lookup uses a runtime iterator and constructs an allocated refusal when the record is absent."
    )]
    pub(in crate::core::companions) fn record_for(
        &self,
        index: u32,
    ) -> crate::core::companions::Attempt<&crate::core::companions::Record> {
        self.records
            .iter()
            .find(|record| record.contract.0 == index)
            .ok_or_else(|| {
                crate::core::companions::Refused::new(
                    "refused",
                    "unknown-contract",
                    std::format!("no retained contract {index}"),
                )
            })
    }

    /// The program slot at one index, with its exact type.
    pub(in crate::core::companions) fn program_slot(
        &mut self,
        index: usize,
    ) -> crate::core::companions::Attempt<(noble_kernel::types::Ty, u64)> {
        let report = attempt!(self.read_stack());
        let entries = attempt!(crate::core::companions::reporting::values::slots(&report));
        let entry = attempt!(entries.get(index).ok_or_else(|| {
            crate::core::companions::Refused::script(std::format!("stack has no slot {index}"))
        }));
        if entry.0 != "Program" {
            return Err(crate::core::companions::Refused::new(
                "malformed",
                "stack-type",
                std::format!("slot {index} is a {}, not a program", entry.0),
            ));
        }
        let handle = attempt!(entry.1.ok_or_else(|| {
            crate::core::companions::Refused::script(std::string::String::from(
                "program slot has no handle",
            ))
        }));
        let ty = attempt!(self.program_type(handle).ok_or_else(|| {
            crate::core::companions::Refused::new(
                "malformed",
                "stack-type",
                std::format!("program handle {handle} is not retained"),
            )
        }));
        Ok((ty, handle))
    }
    fn observe_at(
        &mut self,
        at: usize,
        ty: &noble_kernel::types::Ty,
    ) -> crate::core::companions::Attempt<noble_contracts::companion::Subject> {
        let index =
            attempt!(
                u32::try_from(at).map_err(|_| crate::core::companions::Refused::script(
                    "observation index exceeds the engine index domain"
                ))
            );

        let reply = attempt!(attempt!(self.engine())
            .observe(index)
            .map_err(crate::core::companions::Refused::engine));
        let report = attempt!(crate::core::companions::reporting::parse_report(
            &reply.json
        ));
        if reply.outcome != "observed" {
            return Err(crate::core::companions::Refused::new(
                "refused",
                "observation-failed",
                std::string::String::from("the guest could not observe the subject"),
            ));
        }
        let events =
            attempt!(crate::core::companions::reporting::values::reflection_events(&report));
        let (input, output) = attempt!(crate::core::companions::reporting::program_interfaces(ty));
        Ok(noble_contracts::companion::Core::observe(
            &events,
            noble_contracts::companion::InterfaceSignatures {
                input: noble_contracts::companion::interface_signature(&input),
                output: noble_contracts::companion::interface_signature(&output),
            },
        ))
    }
}
