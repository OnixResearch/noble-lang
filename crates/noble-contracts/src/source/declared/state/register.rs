mod adapter;
mod collection;
mod definitions;
mod exports;
mod logical;
mod publication;
mod schema;

pub(super) struct Registration<'a> {
    pub name: alloc::string::String,
    pub version: u32,
    pub members: alloc::vec::Vec<crate::source::declared::parsing::Member>,
    pub source: &'a [u8],
    pub inputs: &'a [noble_kernel::types::Ty],
    pub limits: crate::Limits,
}

impl super::ModuleSession {
    pub(super) fn register(
        mut self,
        mut request: Registration<'_>,
    ) -> Result<(Self, Option<crate::intrinsic::ProofBatch>), crate::source::Error> {
        if request.name == "result"
            && request.version == 1
            && request.source != crate::source::RESULT_LIBRARY_SOURCE
        {
            return Err(super::super::error(
                crate::source::Stage::Link,
                "result@1 requires the exact checked library source",
            ));
        }
        if !request.inputs.is_empty() {
            return Err(super::super::error(
                crate::source::Stage::Check,
                "module registration does not consume runtime input",
            ));
        }
        if self.modules.len() >= super::super::MODULE_CAP
            || self
                .modules
                .iter()
                .any(|module| module.name == request.name && module.version == request.version)
        {
            return Err(super::super::error(
                crate::source::Stage::Resolve,
                "duplicate module version or module count exceeded",
            ));
        }
        attempt!(self.check_retained(request.source.len()));
        let original_source = alloc::sync::Arc::<[u8]>::from(request.source);
        let identity = self.next_module_identity;
        let next_module_identity = attempt!(identity.checked_add(1)
            .ok_or_else(|| super::super::error(
                crate::source::Stage::Check,
                "module identity limit exceeded"
            )));
        let selected_locals = self
            .source
            .declared
            .as_ref()
            .is_some_and(|context| context.declared_text);
        let mut collected = attempt!(collection::Collected::collect(
            core::mem::take(&mut request.members),
            request.limits,
            selected_locals,
        ));
        self = attempt!(self.outside_words());
        let (next, resolved, meter) =
            attempt!(schema::resolve(self, &collected, identity, request.limits));
        let families =
            crate::source::declared::signatures::families(&next, &collected.schemas, identity);
        let fixed = crate::source::declared::signatures::fixed_nominals(
            &next,
            &collected.schemas,
            identity,
        );
        let mut signatures = alloc::vec::Vec::with_capacity(collected.signatures.len());
        for (name, binders, words) in &collected.signatures {
            let scheme = attempt!(crate::source::declared::signatures::parse(
                binders, words, &families, &fixed,
            ));
            if collected.exports.contains(name)
                && !next.source.declared.as_ref().is_some_and(|context| {
                    crate::source::declared::signatures::exported(&scheme, &context.environment)
                })
            {
                return Err(super::super::error(
                    crate::source::Stage::Link,
                    "exported signature exposes private nominal interface",
                ));
            }
            signatures.push((name.clone(), scheme));
        }
        let exports::Installed {
            session: next,
            local_exports,
            words,
        } = attempt!(exports::install(next, &collected, &resolved));
        let (next, name, adapter_slot) = attempt!(adapter::install(
            next,
            (request.name, request.version),
            identity,
            core::mem::take(&mut collected.requirements),
            words
        ));
        let mut meter = meter;
        let source_stamp_prefix = attempt!(crate::source::preparation::module_stamp_prefix(
            &original_source, super::super::SPAN, &mut meter,
        ).map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
        let mut needs_name_lookup = false;
        if selected_locals {
            'words: for (_, _, tree) in &collected.definitions {
                for node in &tree.nodes {
                    attempt!(meter.charge(1, super::super::SPAN).map_err(|problem|
                        super::super::diagnostic(crate::source::Stage::Check, problem)));
                    if matches!(node.kind, crate::source::Kind::Word(_)) {
                        needs_name_lookup = true;
                        break 'words;
                    }
                }
            }
        }
        let name_index = if needs_name_lookup {
            let Some(context) = next.source.declared.as_ref() else {
                return Err(super::super::error(
                    crate::source::Stage::Check, "indexed source namespace lacks its context",
                ));
            };
            let mut longest_incoming = 0usize;
            for (name, _, _) in &collected.definitions {
                attempt!(meter.charge(1, super::super::SPAN).map_err(|problem|
                    super::super::diagnostic(crate::source::Stage::Check, problem)));
                longest_incoming = longest_incoming.max(name.len());
            }
            let fits = attempt!(crate::source::preparation::name_index::NameIndex::fits_original_work(
                &context.words, collected.definitions.len(), longest_incoming,
                super::super::SPAN, &mut meter,
            ).map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
            if fits {
                Some(attempt!(crate::source::preparation::name_index::NameIndex::build(
                    &context.words, context.owner, next.source.generation,
                    collected.definitions.len(), super::super::SPAN, &mut meter,
                ).map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem))))
            } else {
                // Decide before any index allocation or sorting: the original
                // fully metered reverse lookup remains the whole transaction.
                None
            }
        } else {
            None
        };
        let (mut next, local_exports) = attempt!(definitions::install(
            next,
            definitions::Work {
                definitions: core::mem::take(&mut collected.definitions),
                signatures,
                exports: &collected.exports,
                local_exports,
                name: &name,
                version: request.version,
                original_source: &original_source,
                source_stamp_prefix,
                name_index,
                definition_spans: &collected.definition_spans,
                limits: request.limits,
                meter,
            }
        ));
        let staged = attempt!(logical::stage(logical::Request {
            session: &next,
            collected: &collected,
            name: &name,
            version: request.version,
            identity,
            source: request.source,
            generation: next.generation,
            limits: request.limits,
        }));
        let mut import_aliases = alloc::vec::Vec::with_capacity(next.aliases.len());
        for alias in &next.aliases {
            let Some(imported) = next.modules.get(alias.module) else {
                return Err(super::super::error(
                    crate::source::Stage::Check,
                    "retained import alias has no original module",
                ));
            };
            import_aliases.push(super::super::ImportAlias {
                alias: alias.spelling.clone(),
                owner: imported.identity,
            });
        }
        let mut verified_signed_slots = alloc::vec::Vec::new();
        for (source_slot, definition) in next.source.definitions.iter().enumerate() {
            if definition.owner == Some(identity)
                && definition.signed.is_some()
                && definition.universal.is_some()
                && definition.universal_origin.as_ref().is_some_and(|origin| {
                    origin.module_name == name
                        && origin.module_version == request.version
                        && collected.definition_spans.get(origin.ordinal as usize)
                            .is_some_and(|source| {
                                source.name == definition.name
                                    && source.span == origin.source_span
                            })
                })
            {
                verified_signed_slots.push(attempt!(u32::try_from(source_slot).map_err(|_|
                    super::super::error(crate::source::Stage::Check,
                        "universal source owner slot overflow"))));
            }
        }
        next.modules.push(super::super::Module {
            name,
            version: request.version,
            identity,
            exports: local_exports,
            adapter_slot,
            source: original_source,
            source_stamp_prefix,
            definition_spans: core::mem::take(&mut collected.definition_spans),
            verified_signed_slots,
            import_aliases,
            contracts: staged.contracts,
            proof_exports: staged.proof_exports,
            proofs: alloc::vec::Vec::new(),
        });
        next.next_module_identity = next_module_identity;
        Ok((attempt!(next.outside_words()), staged.pending))
    }
}

/// Re-parse the retained whole module, including the signature declaration
/// paired by exact definition name, rather than trusting a staged Scheme.
pub(super) fn original_signature(
    session: &super::ModuleSession,
    module: &super::super::Module,
    definition: &str,
    limits: crate::Limits,
) -> Result<noble_kernel::words::Scheme, crate::source::Error> {
    let parsed = attempt!(
        crate::source::declared::parsing::parse(&module.source, limits)
            .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem))
    );
    let crate::source::declared::parsing::ParsedUnit::Module {
        name,
        version,
        members,
    } = parsed
    else {
        return Err(super::super::error(
            crate::source::Stage::Check,
            "signed owner original source is not a module",
        ));
    };
    if name != module.name || version != module.version {
        return Err(super::super::error(
            crate::source::Stage::Check,
            "signed owner original module identity differs",
        ));
    }
    let selected_locals = session
        .source
        .declared
        .as_ref()
        .is_some_and(|context| context.declared_text);
    let collected = attempt!(collection::Collected::collect(
        members,
        limits,
        selected_locals
    ));
    let families =
        crate::source::declared::signatures::families(session, &collected.schemas, module.identity);
    let fixed = crate::source::declared::signatures::fixed_nominals(
        session,
        &collected.schemas,
        module.identity,
    );
    let (_, binders, words) = collected
        .signatures
        .iter()
        .find(|(name, _, _)| name == definition)
        .ok_or_else(|| {
            super::super::error(
                crate::source::Stage::Check,
                "signed definition lacks its original signature",
            )
        })?;
    crate::source::declared::signatures::parse(binders, words, &families, &fixed)
}
