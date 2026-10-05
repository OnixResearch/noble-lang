mod namespace;
mod prepare;
mod register;

macro_rules! present {
    ($candidate:expr) => {
        match $candidate {
            Some(value) => value,
            None => return None,
        }
    };
}

// A v1 unit is classified into exactly one of these four syntactic forms.
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleKind {
    Module,
    Import,
    Definition,
    Expression,
}

#[derive(Debug)]
pub struct ModuleSession {
    bindings: alloc::vec::Vec<super::BoundOperation>,
    pub(super) modules: alloc::vec::Vec<super::Module>,
    aliases: alloc::vec::Vec<super::Alias>,
    generation: u64,
    pub(super) source: crate::source::Session,
}

#[derive(Debug)]
pub struct ModulePrepared {
    kind: ModuleKind,
    generation: u64,
    limits: crate::Limits,
    previous_modules: alloc::vec::Vec<super::Module>,
    previous_aliases: alloc::vec::Vec<super::Alias>,
    previous_history: alloc::vec::Vec<u8>,
    staged: ModuleSession,
    submission: Option<noble_kernel::execution::Submission>,
    output: alloc::vec::Vec<noble_kernel::types::Ty>,
    pending: Option<crate::intrinsic::ProofBatch>,
}

impl ModulePrepared {
    pub const fn kind(&self) -> ModuleKind {
        self.kind
    }
    pub const fn submission(&self) -> Option<&noble_kernel::execution::Submission> {
        self.submission.as_ref()
    }
    pub const fn output(&self) -> &[noble_kernel::types::Ty] {
        self.output.as_slice()
    }
    /// Uncommitted source obligations, never evidence of proof acceptance.
    pub const fn proof_obligations(&self) -> Option<&crate::intrinsic::ProofBatch> {
        self.pending.as_ref()
    }
    /// Typed, non-executable contract metadata for the newly staged module,
    /// including modules with no proof; this is not a proof receipt.
    pub fn contract_goals(&self) -> impl Iterator<Item = &crate::intrinsic::ContractGoal> {
        self.staged
            .modules
            .last()
            .filter(|_| self.kind == ModuleKind::Module)
            .into_iter()
            .flat_map(|module| module.contracts.iter().map(|entry| &entry.goal))
    }
    pub fn bindings(&self) -> &[super::BoundOperation] {
        &self.staged.bindings
    }
    pub fn resolved_module(&self) -> Option<(u64, alloc::string::String, u32)> {
        let module = match self.kind {
            ModuleKind::Module => present!(self.staged.modules.last()),
            ModuleKind::Import => {
                let alias = present!(self.staged.aliases.last());
                let module = present!(self.staged.modules.get(alias.module));
                module
            }
            ModuleKind::Definition | ModuleKind::Expression => {
                let submission = present!(self.submission.as_ref());
                let mut owner = None;
                let mut definition_at = 0;
                let mut is_invalid_owner = false;
                while definition_at < submission.definitions.len() {
                    let definition = &submission.definitions[definition_at];
                    let Ok(index) = usize::try_from(definition.definition.0) else {
                        is_invalid_owner = true;
                        break;
                    };
                    if let Some(id) = submission
                        .environment
                        .definition_owners
                        .get(index)
                        .copied()
                        .flatten()
                    {
                        if owner.is_some_and(|previous| previous != id) {
                            is_invalid_owner = true;
                            break;
                        }
                        owner = Some(id);
                    }
                    definition_at += 1;
                }
                if is_invalid_owner {
                    return None;
                }
                let mut found = None;
                let mut module_at = 0usize;
                while module_at < self.staged.modules.len() {
                    let module = &self.staged.modules[module_at];
                    if Some(module.identity) == owner {
                        found = Some(module);
                        break;
                    }
                    module_at += 1;
                }
                present!(found)
            }
        };
        Some((module.identity, module.name.clone(), module.version))
    }
    pub fn linked_binding(&self) -> Option<&super::BoundOperation> {
        if let Some(submission) = self.submission.as_ref() {
            let mut selected = None;
            let mut definition_at = 0;
            let mut is_ambiguous = false;
            while definition_at < submission.definitions.len() && !is_ambiguous {
                let definition = &submission.definitions[definition_at];
                let mut node_at = 0;
                while node_at < definition.body.candidate.nodes.len() {
                    let node = &definition.body.candidate.nodes[node_at];
                    if let noble_kernel::untrusted::Node::Invocation { def, .. } = node {
                        if let Some(
                            noble_kernel::contracts::Behavior::BoundEmit(slot)
                            | noble_kernel::contracts::Behavior::BoundClock(slot),
                        ) = submission.environment.kind(*def) {
                            if selected.is_some_and(|previous| previous != slot) {
                                is_ambiguous = true;
                                break;
                            }
                            selected = Some(slot);
                        }
                    }
                    node_at += 1;
                }
                definition_at += 1;
            }
            if is_ambiguous {
                return None;
            }
            if let Some(slot) = selected {
                return self.binding_for_slot(slot);
            }
        }
        let (_, module, version) = present!(self.resolved_module());
        let mut linked = None;
        let mut module_at = 0usize;
        while module_at < self.staged.modules.len() {
            let entry = &self.staged.modules[module_at];
            if entry.name == module && entry.version == version {
                linked = Some(entry);
                break;
            }
            module_at += 1;
        }
        let linked = present!(linked);
        let slot = present!(linked.adapter_slot);
        self.binding_for_slot(slot)
    }

    fn binding_for_slot(&self, slot: u32) -> Option<&super::BoundOperation> {
        let mut at = 0usize;
        let mut matched_at = None;
        while at < self.staged.bindings.len() && matched_at.is_none() {
            if self.staged.bindings[at].adapter_slot == slot {
                matched_at = Some(at);
            }
            at += 1;
        }
        match matched_at {
            Some(index) => self.staged.bindings.get(index),
            None => None,
        }
    }
}

impl ModuleSession {
    /// Host-owned bindings are exact, immutable adapter declarations, not authorizations.
    pub fn new(bindings: &[super::BoundOperation]) -> Result<Self, crate::source::Error> {
        if bindings.len() > super::MODULE_CAP {
            return Err(super::error(
                crate::source::Stage::Check,
                "too many bound operations",
            ));
        }
        let mut index = 0;
        let mut is_invalid_binding = false;
        while index < bindings.len() {
            if !super::links::valid_binding(&bindings[index]) {
                is_invalid_binding = true;
                break;
            }
            let mut prior = 0usize;
            while prior < index {
                if super::links::duplicate_binding(&bindings[index], &bindings[prior]) {
                    is_invalid_binding = true;
                    break;
                }
                prior += 1;
            }
            if is_invalid_binding {
                break;
            }
            index += 1;
        }
        if is_invalid_binding {
            return Err(super::error(
                crate::source::Stage::Link,
                "invalid, duplicate or mismatched adapter binding",
            ));
        }
        let mut environment = attempt!(noble_kernel::contracts::environment()
            .map_err(|_| super::error(crate::source::Stage::Check, "invalid kernel environment")));
        environment.declared_modules = true;
        let mut source_session = crate::source::Session::without_test_hosts();
        source_session.declared = Some(super::Context {
            environment,
            words: alloc::vec::Vec::new(),
            owner: None,
        });
        Ok(Self {
            source: source_session,
            bindings: bindings.to_vec(),
            modules: alloc::vec::Vec::new(),
            aliases: alloc::vec::Vec::new(),
            generation: 0,
        })
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    fn snapshot(&self) -> Self {
        Self {
            source: crate::source::Session {
                definitions: self.source.definitions.clone(),
                history: self.source.history.clone(),
                generation: self.source.generation,
                hosts: false,
                text_cursor: false,
                live_selected: None,
                bindings: None,
                declared: self.source.declared.clone(),
            },
            bindings: self.bindings.clone(),
            modules: self.modules.clone(),
            aliases: self.aliases.clone(),
            generation: self.generation,
        }
    }
}
