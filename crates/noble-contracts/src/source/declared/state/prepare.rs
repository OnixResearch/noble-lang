impl super::ModuleSession {
    pub fn prepare(
        &self,
        input_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
    ) -> Result<super::ModulePrepared, crate::source::Error> {
        let limit_bytes = attempt!(usize::try_from(limits.bytes).map_err(|_| {
            crate::source::declared::error(
                crate::source::Stage::Parse,
                "source byte limit exceeds the host address space",
            )
        }));
        if input_bytes.len() > limit_bytes {
            return Err(crate::source::declared::error(
                crate::source::Stage::Parse,
                "source byte limit exceeded",
            ));
        }
        let unit = attempt!(
            crate::source::declared::parsing::parse(input_bytes, limits).map_err(|problem| {
                crate::source::declared::diagnostic(crate::source::Stage::Parse, problem)
            })
        );
        let (staged, kind, submission, output) =
            attempt!(self
                .snapshot()
                .stage_unit(unit, input_bytes, inputs, limits));
        attempt!(staged.retained(limits));
        Ok(super::ModulePrepared {
            kind,
            generation: self.generation,
            previous_modules: self.modules.clone(),
            previous_aliases: self.aliases.clone(),
            previous_history: self.source.history.clone(),
            staged,
            submission,
            output,
        })
    }

    fn stage_unit(
        mut self,
        unit: crate::source::declared::parsing::ParsedUnit,
        input_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
    ) -> Result<
        (
            Self,
            super::ModuleKind,
            Option<noble_kernel::execution::Submission>,
            alloc::vec::Vec<noble_kernel::types::Ty>,
        ),
        crate::source::Error,
    > {
        let (kind, submission, output) = match unit {
            crate::source::declared::parsing::ParsedUnit::Module {
                name,
                version,
                members,
            } => {
                self = attempt!(self.register(super::register::Registration {
                    name,
                    version,
                    members,
                    source: input_bytes,
                    inputs,
                    limits,
                }));
                (super::ModuleKind::Module, None, alloc::vec::Vec::new())
            }
            crate::source::declared::parsing::ParsedUnit::Import {
                name,
                version,
                alias,
            } => {
                self = attempt!(self.import(&name, version, alias));
                (super::ModuleKind::Import, None, inputs.to_vec())
            }
            crate::source::declared::parsing::ParsedUnit::Definition
            | crate::source::declared::parsing::ParsedUnit::Expression => {
                let mut prepared = attempt!(self.source.prepare(input_bytes, inputs, limits));
                let kind = if prepared.is_definition() {
                    super::ModuleKind::Definition
                } else {
                    super::ModuleKind::Expression
                };
                if prepared
                    .definition
                    .as_ref()
                    .is_some_and(|definition| self.reserved_word(&definition.name))
                {
                    return Err(crate::source::declared::error(
                        crate::source::Stage::Resolve,
                        "definition conflicts with a qualified module export",
                    ));
                }
                let submission = prepared.submission.take();
                let output = core::mem::take(&mut prepared.output);
                attempt!(self.source.commit(prepared));
                if kind == super::ModuleKind::Definition {
                    self = attempt!(self.outside_words());
                }
                (kind, submission, output)
            }
        };
        Ok((self, kind, submission, output))
    }

    /// Publish the entire scanned declaration/import as one stale-checked transition.
    /// The returned session is the prior one on refusal and the staged one on success.
    pub fn commit(
        self,
        mut prepared: super::ModulePrepared,
    ) -> (Self, Result<(), crate::source::Error>) {
        let is_current_snapshot = self.generation == prepared.generation
            && self.bindings == prepared.staged.bindings
            && self.modules == prepared.previous_modules
            && self.aliases == prepared.previous_aliases
            && self.source.history == prepared.previous_history;
        if !is_current_snapshot {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "stale or foreign module namespace snapshot",
                )),
            );
        }
        let Some(next) = self.generation.checked_add(1) else {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "generation limit exceeded",
                )),
            );
        };
        prepared.staged.generation = next;
        (prepared.staged, Ok(()))
    }
}
