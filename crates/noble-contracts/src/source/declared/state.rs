mod namespace;
mod prepare;
mod proof;
mod rebind;
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
    /// Only a signed executable root retains its original expression bytes;
    /// the module bodies themselves are retained in their module sources.
    signed_expression: alloc::vec::Vec<u8>,
    /// Some(empty) proves a genuine old-to-new import alias rebind with no
    /// current outside dependents. None is an initial import or other unit.
    rebuilt_import_definitions: Option<alloc::vec::Vec<RebuiltImportDefinition>>,
}

#[derive(Clone, Debug)]
pub struct RebuiltImportDefinition {
    name: alloc::string::String,
    previous_identity: u64,
    new_identity: u64,
}

impl RebuiltImportDefinition {
    pub fn name(&self) -> &str { &self.name }
    pub const fn previous_identity(&self) -> u64 { self.previous_identity }
    pub const fn new_identity(&self) -> u64 { self.new_identity }
}

/// Borrowed, unforgeable-from-DTO ingress for signed source rederivation.
/// Neither an Env, a Candidate, nor the structural kernel result can make one.
pub struct SignedSourceOwner<'a> {
    prepared: &'a ModulePrepared,
}

/// Checked source-only declared-text ingress. No public Env or Submission can
/// select the merged declared/Text profile at the selected Wasm compiler.
pub struct DeclaredTextSourceOwner<'a> {
    prepared: &'a ModulePrepared,
}

impl DeclaredTextSourceOwner<'_> {
    pub fn rederive(&self) -> Result<noble_kernel::execution::Submission, crate::source::Error> {
        let prepared = self.prepared;
        if !prepared.staged.source.declared.as_ref()
            .is_some_and(|context| context.declared_text)
            || !prepared.staged.bindings.is_empty()
        {
            return Err(super::error(crate::source::Stage::Check,
                "declared text source is not in the selected pure profile"));
        }
        attempt!(prepared.verify_signed_origins());
        let original = prepared.submission.as_ref().ok_or_else(|| {
            super::error(crate::source::Stage::Check, "declared text expression has no submission")
        })?;
        let replay = attempt!(prepared.staged.snapshot().pending_signed_expression_words());
        let mut rebuilt = attempt!(replay.source.prepare(
            &prepared.signed_expression,
            &original.request.expected.stack_in,
            prepared.limits,
        ));
        rebuilt.submission.take().ok_or_else(|| {
            super::error(crate::source::Stage::Check,
                "declared text source did not rederive an executable body")
        })
    }
}

impl SignedSourceOwner<'_> {
    /// Recreate the executable program from the staged, private source
    /// snapshot after authenticating each signed lexical owner against the
    /// retained whole-module bytes. The backend compiles this new derivation,
    /// never the caller's mutable public Submission.
    pub fn rederive(&self) -> Result<noble_kernel::execution::Submission, crate::source::Error> {
        let prepared = self.prepared;
        attempt!(prepared.verify_signed_origins());
        let Some(original) = prepared.submission.as_ref() else {
            return Err(super::error(crate::source::Stage::Check, "signed expression has no submission"));
        };
        let replay = attempt!(prepared.staged.snapshot().pending_signed_expression_words());
        let mut rebuilt = attempt!(replay.source.prepare(
            &prepared.signed_expression,
            &original.request.expected.stack_in,
            prepared.limits,
        ));
        rebuilt.submission.take().ok_or_else(|| {
            super::error(crate::source::Stage::Check, "signed source did not rederive an executable body")
        })
    }
}

impl ModulePrepared {
    pub fn declared_text_source_owner(&self) -> Option<DeclaredTextSourceOwner<'_>> {
        if self.kind != ModuleKind::Expression || self.signed_expression.is_empty()
            || !self.staged.source.declared.as_ref()
                .is_some_and(|context| context.declared_text)
        {
            return None;
        }
        Some(DeclaredTextSourceOwner { prepared: self })
    }

    pub fn signed_source_owner(&self) -> Option<SignedSourceOwner<'_>> {
        if self.kind != ModuleKind::Expression || self.signed_expression.is_empty() {
            return None;
        }
        Some(SignedSourceOwner { prepared: self })
    }

    fn verify_signed_origins(&self) -> Result<(), crate::source::Error> {
        let Some(submission) = self.submission.as_ref() else {
            return Err(super::error(crate::source::Stage::Check, "missing signed submission"));
        };
        let mut requested_slots = alloc::vec::Vec::new();
        for definition in &submission.definitions {
            let mut has_self = false;
            for node in &definition.body.candidate.nodes {
                if let noble_kernel::untrusted::Node::SignedSelf { def, .. } = node {
                    has_self = true;
                    if *def != definition.definition {
                        return Err(super::error(crate::source::Stage::Check,
                            "signed call does not target its containing definition"));
                    }
                    let index = usize::try_from(def.0).map_err(|_| super::error(
                        crate::source::Stage::Check, "signed owner index overflow"
                    ))?;
                    let owner = submission.environment.definition_owners.get(index).copied().flatten()
                        .ok_or_else(|| super::error(crate::source::Stage::Check,
                            "signed call has no declared module owner"))?;
                    let source_slot = definition.owner_slot.ok_or_else(|| super::error(
                        crate::source::Stage::Check, "signed owner has no exact source slot",
                    ))?;
                    let source_index = usize::try_from(source_slot).map_err(|_| super::error(
                        crate::source::Stage::Check, "signed owner source slot overflow",
                    ))?;
                    let source = self.staged.source.definitions.get(source_index)
                        .ok_or_else(|| super::error(crate::source::Stage::Check,
                            "signed owner source slot is absent"))?;
                    if source.owner != Some(owner)
                        || definition.source_identity != source.identity
                        || source.signed.as_ref().is_none_or(|relation|
                            relation.source_slot != source_slot)
                        || definition.owner_inst.is_none() {
                        return Err(super::error(crate::source::Stage::Check,
                            "signed owner source slot, module and binder differ"));
                    }
                    if !requested_slots.contains(&source_index) {
                        requested_slots.push(source_index);
                    }
                }
            }
            if !has_self && definition.owner_inst.is_some() {
                return Err(super::error(crate::source::Stage::Check,
                    "unsigned body carries a signed owner binder"));
            }
        }
        let mut checked_slots = alloc::vec::Vec::new();
        for (source_index, definition) in self.staged.source.definitions.iter().enumerate() {
            if !definition.tree.nodes.iter().any(|node| {
                matches!(node.kind, crate::source::Kind::Call(crate::source::Target::SelfOwner))
            }) {
                continue;
            }
            if !requested_slots.contains(&source_index) {
                continue;
            }
            let Some(owner) = definition.owner else {
                return Err(super::error(crate::source::Stage::Check, "signed source has no module owner"));
            };
            let Some(relation) = definition.signed.as_ref() else {
                return Err(super::error(crate::source::Stage::Check,
                    "signed source lacks its precommit rigid owner relation"));
            };
            if usize::try_from(relation.source_slot).ok() != Some(source_index)
                || relation.lexical_nodes.is_empty()
            {
                return Err(super::error(crate::source::Stage::Check,
                    "signed owner source slot differs from its checked reservation"));
            }
            let Some(module) = self.staged.modules.iter().find(|module| module.identity == owner) else {
                return Err(super::error(crate::source::Stage::Check, "signed source owner has no original module"));
            };
            let Some((ordinal, span)) = module.definition_spans.iter().enumerate()
                .find(|(_, original)| original.name == definition.name)
            else {
                return Err(super::error(crate::source::Stage::Check, "signed definition absent from original module"));
            };
            let start = usize::try_from(span.span.start).map_err(|_| {
                super::error(crate::source::Stage::Check, "signed source span exceeds address space")
            })?;
            let end = usize::try_from(span.span.end).map_err(|_| {
                super::error(crate::source::Stage::Check, "signed source span exceeds address space")
            })?;
            let Some(bytes) = module.source.get(start..end) else {
                return Err(super::error(crate::source::Stage::Check, "signed definition lies outside original module"));
            };
            let lexical_body = attempt!(super::verify_definition_origin(
                super::DefinitionOrigin {
                    module_source: &module.source,
                    module: &module.name,
                    version: module.version,
                    definition: &definition.name,
                    ordinal: u32::try_from(ordinal).map_err(|_| super::error(
                        crate::source::Stage::Check, "signed definition ordinal overflow"
                    ))?,
                    span: span.span,
                },
                bytes, self.limits,
            ).map_err(|problem| super::diagnostic(crate::source::Stage::Check, problem)));
            if lexical_body != span.body_span {
                return Err(super::error(crate::source::Stage::Check, "signed body differs from original module"));
            }
            let mut meter = crate::Meter::new(self.limits);
            attempt!(meter.charge(
                attempt!(crate::index(module.source.len(), definition.tree.span)
                    .map_err(|problem| super::diagnostic(crate::source::Stage::Check, problem))),
                definition.tree.span,
            ).map_err(|problem| super::diagnostic(crate::source::Stage::Check, problem)));
            let original_scheme = attempt!(register::original_signature(
                &self.staged, module, &definition.name, self.limits,
            ));
            let saved_scheme = definition.signature.as_ref().ok_or_else(|| super::error(
                crate::source::Stage::Check, "signed definition has no retained signature",
            ))?;
            if !same_scheme(&original_scheme, saved_scheme) {
                return Err(super::error(crate::source::Stage::Check,
                    "signed definition signature differs from original module"));
            }
            let source_slot = attempt!(crate::index(source_index, definition.tree.span)
                .map_err(|problem| super::diagnostic(crate::source::Stage::Check, problem)));
            let mut matched_specialization = false;
            for specialization in submission.definitions.iter()
                .filter(|specialization| specialization.owner_slot == Some(source_slot)) {
                attempt!(meter.charge(
                    attempt!(crate::index(module.source.len(), definition.tree.span)
                        .map_err(|problem| super::diagnostic(crate::source::Stage::Check, problem))),
                    definition.tree.span,
                ).map_err(|problem| super::diagnostic(crate::source::Stage::Check, problem)));
                if specialization.source_identity != definition.identity
                    || submission.environment.definition_owners
                        .get(usize::try_from(specialization.definition.0).unwrap_or(usize::MAX))
                        != Some(&Some(owner))
                    || submission.environment.scheme(specialization.definition)
                        .is_none_or(|scheme| !same_scheme(&original_scheme, scheme)) {
                    return Err(super::error(crate::source::Stage::Check,
                        "signed specialization has a foreign source, signature or effect ceiling"));
                }
                matched_specialization = true;
            }
            if !matched_specialization {
                return Err(super::error(crate::source::Stage::Check,
                    "signed definition has no exact specialized owner"));
            }
            let (mut original, name) = attempt!(crate::source::parsing::parse_declared(bytes, &mut meter)
                .map_err(|problem| super::diagnostic(crate::source::Stage::Check, problem)));
            if self.staged.source.declared.as_ref()
                .is_some_and(|context| context.declared_text)
            {
                attempt!(crate::source::locals::lower(&mut original, &mut meter)
                    .map_err(|problem| super::diagnostic(crate::source::Stage::Check, problem)));
            }
            if name.as_deref() != Some(definition.name.as_str())
                || original.body != definition.tree.body
                || original.nodes.len() != definition.tree.nodes.len()
            {
                return Err(super::error(crate::source::Stage::Check, "signed tree differs from original definition"));
            }
            let mut signed_count = 0usize;
            for (node_index, (saved, parsed)) in definition.tree.nodes.iter()
                .zip(&original.nodes).enumerate() {
                if saved.span != parsed.span {
                    return Err(super::error(crate::source::Stage::Check, "signed node moved from original source"));
                }
                match (&saved.kind, &parsed.kind) {
                    (crate::source::Kind::Call(crate::source::Target::SelfOwner),
                     crate::source::Kind::Word(word)) if word == definition.name.as_bytes() => {
                        let index = u32::try_from(node_index).map_err(|_| super::error(
                            crate::source::Stage::Check, "signed lexical node index overflow"
                        ))?;
                        if relation.lexical_nodes.binary_search(&index).is_err() {
                            return Err(super::error(crate::source::Stage::Check,
                                "signed occurrence was not checked in its original quotation path"));
                        }
                        signed_count += 1;
                    }
                    (crate::source::Kind::Call(_), crate::source::Kind::Word(word))
                        if word != definition.name.as_bytes() => {}
                    (crate::source::Kind::Call(crate::source::Target::Builtin(saved)),
                     crate::source::Kind::Call(crate::source::Target::Builtin(parsed)))
                        if saved == parsed => {}
                    (crate::source::Kind::Literal(a), crate::source::Kind::Literal(b)) if a == b => {}
                    (crate::source::Kind::Text(a), crate::source::Kind::Text(b)) if a == b => {}
                    (crate::source::Kind::Quotation(a), crate::source::Kind::Quotation(b)) if a == b => {}
                    _ => return Err(super::error(crate::source::Stage::Check,
                        "signed lexical occurrence differs from original source")),
                }
            }
            if signed_count != relation.lexical_nodes.len() || definition.signature.is_none() {
                return Err(super::error(crate::source::Stage::Check, "signed owner lacks lexical self or signature"));
            }
            attempt!(crate::source::preparation::symbolic::verify(
                &definition.tree, &original, relation, &definition.name, bytes, &mut meter,
            ).map_err(|problem| super::diagnostic(crate::source::Stage::Check, problem)));
            checked_slots.push(source_index);
        }
        if requested_slots.iter().any(|slot| !checked_slots.contains(slot)) {
            return Err(super::error(crate::source::Stage::Check,
                "signed call has no authenticated exact source definition"));
        }
        if submission.body.candidate.nodes.iter().any(|node| {
            matches!(node, noble_kernel::untrusted::Node::SignedSelf { .. })
        }) {
            return Err(super::error(crate::source::Stage::Check,
                "signed self in executable root is not an owner definition"));
        }
        Ok(())
    }

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
    /// The final checked alias introduced by this import, not a source
    /// spelling re-parsed by a consumer or an alias from an earlier session.
    pub fn resolved_import_alias(&self) -> Option<&str> {
        if self.kind != ModuleKind::Import {
            return None;
        }
        self.staged.aliases.last()
            .filter(|alias| !alias.pending)
            .map(|alias| alias.spelling.as_str())
    }
    /// A source-checked transitive rebind preview, not an alias-word refresh.
    /// Publication still requires `ModuleSession::commit` on its original
    /// namespace snapshot; saved old Program values retain prior identities.
    pub fn rebuilt_import_definitions(&self) -> Option<&[RebuiltImportDefinition]> {
        self.rebuilt_import_definitions.as_deref()
    }
    pub fn resolved_module(&self) -> Option<(u64, alloc::string::String, u32)> {
        let module = match self.kind {
            ModuleKind::Module => present!(self.staged.modules.last()),
            ModuleKind::Import => {
                let alias = present!(self.staged.aliases.last());
                if alias.pending { return None; }
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
        Self::initialize(bindings, false, false)
    }

    /// Select the signed source grammar. Valid lexical ownership, kernel
    /// structure and a private compiler ingress are still required.
    pub fn new_signed_self(bindings: &[super::BoundOperation]) -> Result<Self, crate::source::Error> {
        Self::initialize(bindings, true, false)
    }

    /// Selected pure declared text profile; its signed self extension is explicit.
    /// Even a denied host adapter cannot be admitted here.
    pub fn new_declared_text(
        bindings: &[super::BoundOperation],
        signed_self: bool,
    ) -> Result<Self, crate::source::Error> {
        if !bindings.is_empty() {
            return Err(super::error(crate::source::Stage::Link,
                "declared text does not accept host bindings"));
        }
        Self::initialize(bindings, signed_self, true)
    }

    fn initialize(bindings: &[super::BoundOperation], signed_self: bool, declared_text: bool) -> Result<Self, crate::source::Error> {
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
        let mut environment = attempt!((if declared_text {
            noble_kernel::contracts::declared_text_environment()
        } else {
            noble_kernel::contracts::environment()
        })
            .map_err(|_| super::error(crate::source::Stage::Check, "invalid kernel environment")));
        environment.declared_modules = true;
        let mut source_session = crate::source::Session::without_test_hosts();
        source_session.declared = Some(super::Context {
            environment,
            words: alloc::vec::Vec::new(),
            aliases: alloc::vec::Vec::new(),
            owner: None,
            signed_self,
            declared_text,
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

    pub fn declared_text(&self) -> bool {
        self.source.declared.as_ref()
            .is_some_and(|context| context.declared_text)
    }

    fn snapshot(&self) -> Self {
        Self {
            source: crate::source::Session {
                definitions: self.source.definitions.clone(),
                history: self.source.history.clone(),
                generation: self.source.generation,
                hosts: false,
                text_cursor: self.source.text_cursor,
                live_selected: None,
                live_slots: None,
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

fn same_scheme(left: &noble_kernel::words::Scheme, right: &noble_kernel::words::Scheme) -> bool {
    left.var_kinds == right.var_kinds
        && left.stack_in == right.stack_in
        && left.stack_out == right.stack_out
        && left.effects == right.effects
}

#[cfg(test)]
mod symbolic_owner_tests {
    use super::{ModulePrepared, ModuleSession};
    use crate::source::Target;

    fn prepared_owner() -> ModulePrepared {
        let limits = crate::Limits {
            bytes: 65_536, nodes: 16_384, depth: 64, work: 2_000_000,
        };
        let session = ModuleSession::new_signed_self(&[]).expect("signed source session");
        let module = b"module nested@1 [ signature helper forall<S:stack> [ S -- S ! pure ] def helper [ 1 drop ] signature callable forall<S:stack> [ S -- S ! pure ] export callable def callable [ [ callable ] drop helper 1 drop ] ]";
        let staged = session.prepare(module, &[], limits).expect("signed source module");
        let (session, outcome) = session.commit(staged);
        outcome.expect("signed module commit");
        session.prepare(b"nested@1.callable", &[], limits).expect("signed expression")
    }

    #[test]
    fn symbolic_owner_certificate_rejects_missing_edge_forged_target_and_foreign_owner() {
        let mut prepared = prepared_owner();
        prepared.signed_source_owner().expect("source owner").rederive()
            .expect("retained complete nested source graph");
        let relation = prepared.staged.source.definitions.iter_mut()
            .find_map(|definition| definition.signed.as_mut())
            .expect("nested recursive owner relation");
        relation.symbolic_nodes.pop();
        assert!(prepared.signed_source_owner().expect("source owner").rederive().is_err(),
            "a missing original node must not pass sealed signed ingress");

        let mut forged = prepared_owner();
        let relation = forged.staged.source.definitions.iter_mut()
            .find_map(|definition| definition.signed.as_mut())
            .expect("nested recursive owner relation");
        let self_node = relation.symbolic_nodes.iter_mut()
            .find(|node| node.target == Some(Target::SelfOwner))
            .expect("nested owner edge");
        self_node.target = Some(Target::Builtin(0));
        assert!(forged.signed_source_owner().expect("source owner").rederive().is_err(),
            "forging a coherent declaration target must not replace the lexical owner");

        let mut foreign = prepared_owner();
        let definition = foreign.staged.source.definitions.iter_mut()
            .find(|definition| definition.signed.is_some())
            .expect("nested recursive owner definition");
        definition.owner = Some(u64::MAX);
        assert!(foreign.signed_source_owner().expect("source owner").rederive().is_err(),
            "a foreign source owner must not authenticate the submitted signed edge");
    }

    #[test]
    fn signed_namespace_alias_stays_inert_until_each_owner_has_a_concrete_certificate() {
        let limits = crate::Limits {
            bytes: 65_536, nodes: 16_384, depth: 64, work: 2_000_000,
        };
        let session = ModuleSession::new_signed_self(&[]).expect("signed source session");
        let source = b"module dual@1 [ signature left forall<S:stack> [ S -- S ! pure ] export left def left [ left ] signature right forall<S:stack> [ S I64 -- S I64 ! pure ] export right def right [ right ] ]";
        let staged = session.prepare(source, &[], limits).expect("two signed source owners");
        let (session, result) = session.commit(staged);
        result.expect("signed source-only module");
        let generation = session.generation();
        assert!(matches!(session.prepare(b"def capture [ dual@1.left ]", &[], limits),
            Err(error) if error.stage() == crate::source::Stage::Resolve));
        assert!(matches!(session.prepare(
            b"module capture@1 [ export call def call [ dual@1.left ] ]", &[], limits,
        ), Err(error) if error.stage() == crate::source::Stage::Resolve));
        assert_eq!(session.generation(), generation,
            "an unverified versioned signed name must not publish a captured definition");
        let pending = session.prepare(b"import dual@1 as d", &[], limits)
            .expect("pending inert import");
        assert!(pending.resolved_module().is_none(), "inert import cannot expose module");
        let (session, result) = session.commit(pending);
        result.expect("pending inert import commit");
        assert!(matches!(session.prepare(b"def captured_alias [ d.left ]", &[], limits),
            Err(error) if error.stage() == crate::source::Stage::Resolve));
        assert!(matches!(session.prepare(b"d.left", &[], limits),
            Err(error) if error.stage() == crate::source::Stage::Check
                || error.stage() == crate::source::Stage::Resolve));

        let checked_left = session.prepare(b"dual@1.left", &[], limits)
            .expect("concrete left owner certificate");
        let (session, result) = session.commit(checked_left);
        result.expect("left owner commit");
        assert!(session.prepare(b"d.left", &[], limits).is_err(),
            "one certificate must not activate an alias containing a second signed owner");
        assert!(matches!(session.prepare(b"def capture [ dual@1.right ]", &[], limits),
            Err(error) if error.stage() == crate::source::Stage::Resolve));
        assert!(matches!(session.prepare(b"1 d.right", &[], limits),
            Err(error) if error.stage() == crate::source::Stage::Resolve));

        let checked_right = session.prepare(b"1 dual@1.right", &[], limits)
            .expect("concrete right owner certificate");
        let (session, result) = session.commit(checked_right);
        result.expect("right owner commit");
        session.prepare(b"d.left", &[], limits).expect("activated alias rechecks its own instance");
        session.prepare(b"1 d.right", &[], limits).expect("second owner's alias checks its instance");
        session.prepare(b"def capture [ 1 dual@1.right drop ]", &[], limits)
            .expect("qualified capture is admitted only after both concrete certificates");
    }

    #[test]
    fn sealed_ingress_pairs_each_source_owner_with_its_original_signature() {
        let mut prepared = prepared_owner();
        let definition = prepared.staged.source.definitions.iter_mut()
            .find(|definition| definition.signed.is_some())
            .expect("retained signed source definition");
        definition.signature.as_mut().expect("signed owner signature").effects
            .push(noble_kernel::shapes::EffectSlot::Effect(
                noble_kernel::contracts::TEST_EMIT,
            ));
        assert!(prepared.signed_source_owner().expect("sealed owner").rederive().is_err(),
            "a changed effect ceiling must not claim the original signed signature");

        let mut forged = prepared_owner();
        let entry = forged.submission.as_mut().expect("submitted signed definition")
            .definitions.iter_mut()
            .find(|definition| definition.owner_inst.is_some())
            .expect("signed owner specialization");
        entry.source_identity = u64::MAX;
        assert!(forged.signed_source_owner().expect("sealed owner").rederive().is_err(),
            "same-module signed owner may not borrow another source identity");
    }
}
