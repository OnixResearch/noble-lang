//! Source-owned outside alias rebinding. Module bodies and captured Programs
//! keep their old resolved owners; only current outside named definitions are
//! replayed in one unpublished successor, in dependency order.

use alloc::{string::String, vec::Vec};

struct Rebind<'a> {
    spelling: &'a str,
    prior_owner: u64,
    next_owner: u64,
}

fn refusal(message: &str) -> crate::source::Error {
    super::super::error(crate::source::Stage::Acceptance, message)
}

impl super::ModuleSession {
    fn rebind<'a>(&self, staged: &'a Self) -> Result<Option<Rebind<'a>>, crate::source::Error> {
        let after = staged.aliases.last().ok_or_else(|| refusal(
            "prepared import has no staged alias",
        ))?;
        let Some(before) = self.aliases.iter().find(|entry| entry.spelling == after.spelling)
        else { return Ok(None); };
        if before.module == after.module { return Ok(None); }
        if before.pending || after.pending {
            return Err(refusal("unverified signed alias cannot be rebound"));
        }
        let prior_owner = self.modules.get(before.module).ok_or_else(|| refusal(
            "prior alias has no immutable module owner",
        ))?.identity;
        let next_owner = staged.modules.get(after.module).ok_or_else(|| refusal(
            "replacement alias has no checked immutable module owner",
        ))?.identity;
        if prior_owner == next_owner { return Ok(None); }
        Ok(Some(Rebind { spelling: &after.spelling, prior_owner, next_owner }))
    }

    pub(super) fn import_rebind_required(&self, prepared: &super::ModulePrepared) -> bool {
        if prepared.kind != super::ModuleKind::Import { return false; }
        let Some(after) = prepared.staged.aliases.last() else { return true; };
        self.aliases.iter().any(|before| before.spelling == after.spelling
            && before.module != after.module)
    }

    pub(super) fn rebuild_import_alias(
        &self,
        staged: &mut Self,
        limits: crate::Limits,
    ) -> Result<Option<Vec<super::RebuiltImportDefinition>>, crate::source::Error> {
        let Some(preview) = self.rebind(staged)? else { return Ok(None); };
        if !self.source.definitions.iter().any(|definition| {
            definition.owner.is_none() && definition.alias_uses.iter().any(|entry| {
                entry.spelling == preview.spelling && entry.owner == preview.prior_owner
            })
        }) {
            return Ok(Some(Vec::new()));
        }
        // The prospective alias name is borrowed from `staged`; take exactly
        // one owned copy only when replay will mutate that staged namespace.
        let spelling = String::from(preview.spelling);
        let rebind = Rebind {
            spelling: &spelling,
            prior_owner: preview.prior_owner,
            next_owner: preview.next_owner,
        };
        let source_count = self.source.definitions.len();
        let mut affected = alloc::vec![false; source_count];
        let mut current = alloc::vec![false; source_count];
        let mut seen = alloc::collections::BTreeSet::new();
        for (index, definition) in self.source.definitions.iter().enumerate().rev() {
            if definition.owner.is_none() {
                current[index] = seen.insert(definition.name.as_str());
            }
        }
        let mut dependents = Vec::new();
        let mut meter = crate::Meter::new(limits);
        for (index, definition) in self.source.definitions.iter().enumerate() {
            if definition.owner.is_some() {
                continue;
            }
            attempt!(meter.charge(1, definition.tree.span).map_err(|error|
                super::super::diagnostic(crate::source::Stage::Check, error)));
            let directly = definition.alias_uses.iter().any(|entry| {
                entry.spelling == rebind.spelling && entry.owner == rebind.prior_owner
            });
            let mut transitively = false;
            for node in &definition.tree.nodes {
                attempt!(meter.charge(1, node.span).map_err(|error|
                    super::super::diagnostic(crate::source::Stage::Check, error)));
                if let crate::source::Kind::Call(crate::source::Target::Named(target)) = &node.kind {
                    let target = usize::try_from(*target).map_err(|_| refusal(
                        "retained named dependency exceeds address space",
                    ))?;
                    let changed = affected.get(target).copied().ok_or_else(|| refusal(
                        "retained named dependency lacks its checked source",
                    ))?;
                    if changed && current[index] && !current[target] {
                        return Err(refusal(
                            "current definition depends on a shadowed alias owner that cannot be replayed",
                        ));
                    }
                    transitively |= changed;
                }
            }
            if directly || transitively {
                affected[index] = true;
                if current[index] { dependents.push(index); }
            }
        }
        let mut rebuilt = Vec::with_capacity(dependents.len());
        let mut replacements = alloc::vec![None; source_count];
        let count = u32::try_from(dependents.len().max(1)).map_err(|_| refusal(
            "dependent rebuild count exceeds source limit",
        ))?;
        let work = limits.work / count;
        if work == 0 { return Err(refusal("dependent replay work budget exhausted")); }
        for index in dependents {
            let original = &self.source.definitions[index];
            let bytes = attempt!(self.source.definition_source(index));
            let replay_limits = crate::Limits { work, ..limits };
            let prepared = attempt!(staged.source.prepare(bytes, &[], replay_limits));
            let replacement = prepared.definition.as_ref().ok_or_else(|| refusal(
                "retained outside source is no longer a definition",
            ))?;
            if replacement.name != original.name || replacement.owner.is_some()
                || !preserves_rebind_targets(original, replacement, &rebind, &replacements) {
                return Err(refusal("retained definition changed an unapproved resolved owner"));
            }
            let compatible = attempt!(crate::source::replacement::compatible(
                &staged.source, original, replacement, replay_limits,
            ).map_err(|error| super::super::diagnostic(crate::source::Stage::Check, error)));
            if !compatible {
                return Err(refusal("import rebind changes a dependent interface or effect ceiling"));
            }
            let new_identity = replacement.identity;
            let name = replacement.name.clone();
            let new_index = staged.source.definitions.len();
            attempt!(staged.source.commit(prepared));
            attempt!(staged.refresh_outside_words(false));
            replacements[index] = Some(new_index);
            rebuilt.push(super::RebuiltImportDefinition {
                name,
                previous_identity: original.identity,
                new_identity,
            });
        }
        Ok(Some(rebuilt))
    }
}

fn preserves_rebind_targets(
    original: &crate::source::Named,
    replayed: &crate::source::Named,
    rebind: &Rebind<'_>,
    replacements: &[Option<usize>],
) -> bool {
    let old = &original.tree;
    let new = &replayed.tree;
    if old.body != new.body || old.nodes.len() != new.nodes.len()
        || old.span != new.span || original.alias_uses.len() != replayed.alias_uses.len() {
        return false;
    }
    for before in &original.alias_uses {
        let next_owner = if before.spelling == rebind.spelling
            && before.owner == rebind.prior_owner { rebind.next_owner } else { before.owner };
        let Some(after) = replayed.alias_uses.iter().find(|entry|
            entry.spelling == before.spelling && entry.owner == next_owner
        ) else { return false; };
        if before.nodes.len() != after.nodes.len() || before.nodes.iter().zip(&after.nodes)
            .any(|(a, b)| a.0 != b.0) {
            return false;
        }
    }
    old.nodes.iter().zip(&new.nodes).enumerate().all(|(ordinal, (a, b))| {
        let Ok(ordinal) = u32::try_from(ordinal) else { return false; };
        if a.span != b.span { return false; }
        match (&a.kind, &b.kind) {
            (crate::source::Kind::Literal(left), crate::source::Kind::Literal(right)) => left == right,
            (crate::source::Kind::Text(left), crate::source::Kind::Text(right)) => left == right,
            (crate::source::Kind::Quotation(left), crate::source::Kind::Quotation(right)) => left == right,
            (crate::source::Kind::Call(left), crate::source::Kind::Call(right)) => {
                if left == right { return true; }
                let rebuilt_edge = match (left, right) {
                    (crate::source::Target::Named(before), crate::source::Target::Named(after)) =>
                        replacements.get(*before as usize).and_then(|entry| *entry)
                            == Some(*after as usize),
                    _ => false,
                };
                let alias_edge = original.alias_uses.iter().any(|entry| {
                    entry.spelling == rebind.spelling && entry.owner == rebind.prior_owner
                        && entry.nodes.contains(&(ordinal, *left))
                }) && replayed.alias_uses.iter().any(|entry| {
                    entry.spelling == rebind.spelling && entry.owner == rebind.next_owner
                        && entry.nodes.contains(&(ordinal, *right))
                });
                rebuilt_edge || alias_edge
            }
            _ => false,
        }
    })
}
