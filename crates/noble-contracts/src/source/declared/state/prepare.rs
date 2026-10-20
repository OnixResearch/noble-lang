struct StagedUnit {
    session: super::ModuleSession,
    kind: super::ModuleKind,
    submission: Option<noble_kernel::execution::Submission>,
    output: alloc::vec::Vec<noble_kernel::types::Ty>,
    pending: Option<crate::intrinsic::ProofBatch>,
}

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
        // Only modules retain the candidate source. If the aggregate would overflow,
        // classify the first lexical token before parsing or cloning prior sources.
        if let Err(error) = self.check_retained_module_bytes(input_bytes.len(), limits) {
            let mut meter = crate::Meter::new(limits);
            let mut scanner = attempt!(crate::source::lexer::Scanner::new_declared(
                input_bytes,
                &mut meter,
            ).map_err(|problem| crate::source::declared::diagnostic(crate::source::Stage::Parse, problem)));
            let first = attempt!(scanner.next(&mut meter).map_err(|problem| {
                crate::source::declared::diagnostic(crate::source::Stage::Parse, problem)
            }));
            if matches!(first, Some(crate::source::lexer::Token {
                kind: crate::source::lexer::TokenKind::Word(ref spelling),
                ..
            }) if spelling == b"module") {
                return Err(error);
            }
        }
        let unit = attempt!(
            crate::source::declared::parsing::parse(input_bytes, limits).map_err(|problem| {
                crate::source::declared::diagnostic(crate::source::Stage::Parse, problem)
            })
        );
        if self.source.calculator {
            let environment = attempt!(self.source.declared.as_ref()
                .ok_or_else(|| crate::source::declared::diagnostic(
                    crate::source::Stage::Check,
                    crate::source::exhausted(super::super::SPAN,
                        "calculator source environment missing"))));
            if environment.environment.defs.len()
                > noble_kernel::contracts::CALCULATOR_ENVIRONMENT_ROW_LIMIT
            {
                return Err(crate::source::declared::diagnostic(
                    crate::source::Stage::Check,
                    crate::source::exhausted(super::super::SPAN,
                        "calculator environment row limit exceeded")));
            }
            let budget = attempt!(environment.environment.calculator_clone_budget()
                .ok_or_else(|| crate::source::declared::diagnostic(
                    crate::source::Stage::Check,
                    crate::source::exhausted(super::super::SPAN,
                        "calculator environment clone bound exceeded"))));
            let copies = attempt!(budget.reserved_bytes.checked_mul(2)
                .filter(|total| *total <= noble_kernel::contracts::CALCULATOR_ENV_CLONE_FANOUT_LIMIT)
                .ok_or_else(|| crate::source::declared::diagnostic(
                    crate::source::Stage::Check,
                    crate::source::exhausted(super::super::SPAN,
                        "calculator environment clone fanout exceeded"))));
            if copies > u64::from(limits.work) {
                return Err(crate::source::declared::diagnostic(
                    crate::source::Stage::Check,
                    crate::source::exhausted(super::super::SPAN,
                        "calculator environment clone work exceeded")));
            }
        }
        let StagedUnit { session: staged, kind, submission, output, pending } =
            attempt!(self
                .snapshot()
                .stage_unit(unit, input_bytes, inputs, limits));
        if staged.source.calculator && staged.source.declared.as_ref()
            .is_none_or(|context| context.environment.defs.len()
                > noble_kernel::contracts::CALCULATOR_ENVIRONMENT_ROW_LIMIT)
        {
            return Err(crate::source::declared::diagnostic(
                crate::source::Stage::Check,
                crate::source::exhausted(super::super::SPAN,
                    "calculator environment row limit exceeded")));
        }
        attempt!(staged.retained(limits));
        Ok(super::ModulePrepared {
            kind,
            generation: self.generation,
            limits,
            previous_modules: self.modules.clone(),
            previous_aliases: self.aliases.clone(),
            previous_history: self.source.history.clone(),
            staged,
            submission,
            output,
            pending,
        })
    }

    fn stage_unit(
        mut self,
        unit: crate::source::declared::parsing::ParsedUnit,
        input_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
    ) -> Result<StagedUnit, crate::source::Error> {
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
                (super::ModuleKind::Module, None, alloc::vec::Vec::new(), pending)
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
                (kind, submission, output, None)
            }
        };
        Ok(StagedUnit { session: self, kind, submission, output, pending })
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

    fn current_snapshot(&self, prepared: &super::ModulePrepared) -> bool {
        self.generation == prepared.generation
            && self.source.calculator == prepared.staged.source.calculator
            && self.bindings == prepared.staged.bindings
            && self.modules == prepared.previous_modules
            && self.aliases == prepared.previous_aliases
            && self.source.history == prepared.previous_history
    }

    /// This API trusts the **host callback**, not any no_std proof marker.
    /// The callback must perform bounded source checking and a separate pinned
    /// independent exact-claim Lean check before returning checked results.
    pub fn commit_verified<F>(
        self,
        mut prepared: super::ModulePrepared,
        recheck: F,
    ) -> (Self, Result<(), crate::source::Error>)
    where
        F: FnOnce(&crate::intrinsic::ProofBatch) -> Result<alloc::vec::Vec<crate::intrinsic::CheckedProof>, alloc::string::String>,
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
                    Err(crate::source::declared::error(crate::source::Stage::Acceptance, &message)),
                );
            }
        };
        if checked.len() != batch.obligations.len()
            || checked.iter().zip(&source_checked).any(|(result, source)| {
                result.claim != source.claim || result.lean_term != source.lean_term
            })
            || checked.iter().zip(&batch.obligations).any(|(result, obligation)| {
                result.name != obligation.name
                    || !revision(&result.model_revision)
                    || !revision(&result.checker_revision)
                    || result.kind
                        != match &obligation.goal {
                            crate::intrinsic::PendingGoal::Pure { .. } => crate::intrinsic::ProofKind::Pure,
                            crate::intrinsic::PendingGoal::Contract { contract } if
                                contract.revision == 2 && obligation.revision == 2 =>
                                crate::intrinsic::ProofKind::NamedContract,
                            crate::intrinsic::PendingGoal::Contract { .. } => crate::intrinsic::ProofKind::Contract,
                        }
            })
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
        let mut remaining_bytes = usize::try_from(prepared.limits.bytes).unwrap_or(usize::MAX);
        let mut remaining_nodes = usize::try_from(prepared.limits.nodes).unwrap_or(usize::MAX);
        for obligation in &batch.obligations {
            if remaining_bytes.checked_sub(batch.source.len()).is_none()
                || remaining_nodes == 0
            {
                return (
                    self,
                    Err(crate::source::declared::error(
                        crate::source::Stage::Acceptance,
                        "proof publication dependency budget exhausted",
                    )),
                );
            }
            remaining_bytes -= batch.source.len();
            remaining_nodes -= 1;
            for imported in &batch.dependencies {
                if uses(&obligation.term, &imported.name)
                    && !charge_dependency(imported, &mut remaining_bytes, &mut remaining_nodes)
                {
                    return (
                        self,
                        Err(crate::source::declared::error(
                            crate::source::Stage::Acceptance,
                            "proof publication dependency budget exhausted",
                        )),
                    );
                }
            }
            // Earlier obligations are budgeted when materialized below;
            // their recursive closure does not exist yet at this point.
        }
        let Some(module) = prepared.staged.modules.last_mut() else {
            return (
                self,
                Err(crate::source::declared::error(
                    crate::source::Stage::Acceptance,
                    "missing staged proof module",
                )),
            );
        };
        for (obligation, result) in batch.obligations.iter().zip(checked) {
            let mut dependencies = alloc::vec::Vec::new();
            for imported in &batch.dependencies {
                if uses(&obligation.term, &imported.name) {
                    dependencies.push(imported.clone());
                }
            }
            for local in &module.proofs {
                if uses(&obligation.term, &local.reference.name) {
                    if !charge_dependency(
                        &local.reference,
                        &mut remaining_bytes,
                        &mut remaining_nodes,
                    ) {
                        return (
                            self,
                            Err(crate::source::declared::error(
                                crate::source::Stage::Acceptance,
                                "proof publication dependency budget exhausted",
                            )),
                        );
                    }
                    dependencies.push(local.reference.clone());
                }
            }
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
        prepared.pending = None;
        self.commit(prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::super::ModuleSession;
    use noble_kernel::contracts::Behavior;
    use noble_kernel::{shapes::Pattern, words::Scheme};

    #[test]
    fn calculator_source_refuses_897_lightweight_rows_before_namespace_snapshot_clone() {
        let mut session = ModuleSession::new_calculator()
            .unwrap_or_else(|_| panic!("calculator environment unavailable"));
        let environment = &mut session.source.declared.as_mut()
            .unwrap_or_else(|| panic!("calculator declaration context unavailable")).environment;
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
        assert!(environment.calculator_clone_budget().is_some(),
            "lightweight row 897 must not be confused with clone-byte exhaustion");
        let limits = crate::Limits {
            bytes: 262_144, nodes: 524_288, depth: 64, work: 512_000_000,
        };
        let refused = session.prepare(b"1", &[], limits)
            .err().unwrap_or_else(|| panic!("oversized environment reached snapshot cloning"));
        assert_eq!(refused.stage(), crate::source::Stage::Check);
        assert_eq!(refused.diagnostic().kind, crate::DiagnosticKind::Exhausted);
    }

    #[test]
    fn calculator_source_refuses_deep_environment_before_namespace_snapshot_clone() {
        let mut session = ModuleSession::new_calculator()
            .unwrap_or_else(|_| panic!("calculator environment unavailable"));
        let environment = &mut session.source.declared.as_mut()
            .unwrap_or_else(|| panic!("calculator declaration context unavailable")).environment;
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
        assert!(environment.calculator_clone_budget().is_none(),
            "nested program stacks must contribute every retained node to the clone bound");
        let limits = crate::Limits {
            bytes: 262_144, nodes: 524_288, depth: 64, work: 512_000_000,
        };
        let refused = session.prepare(b"1", &[], limits)
            .err().unwrap_or_else(|| panic!("oversized environment reached snapshot cloning"));
        assert_eq!(refused.stage(), crate::source::Stage::Check);
        assert_eq!(refused.diagnostic().kind, crate::DiagnosticKind::Exhausted);
    }
}

fn revision(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn charge_dependency(
    dependency: &crate::intrinsic::ProofDependency,
    remaining_bytes: &mut usize,
    remaining_nodes: &mut usize,
) -> bool {
    let (Some(bytes), Some(nodes)) = (
        remaining_bytes.checked_sub(dependency.source.len()),
        remaining_nodes.checked_sub(1),
    ) else {
        return false;
    };
    *remaining_bytes = bytes;
    *remaining_nodes = nodes;
    for earlier in &dependency.dependencies {
        if !charge_dependency(earlier, remaining_bytes, remaining_nodes) {
            return false;
        }
    }
    true
}

fn uses(form: &crate::intrinsic::Form, name: &str) -> bool {
    use crate::intrinsic::FormKind;
    let FormKind::List(items) = &form.kind else {
        return false;
    };
    if items.len() == 2
        && matches!(&items[0].kind, FormKind::Atom(operation) if operation == "use")
        && matches!(&items[1].kind, FormKind::Atom(reference) if reference == name)
    {
        return true;
    }
    items.iter().any(|item| uses(item, name))
}
