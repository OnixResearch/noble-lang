//! Declared-Modules-v1 namespace transactions and selected-worker expression execution.

impl super::Session {
    pub(super) fn commit_declared(
        &mut self,
        prepared: noble_contracts::source::ModulePrepared,
    ) -> Result<Result<(), noble_contracts::source::Error>, super::output::Failure> {
        let previous = attempt!(self.declared.take().ok_or_else(|| {
            super::output::Failure::new(
                super::output::ErrorContext {
                    stage: "session",
                    outcome: "internal-failure",
                },
                "declared namespace is unavailable",
            )
        }));
        let (next, outcome) = previous.commit(prepared);
        self.declared = Some(next);
        Ok(outcome)
    }

    pub(super) fn commit_declared_proofs(
        &mut self,
        prepared: noble_contracts::source::ModulePrepared,
        source: &[u8],
        limits: noble_contracts::Limits,
    ) -> Result<(Result<(), noble_contracts::source::Error>, crate::workflow::encoding::Json), super::output::Failure> {
        let previous = attempt!(self.declared.take().ok_or_else(|| {
            super::output::Failure::new(
                super::output::ErrorContext { stage: "session", outcome: "internal-failure" },
                "declared namespace is unavailable",
            )
        }));
        let (next, outcome, report) = Self::prepared_declared_proofs(previous, prepared, source, limits);
        self.declared = Some(next);
        Ok((outcome,report))
    }

    pub(super) fn prepared_declared_proofs(
        previous: noble_contracts::source::ModuleSession,
        prepared: noble_contracts::source::ModulePrepared,
        source: &[u8],
        limits: noble_contracts::Limits,
    ) -> (noble_contracts::source::ModuleSession,
        Result<(), noble_contracts::source::Error>, crate::workflow::encoding::Json) {
        let mut report = crate::workflow::encoding::Json::Null;
        let (next, outcome) = previous.commit_verified(prepared, |batch| {
            let (checked,evidence,_) = crate::workflow::intrinsic::verify_with_evidence(
                batch, source, limits, crate::workflow::DEFAULT_TIMEOUT,
                noble_contracts::intrinsic::ProofBudgets::from_limits(limits))
                .map_err(|problem| std::format!("{}: {}",problem.code,problem.message))?;
            report = crate::workflow::intrinsic::session_proof_report(batch,&checked,&evidence);
            Ok(checked)
        });
        (next,outcome,report)
    }

    pub(super) fn submit_declared(
        &mut self,
        source: &[u8],
        options: &super::arguments::Options,
    ) -> Result<super::output::Report, super::output::Failure> {
        let frontend = attempt!(self.declared_namespace());
        let prepared = match frontend.prepare(source, &self.stack, options.limits) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Ok(super::output::Report::declared_source_error(
                    &error,
                    self.submissions,
                    options.signed_self_recursion,
                    options.declared_text(),
                ))
            }
        };
        match prepared.kind() {
            noble_contracts::source::ModuleKind::Module
            | noble_contracts::source::ModuleKind::Import => self.link_declared(prepared,source,options),
            noble_contracts::source::ModuleKind::Definition => self.define_declared(prepared, options),
            noble_contracts::source::ModuleKind::Expression => {
                self.execute_declared(prepared, source, options)
            }
        }
    }

    fn declared_namespace(
        &self,
    ) -> Result<&noble_contracts::source::ModuleSession, super::output::Failure> {
        self.declared.as_ref().ok_or_else(|| {
            super::output::Failure::new(
                super::output::ErrorContext {
                    stage: "session",
                    outcome: "internal-failure",
                },
                "declared namespace is unavailable",
            )
        })
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; linking clones host-binding identity and commits the runtime module namespace before allocating its actual outcome report."
    )]
    fn link_declared(
        &mut self,
        prepared: noble_contracts::source::ModulePrepared,
        source: &[u8],
        options: &super::arguments::Options,
    ) -> Result<super::output::Report, super::output::Failure> {
        let has_proofs = prepared.proof_obligations().is_some();
        let message = if has_proofs {
            "module registered after independent source proof recheck"
        } else if prepared.kind() == noble_contracts::source::ModuleKind::Module {
            "module registered without guest execution"
        } else {
            "module import linked without guest execution"
        };
        let module = prepared.resolved_module();
        let binding = prepared
            .linked_binding()
            .map(|binding| (binding.adapter_identity.clone(), binding.adapter_slot));
        let mut contract_reports = std::vec::Vec::new();
        for goal in prepared.contract_goals() {
            let generated_statement = match goal.revision {
                1 => noble_contracts::intrinsic::prepare_contract(goal, options.limits)
                    .map(|typed| noble_contracts::export_lean(&typed)),
                2 => noble_contracts::intrinsic::prepare_named_contract(goal, options.limits),
                _ => return Err(super::output::Failure::new(
                    super::output::ErrorContext { stage: "check", outcome: "internal-failure" },
                    "unsupported contract revision",
                )),
            };
            let generated_statement = attempt!(generated_statement.map_err(|problem|
                super::output::Failure::new(
                    super::output::ErrorContext { stage: "check", outcome: "internal-failure" },
                    problem.message,
                )));
            let mut fields=std::vec::Vec::from([
                ("name",crate::workflow::encoding::string(&goal.contract_name)),
                ("definition",crate::workflow::encoding::string(&goal.subject.definition)),
                ("definition_identity",crate::workflow::encoding::string(goal.subject.definition_identity.to_string())),
                ("module_source_sha256",crate::workflow::encoding::string(crate::workflow::intrinsic::sha256(&goal.subject.module_source))),
                ("definition_source_sha256",crate::workflow::encoding::string(crate::workflow::intrinsic::sha256(&goal.subject.definition_source))),
            ]);
            if goal.revision == 2 {
                fields.push(("generated_statement_sha256",crate::workflow::encoding::string(
                    crate::workflow::intrinsic::sha256(generated_statement.as_bytes()))));
                fields.push(("generated_claim",crate::workflow::encoding::string("NamedV2Obligation.claim")));
            } else {
                fields.push(("generated_statement",crate::workflow::encoding::string(&generated_statement)));
            }
            fields.push(("accepted_recipe",crate::workflow::encoding::string(
                std::format!("{:?}",goal.subject.accepted_submission.definitions))));
            if goal.revision == 2 {
                fields.push(("revision",crate::workflow::encoding::Json::Number(2)));
                fields.push(("original_sources",crate::workflow::encoding::Json::Array(
                    goal.subject.source_dependencies.iter().map(|module|
                        crate::workflow::encoding::object([
                            ("module",crate::workflow::encoding::string(&module.module)),
                            ("version",crate::workflow::encoding::Json::Number(u64::from(module.version))),
                            ("owner_session_local",crate::workflow::encoding::Json::Number(module.owner)),
                            ("module_source_sha256",crate::workflow::encoding::string(
                                crate::workflow::intrinsic::sha256(&module.full_source))),
                        ])
                    ).collect()
                )));
            }
            // Each goal is a distinct contract declaration within `source`, so
            // this bound is never reached; it is a separate exit ahead of each push.
            if contract_reports.len() >= source.len() {
                return Err(super::output::Failure::new(
                    super::output::ErrorContext {
                        stage: "check",
                        outcome: "internal-failure",
                    },
                    "contract reports exceed the submitted source",
                ));
            }
            contract_reports.push(crate::workflow::encoding::Json::Object(fields));
        }
        let contracts = crate::workflow::encoding::Json::Array(contract_reports);
        let (committed,proof_report) = if has_proofs {
            attempt!(self.commit_declared_proofs(prepared,source,options.limits))
        } else {
            (attempt!(self.commit_declared(prepared)),crate::workflow::encoding::Json::Null)
        };
        match committed {
            Ok(()) => Ok(super::output::Report::declared_link(
                self.submissions,
                options.signed_self_recursion,
                options.declared_text(),
                message,
                module,
                binding,
                contracts,
                proof_report,
            )),
            Err(error) => Ok(super::output::Report::declared_source_error(
                &error,
                self.submissions,
                options.signed_self_recursion,
                options.declared_text(),
            )),
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; a definition commits mutable runtime namespace state and allocates its outcome report."
    )]
    fn define_declared(
        &mut self,
        prepared: noble_contracts::source::ModulePrepared,
        options: &super::arguments::Options,
    ) -> Result<super::output::Report, super::output::Failure> {
        match attempt!(self.commit_declared(prepared)) {
            Ok(()) => Ok(super::output::Report::declared_definition(
                self.submissions, options.signed_self_recursion, options.declared_text(),
            )),
            Err(error) => Ok(super::output::Report::declared_source_error(
                &error,
                self.submissions,
                options.signed_self_recursion,
                options.declared_text(),
            )),
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; independent compiler preparation checks a submitted expression and constructs a prospective owned Wasm module at runtime."
    )]
    fn execute_declared(
        &mut self,
        prepared: noble_contracts::source::ModulePrepared,
        source: &[u8],
        options: &super::arguments::Options,
    ) -> Result<super::output::Report, super::output::Failure> {
        let submission =
            attempt!(prepared
                .submission()
                .ok_or_else(|| super::output::Failure::new(
                    super::output::ErrorContext {
                        stage: "acceptance",
                        outcome: "internal-failure"
                    },
                    "declared expression produced no candidate",
                )));
        let compiled = if options.declared_text() {
            match prepared.declared_text_source_owner() {
                Some(ref owner) => self.compiler.prepare_declared_text(owner),
                None => Err(noble_wasm::Diagnostic::Invalid),
            }
        } else {
            let signed = prepared.signed_source_owner();
            match signed {
                Some(ref owner) => self.compiler.prepare_signed(owner),
                None => self.compiler.prepare(submission),
            }
        };
        let compiled = match compiled {
            Ok(compiled) => compiled,
            Err(error) => {
                return Ok(super::output::Report::declared_backend_error(
                    error,
                    self.submissions,
                    options.signed_self_recursion,
                    options.declared_text(),
                ))
            }
        };
        self.execute_compiled_declared(prepared, compiled, source, options)
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; this transaction exchanges source and WAT with the selected worker, commits prospective compiler and namespace state only after readiness, and executes the real guest."
    )]
    fn execute_compiled_declared(
        &mut self,
        prepared: noble_contracts::source::ModulePrepared,
        compiled: noble_wasm::source::Prepared,
        source: &[u8],
        options: &super::arguments::Options,
    ) -> Result<super::output::Report, super::output::Failure> {
        let ready = attempt!(self.prepare_declared_worker(compiled.wat(), source, options));
        if ready.outcome != "ready" {
            return Ok(ready);
        }
        let output = prepared.output().to_vec();
        attempt!(self
            .compiler
            .commit(compiled)
            .map_err(super::output::Failure::backend));
        let committed = attempt!(self.commit_declared(prepared));
        attempt!(committed.map_err(super::output::Failure::source));
        let mut report = attempt!(self
            .declared_worker()
            .and_then(super::worker::Engine::execute));
        if report.outcome == "normal" {
            self.stack = output;
            if options.declared_text() && options.signed_self_recursion {
                if let Some(receipt) = attempt!(self.try_retire_declared_text_epoch()) {
                    let mut frame: serde_json::Value = attempt!(serde_json::from_str(&report.json)
                        .map_err(|_| Self::epoch_failure("invalid signed source report")));
                    frame["source_epoch_retirement"] = receipt;
                    report.json = frame.to_string();
                }
            }
        }
        Ok(report)
    }

    fn epoch_failure(message: &'static str) -> super::output::Failure {
        super::output::Failure::new(
            super::output::ErrorContext { stage: "wasm", outcome: "internal-failure" },
            message,
        )
    }

    fn try_retire_declared_text_epoch(
        &mut self,
    ) -> Result<Option<serde_json::Value>, super::output::Failure> {
        let (generation, end) = attempt!(self.compiler.declared_text_epoch_state()
            .ok_or_else(|| Self::epoch_failure("unselected source epoch compiler")));
        let retired = attempt!(self.declared_worker()?.retire_source_epoch(generation, end));
        if retired.outcome == "source-epoch-owned" {
            return Ok(None);
        }
        if retired.outcome != "source-epoch-retired" {
            return Err(Self::epoch_failure("unexpected source epoch disposition"));
        }
        let receipt: serde_json::Value = attempt!(serde_json::from_str(&retired.json)
            .map_err(|_| Self::epoch_failure("invalid source epoch receipt")));
        let cells = receipt["released_cells"].as_u64();
        let before = receipt["released_bytes_before"].as_u64();
        let after = receipt["released_bytes_after"].as_u64();
        if receipt["schema"] != "noble-source-epoch-retirement/v1"
            || receipt["generation"].as_u64() != Some(u64::from(generation))
            || receipt["code_end"].as_u64() != Some(u64::from(end))
            || receipt["heap_cells_after"].as_u64() != Some(0)
            || receipt["heap_cells_before"].as_u64() != cells
            || receipt["cleared_code_entries"].as_u64() != Some(u64::from(end - 4))
            || receipt["source_epoch_before"].as_u64()
                .and_then(|epoch| epoch.checked_add(1)) != receipt["source_epoch_after"].as_u64()
            || receipt["source_store_id"].as_str().is_none_or(|store| store.len() != 36)
            || receipt["retained_core_entries"].as_u64() != Some(4)
            || cells.is_none_or(|count| count > 4096)
            || before.zip(cells).and_then(|(bytes, count)| bytes.checked_add(count * 48)) != after
            || receipt["released_programs"].as_u64().is_none_or(|count| count > 4096)
        {
            return Err(Self::epoch_failure("source epoch did not prove checked code and cell retirement"));
        }
        attempt!(self.compiler.retire_declared_text_epoch(generation, end)
            .map_err(super::output::Failure::backend));
        Ok(Some(receipt))
    }

    pub(super) fn release_declared_observation(
        &mut self,
        store: &str,
        id: u64,
        options: &super::arguments::Options,
    ) -> Result<super::output::Report, super::output::Failure> {
        if !options.declared_text() || !options.signed_self_recursion {
            return Err(Self::epoch_failure("observed-source release requires signed declared text"));
        }
        if !self.stack.is_empty() {
            return Ok(super::output::Report {
                outcome: "source-observation-refused".into(),
                json: serde_json::json!({
                    "schema": "noble-core-report/v1", "profile": "Signed-Declared-Text-v1",
                    "stage": "wasm", "outcome": "source-observation-refused",
                    "diagnostic": "checked source stack still owns observed values",
                    "guest_requests": 0, "protected_operations": 0,
                    "candidate_prepare_requests": 0, "session_state": "retained",
                }).to_string(),
            });
        }
        let mut released = attempt!(self.declared_worker()?.release_source_observation(id, store));
        if released.outcome != "source-observation-released" {
            return Ok(released);
        }
        let mut receipt: serde_json::Value = attempt!(serde_json::from_str(&released.json)
            .map_err(|_| Self::epoch_failure("invalid source observation receipt")));
        if receipt["source_observation_id"].as_u64() != Some(id)
            || receipt["source_store_id"].as_str() != Some(store)
            || receipt["released_handles"].as_u64().is_none_or(|count| count == 0)
        {
            return Err(Self::epoch_failure("source observation release lacked owner proof"));
        }
        if let Some(epoch) = attempt!(self.try_retire_declared_text_epoch()) {
            receipt["source_epoch_retirement"] = epoch;
            released.json = receipt.to_string();
        }
        Ok(released)
    }

    fn prepare_declared_worker(
        &mut self,
        wat: &[u8],
        source: &[u8],
        options: &super::arguments::Options,
    ) -> Result<super::output::Report, super::output::Failure> {
        if self.worker.is_none() {
            self.worker = Some(attempt!(super::worker::Engine::start(options)));
        }
        let submission = self.submissions;
        attempt!(self.declared_worker()).prepare(wat, source, submission)
    }

    fn declared_worker(&mut self) -> Result<&mut super::worker::Engine, super::output::Failure> {
        self.worker.as_mut().ok_or_else(|| {
            super::output::Failure::new(
                super::output::ErrorContext {
                    stage: "wasm",
                    outcome: "internal-failure",
                },
                "selected Wasm engine is unavailable",
            )
        })
    }
}
