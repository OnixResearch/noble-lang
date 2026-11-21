struct StagedUnit {
    session: super::ModuleSession,
    kind: super::ModuleKind,
    submission: Option<noble_kernel::execution::Submission>,
    output: alloc::vec::Vec<noble_kernel::types::Ty>,
    pending: Option<crate::intrinsic::ProofBatch>,
}

const DECLARED_INPUT_SOURCE_BYTES: u32 = 65_536;

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether a scanned first token is the `module` keyword.
fn is_module_keyword(first: Option<&crate::source::lexer::Token>) -> bool {
    let Some(token) = first else {
        return false;
    };
    let crate::source::lexer::TokenKind::Word(spelling) = &token.kind else {
        return false;
    };
    spelling.as_slice() == b"module".as_slice()
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether a staged calculator session lost its environment or grew past the
/// calculator row limit.
fn calculator_rows_exceeded(staged: &super::ModuleSession) -> bool {
    staged.source.calculator
        && staged.source.declared.as_ref().is_none_or(|context| {
            context.environment.defs.len()
                > noble_kernel::contracts::CALCULATOR_ENVIRONMENT_ROW_LIMIT
        })
}

/// Authenticates a signed expression's owners against their original sources
/// and records them as certified in the staged namespace; any other prepared
/// unit is returned unchanged.
fn certify_signed_expression(
    mut prepared: super::ModulePrepared,
) -> Result<super::ModulePrepared, crate::source::Error> {
    if prepared.signed_expression.is_empty() {
        return Ok(prepared);
    }
    attempt!(prepared.verify_signed_origins());
    let submission = prepared.submission.as_ref().ok_or_else(|| {
        crate::source::declared::error(
            crate::source::Stage::Check,
            "signed expression lacks its concrete checked submission",
        )
    })?;
    prepared.staged = attempt!(prepared.staged.record_signed_certified(submission));
    attempt!(prepared.staged.check_retained(0));
    Ok(prepared)
}

impl super::ModuleSession {
    pub fn prepare(
        &self,
        input_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
    ) -> Result<super::ModulePrepared, crate::source::Error> {
        let unit = attempt!(self.parse_admitted(input_bytes, limits));
        attempt!(self.admit_calculator_environment(limits));
        let StagedUnit {
            session: mut staged,
            kind,
            submission,
            output,
            pending,
        } = attempt!(self
            .snapshot()
            .stage_unit(unit, input_bytes, inputs, limits));
        if calculator_rows_exceeded(&staged) {
            return Err(crate::source::declared::diagnostic(
                crate::source::Stage::Check,
                crate::source::exhausted(
                    super::super::SPAN,
                    "calculator environment row limit exceeded",
                ),
            ));
        }
        let rebuilt_import_definitions =
            attempt!(self.rebuild_if_import(kind, &mut staged, limits));
        attempt!(staged.check_retained(0));
        let signed_expression = self.retained_expression(kind, submission.as_ref(), input_bytes);
        let prepared = super::ModulePrepared {
            kind,
            generation: self.generation,
            limits,
            previous_source_generation: self.source.generation,
            previous_source_definitions_len: self.source.definitions.len(),
            previous_modules: self.modules.clone(),
            previous_aliases: self.aliases.clone(),
            previous_next_module_identity: self.next_module_identity,
            previous_history: self.source.history.clone(),
            staged,
            submission,
            output,
            pending,
            signed_expression,
            rebuilt_import_definitions,
        };
        certify_signed_expression(prepared)
    }

    /// Admits the input bytes against the declared and retained budgets, then
    /// parses them; a definition must also fit once retained.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
    )]
    fn parse_admitted(
        &self,
        input_bytes: &[u8],
        limits: crate::Limits,
    ) -> Result<crate::source::declared::parsing::ParsedUnit, crate::source::Error> {
        // DX-MODULE-05 caps ordinary declared inputs; the separately selected
        // calculator profile keeps its own per-input and clone budgets.
        if !self.source.calculator && limits.bytes > DECLARED_INPUT_SOURCE_BYTES {
            return Err(crate::source::declared::error(
                crate::source::Stage::Parse,
                "declared per-input source byte limit exceeds 65,536",
            ));
        }
        let limit_bytes = attempt!(usize::try_from(limits.bytes).map_err(|_| {
            crate::source::declared::error(
                crate::source::Stage::Parse,
                "source byte limit exceeds the host address space",
            )
        }));
        if input_bytes.len() > limit_bytes {
            // A budget refusal, so it is exhausted rather than invalid, exactly
            // like the Core lexer's identical check: a larger budget can admit it.
            let end = attempt!(
                crate::index(input_bytes.len(), crate::source::declared::SPAN).map_err(|problem| {
                    crate::source::declared::diagnostic(crate::source::Stage::Parse, problem)
                })
            );
            return Err(crate::source::declared::diagnostic(
                crate::source::Stage::Parse,
                crate::source::exhausted(
                    crate::Span { start: 0, end },
                    "source byte limit exceeded",
                ),
            ));
        }
        // Only modules retain the candidate source. If the aggregate would overflow,
        // classify the first lexical token before parsing or cloning prior sources.
        if let Err(error) = self.check_retained(input_bytes.len()) {
            attempt!(self.refuse_retained_module(input_bytes, limits, error));
        }
        let unit = attempt!(crate::source::declared::parsing::parse(
            input_bytes,
            limits,
            self.source.request_work()
        )
        .map_err(|problem| {
            crate::source::declared::diagnostic(crate::source::Stage::Parse, problem)
        }));
        if matches!(
            &unit,
            crate::source::declared::parsing::ParsedUnit::Definition
        ) {
            let extra = attempt!(input_bytes.len().checked_add(4).ok_or_else(|| {
                crate::source::declared::error(
                    crate::source::Stage::Check,
                    "retained definition source length overflow",
                )
            }));
            attempt!(self.check_retained(extra));
        }
        Ok(unit)
    }

    /// Refuses with `error` a unit whose first lexical token is `module`:
    /// only a module retains its candidate source.
    fn refuse_retained_module(
        &self,
        input_bytes: &[u8],
        limits: crate::Limits,
        error: crate::source::Error,
    ) -> Result<(), crate::source::Error> {
        let mut meter = crate::Meter::pooled(limits, self.source.request_work());
        let mut scanner = attempt!(crate::source::lexer::Scanner::new_declared(
            input_bytes,
            &mut meter,
        )
        .map_err(|problem| crate::source::declared::diagnostic(
            crate::source::Stage::Parse,
            problem
        )));
        let first = attempt!(scanner.next(&mut meter).map_err(|problem| {
            crate::source::declared::diagnostic(crate::source::Stage::Parse, problem)
        }));
        if is_module_keyword(first.as_ref()) {
            return Err(error);
        }
        Ok(())
    }

    /// The calculator profile's environment row, clone and clone-work
    /// budgets; no other profile has them.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
    )]
    fn admit_calculator_environment(
        &self,
        limits: crate::Limits,
    ) -> Result<(), crate::source::Error> {
        if !self.source.calculator {
            return Ok(());
        }
        let environment = attempt!(self.source.declared.as_ref().ok_or_else(|| {
            crate::source::declared::diagnostic(
                crate::source::Stage::Check,
                crate::source::exhausted(
                    super::super::SPAN,
                    "calculator source environment missing",
                ),
            )
        }));
        if environment.environment.defs.len()
            > noble_kernel::contracts::CALCULATOR_ENVIRONMENT_ROW_LIMIT
        {
            return Err(crate::source::declared::diagnostic(
                crate::source::Stage::Check,
                crate::source::exhausted(
                    super::super::SPAN,
                    "calculator environment row limit exceeded",
                ),
            ));
        }
        let budget = attempt!(environment
            .environment
            .calculator_clone_budget()
            .ok_or_else(|| crate::source::declared::diagnostic(
                crate::source::Stage::Check,
                crate::source::exhausted(
                    super::super::SPAN,
                    "calculator environment clone bound exceeded"
                )
            )));
        let copies = attempt!(budget
            .reserved_bytes
            .checked_mul(2)
            .filter(|total| *total <= noble_kernel::contracts::CALCULATOR_ENV_CLONE_FANOUT_LIMIT)
            .ok_or_else(|| crate::source::declared::diagnostic(
                crate::source::Stage::Check,
                crate::source::exhausted(
                    super::super::SPAN,
                    "calculator environment clone fanout exceeded"
                )
            )));
        if copies > u64::from(limits.work) {
            return Err(crate::source::declared::diagnostic(
                crate::source::Stage::Check,
                crate::source::exhausted(
                    super::super::SPAN,
                    "calculator environment clone work exceeded",
                ),
            ));
        }
        Ok(())
    }

    /// The checked transitive dependent rebuild of an import rebind; no
    /// other unit has one.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
    )]
    fn rebuild_if_import(
        &self,
        kind: super::ModuleKind,
        staged: &mut Self,
        limits: crate::Limits,
    ) -> Result<Option<alloc::vec::Vec<super::RebuiltImportDefinition>>, crate::source::Error> {
        if kind != super::ModuleKind::Import {
            return Ok(None);
        }
        self.rebuild_import_alias(staged, limits)
    }

    /// The expression bytes kept for signed source rederivation: only a
    /// declared-text or signed-self expression retains them.
    fn retained_expression(
        &self,
        kind: super::ModuleKind,
        submission: Option<&noble_kernel::execution::Submission>,
        input_bytes: &[u8],
    ) -> alloc::vec::Vec<u8> {
        let is_retained = kind == super::ModuleKind::Expression
            && (self.declared_text() || has_signed_self_body(submission));
        if is_retained {
            input_bytes.to_vec()
        } else {
            alloc::vec::Vec::new()
        }
    }

    fn stage_unit(
        mut self,
        unit: crate::source::declared::parsing::ParsedUnit,
        input_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
    ) -> Result<StagedUnit, crate::source::Error> {
        let is_expression = matches!(
            &unit,
            crate::source::declared::parsing::ParsedUnit::Expression
        );
        let (kind, submission, output, pending) = match unit {
            crate::source::declared::parsing::ParsedUnit::Module {
                name,
                version,
                members,
            } => {
                let (next, pending) = attempt!(self.register(super::register::Registration {
                    name,
                    version,
                    members,
                    source: input_bytes,
                    inputs,
                    limits,
                }));
                self = next;
                (
                    super::ModuleKind::Module,
                    None,
                    alloc::vec::Vec::new(),
                    pending,
                )
            }
            crate::source::declared::parsing::ParsedUnit::Import {
                name,
                version,
                alias,
            } => {
                self = attempt!(self.import(&name, version, alias));
                (super::ModuleKind::Import, None, inputs.to_vec(), None)
            }
            crate::source::declared::parsing::ParsedUnit::Definition
            | crate::source::declared::parsing::ParsedUnit::Expression => {
                if is_expression {
                    self = attempt!(self.pending_signed_expression_words());
                }
                let mut prepared = attempt!(self.source.prepare_with_retention(
                    input_bytes,
                    inputs,
                    limits,
                    self.retained_source_limit,
                ));
                let kind = if prepared.is_definition() {
                    super::ModuleKind::Definition
                } else {
                    super::ModuleKind::Expression
                };
                if is_expression != (kind == super::ModuleKind::Expression) {
                    return Err(crate::source::declared::error(
                        crate::source::Stage::Check,
                        "source unit changed between module and expression parsing",
                    ));
                }
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
                if kind == super::ModuleKind::Definition || is_expression {
                    self = attempt!(self.outside_words());
                }
                (kind, submission, output, None)
            }
        };
        Ok(StagedUnit {
            session: self,
            kind,
            submission,
            output,
            pending,
        })
    }

    /// Publish the entire scanned declaration/import as one stale-checked transition.
    /// The returned session is the prior one on refusal and the staged one on success.
    pub fn commit(
        self,
        mut prepared: super::ModulePrepared,
    ) -> (Self, Result<(), crate::source::Error>) {
        if prepared.pending.is_some() {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "proof declarations require independent host recheck before module publication",
                )),
            );
        }
        if !self.current_snapshot(&prepared) {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "stale or foreign module namespace snapshot",
                )),
            );
        }
        if self.import_rebind_required(&prepared) && prepared.rebuilt_import_definitions.is_none() {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "import rebind lacks its checked transitive dependent rebuild",
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

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
    )]
    fn current_snapshot(&self, prepared: &super::ModulePrepared) -> bool {
        self.generation == prepared.generation
            && self.retained_source_limit == prepared.staged.retained_source_limit
            && self.source.generation == prepared.previous_source_generation
            && self.source.calculator == prepared.staged.source.calculator
            && self.source.definitions.len() == prepared.previous_source_definitions_len
            && prepared
                .staged
                .source
                .definitions
                .get(..prepared.previous_source_definitions_len)
                == Some(self.source.definitions.as_slice())
            && self
                .source
                .declared
                .as_ref()
                .map(|context| (context.signed_self, context.declared_text))
                == prepared
                    .staged
                    .source
                    .declared
                    .as_ref()
                    .map(|context| (context.signed_self, context.declared_text))
            && self.bindings == prepared.staged.bindings
            && self.modules == prepared.previous_modules
            && self.aliases == prepared.previous_aliases
            && self.next_module_identity == prepared.previous_next_module_identity
            && self.source.history == prepared.previous_history
    }

    #[expect(
        tigerstyle::compound_condition,
        reason = "Owner: noble-contracts; the guard is the admission rule verbatim; splitting it would obscure the predicate it states"
    )]
    /// This API trusts the **host callback**, not any no_std proof marker.
    /// The callback must perform bounded source checking and a separate pinned
    /// independent exact-claim Lean check before returning checked results.
    pub fn commit_verified<F>(
        self,
        mut prepared: super::ModulePrepared,
        recheck: F,
    ) -> (Self, Result<(), crate::source::Error>)
    where
        F: FnOnce(
            &crate::intrinsic::ProofBatch,
        )
            -> Result<alloc::vec::Vec<crate::intrinsic::CheckedProof>, alloc::string::String>,
    {
        if !self.current_snapshot(&prepared) {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "stale or foreign module namespace snapshot",
                )),
            );
        }
        let Some(batch) = prepared.pending.as_ref() else {
            return self.commit(prepared);
        };
        let Some(module) = prepared.staged.modules.last() else {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "pending proof has no staged module",
                )),
            );
        };
        if prepared.kind != super::ModuleKind::Module
            || batch.generation != prepared.generation
            || batch.module != module.name
            || batch.version != module.version
            || batch.source != module.source
        {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "proof batch is not bound to the staged module source",
                )),
            );
        }
        // A host callback supplies Lean evidence, but cannot substitute a
        // different source claim or paper over an invalid proof declaration.
        // Check the exact bounded source batch before invoking that callback;
        // the selected host still must recheck the resulting term in Lean.
        let source_checked = match crate::intrinsic::check_batch(batch, prepared.limits) {
            Ok(checked) => checked,
            Err(problem) => {
                return (
                    self,
                    Err(crate::source::declared::diagnostic(
                        crate::source::Stage::Acceptance,
                        problem,
                    )),
                );
            }
        };
        let checked = match recheck(batch) {
            Ok(checked) => checked,
            Err(message) => {
                return (
                    self,
                    Err(crate::source::declared::error(
                        crate::source::Stage::Acceptance,
                        &message,
                    )),
                );
            }
        };
        if checked.len() != batch.obligations.len()
            || claims_differ(&checked, &source_checked)
            || results_differ(&checked, &batch.obligations)
        {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "host proof results do not match every staged declaration",
                )),
            );
        }
        // The namespace stores complete, immutable transitive evidence rather
        // than fetching a dependency by its current spelling on every use.
        // A fan-out of local uses can otherwise duplicate nested proofs
        // exponentially. Charge the exact source bytes and nodes that would
        // be copied before each copy is made.
        let (Ok(remaining_bytes), Ok(remaining_nodes)) = (
            usize::try_from(prepared.limits.bytes),
            usize::try_from(prepared.limits.nodes),
        ) else {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "proof publication dependency budget exceeds address space",
                )),
            );
        };
        // Earlier obligations are budgeted when materialized below; their
        // recursive closure does not exist yet at this point.
        let mut charged = Some((remaining_bytes, remaining_nodes));
        let mut obligation_at = 0usize;
        while charged.is_some() && obligation_at < batch.obligations.len() {
            charged = charge_obligation(batch, &batch.obligations[obligation_at], charged);
            obligation_at += 1;
        }
        let Some((mut remaining_bytes, mut remaining_nodes)) = charged else {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "proof publication dependency budget exhausted",
                )),
            );
        };
        let Some(module) = prepared.staged.modules.last_mut() else {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "missing staged proof module",
                )),
            );
        };
        // Each published proof owns exactly the dependencies its term uses.
        // Select and charge all of them before copying any, then copy each
        // once into a list allocated at its exact length. A selection indexes
        // the imported proofs followed by the earlier local proofs; the first
        // `SELECTION_INLINE` stay on the stack and one spill buffer serves the
        // batch. Results pair with obligations in order, as a zip did.
        let mut selection = Selection {
            inline: [0usize; SELECTION_INLINE],
            spill: alloc::vec::Vec::new(),
            selected: 0,
            bound: 0,
        };
        let mut results = checked.into_iter();
        let mut is_published = true;
        let mut obligation_at = 0usize;
        while is_published && obligation_at < batch.obligations.len() {
            let Some(result) = results.next() else {
                break;
            };
            let obligation = &batch.obligations[obligation_at];
            match selection.select(
                &obligation.term,
                batch,
                &module.proofs,
                (remaining_bytes, remaining_nodes),
            ) {
                Some((bytes, nodes)) => {
                    remaining_bytes = bytes;
                    remaining_nodes = nodes;
                    let dependencies = selection.copy(batch, &module.proofs);
                    module.proofs.push(crate::source::declared::PublishedProof {
                        reference: crate::intrinsic::ProofDependency {
                            name: obligation.name.clone(),
                            module: batch.module.clone(),
                            version: batch.version,
                            source: batch.source.clone(),
                            declaration: obligation.clone(),
                            exported: module.proof_exports.contains(&obligation.name),
                            dependencies,
                            model_revision: result.model_revision.clone(),
                            checker_revision: result.checker_revision.clone(),
                        },
                        checked: result,
                    });
                }
                None => is_published = false,
            }
            obligation_at += 1;
        }
        if !is_published {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "proof publication dependency budget exhausted",
                )),
            );
        }
        prepared.pending = None;
        self.commit(prepared)
    }
}

/// Candidate indices selected for one obligation: the imported proofs, then
/// the earlier local proofs. The first `SELECTION_INLINE` stay on the stack
/// and one spill buffer serves the whole batch.
struct Selection {
    inline: [usize; SELECTION_INLINE],
    spill: alloc::vec::Vec<usize>,
    selected: usize,
    /// At most one selection per imported or earlier local proof.
    bound: usize,
}

const SELECTION_INLINE: usize = 32;

impl Selection {
    /// Selects, in candidate order, every imported and earlier local proof
    /// that `term` uses, charging each local closure before it is selected;
    /// `None` once the publication budget is exhausted.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn select(
        &mut self,
        term: &crate::intrinsic::Form,
        batch: &crate::intrinsic::ProofBatch,
        proofs: &[crate::source::declared::PublishedProof],
        remaining: (usize, usize),
    ) -> Option<(usize, usize)> {
        #[expect(
            tigerstyle::numeric_units,
            reason = "Owner: noble-contracts; the constant is a count of source elements, and the name states its unit at every use site through the surrounding type and call"
        )]
        let imported_count = batch.dependencies.len();
        self.bound = imported_count.saturating_add(proofs.len());
        self.selected = 0;
        self.spill.clear();
        let mut is_selected = true;
        let mut index = 0usize;
        while is_selected && index < imported_count {
            if uses(term, &batch.dependencies[index].name) {
                is_selected = self.push(index);
            }
            index += 1;
        }
        // Imported closures were charged above; each local closure is
        // charged here, in candidate order, before anything is copied.
        let mut charged = if is_selected { Some(remaining) } else { None };
        let mut local = 0usize;
        while charged.is_some() && local < proofs.len() {
            let (next, is_used) = charge_if_used(term, &proofs[local].reference, charged);
            charged = self.push_charged(index, next, is_used);
            index += 1;
            local += 1;
        }
        charged
    }

    /// Selects a used local candidate once its closure was charged; `None`
    /// when the charge failed or every candidate is already selected. An
    /// unused candidate leaves the charge unchanged. The loop calls this on
    /// every step, so no branch of it holds the exclusive borrow.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn push_charged(
        &mut self,
        index: usize,
        charged: Option<(usize, usize)>,
        is_used: bool,
    ) -> Option<(usize, usize)> {
        if !is_used {
            return charged;
        }
        let remaining = charged?;
        if self.push(index) {
            Some(remaining)
        } else {
            None
        }
    }

    /// Records one selected candidate index, or refuses once every candidate
    /// is selected.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
    )]
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn push(&mut self, index: usize) -> bool {
        if self.selected >= self.bound {
            return false;
        }
        if self.selected < SELECTION_INLINE {
            self.inline[self.selected] = index;
        } else {
            // One entry per selection, so this bound is never reached.
            if self.spill.len() >= self.bound {
                return false;
            }
            self.spill.push(index);
        }
        self.selected = self.selected.saturating_add(1);
        true
    }

    /// Copies the selected dependencies, in selection order, into a list
    /// allocated at its exact length.
    fn copy(
        &self,
        batch: &crate::intrinsic::ProofBatch,
        proofs: &[crate::source::declared::PublishedProof],
    ) -> alloc::vec::Vec<crate::intrinsic::ProofDependency> {
        let mut dependencies = alloc::vec::Vec::with_capacity(self.selected);
        for &index in self.inline[..self.selected.min(SELECTION_INLINE)].iter() {
            dependencies.push(selected_copy(index, batch, proofs));
        }
        for &index in self.spill.iter() {
            dependencies.push(selected_copy(index, batch, proofs));
        }
        dependencies
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// An owned copy of selected candidate `index`: an imported proof, or an
/// earlier local proof past the imported ones.
fn selected_copy(
    index: usize,
    batch: &crate::intrinsic::ProofBatch,
    proofs: &[crate::source::declared::PublishedProof],
) -> crate::intrinsic::ProofDependency {
    match index.checked_sub(batch.dependencies.len()) {
        None => batch.dependencies[index].clone(),
        Some(local) => proofs[local].reference.clone(),
    }
}

/// Charges one obligation: its batch source and node, then the closure of
/// each imported proof its term uses, in dependency order; `None` once the
/// publication budget is exhausted.
fn charge_obligation(
    batch: &crate::intrinsic::ProofBatch,
    obligation: &crate::intrinsic::ProofObligation,
    remaining: Option<(usize, usize)>,
) -> Option<(usize, usize)> {
    let mut charged = charge_batch_bytes(batch, remaining);
    let mut dependency_at = 0usize;
    while charged.is_some() && dependency_at < batch.dependencies.len() {
        let imported = &batch.dependencies[dependency_at];
        (charged, _) = charge_if_used(&obligation.term, imported, charged);
        dependency_at += 1;
    }
    charged
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Charges the closure of `proof` when `term` uses it, reporting whether it
/// does; an unused proof leaves the budget untouched.
fn charge_if_used(
    term: &crate::intrinsic::Form,
    proof: &crate::intrinsic::ProofDependency,
    charged: Option<(usize, usize)>,
) -> (Option<(usize, usize)>, bool) {
    if !uses(term, &proof.name) {
        return (charged, false);
    }
    let charged = match charged {
        Some((bytes, nodes)) => charge_dependency(proof, bytes, nodes),
        None => None,
    };
    (charged, true)
}

/// Charges the batch source bytes and one node for an obligation.
fn charge_batch_bytes(
    batch: &crate::intrinsic::ProofBatch,
    remaining: Option<(usize, usize)>,
) -> Option<(usize, usize)> {
    let (bytes, nodes) = remaining?;
    Some((
        bytes.checked_sub(batch.source.len())?,
        nodes.checked_sub(1)?,
    ))
}

/// Whether a host result's claim or term differs from its source-checked
/// counterpart, over the shorter of the two lists.
fn claims_differ(
    checked: &[crate::intrinsic::CheckedProof],
    source_checked: &[crate::intrinsic::CheckedProof],
) -> bool {
    let mut is_different = false;
    let mut at = 0usize;
    while !is_different && at < checked.len() && at < source_checked.len() {
        is_different = claim_differs(&checked[at], &source_checked[at]);
        at += 1;
    }
    is_different
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether a host result's claim or term differs from the source-checked one.
fn claim_differs(
    result: &crate::intrinsic::CheckedProof,
    source_checked: &crate::intrinsic::CheckedProof,
) -> bool {
    result.claim != source_checked.claim || result.lean_term != source_checked.lean_term
}

/// Whether a host result is not bound to its obligation, over the shorter of
/// the two lists.
fn results_differ(
    checked: &[crate::intrinsic::CheckedProof],
    obligations: &[crate::intrinsic::ProofObligation],
) -> bool {
    let mut is_different = false;
    let mut at = 0usize;
    while !is_different && at < checked.len() && at < obligations.len() {
        is_different = result_differs(&checked[at], &obligations[at]);
        at += 1;
    }
    is_different
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether one host result names another obligation, carries a malformed
/// revision or reports another proof kind than the obligation's goal.
fn result_differs(
    result: &crate::intrinsic::CheckedProof,
    obligation: &crate::intrinsic::ProofObligation,
) -> bool {
    result.name != obligation.name
        || !revision(&result.model_revision)
        || !revision(&result.checker_revision)
        || result.kind != expected_kind(obligation)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// The proof kind an obligation's goal requires: a revision-2 contract with
/// a revision-2 obligation is a named contract.
fn expected_kind(obligation: &crate::intrinsic::ProofObligation) -> crate::intrinsic::ProofKind {
    match &obligation.goal {
        crate::intrinsic::PendingGoal::Pure { .. } => crate::intrinsic::ProofKind::Pure,
        crate::intrinsic::PendingGoal::Contract { contract } => {
            if contract.revision == 2 && obligation.revision == 2 {
                crate::intrinsic::ProofKind::NamedContract
            } else {
                crate::intrinsic::ProofKind::Contract
            }
        }
    }
}

/// Whether `value` is 64 lowercase hexadecimal digits.
fn revision(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut is_hex = bytes.len() == 64;
    let mut at = 0usize;
    while is_hex && at < bytes.len() {
        let byte = bytes[at];
        is_hex = byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte);
        at += 1;
    }
    is_hex
}

/// Whether a present `submission` has a definition body with a signed self call.
fn has_signed_self_body(submission: Option<&noble_kernel::execution::Submission>) -> bool {
    let Some(submission) = submission else {
        return false;
    };
    let mut is_found = false;
    let mut definition_at = 0usize;
    while !is_found && definition_at < submission.definitions.len() {
        is_found =
            super::has_signed_self(&submission.definitions[definition_at].body.candidate.nodes);
        definition_at += 1;
    }
    is_found
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
/// Charges a retained dependency and its transitive closure in the same
/// pre-order as the published copy, returning the remaining byte and node
/// budgets, or `None` once either is exhausted.
///
/// Every proof is charged with the number of proofs queued behind it: the
/// later siblings of each proof on its path. One cursor per open sibling list
/// records that position, so the walk is a single loop whose stack holds at
/// most one entry per tree level and nothing recurses.
fn charge_dependency(
    dependency: &crate::intrinsic::ProofDependency,
    remaining_bytes: usize,
    remaining_nodes: usize,
) -> Option<(usize, usize)> {
    let mut charged = charge_proof(dependency, Some((remaining_bytes, remaining_nodes)), 0);
    let mut pending = alloc::vec::Vec::new();
    queue_children(&mut pending, dependency, charged, 0);
    while charged.is_some() && !pending.is_empty() {
        charged = charge_next_child(&mut pending, charged);
    }
    charged
}

/// A charged proof's children from `next` on, still to be charged; `waiting`
/// proofs were queued behind that proof when it was charged.
#[derive(Clone, Copy)]
struct ChildCursor<'a> {
    siblings: &'a alloc::vec::Vec<crate::intrinsic::ProofDependency>,
    next: usize,
    waiting: usize,
}

/// Queues the children of a charged proof, which `waiting` proofs follow;
/// nothing is queued once the charge failed or for a proof without children.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the walk pushes onto its own growable cursor stack, so it takes the owned Vec rather than a slice"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the only mutable input is the walk's locally owned cursor stack, borrowed for one loop-free step"
)]
fn queue_children<'a>(
    pending: &mut alloc::vec::Vec<ChildCursor<'a>>,
    proof: &'a crate::intrinsic::ProofDependency,
    charged: Option<(usize, usize)>,
    waiting: usize,
) {
    if charged.is_some() && !proof.dependencies.is_empty() {
        pending.push(ChildCursor {
            siblings: &proof.dependencies,
            next: 0,
            waiting,
        });
    }
}

/// Charges the next pending child in pre-order: its later siblings stay
/// queued behind it, and its own children are queued once it is charged.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the walk pushes onto its own growable cursor stack, so it takes the owned Vec rather than a slice"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the only mutable input is the walk's locally owned cursor stack, borrowed for one loop-free step"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn charge_next_child<'a>(
    pending: &mut alloc::vec::Vec<ChildCursor<'a>>,
    charged: Option<(usize, usize)>,
) -> Option<(usize, usize)> {
    let Some(cursor) = pending.pop() else {
        return charged;
    };
    let child = &cursor.siblings[cursor.next];
    // `next` indexes a child, so its successor never saturates and the
    // children after it number `len - (next + 1)`.
    let following = cursor.next.saturating_add(1);
    let later = cursor.siblings.len().saturating_sub(following);
    if following < cursor.siblings.len() {
        pending.push(ChildCursor {
            siblings: cursor.siblings,
            next: following,
            waiting: cursor.waiting,
        });
    }
    // The parent's charged fan-out check bounds its waiting proofs plus all of
    // its children, so this sum never saturates.
    let waiting = cursor.waiting.saturating_add(later);
    let charged = charge_proof(child, charged, waiting);
    queue_children(pending, child, charged, waiting);
    charged
}

/// Charges one proof's source bytes and node, refusing before its children
/// are queued if the fan-out would exceed the remaining nodes.
fn charge_proof(
    proof: &crate::intrinsic::ProofDependency,
    remaining: Option<(usize, usize)>,
    waiting: usize,
) -> Option<(usize, usize)> {
    let (bytes, nodes) = remaining?;
    let bytes = bytes.checked_sub(proof.source.len())?;
    let nodes = nodes.checked_sub(1)?;
    // Every queued proof costs a node. Refuse excessive fan-out before
    // queueing any of its children.
    if waiting.checked_add(proof.dependencies.len())? > nodes {
        return None;
    }
    Some((bytes, nodes))
}

/// Whether `form` or a form nested in it is the list `(use name)`, searched in
/// pre-order: a list before its items, and the items in turn. One cursor per
/// open list keeps the search a single loop with no recursion.
fn uses(form: &crate::intrinsic::Form, name: &str) -> bool {
    let mut is_found = is_use_of(form, name);
    let mut pending = alloc::vec::Vec::new();
    queue_items(&mut pending, form);
    while !is_found && !pending.is_empty() {
        is_found = uses_next(&mut pending, name);
    }
    is_found
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether `form` is the two-item list `(use name)`.
fn is_use_of(form: &crate::intrinsic::Form, name: &str) -> bool {
    match &form.kind {
        crate::intrinsic::FormKind::List(items) => {
            items.len() == 2 && atom_is(&items[0], "use") && atom_is(&items[1], name)
        }
        crate::intrinsic::FormKind::Atom(_) => false,
    }
}

/// The items of a list from `next` on, still to be searched.
#[derive(Clone, Copy)]
struct ItemCursor<'a> {
    items: &'a alloc::vec::Vec<crate::intrinsic::Form>,
    next: usize,
}

/// Queues the items of a non-empty list; an atom or an empty list has none.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the search pushes onto its own growable cursor stack, so it takes the owned Vec rather than a slice"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the only mutable input is the search's locally owned cursor stack, borrowed for one loop-free step"
)]
fn queue_items<'a>(
    pending: &mut alloc::vec::Vec<ItemCursor<'a>>,
    form: &'a crate::intrinsic::Form,
) {
    let crate::intrinsic::FormKind::List(items) = &form.kind else {
        return;
    };
    if !items.is_empty() {
        pending.push(ItemCursor { items, next: 0 });
    }
}

/// Searches the next pending item: its later siblings stay queued behind it,
/// then its own items are queued.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the search pushes onto its own growable cursor stack, so it takes the owned Vec rather than a slice"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the only mutable input is the search's locally owned cursor stack, borrowed for one loop-free step"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn uses_next<'a>(pending: &mut alloc::vec::Vec<ItemCursor<'a>>, name: &str) -> bool {
    let Some(cursor) = pending.pop() else {
        return false;
    };
    let item = &cursor.items[cursor.next];
    // `next` indexes an item, so its successor never saturates.
    let following = cursor.next.saturating_add(1);
    if following < cursor.items.len() {
        pending.push(ItemCursor {
            items: cursor.items,
            next: following,
        });
    }
    let is_found = is_use_of(item, name);
    queue_items(pending, item);
    is_found
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether `form` is the atom spelled `text`.
fn atom_is(form: &crate::intrinsic::Form, text: &str) -> bool {
    match &form.kind {
        crate::intrinsic::FormKind::Atom(atom) => atom == text,
        crate::intrinsic::FormKind::List(_) => false,
    }
}

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
#[cfg(test)]
mod tests {
    use super::super::ModuleSession;
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
    )]
    use noble_kernel::contracts::Behavior;
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
    )]
    use noble_kernel::{shapes::Pattern, words::Scheme};

    #[expect(
        tigerstyle::path_segment_repetition,
        reason = "Owner: noble-contracts; the item name reads as the domain term at its use sites; renaming would churn the extracted subject"
    )]
    #[test]
    fn calculator_source_refuses_897_lightweight_rows_before_namespace_snapshot_clone() {
        let mut session = ModuleSession::new_calculator()
            .unwrap_or_else(|_| panic!("calculator environment unavailable"));
        let environment = &mut session
            .source
            .declared
            .as_mut()
            .unwrap_or_else(|| panic!("calculator declaration context unavailable"))
            .environment;
        while environment.defs.len() < 897 {
            environment.defs.push(Scheme {
                var_kinds: alloc::vec::Vec::new(),
                stack_in: alloc::vec::Vec::new(),
                stack_out: alloc::vec![Pattern::Unit],
                effects: alloc::vec::Vec::new(),
            });
            environment.kinds.push(Behavior::Named);
            environment.deps.push(alloc::vec::Vec::new());
            environment.definition_owners.push(None);
        }
        assert!(
            environment.calculator_clone_budget().is_some(),
            "lightweight row 897 must not be confused with clone-byte exhaustion"
        );
        let limits = crate::Limits {
            bytes: 262_144,
            nodes: 524_288,
            depth: 64,
            work: 512_000_000,
        };
        let refused = session
            .prepare(b"1", &[], limits)
            .err()
            .unwrap_or_else(|| panic!("oversized environment reached snapshot cloning"));
        assert_eq!(refused.stage(), crate::source::Stage::Check);
        assert_eq!(refused.diagnostic().kind, crate::DiagnosticKind::Exhausted);
    }

    #[expect(
        tigerstyle::path_segment_repetition,
        reason = "Owner: noble-contracts; the item name reads as the domain term at its use sites; renaming would churn the extracted subject"
    )]
    #[test]
    fn calculator_source_refuses_deep_environment_before_namespace_snapshot_clone() {
        let mut session = ModuleSession::new_calculator()
            .unwrap_or_else(|_| panic!("calculator environment unavailable"));
        let environment = &mut session
            .source
            .declared
            .as_mut()
            .unwrap_or_else(|| panic!("calculator declaration context unavailable"))
            .environment;
        let program = Pattern::program(
            alloc::vec![Pattern::Unit; 128],
            alloc::vec![Pattern::Unit; 128],
            alloc::vec::Vec::new(),
        );
        for _ in 0..8 {
            environment.defs.push(Scheme {
                var_kinds: alloc::vec::Vec::new(),
                stack_in: alloc::vec::Vec::new(),
                stack_out: alloc::vec![program.clone(); 128],
                effects: alloc::vec::Vec::new(),
            });
        }
        assert!(
            environment.calculator_clone_budget().is_none(),
            "nested program stacks must contribute every retained node to the clone bound"
        );
        let limits = crate::Limits {
            bytes: 262_144,
            nodes: 524_288,
            depth: 64,
            work: 512_000_000,
        };
        let refused = session
            .prepare(b"1", &[], limits)
            .err()
            .unwrap_or_else(|| panic!("oversized environment reached snapshot cloning"));
        assert_eq!(refused.stage(), crate::source::Stage::Check);
        assert_eq!(refused.diagnostic().kind, crate::DiagnosticKind::Exhausted);
    }

    fn atom(text: &str) -> crate::intrinsic::Form {
        crate::intrinsic::Form {
            kind: crate::intrinsic::FormKind::Atom(alloc::string::String::from(text)),
            span: crate::Span { start: 0, end: 1 },
        }
    }

    fn proof(
        name: &str,
        source_len: usize,
        dependencies: alloc::vec::Vec<crate::intrinsic::ProofDependency>,
    ) -> crate::intrinsic::ProofDependency {
        crate::intrinsic::ProofDependency {
            name: alloc::string::String::from(name),
            module: alloc::string::String::from("m"),
            version: 1,
            source: alloc::vec![0; source_len],
            declaration: crate::intrinsic::ProofObligation {
                name: alloc::string::String::from(name),
                revision: 1,
                goal: crate::intrinsic::PendingGoal::Pure {
                    proposition: atom("p"),
                },
                term: atom("t"),
                span: crate::Span { start: 0, end: 1 },
                elaboration: false,
            },
            exported: false,
            dependencies,
            model_revision: alloc::string::String::new(),
            checker_revision: alloc::string::String::new(),
        }
    }

    /// The queue walk `charge_dependency` used before extraction, verbatim:
    /// the first child runs next, later siblings wait in 32 stack slots and
    /// then one spill buffer, and fan-out is refused before queueing.
    #[allow(
        tigerstyle::assertion_density,
        tigerstyle::nested_conditionals,
        reason = "Owner: noble-maintainers; test harness: the reference walk is the pre-extraction body verbatim."
    )]
    #[expect(
        tigerstyle::ambiguous_params,
        reason = "Owner: noble-contracts; the reference walk keeps the positional (dependency, remaining_bytes, remaining_nodes) signature of charge_dependency, which every comparison calls with the same arguments"
    )]
    fn queue_charge(
        dependency: &crate::intrinsic::ProofDependency,
        remaining_bytes: usize,
        remaining_nodes: usize,
    ) -> Option<(usize, usize)> {
        const INLINE: usize = 32;
        let mut inline: [Option<&crate::intrinsic::ProofDependency>; INLINE] = [None; INLINE];
        let mut inline_len = 0usize;
        let mut spill = alloc::vec::Vec::new();
        let (mut bytes, mut nodes) = (remaining_bytes, remaining_nodes);
        let mut current = Some(dependency);
        while let Some(next) = current {
            bytes = bytes.checked_sub(next.source.len())?;
            nodes = nodes.checked_sub(1)?;
            let waiting = inline_len.checked_add(spill.len())?;
            if waiting.checked_add(next.dependencies.len())? > nodes {
                return None;
            }
            for child in next.dependencies.iter().skip(1).rev() {
                if inline_len < INLINE && spill.is_empty() {
                    inline[inline_len] = Some(child);
                    inline_len = inline_len.saturating_add(1);
                } else {
                    if spill.len() >= nodes {
                        return None;
                    }
                    spill.push(child);
                }
            }
            current = next
                .dependencies
                .first()
                .or_else(|| spill.pop())
                .or_else(|| {
                    let top = inline_len.checked_sub(1)?;
                    inline_len = top;
                    inline[top].take()
                });
        }
        Some((bytes, nodes))
    }

    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; the test oracle sums each fixture tree by structural recursion, independent of the walk under test; the deepest fixture is the 201-proof chain"
    )]
    fn totals(dependency: &crate::intrinsic::ProofDependency) -> (usize, usize) {
        let mut bytes = dependency.source.len();
        let mut nodes = 1usize;
        for child in &dependency.dependencies {
            let (child_bytes, child_nodes) = totals(child);
            bytes += child_bytes;
            nodes += child_nodes;
        }
        (bytes, nodes)
    }

    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-contracts; fan_out is zero when depth is zero, so the closure computing depth - 1 runs only for a positive depth"
    )]
    fn generated(seed: &mut u64, depth: u32) -> crate::intrinsic::ProofDependency {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let source_len = usize::try_from((*seed >> 33) % 7).unwrap_or(0);
        let fan_out = if depth == 0 {
            0
        } else {
            usize::try_from((*seed >> 40) % 5).unwrap_or(0)
        };
        let children = (0..fan_out).map(|_| generated(seed, depth - 1)).collect();
        proof("g", source_len, children)
    }

    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; the single assert_eq runs inside the shape, byte-budget and node-budget loops and compares both walks for every generated case"
    )]
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-contracts; every fixture tree has at most 341 proofs of at most 6 source bytes each, so the boundary budgets derived from its totals stay in the low thousands"
    )]
    #[test]
    fn recursive_dependency_charge_matches_the_queue_walk() {
        let leaves = |count: usize, len: usize| {
            (0..count)
                .map(|_| proof("leaf", len, alloc::vec::Vec::new()))
                .collect::<alloc::vec::Vec<_>>()
        };
        let mut shapes = alloc::vec![
            proof("wide", 3, leaves(40, 1)),
            proof(
                "nested",
                1,
                (0..3).map(|_| proof("mid", 2, leaves(20, 1))).collect()
            ),
        ];
        let mut chain = proof("end", 1, alloc::vec::Vec::new());
        for _ in 0..200 {
            chain = proof("link", 2, alloc::vec![chain]);
        }
        shapes.push(chain);
        let mut comb = proof("end", 1, alloc::vec::Vec::new());
        for _ in 0..50 {
            comb = proof(
                "tooth",
                1,
                alloc::vec![comb, proof("leaf", 3, leaves(2, 1))],
            );
        }
        shapes.push(comb);
        let mut seed = 7u64;
        for _ in 0..80 {
            shapes.push(generated(&mut seed, 4));
        }
        for shape in &shapes {
            let (total_bytes, total_nodes) = totals(shape);
            for bytes in [
                0,
                total_bytes.saturating_sub(1),
                total_bytes,
                total_bytes + 9,
            ] {
                for nodes in [
                    0,
                    1,
                    2,
                    5,
                    41,
                    total_nodes.saturating_sub(1),
                    total_nodes,
                    total_nodes + 3,
                    total_nodes * 2 + 50,
                ] {
                    assert_eq!(
                        super::charge_dependency(shape, bytes, nodes),
                        queue_charge(shape, bytes, nodes),
                        "{} with {bytes} bytes and {nodes} nodes",
                        shape.name
                    );
                }
            }
        }
    }

    #[expect(
        tigerstyle::function_length,
        reason = "Owner: noble-contracts; the test builds one fixture of 40 imported and 30 local proofs with a term naming two thirds of them, then checks the charge, the copy order and the refusal against that same fixture"
    )]
    #[test]
    fn dependency_selection_keeps_candidate_order_past_the_inline_slots() {
        let names = |prefix: &str, count: usize| {
            (0..count)
                .map(|index| alloc::format!("{prefix}{index}"))
                .collect::<alloc::vec::Vec<_>>()
        };
        let imported = names("i", 40);
        let local = names("l", 30);
        let batch = crate::intrinsic::ProofBatch {
            module: alloc::string::String::from("m"),
            version: 1,
            source: alloc::vec![0; 4],
            generation: 0,
            dependencies: imported
                .iter()
                .map(|name| proof(name, 2, alloc::vec::Vec::new()))
                .collect(),
            obligations: alloc::vec::Vec::new(),
            elaborations: alloc::vec::Vec::new(),
            elaboration_work: crate::intrinsic::ProofWork {
                normalization: 0,
                substitution: 0,
                total: 0,
            },
        };
        let proofs = local
            .iter()
            .map(|name| crate::source::declared::PublishedProof {
                reference: proof(
                    name,
                    3,
                    alloc::vec![proof("inner", 1, alloc::vec::Vec::new())],
                ),
                checked: crate::intrinsic::CheckedProof {
                    name: name.clone(),
                    kind: crate::intrinsic::ProofKind::Pure,
                    lean_term: alloc::string::String::new(),
                    claim: alloc::string::String::new(),
                    model_revision: alloc::string::String::new(),
                    checker_revision: alloc::string::String::new(),
                },
            })
            .collect::<alloc::vec::Vec<_>>();
        // The term names its uses in reverse; selection follows candidate
        // order: every imported proof first, then every local proof.
        let used = imported
            .iter()
            .chain(&local)
            .enumerate()
            .filter(|(index, _)| index % 3 != 1)
            .map(|(_, name)| name.clone())
            .collect::<alloc::vec::Vec<_>>();
        let term = crate::intrinsic::Form {
            kind: crate::intrinsic::FormKind::List(
                used.iter()
                    .rev()
                    .map(|name| crate::intrinsic::Form {
                        kind: crate::intrinsic::FormKind::List(alloc::vec![
                            atom("use"),
                            atom(name)
                        ]),
                        span: crate::Span { start: 0, end: 1 },
                    })
                    .collect(),
            ),
            span: crate::Span { start: 0, end: 1 },
        };
        let mut selection = super::Selection {
            inline: [0; super::SELECTION_INLINE],
            spill: alloc::vec::Vec::new(),
            selected: 0,
            bound: 0,
        };
        let mut expected = Some((10_000usize, 10_000usize));
        for published in &proofs {
            if used.contains(&published.reference.name) {
                expected = expected.and_then(|(bytes, nodes)| {
                    super::charge_dependency(&published.reference, bytes, nodes)
                });
            }
        }
        assert!(used.len() > super::SELECTION_INLINE);
        assert_eq!(
            selection.select(&term, &batch, &proofs, (10_000, 10_000)),
            expected
        );
        let copied = selection.copy(&batch, &proofs);
        assert_eq!(
            copied
                .iter()
                .map(|copy| copy.name.clone())
                .collect::<alloc::vec::Vec<_>>(),
            used
        );
        // The first used local closure costs four bytes, so three refuse it.
        assert_eq!(selection.select(&term, &batch, &proofs, (3, 10_000)), None);
    }

    /// Once the retained sources fill their cap, the first lexical token
    /// classifies a unit before the rest is scanned: a module is refused by the
    /// retained budget even when a later token is malformed, a definition is
    /// scanned first and reports that token, and an expression, which retains
    /// nothing, still prepares.
    #[test]
    fn retained_overflow_refuses_only_modules_before_parsing() {
        let limits = crate::Limits {
            bytes: 65_536,
            nodes: 16_384,
            depth: 64,
            work: 2_000_000,
        };
        let first: &[u8] = b"module a@1 [ export one def one [ 1 + ] ]";
        let cap = u32::try_from(first.len()).expect("small module");
        let retained =
            crate::source::RetainedSourceLimit::new(cap).expect("valid retained source bound");
        let session = ModuleSession::new(&[], retained).expect("source session");
        let prepared = session
            .prepare(first, &[], limits)
            .expect("the first module fits the cap exactly");
        let (session, linked) = session.commit(prepared);
        linked.expect("the first module commits");
        let module = session
            .prepare(b"module b@1 [ \"", &[], limits)
            .expect_err("a full session refuses another module");
        assert_eq!(module.stage(), crate::source::Stage::Check, "{module:?}");
        assert_eq!(
            module.diagnostic().kind,
            crate::DiagnosticKind::Exhausted,
            "{module:?}"
        );
        let definition = session
            .prepare(b"def x [ \"", &[], limits)
            .expect_err("an unterminated text literal is refused");
        assert_eq!(
            definition.stage(),
            crate::source::Stage::Parse,
            "{definition:?}"
        );
        session
            .prepare(b"1", &[], limits)
            .expect("an expression retains no source");
    }

    /// The search before it became a cursor walk, verbatim: the independent
    /// reference the cursor search is compared against.
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; the test oracle is the earlier recursive search verbatim, run on fixtures at most 301 forms deep"
    )]
    fn uses_reference(form: &crate::intrinsic::Form, name: &str) -> bool {
        let mut is_found = false;
        if let crate::intrinsic::FormKind::List(items) = &form.kind {
            is_found = items.len() == 2
                && super::atom_is(&items[0], "use")
                && super::atom_is(&items[1], name);
            let mut at = 0;
            while !is_found && at < items.len() {
                is_found = uses_reference(&items[at], name);
                at += 1;
            }
        }
        is_found
    }

    fn list(items: alloc::vec::Vec<crate::intrinsic::Form>) -> crate::intrinsic::Form {
        crate::intrinsic::Form {
            kind: crate::intrinsic::FormKind::List(items),
            span: crate::Span { start: 0, end: 1 },
        }
    }

    /// A pseudo-random form over the atoms `use`, `a` and `b`.
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; the fixture generator recurses once per level of a form at most four levels deep"
    )]
    fn generated_form(seed: &mut u64, depth: u32) -> crate::intrinsic::Form {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let pick = usize::try_from((*seed >> 33) % 6).unwrap_or(0);
        if depth == 0 || pick < 3 {
            return atom(["use", "a", "b"][pick % 3]);
        }
        let width = usize::try_from((*seed >> 40) % 4).unwrap_or(0);
        let mut items = alloc::vec::Vec::with_capacity(width);
        for _ in 0..width {
            items.push(generated_form(seed, depth.saturating_sub(1)));
        }
        list(items)
    }

    /// The cursor search finds a `(use name)` list exactly where the recursive
    /// search does: on generated forms, a deep chain, a wide list, and near
    /// misses whose length, head or position differs.
    #[test]
    fn use_search_matches_the_recursive_walk() {
        let mut forms = alloc::vec![
            list(alloc::vec![atom("use"), atom("a"), atom("b")]),
            list(alloc::vec![atom("use")]),
            list(alloc::vec![list(alloc::vec![atom("use")]), atom("a")]),
            list(alloc::vec![atom("a"), atom("use")]),
            list(alloc::vec![
                atom("use"),
                list(alloc::vec![atom("use"), atom("b")])
            ]),
        ];
        let mut chain = list(alloc::vec![atom("use"), atom("a")]);
        for _ in 0..300 {
            chain = list(alloc::vec![atom("b"), chain]);
        }
        forms.push(chain);
        let mut wide = alloc::vec![atom("a"); 300];
        wide.push(list(alloc::vec![atom("use"), atom("b")]));
        forms.push(list(wide));
        let mut seed = 11u64;
        for _ in 0..200 {
            forms.push(generated_form(&mut seed, 4));
        }
        let mut found = 0usize;
        for form in &forms {
            for name in ["a", "b", "use", "c"] {
                let is_found = uses_reference(form, name);
                assert_eq!(super::uses(form, name), is_found, "{name}");
                found += usize::from(is_found);
            }
        }
        assert!(found > 0, "some fixtures name a used proof");
        assert!(
            found < forms.len().saturating_mul(4),
            "some fixtures name none"
        );
    }
}
