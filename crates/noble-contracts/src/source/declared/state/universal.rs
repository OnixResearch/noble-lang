//! Source-owned universal closure. Neither a raw kernel seal nor a caller's
//! numeric code entry can construct this private, rederived compiler ingress.

use alloc::{vec, vec::Vec};
use noble_kernel::{
    contracts::{Behavior, Definition, Env},
    execution::Submission,
    shapes::Pattern,
    types::EffSet,
    universal::{CheckedInstantiation, CheckedNodeKind, CheckedScheme, Dependency, SourceStamp,
        SymbolicNode},
    untrusted::{Expected, Limits, Node, NodeId, Request},
    words::{Binding, Inst},
};

use super::{CheckedProgramOwner, DeclaredTextSourceOwner, ModulePrepared};

fn refused(reason: &str) -> crate::source::Error {
    super::super::error(crate::source::Stage::Check, reason)
}

/// Exact absolute whole-module lexical byte spans bound to checked symbolic
/// draft IDs at original source admission. Quotations and signed self
/// references retain their own IDs.
pub struct UniversalLexicalBody {
    spans: Vec<crate::Span>,
}

impl UniversalLexicalBody {
    pub fn node_span(&self, id: NodeId) -> Option<crate::Span> {
        self.spans.get(usize::try_from(id.0).ok()?).copied()
    }
    pub fn node_count(&self) -> usize { self.spans.len() }
}

/// Original immutable module owner; source bytes are borrowed from the staged
/// checked namespace, never supplied by a caller's Submission or stamp.
pub struct UniversalCheckedOwner<'a> {
    program_owner: CheckedProgramOwner,
    source: &'a alloc::sync::Arc<[u8]>,
    seal: &'a CheckedScheme,
    lexical: UniversalLexicalBody,
    dependencies: Vec<usize>,
}

impl UniversalCheckedOwner<'_> {
    pub const fn program_owner(&self) -> &CheckedProgramOwner { &self.program_owner }
    pub fn original_source(&self) -> &[u8] { self.source.as_ref() }
    pub fn original_source_arc(&self) -> alloc::sync::Arc<[u8]> { self.source.clone() }
    pub const fn seal(&self) -> &CheckedScheme { self.seal }
    pub fn stamp(&self) -> SourceStamp { self.seal.stamp() }
    pub const fn lexical_body(&self) -> &UniversalLexicalBody { &self.lexical }
    /// Earlier owner indices, in immutable source-order topological order.
    pub fn dependencies(&self) -> &[usize] { &self.dependencies }
}

/// One exact concrete instantiation. Each named lexical edge points at its
/// own checked target use; quoted self points back to this SAME use only.
pub struct UniversalCheckedUse {
    owner_index: usize,
    owner_inst: Inst,
    checked: CheckedInstantiation,
    targets: Vec<(NodeId, usize)>,
}

impl UniversalCheckedUse {
    pub const fn owner_index(&self) -> usize { self.owner_index }
    pub const fn owner_inst(&self) -> &Inst { &self.owner_inst }
    pub const fn checked(&self) -> &CheckedInstantiation { &self.checked }
    pub fn target_uses(&self) -> &[(NodeId, usize)] { &self.targets }
}

/// A source-private, finite checked DAG and rederived selected expression.
/// CodeSpan/table addresses do not occur here: the compiler resolves them
/// against its OWN previously accepted installed owner code registry.
pub struct UniversalSourceClosure<'a> {
    owners: Vec<UniversalCheckedOwner<'a>>,
    uses: Vec<UniversalCheckedUse>,
    selected_submission: Submission,
    selected_use: usize,
    source_generation: u64,
}

impl UniversalSourceClosure<'_> {
    pub fn owners(&self) -> &[UniversalCheckedOwner<'_>] { &self.owners }
    pub fn uses(&self) -> &[UniversalCheckedUse] { &self.uses }
    pub const fn selected_submission(&self) -> &Submission { &self.selected_submission }
    pub const fn selected_use(&self) -> usize { self.selected_use }
    pub const fn source_generation(&self) -> u64 { self.source_generation }
}

fn same_stack_inst(left: &Inst, right: &Inst) -> bool {
    matches!((left.bindings.as_slice(), right.bindings.as_slice()),
        ([Binding::Stack(a)], [Binding::Stack(b)]) if a == b)
}

fn witness_size(inst: &Inst) -> Option<usize> {
    let [Binding::Stack(values)] = inst.bindings.as_slice() else { return None; };
    values.iter().try_fold(1usize, |count, ty|
        count.checked_add(ty.size()? as usize))
}

fn charge(meter: &mut crate::Meter, span: crate::Span, count: usize)
    -> Result<(), crate::source::Error>
{
    let count = crate::index(count, span)
        .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem))?;
    meter.charge(count, span)
        .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem))
}

fn charge_pattern(
    pattern: &Pattern, meter: &mut crate::Meter, depth: u32,
) -> Result<(), crate::source::Error> {
    let span = crate::source::declared::SPAN;
    attempt!(meter.depth(depth, span)
        .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
    attempt!(charge(meter, span, 1));
    let next = depth.checked_add(1)
        .ok_or_else(|| refused("universal environment pattern depth overflow"))?;
    match pattern {
        Pattern::Pair(left, right) | Pattern::Sum(left, right) => {
            attempt!(charge_pattern(left, meter, next));
            attempt!(charge_pattern(right, meter, next));
        }
        Pattern::List(item) => { attempt!(charge_pattern(item, meter, next)); }
        Pattern::Program(input, output, effects) => {
            attempt!(charge(meter, span, effects.as_slice().len()));
            for item in input.iter().chain(output.iter()) {
                attempt!(charge_pattern(item, meter, next));
            }
        }
        Pattern::GenericNominal(_, args, _) => {
            for item in args.iter() { attempt!(charge_pattern(item, meter, next)); }
        }
        Pattern::Nominal(_, shape) => {
            let size = match shape.as_ref() {
                noble_kernel::types::NominalShape::Opaque(ty) => ty.size(),
                noble_kernel::types::NominalShape::Variant(left, right) =>
                    left.size().and_then(|count| count.checked_add(right.size()?)),
            }.ok_or_else(|| refused("universal nominal shape has invalid size"))?;
            attempt!(charge(meter, span, size as usize));
        }
        _ => {}
    }
    Ok(())
}

fn charge_scheme(
    scheme: &noble_kernel::words::Scheme, meter: &mut crate::Meter,
) -> Result<(), crate::source::Error> {
    attempt!(charge(meter, crate::source::declared::SPAN,
        scheme.var_kinds.len().saturating_add(scheme.effects.len())));
    for pattern in scheme.stack_in.iter().chain(scheme.stack_out.iter()) {
        attempt!(charge_pattern(pattern, meter, 0));
    }
    Ok(())
}

fn source_slot_by_definition(
    named: &[crate::source::Named],
    target: Definition,
    meter: &mut crate::Meter,
    span: crate::Span,
) -> Result<Option<usize>, crate::source::Error> {
    for (slot, candidate) in named.iter().enumerate() {
        attempt!(charge(meter, span, 1));
        if candidate.universal.as_ref().is_some_and(|checked| checked.owner() == target) {
            return Ok(Some(slot));
        }
    }
    Ok(None)
}

fn owner_by_definition(
    owners: &[UniversalCheckedOwner<'_>],
    target: Definition,
    meter: &mut crate::Meter,
) -> Result<Option<usize>, crate::source::Error> {
    for (index, owner) in owners.iter().enumerate() {
        attempt!(charge(meter, crate::source::declared::SPAN, 1));
        if owner.seal.owner() == target { return Ok(Some(index)); }
    }
    Ok(None)
}

fn limits(limits: crate::Limits, work: u32) -> Limits {
    Limits {
        bytes: limits.bytes,
        nodes: limits.nodes.min(4_096),
        depth: limits.depth,
        type_size: crate::syntax::TYPE_CAP,
        stack_height: crate::inference::STACK_CAP,
        work,
        diagnostics: 8,
    }
}

impl<'a> DeclaredTextSourceOwner<'a> {
    /// Choose the universal compiler before ordinary concrete lowering can
    /// duplicate a sealed owner's code. This is only a routing hint from the
    /// private prepared source; rederive_universal authenticates it again.
    pub fn selected_universal_intent(&self) -> bool {
        let prepared = self.prepared;
        if prepared.kind != super::ModuleKind::Expression
            || prepared.signed_expression.is_empty()
            || prepared.pending.is_some()
            || !prepared.staged.bindings.is_empty()
            || !prepared.staged.source.declared.as_ref()
                .is_some_and(|context| context.declared_text)
        {
            return false;
        }
        let Some(submission) = prepared.submission.as_ref() else { return false; };
        let [root] = submission.body.candidate.body.as_slice() else { return false; };
        let Some(Node::Invocation { def, .. }) =
            submission.body.candidate.nodes.get(root.0 as usize)
        else { return false; };
        prepared.staged.source.definitions.iter()
            .any(|source| source.universal.as_ref()
                .is_some_and(|seal| seal.owner() == *def))
    }

    /// Restrict the reusable ABI to the selected signed declared/Text profile.
    /// The older unsigned declared/Text compiler retains its original route.
    pub fn selected_signed_universal_intent(&self) -> bool {
        self.prepared.staged.source.declared.as_ref()
            .is_some_and(|context| context.signed_self)
            && self.selected_universal_intent()
    }

    /// Recheck immutable WHOLE modules, source-bound lexical nodes and current
    /// Env; instantiate every finite `(owner, S)` use independently. No
    /// concrete sample substitutes for registration's rigid-S proof.
    pub fn rederive_universal(&self) -> Result<UniversalSourceClosure<'a>, crate::source::Error> {
        let prepared = self.prepared;
        if prepared.kind != super::ModuleKind::Expression
            || prepared.signed_expression.is_empty()
            || prepared.pending.is_some()
            || !prepared.staged.bindings.is_empty()
            || !prepared.staged.source.declared.as_ref()
                .is_some_and(|context| context.declared_text)
        {
            return Err(refused("universal source requires selected pure declared Text expression"));
        }
        let original = prepared.submission.as_ref()
            .ok_or_else(|| refused("universal selected expression has no checked submission"))?;
        let replay = attempt!(prepared.staged.snapshot().pending_signed_expression_words());
        let mut rebuilt = attempt!(replay.source.prepare_with_retention(
            &prepared.signed_expression,
            &original.request.expected.stack_in,
            prepared.limits,
            replay.retained_source_limit.bytes(),
        ));
        let source_slot = usize::try_from(rebuilt.selected_root
            .ok_or_else(|| refused("universal selection is not one named source call"))?)
            .map_err(|_| refused("universal selected source slot overflow"))?;
        let submission = rebuilt.submission.take()
            .ok_or_else(|| refused("universal selection did not rederive a checked body"))?;
        let root = match submission.body.candidate.body.as_slice() {
            [root] => submission.body.candidate.nodes.get(root.0 as usize),
            _ => None,
        };
        let mut closure_limits = prepared.limits;
        closure_limits.nodes = closure_limits.nodes.min(4_096);
        closure_limits.work = closure_limits.work.checked_sub(rebuilt.frontend_work_spent)
            .ok_or_else(|| refused("universal selected source replay exceeded shared work"))?;
        let mut meter = crate::Meter::new(closure_limits);
        let (selected_def, selected_inst) = match root {
            Some(Node::Invocation { def, inst }) => {
                let size = witness_size(inst)
                    .ok_or_else(|| refused("selected named owner requires one concrete stack row"))?;
                attempt!(charge(&mut meter, crate::source::declared::SPAN, size));
                (*def, inst.clone())
            }
            _ => return Err(refused("universal selected root is not a concrete named invocation")),
        };
        let source = prepared.staged.source.definitions.get(source_slot)
            .ok_or_else(|| refused("selected universal source owner absent"))?;
        if source.universal.as_ref().map(|seal| seal.owner()) != Some(selected_def) {
            return Err(refused("selected owner does not match its source-checked seal"));
        }
        let reachable = attempt!(prepared.universal_reachable(source_slot, &mut meter));
        let mut owners = Vec::new();
        attempt!(charge(&mut meter, crate::source::declared::SPAN,
            prepared.staged.source.definitions.len()));
        let mut source_to_owner = vec![None; prepared.staged.source.definitions.len()];
        for (slot, used) in reachable.iter().enumerate() {
            attempt!(charge(&mut meter, crate::source::declared::SPAN, 1));
            if !used { continue; }
            attempt!(meter.node(crate::source::declared::SPAN)
                .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
            let owner = attempt!(prepared.universal_owner(slot, &mut meter));
            source_to_owner[slot] = Some(owners.len());
            owners.push(owner);
        }
        for (slot, owner_index) in source_to_owner.iter().enumerate() {
            let Some(owner_index) = owner_index else { continue; };
            let source = &prepared.staged.source.definitions[slot];
            let seal = source.universal.as_ref()
                .ok_or_else(|| refused("reachable owner has no independently checked seal"))?;
            for node in &seal.candidate().nodes {
                attempt!(meter.charge(1, source.tree.span)
                    .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
                let SymbolicNode::Invocation { def, .. } = node else { continue; };
                if prepared.staged.source.declared.as_ref()
                    .is_some_and(|context| context.environment.kind(*def) == Some(Behavior::Named))
                {
                    let target_slot = attempt!(source_slot_by_definition(
                        &prepared.staged.source.definitions, *def, &mut meter, source.tree.span,
                    ))
                        .ok_or_else(|| refused("named lexical edge has no original sealed owner"))?;
                    let target = source_to_owner[target_slot]
                        .ok_or_else(|| refused("named source dependency absent from checked closure"))?;
                    if target >= *owner_index {
                        return Err(refused("universal dependency is not earlier and acyclic"));
                    }
                    if !owners[*owner_index].dependencies.contains(&target) {
                        owners[*owner_index].dependencies.push(target);
                    }
                }
            }
        }
        let selected_owner = source_to_owner[source_slot]
            .ok_or_else(|| refused("selected universal owner absent from checked closure"))?;
        let source_env = &prepared.staged.source.declared.as_ref()
            .ok_or_else(|| refused("universal source environment absent"))?.environment;
        let env_cost = source_env.defs.len()
            .saturating_add(source_env.kinds.len())
            .saturating_add(source_env.deps.len())
            .saturating_add(source_env.deps.iter()
                .fold(0usize, |sum, deps| sum.saturating_add(deps.len())))
            .saturating_add(source_env.schemas.len())
            .saturating_add(source_env.nominals.len())
            .saturating_add(source_env.generic_variants.len())
            .saturating_add(source_env.resource_kinds.len())
            .saturating_add(source_env.definition_owners.len())
            .saturating_add(source_env.bound_adapters.len())
            .saturating_add(source_env.effects.len());
        attempt!(charge(&mut meter, crate::source::declared::SPAN, env_cost));
        for scheme in &source_env.defs {
            attempt!(charge_scheme(scheme, &mut meter));
        }
        for schema in &source_env.schemas {
            attempt!(charge_scheme(&schema.scheme, &mut meter));
        }
        for nominal in &source_env.nominals {
            let size = match &nominal.shape {
                noble_kernel::types::NominalShape::Opaque(ty) => ty.size(),
                noble_kernel::types::NominalShape::Variant(left, right) =>
                    left.size().and_then(|count| count.checked_add(right.size()?)),
            }.ok_or_else(|| refused("universal environment nominal shape invalid"))?;
            attempt!(charge(&mut meter, crate::source::declared::SPAN, size as usize));
        }
        let mut environment = source_env.clone();
        attempt!(charge(&mut meter, crate::source::declared::SPAN, owners.len()));
        let dependencies = owners.iter().map(|owner| Dependency {
            seal: owner.seal, current_stamp: owner.seal.stamp(),
        }).collect::<Vec<_>>();
        // A lexical signature's nominal spelling was bound at admission. The
        // immutable source and Env row above freeze that binding; current
        // schema/nominal/effect grants are checked independently in the kernel
        // under each original module owner, not inferred from the old seal.
        for owner in &owners {
            environment.caller_module = Some(owner.program_owner.module_identity);
            let input_bytes = u32::try_from(owner.source.len())
                .map_err(|_| refused("universal original module byte count overflow"))?;
            let checked = noble_kernel::universal::check_scheme(
                &environment,
                &Request {
                    input_bytes,
                    expected: Expected {
                        stack_in: Vec::new(), stack_out: Vec::new(),
                        allowed_effects: EffSet::empty(),
                    },
                    limits: limits(prepared.limits, meter.work),
                },
                owner.seal.candidate(), owner.seal.owner(), owner.seal.stamp(),
                &dependencies,
            ).map_err(|_| refused("current universal owner schema or grant refused"))?;
            attempt!(meter.charge(checked.work_spent(), crate::source::declared::SPAN)
                .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
        }
        let mut uses = Vec::new();
        attempt!(prepared.universal_add_use(&owners, selected_owner, selected_inst,
            &mut uses, &mut environment, &dependencies, &mut meter));
        let mut at = 0;
        while at < uses.len() {
            let from_owner = uses[at].owner_index;
            let mut edges = Vec::new();
            for node in &uses[at].checked.nodes {
                attempt!(charge(&mut meter, crate::source::declared::SPAN, 1));
                let target = match &node.kind {
                    CheckedNodeKind::Invocation { def, inst }
                        if prepared.staged.source.declared.as_ref().is_some_and(|context|
                            context.environment.kind(*def) == Some(Behavior::Named)) =>
                        Some((*def, inst, false)),
                    CheckedNodeKind::SignedSelf { def, inst } => Some((*def, inst, true)),
                    _ => None,
                };
                let Some((def, inst, self_edge)) = target else { continue; };
                let cost = inst.bindings.iter().try_fold(1usize, |sum, binding|
                    match binding {
                        Binding::Stack(values) => values.iter().try_fold(sum, |count, ty|
                            count.checked_add(ty.size()? as usize)),
                        Binding::Value(ty) => sum.checked_add(ty.size()? as usize),
                        Binding::Effect(effects) => sum.checked_add(effects.as_slice().len()),
                        Binding::Ref(_) => None,
                    }
                ).ok_or_else(|| refused("universal named use has invalid or oversized witness"))?;
                attempt!(charge(&mut meter, crate::source::declared::SPAN, cost));
                edges.push((node.id, def, inst.clone(), self_edge));
            }
            for (node, def, inst, self_edge) in edges {
                attempt!(meter.charge(1, crate::source::declared::SPAN)
                    .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
                let target = if self_edge {
                    if def != owners[from_owner].seal.owner() {
                        return Err(refused("signed self does not target its original checked owner"));
                    }
                    if !same_stack_inst(&inst, &uses[at].owner_inst) {
                        return Err(refused("signed self changed its rigid concrete row"));
                    }
                    at
                } else {
                    let target_owner = attempt!(owner_by_definition(&owners, def, &mut meter));
                    let target_owner = target_owner.ok_or_else(||
                        refused("named concrete edge has no original checked source owner"))?;
                    attempt!(prepared.universal_add_use(
                        &owners, target_owner, inst, &mut uses,
                        &mut environment, &dependencies, &mut meter,
                    ))
                };
                uses[at].targets.push((node, target));
            }
            at += 1;
        }
        Ok(UniversalSourceClosure {
            owners, uses, selected_submission: submission,
            selected_use: 0, source_generation: prepared.staged.source.generation,
        })
    }
}

impl ModulePrepared {
    fn universal_reachable(
        &self, root: usize, meter: &mut crate::Meter,
    ) -> Result<Vec<bool>, crate::source::Error> {
        let named = &self.staged.source.definitions;
        attempt!(charge(meter, crate::source::declared::SPAN, named.len()));
        let mut seen = vec![false; named.len()];
        let mut pending = vec![root];
        while let Some(slot) = pending.pop() {
            let source = named.get(slot).ok_or_else(|| refused("universal owner slot absent"))?;
            attempt!(meter.charge(1, source.tree.span)
                .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
            if seen[slot] { continue; }
            seen[slot] = true;
            let seal = source.universal.as_ref()
                .ok_or_else(|| refused("universal dependency lacks source checked seal"))?;
            for node in &seal.candidate().nodes {
                attempt!(meter.charge(1, source.tree.span)
                    .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
                let SymbolicNode::Invocation { def, .. } = node else { continue; };
                if self.staged.source.declared.as_ref()
                    .is_some_and(|context| context.environment.kind(*def) == Some(Behavior::Named))
                {
                    let dependency = attempt!(source_slot_by_definition(
                        named, *def, meter, source.tree.span,
                    ))
                        .ok_or_else(|| refused("named owner not found in original source namespace"))?;
                    if dependency >= slot {
                        return Err(refused("universal dependency graph retargeted or cyclic"));
                    }
                    pending.push(dependency);
                }
            }
        }
        Ok(seen)
    }

    fn universal_owner<'a>(
        &'a self, slot: usize, meter: &mut crate::Meter,
    ) -> Result<UniversalCheckedOwner<'a>, crate::source::Error> {
        let source = self.staged.source.definitions.get(slot)
            .ok_or_else(|| refused("universal source owner absent"))?;
        let seal = source.universal.as_ref()
            .ok_or_else(|| refused("source owner lacks independent universal seal"))?;
        let origin = source.universal_origin.as_ref()
            .ok_or_else(|| refused("source owner lacks original lexical provenance"))?;
        attempt!(charge(meter, crate::source::declared::SPAN,
            self.staged.modules.len()));
        let module = self.staged.modules.iter().find(|module|
            source.owner == Some(module.identity)
                && origin.module_name == module.name
                && origin.module_version == module.version)
            .ok_or_else(|| refused("universal owner whole module missing or stale"))?;
        let ordinal = origin.ordinal as usize;
        // Both references must still own the very same immutable retained
        // allocation. A same-byte replacement cannot inherit its checked
        // owner, and checking the Arc identity does not rescan source bytes.
        if !alloc::sync::Arc::ptr_eq(&module.source, &origin.original_source) {
            return Err(refused("universal retained whole module differs from admitted original"));
        }
        let span = module.definition_spans.get(ordinal)
            .ok_or_else(|| refused("universal source ordinal missing from original module"))?;
        if span.name != source.name || span.span != origin.source_span
            || span.body_span != origin.body_span
            || source.signed.as_ref().is_some_and(|relation| relation != &origin.relation)
            || self.staged.source.declared.as_ref().is_none_or(|context| {
                context.environment.scheme(seal.owner()) != source.signature.as_ref()
                    || context.environment.kind(seal.owner()) != Some(Behavior::Named)
                    || context.environment.definition_owners.get(seal.owner().0 as usize)
                        != Some(&source.owner)
            })
        { return Err(refused("universal source owner, signature or Env row changed")); }
        if source.signed.is_some()
            && !module.verified_signed_slots.contains(&(slot as u32))
        { return Err(refused("signed source owner is not independently certified")); }
        // A module's imported spellings were bound to immutable module owners
        // at admission; a later outer alias is not allowed to retarget them.
        attempt!(charge(meter, crate::source::declared::SPAN,
            module.import_aliases.len().saturating_mul(self.staged.modules.len())));
        for alias in &module.import_aliases {
            if !self.staged.modules.iter().any(|imported|
                imported.identity == alias.owner)
            { return Err(refused("original module import owner was retired")); }
        }
        let start = usize::try_from(origin.source_span.start)
            .map_err(|_| refused("original universal source span overflow"))?;
        let end = usize::try_from(origin.source_span.end)
            .map_err(|_| refused("original universal source span overflow"))?;
        let bytes = module.source.get(start..end)
            .ok_or_else(|| refused("universal owner outside original whole module"))?;
        // The registered span and Scheme came from parsing this exact immutable
        // Arc at admission. Reparse the lexical body below with the shared
        // meter; redundant whole-module reparses here would escape its budget.
        let tag = attempt!(crate::source::preparation::original_stamp(
            crate::source::preparation::OriginalDefinition {
                module_source: &module.source, module_original: &module.source,
                module_prefix: &module.source_stamp_prefix,
                name_index: None,
                module_name: &module.name,
                module_version: module.version, ordinal: origin.ordinal,
                source_span: origin.source_span, body_span: origin.body_span,
            }, bytes, seal.stamp().generation, source.tree.span, meter,
        ).map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
        if tag != seal.stamp() {
            return Err(refused("universal owner original source or generation stale"));
        }
        let (mut parsed, name) = attempt!(crate::source::parsing::parse_declared(bytes, meter)
            .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
        attempt!(crate::source::locals::lower(&mut parsed, meter)
            .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
        if name.as_deref() != Some(source.name.as_str()) {
            return Err(refused("universal lexical owner name changed"));
        }
        attempt!(crate::source::preparation::symbolic::verify(
            &source.tree, &parsed, &origin.relation, &source.name, bytes,
            source.signed.is_some(), meter,
        ).map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
        let count = seal.candidate().nodes.len();
        attempt!(charge(meter, source.tree.span, count));
        let mut spans = vec![None; count];
        for entry in &origin.relation.symbolic_nodes {
            attempt!(meter.charge(1, source.tree.span)
                .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
            let Some(entry) = entry else { continue; };
            let index = usize::try_from(entry.draft)
                .map_err(|_| refused("universal lexical draft index overflow"))?;
            let binding = spans.get_mut(index)
                .ok_or_else(|| refused("universal lexical draft missing from checked candidate"))?;
            let absolute = crate::Span {
                start: origin.source_span.start.checked_add(entry.span.start)
                    .ok_or_else(|| refused("universal lexical absolute span overflow"))?,
                end: origin.source_span.start.checked_add(entry.span.end)
                    .ok_or_else(|| refused("universal lexical absolute span overflow"))?,
            };
            if absolute.start > absolute.end || absolute.end > origin.source_span.end {
                return Err(refused("universal lexical node outside original definition"));
            }
            if binding.replace(absolute).is_some() {
                return Err(refused("universal lexical draft has duplicate source occurrence"));
            }
        }
        let spans = spans.into_iter().collect::<Option<Vec<_>>>()
            .ok_or_else(|| refused("universal checked candidate has no lexical source occurrence"))?;
        let owner = CheckedProgramOwner {
            module_identity: module.identity,
            module_version: module.version,
            definition_ordinal: origin.ordinal,
            definition_identity: source.identity,
            definition_slot: u32::try_from(slot)
                .map_err(|_| refused("universal source slot overflow"))?,
        };
        Ok(UniversalCheckedOwner {
            program_owner: owner,
            source: &module.source,
            seal,
            lexical: UniversalLexicalBody { spans },
            dependencies: Vec::new(),
        })
    }

    fn universal_add_use(
        &self,
        owners: &[UniversalCheckedOwner<'_>],
        owner_index: usize,
        inst: Inst,
        uses: &mut Vec<UniversalCheckedUse>,
        environment: &mut Env,
        dependencies: &[Dependency<'_>],
        meter: &mut crate::Meter,
    ) -> Result<usize, crate::source::Error> {
        let owner = owners.get(owner_index)
            .ok_or_else(|| refused("concrete use has no source owner"))?;
        let span = crate::source::declared::SPAN;
        let inst_cost = witness_size(&inst)
            .ok_or_else(|| refused("universal owner requires one concrete stack binding"))?;
        for (index, use_site) in uses.iter().enumerate() {
            attempt!(charge(meter, span, 1));
            if use_site.owner_index != owner_index { continue; }
            let prior_cost = witness_size(&use_site.owner_inst)
                .ok_or_else(|| refused("retained universal instance witness changed"))?;
            attempt!(charge(meter, span, prior_cost.saturating_add(inst_cost)));
            if same_stack_inst(&use_site.owner_inst, &inst) { return Ok(index); }
        }
        attempt!(meter.node(span)
            .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
        environment.caller_module = Some(owner.program_owner.module_identity);
        let budget = meter.work;
        let checked = noble_kernel::universal::instantiate_checked(
            owner.seal, environment, owner.seal.stamp(), &inst,
            limits(self.limits, budget), dependencies,
        ).map_err(|_| refused("concrete universal use, ABI or current grant refused"))?;
        attempt!(meter.charge(checked.work_spent(), span)
            .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
        // Source-module limits concern each original owner; a selected Wasm
        // closure additionally has the original 4096 aggregate node budget.
        // Every separately instantiated `(owner, S)` contributes its whole
        // checked body, not just one entry node or one symbolic owner.
        for _ in &checked.nodes {
            attempt!(meter.node(span)
                .map_err(|problem| super::super::diagnostic(crate::source::Stage::Check, problem)));
        }
        let index = uses.len();
        uses.push(UniversalCheckedUse {
            owner_index, owner_inst: inst, checked, targets: Vec::new(),
        });
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::super::ModuleSession;
    use super::*;

    fn selected() -> Result<ModulePrepared, crate::source::Error> {
        let limits = crate::Limits::default();
        let retained = crate::source::RetainedSourceLimit::new(262_144)
            .ok_or_else(|| refused("invalid selected retained source bound"))?;
        let mut session = ModuleSession::new_declared_text(&[], true, retained)?;
        for source in [
            b"module inc@1 [ signature once forall<S:stack> [ S I64 -- S I64 ! pure ] export once def once [ 1 + ] ]".as_slice(),
            b"module twice@1 [ signature twice forall<S:stack> [ S I64 -- S I64 ! pure ] export twice def twice [ inc@1.once inc@1.once ] ]".as_slice(),
        ] {
            let prepared = session.prepare(source, &[], limits)?;
            let (next, outcome) = session.commit(prepared);
            outcome?;
            session = next;
        }
        session.prepare(b"twice@1.twice", &[noble_kernel::types::Ty::I64], limits)
    }

    #[test]
    fn retained_module_and_history_capacity_is_exact_and_failed_registration_is_atomic() {
        let limits = crate::Limits::default();
        let first = b"module kept@1 [ signature value forall<S:stack> [ S -- S I64 ! pure ] export value def value [ 7 ] ]";
        let second = b"module next@1 [ signature value forall<S:stack> [ S -- S I64 ! pure ] export value def value [ 8 ] ]";
        let new_session = |cap| {
            ModuleSession::new_declared_text(&[], true,
                crate::source::RetainedSourceLimit::new(cap).expect("bounded retention"))
                .expect("selected source session")
        };
        let mut admitted = new_session(262_144);
        for source in [first.as_slice(), second.as_slice()] {
            let prepared = admitted.prepare(source, &[], limits).expect("source fits retention");
            let (next, outcome) = admitted.commit(prepared);
            outcome.expect("publish exact source");
            admitted = next;
        }
        let exact = admitted.modules.iter().map(|module| module.source.len()).sum::<usize>()
            + admitted.source.history.len();
        let exact = u32::try_from(exact).expect("bounded retained byte count");
        assert!(exact > first.len() as u32);

        let mut one_short = new_session(exact - 1);
        let prepared = one_short.prepare(first, &[], limits).expect("first source fits");
        let (next, outcome) = one_short.commit(prepared);
        outcome.expect("publish first source");
        one_short = next;
        let refused = one_short.prepare(second, &[], limits)
            .expect_err("one byte below exact module plus history capacity must refuse");
        assert_eq!(refused.stage(), crate::source::Stage::Check);
        assert_eq!(one_short.generation(), 1);
        one_short.prepare(b"kept@1.value", &[], limits)
            .expect("failed second module must not replace the prior namespace");

        let mut exact_session = new_session(exact);
        for source in [first.as_slice(), second.as_slice()] {
            let prepared = exact_session.prepare(source, &[], limits)
                .expect("exact aggregate capacity admits both modules");
            let (next, outcome) = exact_session.commit(prepared);
            outcome.expect("publish at exact capacity");
            exact_session = next;
        }
        exact_session.prepare(b"next@1.value", &[], limits)
            .expect("exactly retained second owner remains selectable");
    }

    #[test]
    fn individually_admissible_definitions_cannot_overdraw_shared_module_work() {
        let first = b"module shared@1 [ signature one forall<S:stack> [ S -- S I64 ! pure ] \
            export one def one [ 7 ] ]";
        let second = b"module shared@1 [ signature two forall<S:stack> [ S -- S I64 ! pure ] \
            export two def two [ 8 ] ]";
        let both = b"module shared@1 [ signature one forall<S:stack> [ S -- S I64 ! pure ] \
            export one def one [ 7 ] signature two forall<S:stack> [ S -- S I64 ! pure ] \
            export two def two [ 8 ] ]";
        let retained = crate::source::RetainedSourceLimit::new(262_144)
            .expect("bounded original retention");
        let source_fits = |source: &[u8], work| {
            let session = ModuleSession::new_declared_text(&[], true, retained)
                .expect("fresh declared namespace");
            let limits = crate::Limits { work, ..crate::Limits::default() };
            session.prepare(source, &[], limits).is_ok()
        };
        let minimum = |source: &[u8]| {
            let mut low = 0;
            let mut high = crate::Limits::default().work;
            assert!(source_fits(source, high), "each single definition must be valid");
            while low < high {
                let mid = low + (high - low) / 2;
                if source_fits(source, mid) { high = mid; } else { low = mid + 1; }
            }
            low
        };
        let minimum_parse = || {
            let mut low = 0;
            let mut high = crate::Limits::default().work;
            while low < high {
                let mid = low + (high - low) / 2;
                let session = ModuleSession::new_declared_text(&[], true, retained)
                    .expect("fresh declared namespace");
                let limits = crate::Limits { work: mid, ..crate::Limits::default() };
                if session.prepare(both, &[], limits).is_err_and(|error|
                    error.stage() == crate::source::Stage::Parse)
                {
                    low = mid + 1;
                } else {
                    high = mid;
                }
            }
            low
        };
        let work = minimum(first).max(minimum(second)).max(minimum_parse());
        assert!(source_fits(first, work));
        assert!(source_fits(second, work));
        let session = ModuleSession::new_declared_text(&[], true, retained)
            .expect("fresh declared namespace");
        let limits = crate::Limits { work, ..crate::Limits::default() };
        let refused = session.prepare(both, &[], limits)
            .expect_err("two individually admitted bodies must not bypass aggregate Work");
        assert_eq!(refused.stage(), crate::source::Stage::Check);
        assert_eq!(refused.diagnostic().kind, crate::DiagnosticKind::Exhausted);
        assert_eq!(session.generation(), 0);
        assert!(session.prepare(b"shared@1.one", &[], crate::Limits::default()).is_err());
        session.prepare(first, &[], crate::Limits::default())
            .expect("failed work overdraw leaves original namespace usable");
    }

    #[test]
    fn frozen_original_signed_addition_chain_rederives_under_original_limits() {
        let limits = crate::Limits::default();
        let retained = crate::source::RetainedSourceLimit::new(262_144)
            .expect("valid original retained-source limit");
        let mut session = ModuleSession::new_declared_text(&[], true, retained)
            .expect("selected declared text owner");
        let modules: [&[u8]; 10] = [
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../verification/calculator-exact-library/bitstep.noble")),
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../verification/calculator-exact-library/bitpair.noble")),
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../verification/calculator-exact-library/bitfull.noble")),
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../verification/calculator-exact-library/bitcompare.noble")),
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../verification/calculator-exact-library/bittrim.noble")),
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../verification/calculator-exact-library/bitsigned.noble")),
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../verification/calculator-exact-library/bitsum.noble")),
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../verification/calculator-exact-library/bitaddcarry.noble")),
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../verification/calculator-exact-library/bitorder.noble")),
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
                "/../../verification/calculator-exact-library/bitaddsigned.noble")),
        ];
        for (index, source) in modules.into_iter().enumerate() {
            let prepared = session.prepare(source, &[], limits)
                .unwrap_or_else(|error| panic!("frozen module {index} refused: {error:?}"));
            let (next, outcome) = session.commit(prepared);
            outcome.unwrap_or_else(|error|
                panic!("frozen module {index} failed publication: {error:?}"));
            session = next;
        }
        let (scan_slot, signed) = session.source.definitions.iter().enumerate()
            .find(|(_, definition)| definition.name == "scan"
                && definition.owner == Some(session.modules[6].identity))
            .expect("original bitsum.scan");
        assert!(signed.signed.is_some() && signed.universal.is_some(),
            "the original signed recursion must retain an independent universal seal");
        let magnitude = noble_kernel::types::Ty::Pair(
            alloc::boxed::Box::new(noble_kernel::types::Ty::Bool),
            alloc::boxed::Box::new(noble_kernel::types::Ty::List(
                alloc::boxed::Box::new(noble_kernel::types::Ty::Bool))),
        );
        let selected = session.prepare(b"bitaddsigned@1.add",
            &[magnitude.clone(), magnitude.clone()], limits)
            .expect("frozen signed addition source selection");
        let closure = selected.declared_text_source_owner()
            .expect("selected source owner")
            .rederive_universal()
            .expect("finite checked original signed addition graph");
        let scan_owner = closure.owners().iter().position(|owner|
            owner.program_owner().definition_identity() == signed.identity
                && owner.program_owner().definition_slot() as usize == scan_slot)
            .expect("original signed scan owner is reachable from selected addition");
        let bits = noble_kernel::types::Ty::List(
            alloc::boxed::Box::new(noble_kernel::types::Ty::Bool));
        assert!(closure.uses().iter().any(|use_site|
            use_site.owner_index() == scan_owner
                && use_site.checked().interface.stack_out.last() == Some(&bits)),
            "selected addition requires a concrete signed scan use returning bits");
        let selected = &closure.uses()[closure.selected_use()];
        assert_eq!(selected.checked().interface.stack_in,
            vec![magnitude.clone(), magnitude.clone()]);
        assert_eq!(selected.checked().interface.stack_out, vec![magnitude]);
    }

    #[test]
    fn original_modules_bind_distinct_checked_uses_without_expanding_named_bodies() {
        let prepared = selected().expect("two original source modules must register");
        let closure = prepared.declared_text_source_owner()
            .expect("selected original source")
            .rederive_universal().expect("source-bound universal concrete closure");
        assert_eq!(closure.owners().len(), 2);
        assert_eq!(closure.uses().len(), 2);
        assert_eq!(closure.selected_use(), 0);
        let twice = &closure.uses()[0];
        let inc = &closure.uses()[1];
        assert_eq!(twice.target_uses().len(), 2);
        assert!(twice.target_uses().iter().all(|(_, target)| *target == 1));
        assert!(inc.target_uses().is_empty());
        assert!(closure.owners()[twice.owner_index()].dependencies()
            .contains(&inc.owner_index()));
        assert_eq!(twice.checked().interface.stack_in, vec![noble_kernel::types::Ty::I64]);
        assert_eq!(twice.checked().interface.stack_out, vec![noble_kernel::types::Ty::I64]);
        assert_ne!(closure.owners()[twice.owner_index()].original_source(),
            closure.owners()[inc.owner_index()].original_source());
        let source = &closure.owners()[inc.owner_index()];
        let literal = source.lexical_body().node_span(NodeId(0))
            .expect("original literal has a checked whole-module lexical span");
        let start = usize::try_from(literal.start).expect("bounded literal start");
        let end = usize::try_from(literal.end).expect("bounded literal end");
        assert_eq!(&source.original_source()[start..end], b"1");
    }

    #[test]
    fn changed_whole_module_bytes_owner_and_grants_refuse_source_reuse() {
        let valid = selected().expect("source fixture");
        assert!(valid.declared_text_source_owner().expect("source owner")
            .rederive_universal().is_ok(), "unchanged owner must rederive");

        let mut bytes = selected().expect("source fixture");
        let module = bytes.staged.modules.iter_mut()
            .find(|module| module.name == "inc").expect("original inc module");
        alloc::sync::Arc::make_mut(&mut module.source)[6] = b'\n';
        assert!(bytes.declared_text_source_owner().expect("source owner")
            .rederive_universal().is_err(), "changed original module must be refused");

        let mut copy = selected().expect("source fixture");
        let module = copy.staged.modules.iter_mut()
            .find(|module| module.name == "inc").expect("original inc module");
        module.source = alloc::sync::Arc::<[u8]>::from(module.source.as_ref());
        assert!(copy.declared_text_source_owner().expect("source owner")
            .rederive_universal().is_err(),
            "equal bytes from a different allocation cannot inherit the original owner");

        let mut span = selected().expect("source fixture");
        let module = span.staged.modules.iter_mut()
            .find(|module| module.name == "inc").expect("original inc module");
        module.definition_spans[0].body_span.start += 1;
        assert!(span.declared_text_source_owner().expect("source owner")
            .rederive_universal().is_err(), "changed original body span must be refused");

        let mut foreign = selected().expect("source fixture");
        let source = foreign.staged.source.definitions.iter_mut()
            .find(|definition| definition.name == "once").expect("once owner");
        source.owner = Some(u64::MAX);
        assert!(foreign.declared_text_source_owner().expect("source owner")
            .rederive_universal().is_err(), "foreign owner must be refused");

        let mut revoked = selected().expect("source fixture");
        let environment = &mut revoked.staged.source.declared.as_mut()
            .expect("checked declared environment").environment;
        environment.effects.clear();
        assert!(revoked.declared_text_source_owner().expect("source owner")
            .rederive_universal().is_err(), "changed effect universe must be refused");
    }
}
