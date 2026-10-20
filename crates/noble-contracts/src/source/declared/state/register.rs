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
        attempt!(self.check_retained_module_bytes(request.source.len(), request.limits));
        let identity = attempt!(u64::try_from(self.modules.len())
            .ok()
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| super::super::error(
                crate::source::Stage::Check,
                "module identity limit exceeded"
            )));
        let mut collected = attempt!(collection::Collected::collect(
            core::mem::take(&mut request.members),
            request.limits
        ));
        self = attempt!(self.outside_words());
        let (next, resolved, meter) =
            attempt!(schema::resolve(self, &collected, identity, request.limits));
        let families =
            crate::source::declared::signatures::families(&next, &collected.schemas, identity);
        let mut signatures = alloc::vec::Vec::with_capacity(collected.signatures.len());
        for (name, binders, words, span) in &collected.signatures {
            let scheme = attempt!(crate::source::declared::signatures::parse(
                binders, words, &families,
            )
            .map_err(|mut error| {
                error.diagnostic.span = *span;
                error
            }));
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
        let (mut next, local_exports) = attempt!(definitions::install(
            next,
            definitions::Work {
                definitions: core::mem::take(&mut collected.definitions),
                signatures,
                exports: &collected.exports,
                local_exports,
                name: &name,
                version: request.version,
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
        next.modules.push(super::super::Module {
            name,
            version: request.version,
            identity,
            exports: local_exports,
            adapter_slot,
            source: request.source.to_vec(),
            definition_spans: core::mem::take(&mut collected.definition_spans),
            import_aliases,
            contracts: staged.contracts,
            proof_exports: staged.proof_exports,
            proofs: alloc::vec::Vec::new(),
        });
        Ok((attempt!(next.outside_words()), staged.pending))
    }
}

#[expect(
    tigerstyle::type_accumulation,
    reason = "Owner: noble-contracts; the accumulator is the checked declaration this stage builds, not a free-form type bag"
)]
impl super::ModuleSession {
    pub(super) fn check_retained_module_bytes(
        &self,
        candidate_bytes: usize,
        limits: crate::Limits,
    ) -> Result<(), crate::source::Error> {
        #[expect(
            tigerstyle::numeric_units,
            reason = "Owner: noble-contracts; the constant is a count of source elements, and the name states its unit at every use site through the surrounding type and call"
        )]
        let limit = attempt!(self.retained_source_limit(limits));
        let mut bytes = candidate_bytes;
        for module in &self.modules {
            bytes = attempt!(bytes.checked_add(module.source.len()).ok_or_else(|| {
                super::super::error(
                    crate::source::Stage::Check,
                    "retained module byte count overflow",
                )
            }));
        }
        if bytes > limit {
            return Err(super::super::error(
                crate::source::Stage::Check,
                "retained module source byte limit exceeded",
            ));
        }
        Ok(())
    }
}
