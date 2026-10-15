#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; preparation's mutable parameters are only its request-local meter; draft trees and output buffers are newly owned, while source bytes, supplied types, and the committed session are borrowed unchanged until an explicit later commit."
)]

use alloc::borrow::Cow;

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
        self.prepare_signed_metered(source_bytes, inputs, signature, &mut meter)
    }

    pub(super) fn prepare_signed_metered(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        signature: Option<&noble_kernel::words::Scheme>,
        meter: &mut crate::Meter,
    ) -> Result<super::Prepared, super::Error> {
        let parsed = if self.declared.is_some() {
            super::parsing::parse_declared(source_bytes, meter)
        } else {
            super::parsing::parse(source_bytes, meter)
        };
        let (mut tree, name) = match parsed {
            Ok(parsed) => parsed,
            Err(error) => return Err(super::Error::at(super::Stage::Parse, error)),
        };
        if let Err(error) = super::resolution::resolve(&mut tree, name.as_deref(), self, meter) {
            return Err(super::Error::at(super::Stage::Resolve, error));
        }
        if name.is_some()
            && self.live_slots.is_some()
            && tree
                .nodes
                .iter()
                .any(|node| matches!(node.kind, super::Kind::Call(super::Target::SlotInvoke(_))))
        {
            return Err(super::Error::at(
                super::Stage::Check,
                crate::invalid(
                    tree.span,
                    "live-slot declarations cannot contain borrowed dispatch",
                ),
            ));
        }
        if let Err(error) = super::preflight::check(inputs, self, tree.span, meter) {
            return Err(super::Error::at(super::Stage::Check, error));
        }
        let selected_root = if tree.body.len() == 1 {
            tree.nodes
                .get(tree.body[0] as usize)
                .and_then(|node| match &node.kind {
                    super::Kind::Call(super::Target::Named(index)) => Some(*index),
                    _ => None,
                })
        } else {
            None
        };
        let (extra, mode) = if name.is_some() {
            (
                source_bytes.len().saturating_add(4),
                super::inference::Mode::Declaration,
            )
        } else {
            (0, super::inference::Mode::Submission)
        };
        if let Err(error) = self.retained(extra, tree.span, meter) {
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
        let state = match super::inference::infer(scope, mode, inputs, meter) {
            Ok(state) => state,
            Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
        };
        let mut addition = alloc::vec::Vec::new();
        let (definition, submission, checked, output) = match name {
            Some(name) => {
                let definition = match declaration(tree, name, source_bytes, signature, self, meter)
                {
                    Ok((definition, bytes)) => {
                        addition = bytes;
                        definition
                    }
                    Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
                };
                (Some(definition), None, None, inputs.to_vec())
            }
            None => {
                if let Cow::Borrowed(borrowed) = &environment {
                    if borrowed.defs.len()
                        > noble_kernel::contracts::CALCULATOR_ENVIRONMENT_ROW_LIMIT
                    {
                        return Err(super::Error::at(
                            super::Stage::Check,
                            super::exhausted(
                                tree.span,
                                "calculator environment row limit exceeded",
                            ),
                        ));
                    }
                    let budget = match borrowed.calculator_clone_budget() {
                        Some(budget) => budget,
                        None => {
                            return Err(super::Error::at(
                                super::Stage::Check,
                                super::exhausted(
                                    tree.span,
                                    "calculator environment clone bound exceeded",
                                ),
                            ))
                        }
                    };
                    if budget.reserved_bytes > u64::from(meter.limits.work) {
                        return Err(super::Error::at(
                            super::Stage::Check,
                            super::exhausted(
                                tree.span,
                                "calculator environment clone work exceeded",
                            ),
                        ));
                    }
                    let bytes = match u32::try_from(budget.bytes) {
                        Ok(bytes) => bytes,
                        Err(_) => {
                            return Err(super::Error::at(
                                super::Stage::Check,
                                super::exhausted(
                                    tree.span,
                                    "calculator environment clone work exceeded",
                                ),
                            ))
                        }
                    };
                    if let Err(error) = meter.charge(bytes, tree.span) {
                        return Err(super::Error::at(super::Stage::Check, error));
                    }
                }
                let (submission, checked, output) = attempt!(super::emission::emit(
                    state,
                    environment.into_owned(),
                    source_bytes.len(),
                    meter
                ));
                (None, Some(submission), Some(checked), output)
            }
        };
        Ok(super::Prepared {
            generation: self.generation,
            history: self.history.clone(),
            hosts: self.hosts,
            text_cursor: self.text_cursor,
            live_selected: self.live_selected.clone(),
            live_slots: self.live_slots.is_some(),
            limits: meter.limits,
            calculator: self.calculator,
            boundary: self.bindings.as_ref().map(|bindings| bindings.key.clone()),
            definition,
            selected_root,
            addition,
            submission,
            checked,
            output,
        })
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; calculator declared inference borrows its retained environment, while ordinary profiles still clone bindings or build a checked bootstrap environment."
    )]
    pub(super) fn environment(
        &self,
    ) -> Result<Cow<'_, noble_kernel::contracts::Env>, crate::Diagnostic> {
        if let Some(declared) = &self.declared {
            if self.calculator {
                return Ok(Cow::Borrowed(&declared.environment));
            }
            return Ok(Cow::Owned(declared.environment.clone()));
        }
        if let Some(profile) = &self.live_slots {
            return Ok(Cow::Owned(profile.environment.clone()));
        }
        match &self.bindings {
            Some(bindings) => Ok(Cow::Owned(bindings.environment.clone())),
            None if self.calculator => noble_kernel::contracts::calculator_environment()
                .map(Cow::Owned)
                .map_err(|_| crate::internal(crate::Span { start: 0, end: 0 })),
            None if self.text_cursor => noble_kernel::contracts::text_cursor_environment()
                .map(Cow::Owned)
                .map_err(|_| crate::internal(crate::Span { start: 0, end: 0 })),
            None if self.live_selected.is_some() => super::live_environment().map(Cow::Owned),
            None => super::environment().map(Cow::Owned),
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
