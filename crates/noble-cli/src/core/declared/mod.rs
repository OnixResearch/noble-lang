//! Declared-Modules-v1 namespace transactions and selected-worker expression execution.

impl super::Session {
    fn commit_declared(
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
                ))
            }
        };
        match prepared.kind() {
            noble_contracts::source::ModuleKind::Module
            | noble_contracts::source::ModuleKind::Import => self.link_declared(prepared),
            noble_contracts::source::ModuleKind::Definition => self.define_declared(prepared),
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
    ) -> Result<super::output::Report, super::output::Failure> {
        let message = if prepared.kind() == noble_contracts::source::ModuleKind::Module {
            "module registered without guest execution"
        } else {
            "module import linked without guest execution"
        };
        let module = prepared.resolved_module();
        let binding = prepared
            .linked_binding()
            .map(|binding| (binding.adapter_identity.clone(), binding.adapter_slot));
        match attempt!(self.commit_declared(prepared)) {
            Ok(()) => Ok(super::output::Report::declared_link(
                self.submissions,
                message,
                module,
                binding,
            )),
            Err(error) => Ok(super::output::Report::declared_source_error(
                &error,
                self.submissions,
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
    ) -> Result<super::output::Report, super::output::Failure> {
        match attempt!(self.commit_declared(prepared)) {
            Ok(()) => Ok(super::output::Report::declared_definition(self.submissions)),
            Err(error) => Ok(super::output::Report::declared_source_error(
                &error,
                self.submissions,
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
        let compiled = match self.compiler.prepare(submission) {
            Ok(compiled) => compiled,
            Err(error) => {
                return Ok(super::output::Report::declared_backend_error(
                    error,
                    self.submissions,
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
        let report = attempt!(self
            .declared_worker()
            .and_then(super::worker::Engine::execute));
        if report.outcome == "normal" {
            self.stack = output;
        }
        Ok(report)
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
