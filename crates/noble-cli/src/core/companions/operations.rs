pub(in crate::core::companions) mod admission;
pub(in crate::core::companions) mod binding;
pub(in crate::core::companions) mod composition;
pub(in crate::core::companions) mod derivations;
pub(in crate::core::companions) mod instantiation;
pub(in crate::core::companions) mod invocation;

impl crate::core::companions::Driver {
    /// `submit PATH`: one ordinary submission with no contract profile.
    pub(in crate::core::companions) fn op_submit(
        &mut self,
        args: &[&str],
        plain: bool,
    ) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
        let path = std::path::PathBuf::from(args[0]);
        let source = attempt!(crate::core::companions::entry::read_bounded(
            &path,
            crate::core::companions::SOURCE_LIMIT,
        )
        .map_err(crate::core::companions::Refused::script));
        let input = self.stack.clone();
        let report = attempt!(self.execute(&source, &input, &[]));
        let outcome = report
            .member("outcome")
            .and_then(crate::core::report::Value::text)
            .unwrap_or_default()
            .to_owned();
        Ok(crate::core::companions::reporting::line(
            crate::core::companions::reporting::Header {
                operation: if plain { "plain" } else { "submit" },
                outcome: &outcome,
            },
            std::vec::Vec::from([
                (
                    "stack",
                    attempt!(crate::core::companions::reporting::values::stack_json(
                        &report
                    )),
                ),
                (
                    "output",
                    crate::core::companions::reporting::values::top_value(&report),
                ),
                (
                    "module",
                    crate::workflow::encoding::string(
                        crate::core::companions::reporting::values::text_of(attempt!(report
                            .member("module")
                            .ok_or_else(|| {
                                crate::core::companions::Refused::script(
                                    "runtime omitted module provenance",
                                )
                            }))),
                    ),
                ),
                (
                    "guest_requests",
                    attempt!(crate::core::companions::reporting::runtime_count(
                        &report,
                        "guest_requests"
                    )),
                ),
                (
                    "protected_operations",
                    attempt!(crate::core::companions::reporting::runtime_count(
                        &report,
                        "protected_operations",
                    )),
                ),
                (
                    "behavioral_certification_claimed",
                    crate::workflow::encoding::Json::Bool(false),
                ),
                (
                    "contract_profile",
                    crate::workflow::encoding::string(if plain { "absent" } else { "available" }),
                ),
                ("prover_calls", crate::workflow::encoding::Json::Number(0)),
                (
                    "candidate_prepare_requests",
                    crate::workflow::encoding::Json::Number(0),
                ),
            ]),
        ))
    }

    /// `stack`: report the live stack without executing anything.
    pub(in crate::core::companions) fn op_stack(
        &mut self,
    ) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
        let report = attempt!(self.read_stack());
        Ok(crate::core::companions::reporting::line(
            crate::core::companions::reporting::Header {
                operation: "stack",
                outcome: "reported",
            },
            std::vec::Vec::from([
                (
                    "stack",
                    attempt!(crate::core::companions::reporting::values::stack_json(
                        &report
                    )),
                ),
                ("guest_requests", crate::workflow::encoding::Json::Number(0)),
                (
                    "protected_operations",
                    crate::workflow::encoding::Json::Number(0),
                ),
            ]),
        ))
    }

    /// `policy N`: select the consumer policy revision. Adopting a different
    /// revision invalidates every applicability decision made under the old
    /// one, so retained evidence reports `stale-context` until revalidated.
    pub(in crate::core::companions) fn op_policy(
        &mut self,
        args: &[&str],
    ) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
        let policy = attempt!(args[0].parse::<u32>().map_err(|_| {
            crate::core::companions::Refused::script(std::format!(
                "'{}' is not a policy revision",
                args[0]
            ))
        }));
        self.core.set_policy(policy);
        self.policy = policy;
        Ok(crate::core::companions::reporting::line(
            crate::core::companions::reporting::Header {
                operation: "policy",
                outcome: "selected",
            },
            std::vec::Vec::from([
                (
                    "policy_revision",
                    crate::workflow::encoding::Json::Number(u64::from(policy)),
                ),
                ("prover_calls", crate::workflow::encoding::Json::Number(0)),
                ("guest_requests", crate::workflow::encoding::Json::Number(0)),
            ]),
        ))
    }

    /// `inspect INDEX`: retained metadata only; no fetch, no authority.
    pub(in crate::core::companions) fn op_inspect(
        &mut self,
        args: &[&str],
    ) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
        let index = attempt!(crate::core::companions::reporting::index_of(
            args[0],
            self.stack.len()
        ));
        let report = attempt!(self.read_stack());
        let entry = attempt!(report
            .member("stack")
            .and_then(crate::core::report::Value::items)
            .and_then(|items| items.get(index))
            .ok_or_else(|| {
                crate::core::companions::Refused::script(std::format!("stack has no slot {index}"))
            }));
        Ok(crate::core::companions::reporting::line(
            crate::core::companions::reporting::Header {
                operation: "inspect",
                outcome: "inspected",
            },
            std::vec::Vec::from([
                (
                    "type",
                    crate::workflow::encoding::string(
                        entry
                            .member("type")
                            .and_then(crate::core::report::Value::text)
                            .unwrap_or_default(),
                    ),
                ),
                (
                    "detail",
                    crate::workflow::encoding::string(
                        crate::core::companions::reporting::values::text_of(entry),
                    ),
                ),
                (
                    "implicit_evidence_fetches",
                    crate::workflow::encoding::Json::Number(0),
                ),
                (
                    "authority_created",
                    crate::workflow::encoding::Json::Bool(false),
                ),
                ("prover_calls", crate::workflow::encoding::Json::Number(0)),
            ]),
        ))
    }

    /// `project INDEX`: explicit projection of a companion to its subject.
    pub(in crate::core::companions) fn op_project(
        &mut self,
        args: &[&str],
    ) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
        let index = attempt!(crate::core::companions::reporting::index_of(
            args[0],
            self.stack.len()
        ));
        let companion = attempt!(self.certified_at(index));
        let subject = companion.subject;
        let contract = companion.contract.0;
        let identity = companion.identity;
        let ty = attempt!(self
            .program_type(subject)
            .ok_or_else(|| crate::core::companions::Refused::script("subject is not retained")));
        attempt!(self.push_cell(
            &crate::core::companions::reporting::inputs::program_injection(subject),
            ty,
        ));
        Ok(crate::core::companions::reporting::line(
            crate::core::companions::reporting::Header {
                operation: "project",
                outcome: "projected",
            },
            std::vec::Vec::from([
                (
                    "subject_handle",
                    crate::workflow::encoding::Json::Number(subject),
                ),
                (
                    "contract_reference",
                    crate::workflow::encoding::Json::Number(u64::from(contract)),
                ),
                (
                    "subject_identity",
                    crate::workflow::encoding::string(identity),
                ),
                (
                    "identity_unchanged",
                    crate::workflow::encoding::Json::Bool(true),
                ),
                ("prover_calls", crate::workflow::encoding::Json::Number(0)),
                ("guest_requests", crate::workflow::encoding::Json::Number(0)),
            ]),
        ))
    }
}
