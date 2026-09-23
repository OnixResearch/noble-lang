mod binding;
mod stack;

impl crate::core::companions::Driver {
    pub(in crate::core::companions) fn new(timeout_ms: u64, optimized: bool) -> Self {
        Self {
            frontend: noble_contracts::source::Session::new(),
            compiler: noble_wasm::source::Compiler::new(),
            stack: std::vec::Vec::new(),
            programs: std::vec::Vec::new(),
            worker: None,
            submissions: 0,
            core: noble_contracts::companion::Core::new(crate::core::EVIDENCE_LIMITS),
            records: std::vec::Vec::new(),
            policy: 0,
            timeout_ms,
            optimized,
        }
    }

    pub(in crate::core::companions) fn options(&self) -> crate::core::arguments::Options {
        crate::core::arguments::Options {
            source: None,
            compile_only: false,
            inputs: std::vec::Vec::new(),
            framed: false,
            optimized: self.optimized,
            emit: None,
            limits: crate::core::SOURCE_LIMITS,
        }
    }

    pub(in crate::core::companions) fn engine(
        &mut self,
    ) -> crate::core::companions::Attempt<&mut crate::core::worker::Engine> {
        if self.worker.is_none() {
            self.worker = Some(attempt!(crate::core::worker::Engine::start(
                &self.options()
            )
            .map_err(crate::core::companions::Refused::engine)));
        }
        self.worker.as_mut().ok_or_else(|| {
            crate::core::companions::Refused::new(
                "malformed",
                "engine",
                std::string::String::from("missing engine"),
            )
        })
    }

    /// Preparation stays private until the worker accepts the module. The
    /// session stack advances only on a normal execution result.
    pub(in crate::core::companions) fn execute(
        &mut self,
        source: &[u8],
        input: &[noble_kernel::types::Ty],
        injections: &[crate::workflow::encoding::Json],
    ) -> crate::core::companions::Attempt<crate::core::report::Value> {
        let (prepared, compiled) = attempt!(self.prepare_submission(source, input));
        let submission_number = self.submissions;
        let prepared_report = attempt!(attempt!(self.engine())
            .prepare(compiled.wat(), source, submission_number)
            .map_err(crate::core::companions::Refused::engine));
        if prepared_report.outcome != "ready" {
            return crate::core::companions::reporting::parse_report(&prepared_report.json);
        }
        let output = prepared.output().to_vec();
        attempt!(self.compiler.commit(compiled).map_err(|diagnostic| {
            crate::core::companions::Refused::new(
                "backend-error",
                "backend",
                std::string::String::from(crate::core::companions::reporting::backend_diagnostic(
                    diagnostic,
                )),
            )
        }));
        attempt!(self.frontend.commit(prepared).map_err(|error| {
            crate::core::companions::Refused::new(
                "source-error",
                "source",
                std::format!("{error:?}"),
            )
        }));
        let reply = attempt!(if injections.is_empty() {
            attempt!(self.engine()).execute()
        } else {
            let frame = crate::core::companions::reporting::inputs::injection_frame(injections);
            attempt!(self.engine()).execute_inputs(&frame)
        }
        .map_err(crate::core::companions::Refused::engine));
        let report = attempt!(crate::core::companions::reporting::parse_report(
            &reply.json
        ));
        if reply.outcome == "normal" {
            self.stack = output;
            attempt!(self.record_programs(&report));
        }
        Ok(report)
    }

    fn prepare_submission(
        &mut self,
        source: &[u8],
        input: &[noble_kernel::types::Ty],
    ) -> crate::core::companions::Attempt<(
        noble_contracts::source::Prepared,
        noble_wasm::source::Prepared,
    )> {
        self.submissions = attempt!(self.submissions.checked_add(1).ok_or_else(|| {
            crate::core::companions::Refused::new(
                "refused",
                "exhausted",
                std::string::String::from("submission counter exhausted"),
            )
        }));
        let limits = crate::core::SOURCE_LIMITS;
        let prepared = attempt!(self
            .frontend
            .prepare(source, input, limits)
            .map_err(|error| {
                crate::core::companions::Refused::new(
                    "source-error",
                    "source",
                    std::format!("{error:?}"),
                )
            }));
        if prepared.is_definition() {
            attempt!(self.frontend.commit(prepared).map_err(|error| {
                crate::core::companions::Refused::new(
                    "source-error",
                    "source",
                    std::format!("{error:?}"),
                )
            }));
            return Err(crate::core::companions::Refused::new(
                "defined",
                "definition",
                std::string::String::from("a definition submission has no stack effect"),
            ));
        }
        let submission = attempt!(prepared.submission().ok_or_else(|| {
            crate::core::companions::Refused::new(
                "malformed",
                "internal",
                std::string::String::from("missing expression candidate"),
            )
        }));
        let compiled = attempt!(self.compiler.prepare(submission).map_err(|diagnostic| {
            crate::core::companions::Refused::new(
                "backend-error",
                "backend",
                std::string::String::from(crate::core::companions::reporting::backend_diagnostic(
                    diagnostic,
                )),
            )
        }));

        Ok((prepared, compiled))
    }
}
