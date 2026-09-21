impl super::Session {
    pub fn prepare(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
    ) -> Result<super::Prepared, super::Error> {
        let mut meter = crate::Meter::new(limits);
        let (mut tree, name) = match super::parsing::parse(source_bytes, &mut meter) {
            Ok(parsed) => parsed,
            Err(error) => return Err(super::Error::at(super::Stage::Parse, error)),
        };
        if let Err(error) = super::resolution::resolve(&mut tree, name.as_deref(), self, &mut meter)
        {
            return Err(super::Error::at(super::Stage::Resolve, error));
        }
        if let Err(error) = super::preflight::check(inputs, self.hosts, tree.span, &mut meter) {
            return Err(super::Error::at(super::Stage::Check, error));
        }
        let extra = if name.is_some() {
            source_bytes.len().saturating_add(4)
        } else {
            0
        };
        if let Err(error) = self.retained(extra, tree.span, &mut meter) {
            return Err(super::Error::at(super::Stage::Check, error));
        }
        let environment = match super::environment() {
            Ok(environment) => environment,
            Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
        };
        let mode = if name.is_some() {
            super::inference::Mode::Declaration
        } else {
            super::inference::Mode::Submission
        };
        let scope = super::inference::Scope {
            root: &tree,
            session: self,
            environment: &environment,
        };
        let state = match super::inference::infer(scope, mode, inputs, &mut meter) {
            Ok(state) => state,
            Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
        };
        let mut addition = alloc::vec::Vec::new();
        let (definition, submission, output) = match name {
            Some(name) => {
                let definition = match declaration(tree, name, source_bytes, self, &mut meter) {
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
            definition,
            addition,
            submission,
            output,
        })
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
    session: &super::Session,
    meter: &mut crate::Meter,
) -> Result<(super::Named, alloc::vec::Vec<u8>), crate::Diagnostic> {
    // Inference has solved the open graph, including every dependency body.
    // No Unit instance decides generic validity.
    let identity = attempt!(super::resolution::identity(&tree, session, meter));
    let count = attempt!(crate::index(source_bytes.len(), tree.span));
    attempt!(meter.charge(count.saturating_add(4), tree.span));
    let mut addition = alloc::vec::Vec::with_capacity(source_bytes.len().saturating_add(4));
    addition.extend_from_slice(&count.to_le_bytes());
    addition.extend_from_slice(source_bytes);
    Ok((
        super::Named {
            name,
            identity,
            tree,
        },
        addition,
    ))
}
