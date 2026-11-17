//! Source-owned outside alias rebinding. Module bodies and captured Programs
//! keep their old resolved owners; only current outside named definitions are
//! replayed in one unpublished successor, in dependency order.

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
)]
use alloc::{string::String, vec::Vec};

#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-contracts; the name matches the established vocabulary used by callers; renaming it would churn call sites without clarifying them."
)]
#[derive(Clone, Copy)]
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
        let after = staged
            .aliases
            .last()
            .ok_or_else(|| refusal("prepared import has no staged alias"))?;
        let Some(before) =
            crate::scan::find_first(&self.aliases, |entry| entry.spelling == after.spelling)
        else {
            return Ok(None);
        };
        if before.module == after.module {
            return Ok(None);
        }
        if before.pending || after.pending {
            return Err(refusal("unverified signed alias cannot be rebound"));
        }
        let prior_owner = self
            .modules
            .get(before.module)
            .ok_or_else(|| refusal("prior alias has no immutable module owner"))?
            .identity;
        let next_owner = staged
            .modules
            .get(after.module)
            .ok_or_else(|| refusal("replacement alias has no checked immutable module owner"))?
            .identity;
        if prior_owner == next_owner {
            return Ok(None);
        }
        Ok(Some(Rebind {
            spelling: &after.spelling,
            prior_owner,
            next_owner,
        }))
    }

    pub(super) fn import_rebind_required(&self, prepared: &super::ModulePrepared) -> bool {
        if prepared.kind != super::ModuleKind::Import {
            return false;
        }
        let Some(after) = prepared.staged.aliases.last() else {
            return true;
        };
        crate::scan::find_first(&self.aliases, |before| {
            before.spelling == after.spelling && before.module != after.module
        })
        .is_some()
    }

    #[expect(
        tigerstyle::mutating_input_in_pure,
        tigerstyle::unbounded_collection_growth,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; growth is bounded by the checked input or admission limits that drive the loop."
    )]
    pub(super) fn rebuild_import_alias(
        &self,
        staged: &mut Self,
        limits: crate::Limits,
    ) -> Result<Option<Vec<super::RebuiltImportDefinition>>, crate::source::Error> {
        let Some(preview) = self.rebind(staged)? else {
            return Ok(None);
        };
        if !has_outside_alias_use(
            &self.source.definitions,
            preview.spelling,
            preview.prior_owner,
        ) {
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
        // The current outside definition of a name is its last one.
        let mut current = alloc::vec![false; source_count];
        let mut seen = alloc::collections::BTreeSet::new();
        let mut index = source_count;
        while index > 0 {
            index -= 1;
            if self.source.definitions[index].owner.is_none() {
                current[index] = seen.insert(self.source.definitions[index].name.clone());
            }
        }
        let mut dependents = Vec::new();
        let mut meter = crate::Meter::pooled(limits, self.source.request_work());
        let mut scanned = Ok(());
        let mut definition_at = 0usize;
        while scanned.is_ok() && definition_at < source_count {
            match self.affected_by_rebind(definition_at, rebind, &current, &affected, &mut meter) {
                Ok(is_affected) => {
                    if is_affected {
                        affected[definition_at] = true;
                        if current[definition_at] {
                            dependents.push(definition_at);
                        }
                    }
                }
                Err(error) => scanned = Err(error),
            }
            definition_at += 1;
        }
        attempt!(scanned);
        let mut rebuilt = Vec::with_capacity(dependents.len());
        let mut replacements = alloc::vec![None; source_count];
        let count = u32::try_from(dependents.len().max(1))
            .map_err(|_| refusal("dependent rebuild count exceeds source limit"))?;
        // count >= 1 by construction; checked_div keeps the zero case explicit.
        let Some(work) = limits.work.checked_div(count) else {
            return Err(refusal("dependent replay work budget exhausted"));
        };
        if work == 0 {
            return Err(refusal("dependent replay work budget exhausted"));
        }
        let mut replayed = Ok(());
        let mut dependent_at = 0usize;
        while replayed.is_ok() && dependent_at < dependents.len() {
            let index = dependents[dependent_at];
            let replay_limits = crate::Limits { work, ..limits };
            match self.replay_dependent(staged, index, rebind, &replacements, replay_limits) {
                Ok((new_index, row)) => {
                    replacements[index] = Some(new_index);
                    rebuilt.push(row);
                }
                Err(error) => replayed = Err(error),
            }
            dependent_at += 1;
        }
        attempt!(replayed);
        Ok(Some(rebuilt))
    }

    /// Whether the outside definition at `index` uses the rebound alias
    /// directly or calls an already affected definition, charging it and
    /// each of its nodes; a module-owned definition is never affected.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn affected_by_rebind(
        &self,
        index: usize,
        rebind: Rebind<'_>,
        current: &[bool],
        affected: &[bool],
        meter: &mut crate::Meter,
    ) -> Result<bool, crate::source::Error> {
        let definition = &self.source.definitions[index];
        if definition.owner.is_some() {
            return Ok(false);
        }
        attempt!(meter
            .charge(1, definition.tree.span)
            .map_err(|error| super::super::diagnostic(crate::source::Stage::Check, error)));
        let is_directly = alias_used(&definition.alias_uses, rebind.spelling, rebind.prior_owner);
        let mut is_transitively = false;
        let mut scanned = Ok(());
        let mut node_at = 0usize;
        while scanned.is_ok() && node_at < definition.tree.nodes.len() {
            let node = &definition.tree.nodes[node_at];
            match Self::changed_call(node, index, current, affected, meter) {
                Ok(is_changed) => is_transitively |= is_changed,
                Err(error) => scanned = Err(error),
            }
            node_at += 1;
        }
        attempt!(scanned);
        Ok(is_directly || is_transitively)
    }

    /// Charges one node of the outside definition at `index` and reports
    /// whether it calls an affected definition; an affected call from a
    /// current definition into a shadowed one cannot be replayed.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn changed_call(
        node: &crate::source::Node,
        index: usize,
        current: &[bool],
        affected: &[bool],
        meter: &mut crate::Meter,
    ) -> Result<bool, crate::source::Error> {
        attempt!(meter
            .charge(1, node.span)
            .map_err(|error| super::super::diagnostic(crate::source::Stage::Check, error)));
        let crate::source::Kind::Call(crate::source::Target::Named(target)) = &node.kind else {
            return Ok(false);
        };
        let target = usize::try_from(*target)
            .map_err(|_| refusal("retained named dependency exceeds address space"))?;
        let is_changed = affected
            .get(target)
            .copied()
            .ok_or_else(|| refusal("retained named dependency lacks its checked source"))?;
        if is_changed && current[index] && !current[target] {
            return Err(refusal(
                "current definition depends on a shadowed alias owner that cannot be replayed",
            ));
        }
        Ok(is_changed)
    }

    /// Replays one current outside dependent into `staged` under the rebound
    /// alias, refusing any unapproved owner or interface change; returns its
    /// new staged index and rebuilt row.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn replay_dependent(
        &self,
        staged: &mut Self,
        index: usize,
        rebind: Rebind<'_>,
        replacements: &[Option<usize>],
        replay_limits: crate::Limits,
    ) -> Result<(usize, super::RebuiltImportDefinition), crate::source::Error> {
        let original = &self.source.definitions[index];
        let bytes = attempt!(self.source.definition_source(index));
        let prepared = attempt!(staged.source.prepare_with_retention(
            bytes,
            &[],
            replay_limits,
            staged.retained_source_limit,
        ));
        let replacement = prepared
            .definition
            .as_ref()
            .ok_or_else(|| refusal("retained outside source is no longer a definition"))?;
        if replacement.name != original.name
            || replacement.owner.is_some()
            || !preserves_rebind_targets(original, replacement, rebind, replacements)
        {
            return Err(refusal(
                "retained definition changed an unapproved resolved owner",
            ));
        }
        let compatibility = attempt!(crate::source::replacement::compatible(
            &staged.source,
            original,
            replacement,
            replay_limits,
        )
        .map_err(|error| super::super::diagnostic(crate::source::Stage::Check, error)));
        if compatibility != crate::source::replacement::Compatibility::Compatible {
            return Err(refusal(
                "import rebind changes a dependent interface or effect ceiling",
            ));
        }
        let new_identity = replacement.identity;
        let name = replacement.name.clone();
        let new_index = staged.source.definitions.len();
        attempt!(staged.source.commit(prepared));
        attempt!(staged.refresh_outside_words(false));
        Ok((
            new_index,
            super::RebuiltImportDefinition {
                name,
                previous_identity: original.identity,
                new_identity,
            },
        ))
    }
}

/// Whether an outside definition uses alias `spelling` bound to `owner`.
fn has_outside_alias_use(definitions: &[crate::source::Named], spelling: &str, owner: u64) -> bool {
    let mut is_used = false;
    let mut at = 0usize;
    while !is_used && at < definitions.len() {
        is_used = outside_alias_use(&definitions[at], spelling, owner);
        at += 1;
    }
    is_used
}

/// Whether one outside definition uses alias `spelling` bound to `owner`.
fn outside_alias_use(definition: &crate::source::Named, spelling: &str, owner: u64) -> bool {
    definition.owner.is_none() && alias_used(&definition.alias_uses, spelling, owner)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether an alias use spelled `spelling` is bound to `owner`.
fn alias_used(alias_uses: &[crate::source::AliasUse], spelling: &str, owner: u64) -> bool {
    crate::scan::find_first(alias_uses, |entry| {
        entry.spelling == spelling && entry.owner == owner
    })
    .is_some()
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::compound_condition,
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation; the name matches the established vocabulary used by callers; renaming it would churn call sites without clarifying them."
)]
fn preserves_rebind_targets(
    original: &crate::source::Named,
    replayed: &crate::source::Named,
    rebind: Rebind<'_>,
    replacements: &[Option<usize>],
) -> bool {
    let old = &original.tree;
    let new = &replayed.tree;
    if old.body != new.body
        || old.nodes.len() != new.nodes.len()
        || old.span != new.span
        || original.alias_uses.len() != replayed.alias_uses.len()
    {
        return false;
    }
    let mut is_preserved = true;
    let mut use_at = 0usize;
    while is_preserved && use_at < original.alias_uses.len() {
        is_preserved = alias_use_preserved(&original.alias_uses[use_at], replayed, rebind);
        use_at += 1;
    }
    let mut at = 0usize;
    while is_preserved && at < old.nodes.len() && at < new.nodes.len() {
        is_preserved = node_preserved(original, replayed, rebind, replacements, at);
        at += 1;
    }
    is_preserved
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether a replayed alias use with the rebound owner names exactly the
/// original use's node ordinals.
fn alias_use_preserved(
    before: &crate::source::AliasUse,
    replayed: &crate::source::Named,
    rebind: Rebind<'_>,
) -> bool {
    let next_owner = if before.spelling == rebind.spelling && before.owner == rebind.prior_owner {
        rebind.next_owner
    } else {
        before.owner
    };
    let Some(after) = crate::scan::find_first(&replayed.alias_uses, |entry| {
        entry.spelling == before.spelling && entry.owner == next_owner
    }) else {
        return false;
    };
    before.nodes.len() == after.nodes.len() && same_node_ordinals(&before.nodes, &after.nodes)
}

/// Whether two alias node lists name the same ordinals, pairwise in order.
fn same_node_ordinals(
    before: &[(u32, crate::source::Target)],
    after: &[(u32, crate::source::Target)],
) -> bool {
    let mut is_same = true;
    let mut at = 0usize;
    while is_same && at < before.len() && at < after.len() {
        is_same = before[at].0 == after[at].0;
        at += 1;
    }
    is_same
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply."
)]
/// Whether the replayed node at `at` keeps the original node, or changes only
/// a call to a replaced dependency or exactly the rebound alias edge.
fn node_preserved(
    original: &crate::source::Named,
    replayed: &crate::source::Named,
    rebind: Rebind<'_>,
    replacements: &[Option<usize>],
    at: usize,
) -> bool {
    let Ok(ordinal) = u32::try_from(at) else {
        return false;
    };
    let a = &original.tree.nodes[at];
    let b = &replayed.tree.nodes[at];
    if a.span != b.span {
        return false;
    }
    match (&a.kind, &b.kind) {
        (crate::source::Kind::Literal(left), crate::source::Kind::Literal(right)) => left == right,
        (crate::source::Kind::Text(left), crate::source::Kind::Text(right)) => left == right,
        (crate::source::Kind::Quotation(left), crate::source::Kind::Quotation(right)) => {
            left == right
        }
        (crate::source::Kind::Call(left), crate::source::Kind::Call(right)) => call_preserved(
            original,
            replayed,
            rebind,
            replacements,
            (ordinal, *left, *right),
        ),
        _ => false,
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether a replayed call at `ordinal` keeps the original target, or changes
/// only a call to a replaced dependency or exactly the rebound alias edge.
/// Every verdict is read before combining, so only booleans join.
fn call_preserved(
    original: &crate::source::Named,
    replayed: &crate::source::Named,
    rebind: Rebind<'_>,
    replacements: &[Option<usize>],
    call: (u32, crate::source::Target, crate::source::Target),
) -> bool {
    let (ordinal, left, right) = call;
    let is_same = left == right;
    let is_rebuilt_edge = rebuilt_edge(left, right, replacements);
    let is_prior_alias = alias_covers(
        &original.alias_uses,
        rebind.spelling,
        rebind.prior_owner,
        (ordinal, left),
    );
    let is_next_alias = alias_covers(
        &replayed.alias_uses,
        rebind.spelling,
        rebind.next_owner,
        (ordinal, right),
    );
    is_same || is_rebuilt_edge || (is_prior_alias && is_next_alias)
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::platform_dependent_cast,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply; usize is at least 32 bits on every supported target, so the u32-to-usize widening is lossless."
)]
/// Whether a replayed named call targets the replacement of the original
/// named target.
fn rebuilt_edge(
    left: crate::source::Target,
    right: crate::source::Target,
    replacements: &[Option<usize>],
) -> bool {
    let (crate::source::Target::Named(before), crate::source::Target::Named(after)) = (left, right)
    else {
        return false;
    };
    replacements.get(before as usize).and_then(|entry| *entry) == Some(after as usize)
}

/// Whether an alias use spelled `spelling` and bound to `owner` covers `node`.
fn alias_covers(
    alias_uses: &[crate::source::AliasUse],
    spelling: &str,
    owner: u64,
    node: (u32, crate::source::Target),
) -> bool {
    let mut is_covered = false;
    let mut at = 0usize;
    while !is_covered && at < alias_uses.len() {
        is_covered = alias_entry_covers(&alias_uses[at], spelling, owner, node);
        at += 1;
    }
    is_covered
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether one alias use spelled `spelling` and bound to `owner` covers `node`.
fn alias_entry_covers(
    entry: &crate::source::AliasUse,
    spelling: &str,
    owner: u64,
    node: (u32, crate::source::Target),
) -> bool {
    entry.spelling == spelling && entry.owner == owner && entry.nodes.contains(&node)
}
