impl super::World {
    /// Compile a source body on an isolated stack containing exactly the WIT
    /// parameters. The complete inferred normal stack must equal the WIT result.
    /// All requests retain their versioned effects; no host executes here.
    pub fn prepare_export(
        &self,
        name: &str,
        source_bytes: &[u8],
        limits: crate::Limits,
    ) -> Result<super::CheckedExport, super::Error> {
        let index = match export_position(&self.exports, name) {
            Some(index) => index,
            None => {
                return Err(super::error(
                    super::Stage::Export,
                    crate::DiagnosticKind::Invalid,
                    "unknown selected-world export",
                ))
            }
        };
        let operation = &self.exports[index];
        let bindings = attempt!(super::bindings::make(self));
        let callable_effects = bindings.effects;
        let session = crate::source::Session::with_bindings(bindings);
        let input = attempt!(operation.input_types(self).ok_or_else(|| super::error(
            super::Stage::Binding,
            crate::DiagnosticKind::Invalid,
            "unsigned WIT requires the selected checked boundary",
        )));
        let output = attempt!(operation.output_types(self).ok_or_else(|| super::error(
            super::Stage::Binding,
            crate::DiagnosticKind::Invalid,
            "unsigned WIT requires the selected checked boundary",
        )));
        let prepared = match session.prepare(source_bytes, &input, limits) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(super::Error {
                    stage: super::Stage::Export,
                    diagnostic: error.diagnostic().clone(),
                })
            }
        };
        if prepared.is_definition() || prepared.output() != output {
            return Err(super::error(
                super::Stage::Export,
                crate::DiagnosticKind::Invalid,
                "Noble export does not have the closed exact WIT interface",
            ));
        }
        let submission = attempt!(prepared.submission().ok_or_else(|| super::error(
            super::Stage::Export,
            crate::DiagnosticKind::Internal,
            "missing checked export body",
        )));
        // Type-known latent Program effects never become guest-callable imports.
        if !effects_callable(
            submission.request.expected.allowed_effects.as_slice(),
            callable_effects,
        ) {
            return Err(super::error(
                super::Stage::Export,
                crate::DiagnosticKind::Invalid,
                "Noble export effect exceeds selected WIT imports",
            ));
        }
        Ok(super::CheckedExport {
            world: self.build_context(),
            index,
            source_bytes: source_bytes.to_vec(),
            prepared,
        })
    }

    /// Static effect claim check for the generated operation. A reviewed-pure
    /// host still requires this exact WitOpId. Import availability grants no authority.
    ///
    /// Reviewed extraction exclusion: `&[&str]` is a nested borrow the pinned
    /// Aeneas cannot represent; the checked logic is `import_identity` and
    /// `import_effect_verdict`.
    #[charon::exclude]
    pub fn check_import_effect(&self, word: &str, claimed: &[&str]) -> Result<(), super::Error> {
        if claimed.len() > super::MAX_OPERATIONS {
            return Err(super::exhausted());
        }
        let identity = attempt!(self.import_identity(word));
        let mut is_claimed = false;
        let mut at = 0;
        while !is_claimed && at < claimed.len() {
            is_claimed = claimed[at] == identity;
            at += 1;
        }
        import_effect_verdict(is_claimed)
    }

    /// The semantic identity of the generated import named `word`.
    fn import_identity(&self, word: &str) -> Result<&str, super::Error> {
        let mut index = None;
        let mut at = 0;
        while index.is_none() && at < self.imports.len() {
            if has_word(&self.imports[at], word) {
                index = Some(at);
            }
            at += 1;
        }
        match index {
            Some(index) => Ok(&self.imports[index].identity),
            None => Err(super::error(
                super::Stage::Binding,
                crate::DiagnosticKind::Invalid,
                "unknown generated import word",
            )),
        }
    }
}

/// Accept an import effect only when its identity is among the claimed bound.
fn import_effect_verdict(is_claimed: bool) -> Result<(), super::Error> {
    if is_claimed {
        Ok(())
    } else {
        Err(super::error(
            super::Stage::Acceptance,
            crate::DiagnosticKind::Invalid,
            "import request effect is absent from the claimed bound",
        ))
    }
}

/// The position of the first export named `name`.
fn export_position(exports: &[super::Operation], name: &str) -> Option<usize> {
    let mut index = None;
    let mut at = 0;
    while index.is_none() && at < exports.len() {
        if has_export_name(&exports[at], name) {
            index = Some(at);
        }
        at += 1;
    }
    index
}

/// Whether `operation` is the generated word `word`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; String content equality is a non-const trait call."
)]
fn has_word(operation: &super::Operation, word: &str) -> bool {
    operation.word == word
}

/// Whether `operation` is exported as `name`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; String content equality is a non-const trait call."
)]
fn has_export_name(operation: &super::Operation, name: &str) -> bool {
    operation.export_name == name
}

/// Whether every effect is a known bit within `callable_effects`.
fn effects_callable(effects: &[noble_kernel::types::EffId], callable_effects: u64) -> bool {
    let mut is_callable = true;
    let mut at = 0;
    while is_callable && at < effects.len() {
        is_callable = 1u64
            .checked_shl(effects[at].0)
            .is_some_and(|bit| bit & callable_effects != 0);
        at += 1;
    }
    is_callable
}
