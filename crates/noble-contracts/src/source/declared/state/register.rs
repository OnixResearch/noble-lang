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

#[expect(
    tigerstyle::type_accumulation,
    reason = "Owner: noble-contracts; the session's responsibilities are split into per-stage modules, each with its own impl block."
)]
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
            || crate::scan::find_first(&self.modules, |module| {
                module.name == request.name && module.version == request.version
            })
            .is_some()
        {
            return Err(super::super::error(
                crate::source::Stage::Resolve,
                "duplicate module version or module count exceeded",
            ));
        }
        attempt!(self.check_retained(request.source.len()));
        let identity = self.next_module_identity;
        let next_module_identity =
            attempt!(identity.checked_add(1).ok_or_else(|| super::super::error(
                crate::source::Stage::Check,
                "module identity limit exceeded"
            )));
        let is_selected_locals = self
            .source
            .declared
            .as_ref()
            .is_some_and(|context| context.declared_text);
        let mut collected = attempt!(collection::Collected::collect(
            core::mem::take(&mut request.members),
            request.limits,
            is_selected_locals,
            self.source.request_work(),
        ));
        self = attempt!(self.outside_words());
        let (next, resolved, meter) =
            attempt!(schema::resolve(self, &collected, identity, request.limits));
        let signatures = attempt!(parsed_signatures(&next, &collected, identity));
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
        let import_aliases = attempt!(session_import_aliases(&next));
        next.modules.push(super::super::Module {
            name,
            version: request.version,
            identity,
            exports: local_exports,
            adapter_slot,
            source: request.source.to_vec(),
            definition_spans: core::mem::take(&mut collected.definition_spans),
            verified_signed_slots: alloc::vec::Vec::new(),
            import_aliases,
            contracts: staged.contracts,
            proof_exports: staged.proof_exports,
            proofs: alloc::vec::Vec::new(),
        });
        next.next_module_identity = next_module_identity;
        Ok((attempt!(next.outside_words()), staged.pending))
    }
}

/// The collected signatures parsed in order against the module's nominal
/// families; the first failure wins.
fn parsed_signatures(
    session: &super::ModuleSession,
    collected: &collection::Collected,
    identity: u64,
) -> Result<
    alloc::vec::Vec<(alloc::string::String, noble_kernel::words::Scheme)>,
    crate::source::Error,
> {
    let families =
        crate::source::declared::signatures::families(session, &collected.schemas, identity);
    let mut signatures = alloc::vec::Vec::with_capacity(collected.signatures.len());
    let mut signature_at = 0;
    let mut failure = None;
    while failure.is_none() && signature_at < collected.signatures.len() {
        match checked_signature(
            session,
            collected,
            &families,
            &collected.signatures[signature_at],
        ) {
            Ok(signature) => signatures.push(signature),
            Err(problem) => failure = Some(problem),
        }
        signature_at += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(signatures),
    }
}

/// Parses one collected signature against the module's nominal families,
/// refusing an exported signature that exposes a private nominal interface.
fn checked_signature(
    session: &super::ModuleSession,
    collected: &collection::Collected,
    families: &crate::source::declared::signatures::Families<'_>,
    signature: &(
        alloc::string::String,
        alloc::string::String,
        alloc::vec::Vec<alloc::string::String>,
        crate::Span,
    ),
) -> Result<(alloc::string::String, noble_kernel::words::Scheme), crate::source::Error> {
    let (name, binders, words, span) = signature;
    let scheme = attempt!(
        crate::source::declared::signatures::parse(binders, words, families).map_err(
            |mut error| {
                error.diagnostic.span = *span;
                error
            }
        )
    );
    if collected.exports.contains(name)
        && !session.source.declared.as_ref().is_some_and(|context| {
            crate::source::declared::signatures::exported(&scheme, &context.environment)
        })
    {
        return Err(super::super::error(
            crate::source::Stage::Link,
            "exported signature exposes private nominal interface",
        ));
    }
    Ok((name.clone(), scheme))
}

/// The session's import aliases, each owned by its original module's
/// identity, in alias order; the first alias without a module fails.
fn session_import_aliases(
    session: &super::ModuleSession,
) -> Result<alloc::vec::Vec<super::super::ImportAlias>, crate::source::Error> {
    let mut aliases = alloc::vec::Vec::with_capacity(session.aliases.len());
    let mut alias_at = 0;
    let mut failure = None;
    while failure.is_none() && alias_at < session.aliases.len() {
        match import_alias(session, alias_at) {
            Ok(alias) => aliases.push(alias),
            Err(problem) => failure = Some(problem),
        }
        alias_at += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(aliases),
    }
}

/// The import alias at `alias_at`, owned by its original module's identity.
fn import_alias(
    session: &super::ModuleSession,
    alias_at: usize,
) -> Result<super::super::ImportAlias, crate::source::Error> {
    let alias = &session.aliases[alias_at];
    match session.modules.get(alias.module) {
        Some(imported) => Ok(super::super::ImportAlias {
            alias: alias.spelling.clone(),
            owner: imported.identity,
        }),
        None => Err(super::super::error(
            crate::source::Stage::Check,
            "retained import alias has no original module",
        )),
    }
}

/// Re-parse the retained whole module, including the signature declaration
/// paired by exact definition name, rather than trusting a staged Scheme.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
pub(super) fn original_signature(
    session: &super::ModuleSession,
    module: &super::super::Module,
    definition: &str,
    limits: crate::Limits,
) -> Result<noble_kernel::words::Scheme, crate::source::Error> {
    let parsed = attempt!(crate::source::declared::parsing::parse(
        &module.source,
        limits,
        session.source.request_work()
    )
    .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
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
    let is_selected_locals = session
        .source
        .declared
        .as_ref()
        .is_some_and(|context| context.declared_text);
    let collected = attempt!(collection::Collected::collect(
        members,
        limits,
        is_selected_locals,
        session.source.request_work()
    ));
    let families =
        crate::source::declared::signatures::families(session, &collected.schemas, module.identity);
    let (_, binders, words, _) =
        crate::scan::find_first(&collected.signatures, |(name, _, _, _)| name == definition)
            .ok_or_else(|| {
                super::super::error(
                    crate::source::Stage::Check,
                    "signed definition lacks its original signature",
                )
            })?;
    crate::source::declared::signatures::parse(binders, words, &families)
}
