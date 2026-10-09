#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; preparation's mutable parameters are only its request-local meter; draft trees and output buffers are newly owned, while source bytes, supplied types, and the committed session are borrowed unchanged until an explicit later commit."
)]

pub(in crate::source) mod name_index;
pub(in crate::source) mod symbolic;
pub(in crate::source) mod universal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::source) struct SourceStampPrefix([u64; 4]);

/// Hash the one immutable retained module once, charged to its module
/// transaction. Each definition subsequently hashes only its own suffix.
pub(in crate::source) fn module_stamp_prefix(
    module: &alloc::sync::Arc<[u8]>,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<SourceStampPrefix, crate::Diagnostic> {
    attempt!(meter.charge(attempt!(crate::index(module.len(), span)), span));
    let mut lanes = [
        0xcbf29ce484222325u64,
        0x84222325cbf29ce4,
        0x9e3779b97f4a7c15,
        0x517cc1b727220a95,
    ];
    for byte in module.iter() {
        for (index, lane) in lanes.iter_mut().enumerate() {
            *lane = lane.wrapping_mul(0x100000001b3)
                ^ u64::from(*byte).wrapping_add(index as u64);
        }
    }
    Ok(SourceStampPrefix(lanes))
}

#[derive(Clone, Copy)]
pub(in crate::source) struct OriginalDefinition<'a> {
    pub module_source: &'a [u8],
    pub module_original: &'a alloc::sync::Arc<[u8]>,
    pub module_prefix: &'a SourceStampPrefix,
    pub name_index: Option<&'a name_index::NameIndex>,
    pub module_name: &'a str,
    pub module_version: u32,
    pub ordinal: u32,
    pub source_span: crate::Span,
    pub body_span: crate::Span,
}

pub(in crate::source) fn original_stamp(
    original: OriginalDefinition<'_>,
    definition: &[u8],
    generation: u64,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<noble_kernel::universal::SourceStamp, crate::Diagnostic> {
    if !core::ptr::eq(original.module_source, original.module_original.as_ref()) {
        return Err(crate::invalid(span, "source prefix is not bound to the original retained module"));
    }
    let start = attempt!(crate::offset(original.source_span.start, span));
    let end = attempt!(crate::offset(original.source_span.end, span));
    attempt!(meter.charge(attempt!(crate::index(definition.len(), span)), span));
    if original.module_source.get(start..end) != Some(definition) {
        return Err(crate::invalid(span, "definition differs from original whole module source"));
    }
    // A comparison tag, never a credential: source authority is the exact
    // immutable registered Arc, checked again by identity at each owner use.
    let mut lanes = original.module_prefix.0;
    attempt!(meter.charge(attempt!(crate::index(
        original.module_name.len()
            .saturating_add(12), span,
    )), span));
    let version = original.module_version.to_le_bytes();
    let ordinal = original.ordinal.to_le_bytes();
    let start = original.source_span.start.to_le_bytes();
    for byte in original.module_name.as_bytes().iter()
        .chain(version.iter())
        .chain(ordinal.iter())
        .chain(start.iter())
    {
        for (index, lane) in lanes.iter_mut().enumerate() {
            *lane = lane.wrapping_mul(0x100000001b3)
                ^ u64::from(*byte).wrapping_add(index as u64);
        }
    }
    let mut source_tag = [0u8; 32];
    for (slot, lane) in lanes.iter().enumerate() {
        source_tag[slot * 8..slot * 8 + 8].copy_from_slice(&lane.to_le_bytes());
    }
    Ok(noble_kernel::universal::SourceStamp {
        source_tag, grant_epoch: generation, generation,
    })
}

impl super::Session {
    pub fn prepare(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
    ) -> Result<super::Prepared, super::Error> {
        self.prepare_signed(source_bytes, inputs, None, limits, limits.bytes)
    }

    pub(super) fn prepare_with_retention(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
        retained_bytes: u32,
    ) -> Result<super::Prepared, super::Error> {
        self.prepare_signed(source_bytes, inputs, None, limits, retained_bytes)
    }

    pub(super) fn prepare_signed(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        signature: Option<&noble_kernel::words::Scheme>,
        limits: crate::Limits,
        retained_bytes: u32,
    ) -> Result<super::Prepared, super::Error> {
        self.prepare_with_original(
            source_bytes, inputs, signature, limits, retained_bytes, None,
        )
    }

    pub(in crate::source) fn prepare_signed_from_module(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        signature: Option<&noble_kernel::words::Scheme>,
        limits: crate::Limits,
        retained_bytes: u32,
        original: OriginalDefinition<'_>,
    ) -> Result<super::Prepared, super::Error> {
        self.prepare_with_original(
            source_bytes, inputs, signature, limits, retained_bytes, Some(original),
        )
    }

    fn prepare_with_original(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        signature: Option<&noble_kernel::words::Scheme>,
        limits: crate::Limits,
        retained_bytes: u32,
        original: Option<OriginalDefinition<'_>>,
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
        if self
            .declared
            .as_ref()
            .is_some_and(|context| context.declared_text)
        {
            attempt!(super::locals::lower(&mut tree, &mut meter)
                .map_err(|error| super::Error::at(super::Stage::Resolve, error)));
        }
        let alias_candidates = if name.is_some() {
            attempt!(alias_candidates(&tree, self, &mut meter)
                .map_err(|error| super::Error::at(super::Stage::Resolve, error)))
        } else {
            alloc::vec::Vec::new()
        };
        let signed_owner = signature.is_some()
            && self
                .declared
                .as_ref()
                .is_some_and(|context| context.signed_self);
        if let Err(error) =
            super::resolution::resolve(
                &mut tree, name.as_deref(), self, signed_owner,
                original.as_ref().and_then(|owner| owner.name_index), &mut meter,
            )
        {
            return Err(super::Error::at(super::Stage::Resolve, error));
        }
        let alias_uses = if alias_candidates.is_empty() {
            alloc::vec::Vec::new()
        } else {
            attempt!(
                checked_alias_uses(&tree, self, &alias_candidates, &mut meter)
                    .map_err(|error| super::Error::at(super::Stage::Check, error))
            )
        };
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
        if let Err(error) = super::preflight::check(inputs, self, tree.span, &mut meter) {
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
                attempt!(source_bytes.len().checked_add(4).ok_or_else(|| {
                    super::Error::at(
                        super::Stage::Check,
                        super::exhausted(tree.span, "retained definition source length overflows"),
                    )
                })),
                super::inference::Mode::Declaration,
            )
        } else {
            (0, super::inference::Mode::Submission)
        };
        if let Err(error) = self.retained(extra, retained_bytes, tree.span, &mut meter) {
            return Err(super::Error::at(super::Stage::Check, error));
        }
        let mut environment = match self.environment() {
            Ok(environment) => environment,
            Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
        };
        let mut universal_bindings = alloc::vec::Vec::new();
        for (index, definition) in self.definitions.iter().enumerate() {
            attempt!(meter.charge(1, tree.span)
                .map_err(|problem| super::Error::at(super::Stage::Check, problem)));
            let Some(seal) = definition.universal.as_ref() else { continue; };
            if environment.scheme(seal.owner()) != definition.signature.as_ref()
                || environment.kind(seal.owner())
                    != Some(noble_kernel::contracts::Behavior::Named)
                || environment.definition_owners
                    .get(seal.owner().0 as usize) != Some(&definition.owner)
            {
                return Err(super::Error::at(super::Stage::Check, crate::invalid(
                    tree.span, "sealed source owner differs from persistent environment",
                )));
            }
            universal_bindings.push((
                attempt!(crate::index(index, tree.span)
                    .map_err(|problem| super::Error::at(super::Stage::Check, problem))),
                seal.owner(),
            ));
        }
        let has_self = tree.nodes.iter().any(|node|
            matches!(node.kind, super::Kind::Call(super::Target::SelfOwner)));
        let universal_eligible = if name.is_some()
            && original.is_some()
            && self.declared.as_ref().is_some_and(|context|
                context.declared_text && context.owner.is_some())
        {
            match signature {
                Some(scheme) => attempt!(universal::eligible(scheme, tree.span, &mut meter)
                    .map_err(|problem| super::Error::at(super::Stage::Check, problem))),
                None => false,
            }
        } else {
            false
        };
        // An actual unpublished owner interface occupies this transaction's
        // prospective Env row while its body is checked. It is never installed
        // into the committed namespace or exposed as a raw graph self edge.
        let prospective_owner = if (signed_owner && has_self) || universal_eligible {
            let Some(scheme) = signature else {
                return Err(super::Error::at(
                    super::Stage::Check,
                    crate::invalid(tree.span, "signed owner has no signature"),
                ));
            };
            let Some(owner) = self.declared.as_ref().and_then(|context| context.owner) else {
                return Err(super::Error::at(
                    super::Stage::Check,
                    crate::invalid(tree.span, "signed owner has no module"),
                ));
            };
            let def = noble_kernel::contracts::Definition(attempt!(crate::index(
                environment.defs.len(),
                tree.span,
            )
            .map_err(|problem| super::Error::at(super::Stage::Check, problem))));
            attempt!(meter
                .node(tree.span)
                .map_err(|problem| super::Error::at(super::Stage::Check, problem)));
            environment.defs.push(scheme.clone());
            environment
                .kinds
                .push(noble_kernel::contracts::Behavior::Named);
            let mut deps = alloc::vec::Vec::new();
            for node in &tree.nodes {
                attempt!(meter.charge(1, node.span)
                    .map_err(|problem| super::Error::at(super::Stage::Check, problem)));
                if let super::Kind::Call(super::Target::Named(source_slot)) = &node.kind {
                    if let Some((_, def)) = universal_bindings.iter()
                        .find(|(slot, _)| slot == source_slot)
                    {
                        if !deps.contains(def) { deps.push(*def); }
                    }
                }
            }
            environment.deps.push(deps);
            environment.definition_owners.push(Some(owner));
            Some(def)
        } else {
            None
        };
        let scope = super::inference::Scope {
            root: &tree,
            session: self,
            environment: &environment,
            root_signature: signature,
            prospective_owner,
            universal_bindings: &universal_bindings,
        };
        let state = match super::inference::infer(scope, mode, inputs, &mut meter) {
            Ok(state) => state,
            Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
        };
        let signed_relation = match prospective_owner {
            Some(owner) if has_self => {
                let scheme = signature.ok_or_else(|| {
                    super::Error::at(
                        super::Stage::Check,
                        crate::invalid(tree.span, "signed owner has no signature"),
                    )
                })?;
                Some(attempt!(symbolic::check(
                    &tree,
                    &state,
                    scheme,
                    source_bytes,
                    owner,
                    self,
                    &environment,
                    true,
                    original.as_ref().and_then(|owner| owner.name_index),
                    &mut meter
                )
                .map_err(|problem| super::Error::at(
                    super::Stage::Check,
                    problem
                ))))
            }
            _ => None,
        };
        let can_reify = universal_eligible && state.bodies.len() == 1
            && !tree.nodes.iter().any(|node| matches!(
                node.kind, super::Kind::Text(_)
                    | super::Kind::Call(super::Target::SlotInvoke(_))
            ))
            && tree.nodes.iter().all(|node| match &node.kind {
                super::Kind::Call(super::Target::Named(slot)) =>
                    universal_bindings.iter().any(|(bound, _)| bound == slot),
                _ => true,
            });
        let universal_candidate = if can_reify {
            match universal::candidate(&state, &mut meter) {
                Ok(candidate) => Some(candidate),
                Err(problem) if signed_relation.is_some()
                    && problem.kind == crate::DiagnosticKind::Unsupported => None,
                Err(problem) => return Err(super::Error::at(super::Stage::Check, problem)),
            }
        } else {
            None
        };
        let universal = if let Some(candidate) = universal_candidate {
            let owner = prospective_owner.ok_or_else(|| super::Error::at(
                super::Stage::Check, crate::invalid(tree.span, "universal owner has no Env row"),
            ))?;
            let scheme = signature.ok_or_else(|| super::Error::at(
                super::Stage::Check, crate::invalid(tree.span, "universal owner has no signature"),
            ))?;
            let relation = match signed_relation.as_ref() {
                Some(relation) => relation.clone(),
                None => attempt!(symbolic::check(
                    &tree, &state, scheme, source_bytes, owner, self, &environment,
                    false, original.as_ref().and_then(|owner| owner.name_index), &mut meter,
                ).map_err(|problem| super::Error::at(super::Stage::Check, problem))),
            };
            let original = original.ok_or_else(|| super::Error::at(
                super::Stage::Check, crate::invalid(tree.span, "universal original module missing"),
            ))?;
            let stamp = attempt!(original_stamp(
                original, source_bytes, self.generation, tree.span, &mut meter,
            ).map_err(|problem| super::Error::at(super::Stage::Check, problem)));
            let dependencies = self.definitions.iter().filter_map(|definition|
                definition.universal.as_ref().map(|seal| noble_kernel::universal::Dependency {
                    seal, current_stamp: seal.stamp(),
                })
            ).collect::<alloc::vec::Vec<_>>();
            let available_work = meter.work;
            let request = noble_kernel::untrusted::Request {
                input_bytes: attempt!(crate::index(source_bytes.len(), tree.span)
                    .map_err(|problem| super::Error::at(super::Stage::Check, problem))),
                expected: noble_kernel::untrusted::Expected {
                    stack_in: alloc::vec::Vec::new(),
                    stack_out: alloc::vec::Vec::new(),
                    allowed_effects: noble_kernel::types::EffSet::empty(),
                },
                limits: noble_kernel::untrusted::Limits {
                    bytes: limits.bytes, nodes: limits.nodes, depth: limits.depth,
                    type_size: crate::syntax::TYPE_CAP,
                    stack_height: crate::inference::STACK_CAP,
                    work: available_work, diagnostics: 8,
                },
            };
            let seal = noble_kernel::universal::check_scheme(
                &environment, &request, &candidate, owner, stamp, &dependencies,
            ).map_err(|_| super::Error::at(super::Stage::Check,
                crate::invalid(tree.span, "independent universal source check refused")))?;
            attempt!(meter.charge(seal.work_spent(), tree.span)
                .map_err(|problem| super::Error::at(super::Stage::Check, problem)));
            Some((seal, super::UniversalOrigin {
                relation,
                original_source: original.module_original.clone(),
                module_name: alloc::string::String::from(original.module_name),
                module_version: original.module_version,
                ordinal: original.ordinal,
                source_span: original.source_span,
                body_span: original.body_span,
            }))
        } else {
            None
        };
        let mut addition = alloc::vec::Vec::new();
        let (definition, submission, checked, output) = match name {
            Some(name) => {
                let definition = match declaration(
                    tree,
                    name,
                    source_bytes,
                    signature,
                    signed_relation,
                    universal,
                    alias_uses,
                    self,
                    &mut meter,
                ) {
                    Ok((definition, bytes)) => {
                        addition = bytes;
                        definition
                    }
                    Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
                };
                (Some(definition), None, None, inputs.to_vec())
            }
            None => {
                let (submission, checked, output) = attempt!(super::emission::emit(
                    state,
                    environment,
                    &self.definitions,
                    source_bytes.len(),
                    &mut meter
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
            limits,
            frontend_nodes: meter.nodes,
            frontend_work_spent: limits.work.saturating_sub(meter.work),
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
        reason = "Owner: noble-maintainers; preparation owns a runtime clone of the retained binding environment or builds the checked bootstrap environment, both allocating operations."
    )]
    pub(super) fn environment(&self) -> Result<noble_kernel::contracts::Env, crate::Diagnostic> {
        if let Some(declared) = &self.declared {
            return Ok(declared.environment.clone());
        }
        if let Some(profile) = &self.live_slots {
            return Ok(profile.environment.clone());
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
        retained_bytes: u32,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        let history_bytes = attempt!(crate::index(self.history.len(), span));
        let limit_bytes = attempt!(crate::offset(retained_bytes, span));
        let total = attempt!(self.history.len().checked_add(extra).ok_or_else(|| {
            super::exhausted(span, "retained definition history byte count overflows")
        }));
        if total > limit_bytes {
            return Err(super::exhausted(
                span,
                "retained definition history byte limit exceeded",
            ));
        }
        meter.charge(history_bytes, span)
    }
}

/// Preserve exact source node ordinals before resolution erases spellings.
/// Only current outside aliases on a top-level named body are mutable;
/// module-local imports belong to that module's immutable source snapshot.
fn alias_candidates(
    tree: &super::Tree,
    session: &super::Session,
    meter: &mut crate::Meter,
) -> Result<alloc::vec::Vec<(u32, usize)>, crate::Diagnostic> {
    let Some(context) = session.declared.as_ref() else {
        return Ok(alloc::vec::Vec::new());
    };
    if context.owner.is_some() || context.aliases.is_empty() {
        return Ok(alloc::vec::Vec::new());
    }
    let mut candidates = alloc::vec::Vec::new();
    for (node_index, node) in tree.nodes.iter().enumerate() {
        attempt!(meter.charge(1, node.span));
        let super::Kind::Word(word) = &node.kind else {
            continue;
        };
        for (alias_index, (alias, _)) in context.aliases.iter().enumerate() {
            attempt!(meter.charge(1, node.span));
            if word.len() > alias.len() + 1
                && word.starts_with(alias.as_bytes())
                && word[alias.len()] == b'.'
            {
                candidates.push((attempt!(crate::index(node_index, node.span)), alias_index));
                break;
            }
        }
    }
    Ok(candidates)
}

fn checked_alias_uses(
    tree: &super::Tree,
    session: &super::Session,
    candidates: &[(u32, usize)],
    meter: &mut crate::Meter,
) -> Result<alloc::vec::Vec<super::AliasUse>, crate::Diagnostic> {
    let context = session
        .declared
        .as_ref()
        .ok_or_else(|| crate::internal(tree.span))?;
    let mut uses: alloc::vec::Vec<super::AliasUse> = alloc::vec::Vec::new();
    for &(node_index, alias_index) in candidates {
        let node = tree
            .nodes
            .get(attempt!(crate::offset(node_index, tree.span)))
            .ok_or_else(|| crate::internal(tree.span))?;
        let (alias, owner) = context
            .aliases
            .get(alias_index)
            .ok_or_else(|| crate::internal(node.span))?;
        let super::Kind::Call(target) = &node.kind else {
            return Err(crate::invalid(
                node.span,
                "checked import alias lost its source word",
            ));
        };
        match *target {
            super::Target::Named(index) => {
                let definition = session
                    .definitions
                    .get(attempt!(crate::offset(index, node.span)))
                    .ok_or_else(|| crate::internal(node.span))?;
                if definition.owner != Some(*owner) {
                    return Err(crate::invalid(
                        node.span,
                        "checked alias call has a foreign module owner",
                    ));
                }
            }
            super::Target::Builtin(_) => {}
            super::Target::SelfOwner | super::Target::SlotInvoke(_) => {
                return Err(crate::invalid(
                    node.span,
                    "checked alias call has an unsupported target",
                ));
            }
        }
        attempt!(meter.charge(1, node.span));
        if let Some(existing) = uses
            .iter_mut()
            .find(|entry| entry.spelling == alias.as_str() && entry.owner == *owner)
        {
            existing.nodes.push((node_index, *target));
        } else {
            attempt!(meter.node(node.span));
            uses.push(super::AliasUse {
                spelling: alias.clone(),
                owner: *owner,
                nodes: alloc::vec![(node_index, *target)],
            });
        }
    }
    Ok(uses)
}

fn declaration(
    tree: super::Tree,
    name: alloc::string::String,
    source_bytes: &[u8],
    signature: Option<&noble_kernel::words::Scheme>,
    signed: Option<super::SignedRelation>,
    universal: Option<(noble_kernel::universal::CheckedScheme, super::UniversalOrigin)>,
    alias_uses: alloc::vec::Vec<super::AliasUse>,
    session: &super::Session,
    meter: &mut crate::Meter,
) -> Result<(super::Named, alloc::vec::Vec<u8>), crate::Diagnostic> {
    // Inference has solved the open graph. A direct sealed named call uses its
    // already checked scheme; unsealed named dependencies still expand here.
    // No concrete sample decides generic validity.
    let identity = attempt!(super::resolution::comparison::identity(
        &tree, session, signature, meter
    ));
    let count = attempt!(crate::index(source_bytes.len(), tree.span));
    attempt!(meter.charge(count.saturating_add(4), tree.span));
    let mut addition = alloc::vec::Vec::with_capacity(source_bytes.len().saturating_add(4));
    addition.extend_from_slice(&count.to_le_bytes());
    addition.extend_from_slice(source_bytes);
    let (universal, universal_origin) = match universal {
        Some((seal, origin)) => (Some(alloc::sync::Arc::new(seal)), Some(origin)),
        None => (None, None),
    };
    Ok((
        super::Named {
            name,
            identity,
            owner: session.declared.as_ref().and_then(|context| context.owner),
            tree,
            signature: signature.cloned(),
            signed,
            universal,
            universal_origin,
            alias_uses,
            history_start: None,
        },
        addition,
    ))
}
