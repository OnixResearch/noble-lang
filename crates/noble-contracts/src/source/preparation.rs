#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; preparation's mutable parameters are only its request-local meter; draft trees and output buffers are newly owned, while source bytes, supplied types, and the committed session are borrowed unchanged until an explicit later commit."
)]

impl super::Session {
    pub fn prepare(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
    ) -> Result<super::Prepared, super::Error> {
        self.prepare_signed(source_bytes, inputs, None, limits)
    }

    pub(super) fn prepare_signed(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        signature: Option<&noble_kernel::words::Scheme>,
        limits: crate::Limits,
    ) -> Result<super::Prepared, super::Error> {
        let mut meter = crate::Meter::new(limits);
        let parsed = if self.declared.is_some() {
            super::parsing::parse_declared(source_bytes, &mut meter)
        } else {
            super::parsing::parse(source_bytes, &mut meter)
        };
        let (mut tree, name) = match parsed {
            Ok(parsed) => parsed,
            Err(error) => return Err(super::Error::at(super::Stage::Parse, error)),
        };
        if let Err(error) = super::resolution::resolve(&mut tree, name.as_deref(), self, &mut meter)
        {
            return Err(super::Error::at(super::Stage::Resolve, error));
        }
        if let Err(error) = super::preflight::check(inputs, self, tree.span, &mut meter) {
            return Err(super::Error::at(super::Stage::Check, error));
        }
        let (extra, mode) = if name.is_some() {
            (
                source_bytes.len().saturating_add(4),
                super::inference::Mode::Declaration,
            )
        } else {
            (0, super::inference::Mode::Submission)
        };
        if let Err(error) = self.retained(extra, tree.span, &mut meter) {
            return Err(super::Error::at(super::Stage::Check, error));
        }
        let environment = match self.environment() {
            Ok(environment) => environment,
            Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
        };
        let scope = super::inference::Scope {
            root: &tree,
            session: self,
            environment: &environment,
            root_signature: signature,
        };
        let state = match super::inference::infer(scope, mode, inputs, &mut meter) {
            Ok(state) => state,
            Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
        };
        let mut addition = alloc::vec::Vec::new();
        let (definition, submission, output) = match name {
            Some(name) => {
                let definition =
                    match declaration(tree, name, source_bytes, signature, self, &mut meter) {
                        Ok((definition, bytes)) => {
                            addition = bytes;
                            definition
                        }
                        Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
                    };
                (Some(definition), None, inputs.to_vec())
            }
            None => {
                let (submission, output) = attempt!(super::emission::emit(
                    state,
                    environment,
                    source_bytes.len(),
                    &mut meter
                ));
                (None, Some(submission), output)
            }
        };
        Ok(super::Prepared {
            generation: self.generation,
            history: self.history.clone(),
            hosts: self.hosts,
            text_cursor: self.text_cursor,
            live_selected: self.live_selected.clone(),
            limits,
            boundary: self.bindings.as_ref().map(|bindings| bindings.key.clone()),
            definition,
            addition,
            submission,
            output,
        })
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; preparation owns a runtime clone of the retained binding environment or builds the checked bootstrap environment, both allocating operations."
    )]
    pub(super) fn environment(&self) -> Result<noble_kernel::contracts::Env, crate::Diagnostic> {
        if let Some(declared) = &self.declared {
            return Ok(declared.environment.clone());
        }
        match &self.bindings {
            Some(bindings) => Ok(bindings.environment.clone()),
            None if self.text_cursor => noble_kernel::contracts::text_cursor_environment()
                .map_err(|_| crate::internal(crate::Span { start: 0, end: 0 })),
            None if self.live_selected.is_some() => super::live_environment(),
            None => super::environment(),
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; retained-byte admission uses checked conversions and metering whose rejection paths construct owned diagnostics."
    )]
    fn retained(
        &self,
        extra: usize,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        let history_bytes = attempt!(crate::index(self.history.len(), span));
        let limit_bytes = attempt!(crate::offset(meter.limits.bytes, span));
        if self.history.len().saturating_add(extra) > limit_bytes {
            return Err(super::exhausted(
                span,
                "retained namespace byte limit exceeded",
            ));
        }
        meter.charge(history_bytes, span)
    }
}

fn declaration(
    tree: super::Tree,
    name: alloc::string::String,
    source_bytes: &[u8],
    signature: Option<&noble_kernel::words::Scheme>,
    session: &super::Session,
    meter: &mut crate::Meter,
) -> Result<(super::Named, alloc::vec::Vec<u8>), crate::Diagnostic> {
    // Inference has solved the open graph, including every dependency body.
    // No Unit instance decides generic validity.
    let identity = attempt!(super::resolution::comparison::identity(
        &tree, session, signature, meter
    ));
    let count = attempt!(crate::index(source_bytes.len(), tree.span));
    attempt!(meter.charge(count.saturating_add(4), tree.span));
    let mut addition = alloc::vec::Vec::with_capacity(source_bytes.len().saturating_add(4));
    addition.extend_from_slice(&count.to_le_bytes());
    addition.extend_from_slice(source_bytes);
    Ok((
        super::Named {
            name,
            identity,
            owner: session.declared.as_ref().and_then(|context| context.owner),
            tree,
            signature: signature.cloned(),
        },
        addition,
    ))
}
