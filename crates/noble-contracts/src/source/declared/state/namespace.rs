mod words;

impl super::ModuleSession {
    /// The checked rank-1 interface exported by one immutable module version.
    /// No parallel Rust signature table is consulted.
    pub fn checked_signature(
        &self,
        module_name: &str,
        version: u32,
        exported_name: &str,
    ) -> Option<&noble_kernel::words::Scheme> {
        let module = self
            .modules
            .iter()
            .find(|module| module.name == module_name && module.version == version)?;
        let export = module
            .exports
            .iter()
            .find(|export| export.name == exported_name)?;
        let (_, crate::source::Target::Named(index)) = export
            .words
            .iter()
            .find(|(name, _)| name == exported_name)?
        else {
            return None;
        };
        let definition = self.source.definitions.get(usize::try_from(*index).ok()?)?;
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
        let located = if let Some((name, version)) = super::super::split_once_ascii(module, b'@') {
            let version = attempt!(version.parse::<u32>().map_err(|_| super::super::error(
                crate::source::Stage::Resolve,
                "invalid type module version"
            )));
            let mut found = None;
            let mut at = 0usize;
            while at < self.modules.len() {
                let entry = &self.modules[at];
                if entry.name == name && entry.version == version {
                    found = Some(entry);
                    break;
                }
                at += 1;
            }
            found
        } else {
            let mut found = None;
            let mut at = self.aliases.len();
            while at != 0 {
                at -= 1;
                let alias = &self.aliases[at];
                if alias.spelling == module {
                    found = self.modules.get(alias.module);
                    break;
                }
            }
            found
        };
        let mut ty = None;
        if let Some(entry) = located {
            let mut types = alloc::vec::Vec::new();
            let mut families = alloc::vec::Vec::new();
            let mut at = 0usize;
            while at < entry.exports.len() {
                let export = &entry.exports[at];
                if export.name == type_name && export.ty.is_some() {
                    ty = export.ty.clone();
                    break;
                }
                if let Some(value) = &export.ty {
                    types.push((export.name.clone(), value.clone()));
                }
                if let Some(id) = export.generic {
                    families.push((export.name.clone(), id));
                }
                at += 1;
            }
            if ty.is_none() && !families.is_empty() {
                if let Some(context) = &self.source.declared {
                    ty = super::super::types::parse_with_families(
                        type_name,
                        &types,
                        &families,
                        Some(&context.environment),
                        super::super::SPAN,
                    )
                    .ok()
                    .map(|parsed| parsed.ty);
                }
            }
        }
        ty.ok_or_else(|| {
            super::super::error(
                crate::source::Stage::Resolve,
                "unknown or unexported versioned type",
            )
        })
    }

    pub fn bindings(&self) -> &[super::super::BoundOperation] {
        &self.bindings
    }

    pub(super) fn retained(&self, limits: crate::Limits) -> Result<(), crate::source::Error> {
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
            return Err(super::super::error(
                crate::source::Stage::Check,
                "retained module byte count overflow",
            ));
        }
        let total = attempt!(module_bytes
            .checked_add(self.source.history.len())
            .ok_or_else(|| super::super::error(
                crate::source::Stage::Check,
                "retained namespace byte count overflow"
            )));
        let limit_bytes = attempt!(usize::try_from(limits.bytes).map_err(|_| {
            super::super::error(
                crate::source::Stage::Check,
                "retained byte limit exceeds the host address space",
            )
        }));
        if total > limit_bytes {
            return Err(super::super::error(
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
        let index = attempt!(self
            .modules
            .iter()
            .position(|module| module.name == name && module.version == version)
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
        self.aliases.push(super::super::Alias {
            spelling: alias,
            module: index,
        });
        self.outside_words()
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
