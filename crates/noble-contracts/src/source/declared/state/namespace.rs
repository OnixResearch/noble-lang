mod words;

impl super::ModuleSession {
    #[expect(
        tigerstyle::ambiguous_params,
        reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
    )]
    /// The checked rank-1 interface exported by one immutable module version.
    /// No parallel Rust signature table is consulted.
    pub fn checked_signature(
        &self,
        module_name: &str,
        version: u32,
        exported_name: &str,
    ) -> Option<&noble_kernel::words::Scheme> {
        let module = crate::scan::find_first(&self.modules, |module| {
            module.name == module_name && module.version == version
        })?;
        let export =
            crate::scan::find_first(&module.exports, |export| export.name == exported_name)?;
        let (_, crate::source::Target::Named(index)) =
            crate::scan::find_first(&export.words, |(name, _)| name == exported_name)?
        else {
            return None;
        };
        let definition = self.source.definitions.get(usize::try_from(*index).ok()?)?;
        if definition.signed.is_some() && !module.verified_signed_slots.contains(index) {
            return None;
        }
        (definition.owner == Some(module.identity))
            .then_some(definition.signature.as_ref())
            .flatten()
    }

    pub fn resolve_type(
        &self,
        spelling: &str,
    ) -> Result<noble_kernel::types::Ty, crate::source::Error> {
        let (module, type_name) = attempt!(super::super::split_once_ascii(spelling, b'.')
            .ok_or_else(|| super::super::error(
                crate::source::Stage::Resolve,
                "expected versioned exported type"
            )));
        let located = attempt!(self.type_module(module));
        let ty = attempt!(self.exported_type(located, type_name));
        ty.ok_or_else(|| {
            super::super::error(
                crate::source::Stage::Resolve,
                "unknown or unexported versioned type",
            )
        })
    }

    /// The concrete type a located module exports under `type_name`, else a
    /// checked instance of one of its generic families.
    fn exported_type(
        &self,
        located: Option<&super::super::Module>,
        type_name: &str,
    ) -> Result<Option<noble_kernel::types::Ty>, crate::source::Error> {
        let Some(entry) = located else {
            return Ok(None);
        };
        let (exported, (types, families)) = attempt!(scan_type_exports(entry, type_name));
        if exported.is_some() || families.is_empty() {
            return Ok(exported);
        }
        Ok(self.family_type(type_name, &types, &families))
    }

    /// The generic family instance `type_name` spells, checked in the declared
    /// environment; no declared context resolves none.
    fn family_type(
        &self,
        type_name: &str,
        types: &[(alloc::string::String, noble_kernel::types::Ty)],
        families: &[(alloc::string::String, noble_kernel::types::NominalTypeId)],
    ) -> Option<noble_kernel::types::Ty> {
        let context = self.source.declared.as_ref()?;
        super::super::types::parse_with_families(
            type_name,
            types,
            families,
            Some(&context.environment),
            super::super::SPAN,
        )
        .ok()
        .map(|parsed| parsed.ty)
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
    )]
    /// The module a type spelling's module part names: `name@version`, else
    /// the latest checked alias spelled that way.
    fn type_module(
        &self,
        module: &str,
    ) -> Result<Option<&super::super::Module>, crate::source::Error> {
        let Some((name, version)) = super::super::split_once_ascii(module, b'@') else {
            return Ok(self.aliased_module(module));
        };
        let version = attempt!(super::super::canonical_version(version).ok_or_else(|| {
            super::super::error(crate::source::Stage::Resolve, "invalid type module version")
        }));
        Ok(crate::scan::find_first(&self.modules, |entry| {
            entry.name == name && entry.version == version
        }))
    }

    /// The module of the latest checked alias spelled `spelling`.
    fn aliased_module(&self, spelling: &str) -> Option<&super::super::Module> {
        let mut found_at = None;
        let mut at = self.aliases.len();
        while found_at.is_none() && at != 0 {
            at -= 1;
            if checked_alias_spelled(&self.aliases[at], spelling) {
                found_at = Some(at);
            }
        }
        match found_at {
            Some(alias_at) => self.modules.get(self.aliases[alias_at].module),
            None => None,
        }
    }

    pub fn bindings(&self) -> &[super::super::BoundOperation] {
        &self.bindings
    }

    pub(super) fn check_retained(&self, extra: usize) -> Result<(), crate::source::Error> {
        let mut module_bytes = 0usize;
        let mut index = 0;
        let mut has_overflow = false;
        while index < self.modules.len() {
            match module_bytes.checked_add(self.modules[index].source.len()) {
                Some(total) => module_bytes = total,
                None => {
                    has_overflow = true;
                    break;
                }
            }
            index += 1;
        }
        if has_overflow {
            return Err(super::super::exhausted(
                crate::source::Stage::Check,
                "retained module byte count overflow",
            ));
        }
        let retained = attempt!(module_bytes
            .checked_add(self.source.history.len())
            .ok_or_else(|| super::super::exhausted(
                crate::source::Stage::Check,
                "retained namespace byte count overflow"
            )));
        let total = attempt!(retained
            .checked_add(extra)
            .ok_or_else(|| super::super::exhausted(
                crate::source::Stage::Check,
                "retained namespace byte count overflow",
            )));
        let limit_bytes = attempt!(usize::try_from(self.retained_source_limit).map_err(|_| {
            super::super::error(
                crate::source::Stage::Check,
                "retained byte limit exceeds the host address space",
            )
        }));
        if total > limit_bytes {
            return Err(super::super::exhausted(
                crate::source::Stage::Check,
                "retained module and definition byte limit exceeded",
            ));
        }
        Ok(())
    }

    pub(super) fn reserved_word(&self, name: &str) -> bool {
        let mut alias_at = 0usize;
        let mut is_found = false;
        while alias_at < self.aliases.len() && !is_found {
            is_found = self.aliased_word_at(alias_at, name);
            alias_at += 1;
        }
        let mut module_at = 0usize;
        while module_at < self.modules.len() && !is_found {
            is_found = super::super::links::module_word(&self.modules[module_at], name);
            module_at += 1;
        }
        is_found
    }

    fn aliased_word_at(&self, alias_at: usize, name: &str) -> bool {
        let alias = &self.aliases[alias_at];
        if let Some(module) = self.modules.get(alias.module) {
            super::super::links::alias_word(module, alias.spelling.as_bytes(), name)
        } else {
            false
        }
    }

    pub(super) fn import(
        mut self,
        name: &str,
        version: u32,
        alias: alloc::string::String,
    ) -> Result<Self, crate::source::Error> {
        let index = attempt!(super::position_first(&self.modules, |module| {
            module.name == name && module.version == version
        })
        .ok_or_else(|| super::super::error(
            crate::source::Stage::Resolve,
            "missing immutable module version"
        )));
        if self.alias_conflicts(index, alias.as_str()) {
            return Err(super::super::error(
                crate::source::Stage::Resolve,
                "import alias conflicts with an existing definition",
            ));
        }
        self.aliases.retain(|entry| entry.spelling != alias);
        if self.aliases.len() >= super::super::MODULE_CAP {
            return Err(super::super::error(
                crate::source::Stage::Check,
                "module alias count exceeded",
            ));
        }
        let is_pending = self.module_has_unverified_signed_owner(index);
        self.aliases.push(super::super::Alias {
            spelling: alias,
            module: index,
            pending: is_pending,
        });
        self.outside_words()
    }

    fn module_has_unverified_signed_owner(&self, module_index: usize) -> bool {
        let Some(module) = self.modules.get(module_index) else {
            return true;
        };
        let mut is_unverified = false;
        let mut index = 0usize;
        while !is_unverified && index < self.source.definitions.len() {
            is_unverified = unverified_signed_owner(module, &self.source.definitions[index], index);
            index += 1;
        }
        is_unverified
    }

    /// Pending aliases remain unresolvable until each signed owner in their
    /// module has a concrete kernel-checked and original-source-paired body.
    pub(super) fn record_signed_certified(
        mut self,
        submission: &noble_kernel::execution::Submission,
    ) -> Result<Self, crate::source::Error> {
        let mut certified = Ok(());
        let mut definition_at = 0usize;
        while certified.is_ok() && definition_at < submission.definitions.len() {
            certified = self.certify_signed_owner(&submission.definitions[definition_at]);
            definition_at += 1;
        }
        attempt!(certified);
        let mut at = 0usize;
        while at < self.aliases.len() {
            if self.aliases[at].pending
                && !self.module_has_unverified_signed_owner(self.aliases[at].module)
            {
                self.aliases[at].pending = false;
            }
            at += 1;
        }
        self.outside_words()
    }

    /// Marks the source slot of one certified signed owner definition as
    /// verified in its module; any other definition is left alone.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    #[expect(
        tigerstyle::sentinel_fallback,
        reason = "Owner: noble-contracts; an unrepresentable slot maps to usize::MAX, which get() reports as absent, so the lookup fails closed with its diagnostic."
    )]
    fn certify_signed_owner(
        &mut self,
        definition: &noble_kernel::execution::Definition,
    ) -> Result<(), crate::source::Error> {
        if !super::has_signed_self(&definition.body.candidate.nodes) {
            return Ok(());
        }
        let slot = definition.owner_slot.ok_or_else(|| {
            super::super::error(
                crate::source::Stage::Check,
                "certified signed owner has no source slot",
            )
        })?;
        let owner = self
            .source
            .definitions
            .get(usize::try_from(slot).unwrap_or(usize::MAX))
            .ok_or_else(|| {
                super::super::error(
                    crate::source::Stage::Check,
                    "certified signed owner source slot is absent",
                )
            })?
            .owner;
        let module =
            crate::scan::find_first_mut(&mut self.modules, |module| owner == Some(module.identity))
                .ok_or_else(|| {
                    super::super::error(
                        crate::source::Stage::Check,
                        "certified signed owner module is absent",
                    )
                })?;
        if !module.verified_signed_slots.contains(&slot) {
            module.verified_signed_slots.push(slot);
        }
        Ok(())
    }

    fn alias_conflicts(&self, index: usize, alias: &str) -> bool {
        let mut at = 0usize;
        let mut is_conflicting = false;
        while at < self.source.definitions.len() && !is_conflicting {
            is_conflicting = super::super::links::alias_conflicts_definition(
                &self.modules[index],
                alias,
                &self.source.definitions[at],
            );
            at += 1;
        }
        is_conflicting
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether a checked, non-pending alias is spelled `spelling`.
fn checked_alias_spelled(alias: &super::super::Alias, spelling: &str) -> bool {
    !alias.pending && alias.spelling == spelling
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether an export is the concrete type named `type_name`.
fn is_type_export(export: &super::super::Export, type_name: &str) -> bool {
    export.name == type_name && export.ty.is_some()
}

/// Concrete and generic family type exports of one module, by name.
type TypeRows = (
    alloc::vec::Vec<(alloc::string::String, noble_kernel::types::Ty)>,
    alloc::vec::Vec<(alloc::string::String, noble_kernel::types::NominalTypeId)>,
);

/// Scans a module's exports in order: the concrete type named `type_name`
/// once it is reached, and every concrete and generic family row before it.
fn scan_type_exports(
    entry: &super::super::Module,
    type_name: &str,
) -> Result<(Option<noble_kernel::types::Ty>, TypeRows), crate::source::Error> {
    let mut ty = None;
    let mut rows = Ok((alloc::vec::Vec::new(), alloc::vec::Vec::new()));
    let mut at = 0usize;
    while ty.is_none() && rows.is_ok() && at < entry.exports.len() {
        let export = &entry.exports[at];
        if is_type_export(export, type_name) {
            ty = export.ty.clone();
        } else {
            rows = match rows {
                Ok(scanned) => record_type_export(export, entry.exports.len(), scanned),
                Err(problem) => Err(problem),
            };
        }
        at += 1;
    }
    let rows = attempt!(rows);
    Ok((ty, rows))
}

/// Appends one export's concrete type and generic family rows; a row past
/// `bound`, which one row per scanned export never reaches, is refused.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn record_type_export(
    export: &super::super::Export,
    bound: usize,
    rows: TypeRows,
) -> Result<TypeRows, crate::source::Error> {
    let (mut types, mut families) = rows;
    if let Some(value) = &export.ty {
        if types.len() >= bound {
            return Err(super::super::error(
                crate::source::Stage::Resolve,
                "type export scan exceeds module exports",
            ));
        }
        types.push((export.name.clone(), value.clone()));
    }
    if let Some(id) = export.generic {
        if families.len() >= bound {
            return Err(super::super::error(
                crate::source::Stage::Resolve,
                "type export scan exceeds module exports",
            ));
        }
        families.push((export.name.clone(), id));
    }
    Ok((types, families))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether the definition at `index` is a signed owner in `module` whose
/// slot the module has not verified.
fn unverified_signed_owner(
    module: &super::super::Module,
    definition: &crate::source::Named,
    index: usize,
) -> bool {
    definition.owner == Some(module.identity)
        && definition.signed.is_some()
        && u32::try_from(index)
            .ok()
            .is_none_or(|slot| !module.verified_signed_slots.contains(&slot))
}
