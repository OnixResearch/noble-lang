//! Source-bound, pure rank-one code reuse. The ordinary concrete admission
//! and its exact named-call ABI guard remain unchanged.

use alloc::vec;
use alloc::vec::Vec;
use noble_kernel::{
    contracts::{Behavior, Definition},
    types::{EffSet, Ty},
    universal::{CheckedNode, CheckedNodeKind, SymbolicNode},
    untrusted::{Lit, Node, NodeId},
    words::{Binding, Inst},
};

use super::{plan, CodeSpan, Compiler, InstalledUniversalCode, Prepared, UniversalMetadata,
    UniversalNodeOrigin, UniversalOwnerMetadata, UniversalProgramMetadata,
    UniversalSiteMetadata, Work};

type Closure<'a> = noble_contracts::source::UniversalSourceClosure<'a>;

struct UsePrograms {
    root: usize,
    quotations: Vec<Option<usize>>,
}

fn number(value: usize) -> Result<u32, crate::Diagnostic> {
    plan::number(value)
}

fn same_stack(left: &Inst, right: &Inst) -> bool {
    matches!((left.bindings.as_slice(), right.bindings.as_slice()),
        ([Binding::Stack(a)], [Binding::Stack(b)]) if a == b)
}

fn checked_node(
    use_entry: &noble_contracts::source::UniversalCheckedUse,
    node: NodeId,
) -> Result<&CheckedNode, crate::Diagnostic> {
    let index = usize::try_from(node.0).map_err(|_| crate::Diagnostic::Invalid)?;
    let checked = use_entry.checked().nodes.get(index).ok_or(crate::Diagnostic::Invalid)?;
    if checked.id != node { return Err(crate::Diagnostic::Invalid); }
    Ok(checked)
}

fn program(
    compiler: &mut Compiler, input: &[Ty], output: &[Ty], work: &mut Work,
) -> Result<plan::Program, crate::Diagnostic> {
    Ok(plan::Program {
        entry: 0, emit_code: false,
        input: attempt!(compiler.signature(input, work)),
        output: attempt!(compiler.signature(output, work)),
        effects: 0, logical: None, operations: Vec::new(), depth: 0, leaves: 0,
        universal: None,
    })
}

fn verify_source(closure: &Closure<'_>, work: &mut Work) -> Result<(), crate::Diagnostic> {
    if closure.owners().is_empty() || closure.uses().is_empty()
        || closure.selected_use() >= closure.uses().len()
        || closure.uses().len() >= super::NODE_LIMIT
    {
        return Err(crate::Diagnostic::Invalid);
    }
    let env = &closure.selected_submission().environment;
    if !env.declared_modules || !env.text_cursor || env.live_slots {
        return Err(crate::Diagnostic::Unsupported);
    }
    for owner in closure.owners() {
        let candidate = owner.seal().candidate();
        attempt!(work.entries(candidate.nodes.len().saturating_add(
            candidate.body.len()).saturating_add(owner.dependencies().len())));
        if env.kind(owner.seal().owner()) != Some(Behavior::Named)
            || owner.seal().effects() != &EffSet::empty()
            || candidate.nodes.len() != owner.lexical_body().node_count()
            || candidate.nodes.len() > super::NODE_LIMIT
            || owner.dependencies().iter().any(|index| *index >= closure.owners().len())
        {
            return Err(crate::Diagnostic::Invalid);
        }
        for index in 0..candidate.nodes.len() {
            let span = owner.lexical_body().node_span(NodeId(attempt!(number(index))))
                .ok_or(crate::Diagnostic::Invalid)?;
            if span.start > span.end || usize::try_from(span.end).ok()
                .is_none_or(|end| end > owner.original_source().len())
            {
                return Err(crate::Diagnostic::Invalid);
            }
            if matches!(&candidate.nodes[index], SymbolicNode::Literal(Lit::Text)) {
                return Err(crate::Diagnostic::Unsupported);
            }
        }
    }
    for use_entry in closure.uses() {
        let owner = closure.owners().get(use_entry.owner_index())
            .ok_or(crate::Diagnostic::Invalid)?;
        let nodes = &owner.seal().candidate().nodes;
        attempt!(work.entries(nodes.len().saturating_add(use_entry.target_uses().len())));
        if use_entry.checked().nodes.len() != nodes.len()
            || !use_entry.checked().interface.effects.is_empty()
        {
            return Err(crate::Diagnostic::Invalid);
        }
        for (index, (symbolic, checked)) in nodes.iter()
            .zip(&use_entry.checked().nodes).enumerate()
        {
            if checked.id.0 != attempt!(number(index)) || !checked.interface.effects.is_empty() {
                return Err(crate::Diagnostic::Invalid);
            }
            let matches = match (symbolic, &checked.kind) {
                (SymbolicNode::Literal(left), CheckedNodeKind::Literal { lit: right, .. }) =>
                    left == right,
                (SymbolicNode::Invocation { def: left, .. },
                    CheckedNodeKind::Invocation { def: right, .. }) => left == right,
                (SymbolicNode::SignedSelf { def: left },
                    CheckedNodeKind::SignedSelf { def: right, .. }) => left == right,
                (SymbolicNode::Quotation { body: left, .. },
                    CheckedNodeKind::Quotation { body: right, .. }) => left == right,
                _ => false,
            };
            if !matches { return Err(crate::Diagnostic::Invalid); }
        }
        for (node, target) in use_entry.target_uses() {
            if *target >= closure.uses().len() || checked_node(use_entry, *node).is_err() {
                return Err(crate::Diagnostic::Invalid);
            }
            let checked = attempt!(checked_node(use_entry, *node));
            let (def, inst) = match &checked.kind {
                CheckedNodeKind::Invocation { def, inst }
                | CheckedNodeKind::SignedSelf { def, inst } => (def, inst),
                _ => return Err(crate::Diagnostic::Invalid),
            };
            let target_use = &closure.uses()[*target];
            let target_owner = &closure.owners()[target_use.owner_index()];
            if *def != target_owner.seal().owner()
                || !same_stack(inst, target_use.owner_inst())
            {
                return Err(crate::Diagnostic::Invalid);
            }
            if matches!(&checked.kind, CheckedNodeKind::SignedSelf { .. })
                && (*target != closure.uses().iter().position(|item|
                    core::ptr::eq(item, use_entry)).ok_or(crate::Diagnostic::Invalid)?
                    || !same_stack(inst, use_entry.owner_inst()))
            {
                return Err(crate::Diagnostic::Invalid);
            }
        }
    }
    let selected = closure.selected_submission();
    let use_entry = &closure.uses()[closure.selected_use()];
    let owner = &closure.owners()[use_entry.owner_index()];
    let [node] = selected.body.candidate.body.as_slice() else {
        return Err(crate::Diagnostic::Invalid);
    };
    let Some(Node::Invocation { def, inst }) = selected.body.candidate.nodes.get(node.0 as usize)
    else { return Err(crate::Diagnostic::Invalid); };
    if *def != owner.seal().owner() || !same_stack(inst, use_entry.owner_inst())
        || selected.request.expected.stack_in != use_entry.checked().interface.stack_in
        || selected.request.expected.stack_out != use_entry.checked().interface.stack_out
        || !selected.request.expected.allowed_effects.is_empty()
        || !selected.definitions.is_empty()
    {
        return Err(crate::Diagnostic::Invalid);
    }
    Ok(())
}

fn existing_owner(
    compiler: &Compiler, owner: &noble_contracts::source::UniversalCheckedOwner<'_>,
    work: &mut Work,
) -> Result<Option<usize>, crate::Diagnostic> {
    let program_owner = owner.program_owner();
    let mut found = None;
    for (index, installed) in compiler.universal_code.iter().enumerate() {
        attempt!(work.entries(1));
        if installed.definition != owner.seal().owner() { continue; }
        let original = owner.original_source();
        let same_allocation = core::ptr::eq(installed.source.as_ref(), original);
        attempt!(work.entries(if same_allocation { 1 } else {
            installed.source.len().max(original.len())
        }));
        if found.is_some()
            || installed.definition_slot != program_owner.definition_slot()
            || installed.definition_identity != program_owner.definition_identity()
            || installed.module_identity != program_owner.module_identity()
            || installed.module_version != program_owner.module_version()
            || installed.definition_ordinal != program_owner.definition_ordinal()
            || installed.stamp != owner.stamp()
            || (!same_allocation && installed.source.as_ref() != original)
        {
            return Err(crate::Diagnostic::Invalid);
        }
        found = Some(index);
    }
    Ok(found)
}

fn allocate_uses(
    closure: &Closure<'_>, compiler: &mut Compiler, work: &mut Work,
) -> Result<(Vec<plan::Program>, Vec<UsePrograms>), crate::Diagnostic> {
    let mut programs = Vec::new();
    let mut use_programs = Vec::with_capacity(closure.uses().len());
    for use_entry in closure.uses() {
        let owner = &closure.owners()[use_entry.owner_index()];
        let nodes = &owner.seal().candidate().nodes;
        attempt!(work.entries(nodes.len().saturating_add(1)));
        let quotations = nodes.iter()
            .filter(|node| matches!(node, SymbolicNode::Quotation { .. })).count();
        if programs.len().checked_add(quotations.saturating_add(1))
            .is_none_or(|count| count >= super::NODE_LIMIT)
        {
            return Err(crate::Diagnostic::Exhausted);
        }
        let root = programs.len();
        programs.push(attempt!(program(compiler,
            &use_entry.checked().interface.stack_in,
            &use_entry.checked().interface.stack_out, work)));
        let mut quotations = vec![None; nodes.len()];
        for (index, node) in nodes.iter().enumerate() {
            if !matches!(node, SymbolicNode::Quotation { .. }) { continue; }
            let id = NodeId(attempt!(number(index)));
            let span = owner.lexical_body().node_span(id).ok_or(crate::Diagnostic::Invalid)?;
            if span.start == span.end { return Err(crate::Diagnostic::Unsupported); }
            let checked = attempt!(checked_node(use_entry, id));
            let Some(Ty::Program(input, output, effects)) = checked.interface.stack_out.last()
            else { return Err(crate::Diagnostic::Invalid); };
            if !effects.is_empty() { return Err(crate::Diagnostic::Invalid); }
            quotations[index] = Some(programs.len());
            programs.push(attempt!(program(compiler, input, output, work)));
        }
        use_programs.push(UsePrograms { root, quotations });
    }
    if programs.len() >= super::NODE_LIMIT { return Err(crate::Diagnostic::Exhausted); }
    Ok((programs, use_programs))
}

fn owner_code(
    closure: &Closure<'_>, compiler: &mut Compiler, use_programs: &[UsePrograms],
    work: &mut Work,
) -> Result<(Vec<usize>, Vec<UniversalOwnerMetadata>), crate::Diagnostic> {
    let mut registry = Vec::with_capacity(closure.owners().len());
    let mut metadata = Vec::with_capacity(closure.owners().len());
    for (owner_index, owner) in closure.owners().iter().enumerate() {
        let existing = attempt!(existing_owner(compiler, owner, work));
        let newly_compiled = existing.is_none();
        let code_index = if let Some(index) = existing { index } else {
            let candidate = owner.seal().candidate();
            let mut count = attempt!(number(candidate.body.len().max(1)));
            for node in &candidate.nodes {
                if let SymbolicNode::Quotation { body, .. } = node {
                    count = count.checked_add(attempt!(number(body.len().max(1))))
                        .ok_or(crate::Diagnostic::Exhausted)?;
                }
            }
            attempt!(work.entries(candidate.nodes.len().saturating_add(count as usize)));
            let span = attempt!(compiler.reserve_code(count));
            let mut entry = span.start;
            let body_entry = entry;
            entry = entry.checked_add(attempt!(number(candidate.body.len().max(1))))
                .ok_or(crate::Diagnostic::Exhausted)?;
            let mut quotes = Vec::new();
            for (index, node) in candidate.nodes.iter().enumerate() {
                if let SymbolicNode::Quotation { body, .. } = node {
                    quotes.push((NodeId(attempt!(number(index))), entry));
                    entry = entry.checked_add(attempt!(number(body.len().max(1))))
                        .ok_or(crate::Diagnostic::Exhausted)?;
                }
            }
            if entry != span.start + span.length { return Err(crate::Diagnostic::Defective); }
            let program_owner = owner.program_owner();
            compiler.universal_code.push(InstalledUniversalCode {
                definition: owner.seal().owner(),
                definition_slot: program_owner.definition_slot(),
                definition_identity: program_owner.definition_identity(),
                module_identity: program_owner.module_identity(),
                module_version: program_owner.module_version(),
                definition_ordinal: program_owner.definition_ordinal(),
                source: owner.original_source_arc(), stamp: owner.stamp(),
                span, body_entry, quotes,
            });
            compiler.universal_code.len() - 1
        };
        let code = &compiler.universal_code[code_index];
        // Reused entries are never re-emitted. New code is emitted by the
        // first checked use of this owner, whichever concrete S it carries.
        if newly_compiled && !use_programs.iter().enumerate().any(|(index, _)|
            closure.uses()[index].owner_index() == owner_index)
        {
            return Err(crate::Diagnostic::Invalid);
        }
        registry.push(code_index);
        let mut nodes = Vec::with_capacity(owner.seal().candidate().nodes.len());
        for index in 0..owner.seal().candidate().nodes.len() {
            let node = NodeId(attempt!(number(index)));
            let span = owner.lexical_body().node_span(node).ok_or(crate::Diagnostic::Invalid)?;
            nodes.push(UniversalNodeOrigin { node, span });
        }
        metadata.push(UniversalOwnerMetadata {
            owner_index, definition: code.definition, definition_slot: code.definition_slot,
            definition_identity: code.definition_identity,
            module_identity: code.module_identity,
            module_version: code.module_version,
            definition_ordinal: code.definition_ordinal,
            source: code.source.clone(), nodes, stamp: code.stamp, code_span: code.span,
            body_entry: code.body_entry, quotations: code.quotes.clone(), newly_compiled,
        });
    }
    Ok((registry, metadata))
}

fn assign_entries(
    closure: &Closure<'_>, registry: &[usize], compiler: &Compiler,
    use_programs: &[UsePrograms], programs: &mut [plan::Program],
) -> Result<(), crate::Diagnostic> {
    let mut emitted = vec![false; closure.owners().len()];
    for (use_index, use_entry) in closure.uses().iter().enumerate() {
        let owner_index = use_entry.owner_index();
        let owner = &closure.owners()[owner_index];
        let code = &compiler.universal_code[registry[owner_index]];
        let emit = code.span.generation == compiler.generation && !emitted[owner_index];
        emitted[owner_index] = true;
        let instance = &use_programs[use_index];
        programs[instance.root].entry = code.body_entry;
        programs[instance.root].emit_code = emit;
        for (node, entry) in &code.quotes {
            let index = usize::try_from(node.0).map_err(|_| crate::Diagnostic::Invalid)?;
            let child = instance.quotations.get(index).copied().flatten()
                .ok_or(crate::Diagnostic::Invalid)?;
            programs[child].entry = *entry;
            programs[child].emit_code = emit;
        }
        if code.quotes.len() != owner.seal().candidate().nodes.iter()
            .filter(|node| matches!(node, SymbolicNode::Quotation { .. })).count()
        {
            return Err(crate::Diagnostic::Invalid);
        }
    }
    Ok(())
}

fn target_use(
    use_entry: &noble_contracts::source::UniversalCheckedUse, node: NodeId,
) -> Result<usize, crate::Diagnostic> {
    let mut found = None;
    for (id, target) in use_entry.target_uses() {
        if *id == node {
            if found.is_some() { return Err(crate::Diagnostic::Invalid); }
            found = Some(*target);
        }
    }
    found.ok_or(crate::Diagnostic::Invalid)
}

struct Lower<'a, 'source> {
    closure: &'a Closure<'source>,
    uses: &'a [UsePrograms],
    registry: &'a [usize],
    compiler: &'a mut Compiler,
    work: &'a mut Work,
}

impl Lower<'_, '_> {
    fn sequence(
        &mut self, use_index: usize, entries: &[NodeId],
        sites: &mut Vec<UniversalSiteMetadata>,
    ) -> Result<Vec<plan::Operation>, crate::Diagnostic> {
        let mut operations = Vec::with_capacity(entries.len());
        for node in entries {
            let use_entry = &self.closure.uses()[use_index];
            let owner = &self.closure.owners()[use_entry.owner_index()];
            let checked = attempt!(checked_node(use_entry, *node));
            let span = owner.lexical_body().node_span(*node).ok_or(crate::Diagnostic::Invalid)?;
            if span.start == span.end
                && matches!(&checked.kind,
                    CheckedNodeKind::Quotation { .. }
                    | CheckedNodeKind::SignedSelf { .. }
                    | CheckedNodeKind::Invocation { def: Definition(8), .. })
            {
                return Err(crate::Diagnostic::Unsupported);
            }
            let mut site = None;
            let action = match &checked.kind {
                CheckedNodeKind::Literal { lit: Lit::I64(value), .. } => plan::Action::I64(*value),
                CheckedNodeKind::Literal { lit: Lit::Bool(value), .. } => plan::Action::Bool(*value),
                CheckedNodeKind::Literal { lit: Lit::Unit, .. } => plan::Action::Unit,
                CheckedNodeKind::Literal { lit: Lit::Text, .. } => return Err(crate::Diagnostic::Unsupported),
                CheckedNodeKind::Quotation { .. } => {
                    let child = self.uses[use_index].quotations
                        .get(node.0 as usize).copied().flatten()
                        .ok_or(crate::Diagnostic::Invalid)?;
                    let Some(Ty::Program(input, output, effects)) =
                        checked.interface.stack_out.last()
                    else { return Err(crate::Diagnostic::Invalid); };
                    if !effects.is_empty() { return Err(crate::Diagnostic::Invalid); }
                    let code = &self.compiler.universal_code[self.registry[use_entry.owner_index()]];
                    let identity = code.definition_identity;
                    let definition = code.definition;
                    let quote = code.quotes.binary_search_by_key(&node.0, |(id, _)| id.0)
                        .map_err(|_| crate::Diagnostic::Invalid)?;
                    let target_entry = code.quotes[quote].1;
                    let site_index = attempt!(number(sites.len()));
                    site = Some(UniversalSiteMetadata {
                        node: *node, source_span: span, action: 1,
                        target_use: Some(use_index), target_program_index: Some(child),
                        target_slot: 0,
                        input_signature: attempt!(self.compiler.signature(input, self.work)),
                        output_signature: attempt!(self.compiler.signature(output, self.work)),
                        effect_or_witness: 0, target_identity: identity,
                        target_definition: definition, target_entry,
                    });
                    plan::Action::UniversalProgram { site: site_index, child }
                }
                CheckedNodeKind::SignedSelf { .. } => {
                    let target = attempt!(target_use(use_entry, *node));
                    if target != use_index { return Err(crate::Diagnostic::Invalid); }
                    let site_index = attempt!(number(sites.len()));
                    site = Some(attempt!(named_site(
                        self.closure, self.uses, self.registry, self.compiler, self.work,
                        *node, span, target, &checked.interface,
                    )));
                    let identity = site.as_ref().ok_or(crate::Diagnostic::Defective)?.target_identity;
                    plan::Action::UniversalCall { site: site_index, identity }
                }
                CheckedNodeKind::Invocation { def, .. } if *def == Definition(8) => {
                    let Some(Ty::Program(input, output, effects)) = checked.interface.stack_out.last()
                    else { return Err(crate::Diagnostic::Invalid); };
                    if !effects.is_empty() { return Err(crate::Diagnostic::Invalid); }
                    let witness = checked.interface.stack_in.last().ok_or(crate::Diagnostic::Invalid)?;
                    let code = &self.compiler.universal_code[self.registry[use_entry.owner_index()]];
                    let identity = code.definition_identity;
                    let definition = code.definition;
                    let site_index = attempt!(number(sites.len()));
                    site = Some(UniversalSiteMetadata {
                        node: *node, source_span: span, action: 3,
                        target_use: None, target_program_index: None, target_slot: 0,
                        input_signature: attempt!(self.compiler.signature(input, self.work)),
                        output_signature: attempt!(self.compiler.signature(output, self.work)),
                        effect_or_witness: attempt!(self.compiler.signature(
                            core::slice::from_ref(witness), self.work)),
                        target_identity: identity,
                        target_definition: definition, target_entry: 0,
                    });
                    plan::Action::UniversalQuote { site: site_index }
                }
                CheckedNodeKind::Invocation { def, .. } => {
                    let env = &self.closure.selected_submission().environment;
                    if env.kind(*def) == Some(Behavior::Named)
                        && def.0 >= super::builtin_count(env)
                    {
                        if span.start == span.end {
                            return Err(crate::Diagnostic::Unsupported);
                        }
                        let target = attempt!(target_use(use_entry, *node));
                        let target_owner = self.closure.uses()[target].owner_index();
                        if target_owner >= use_entry.owner_index()
                            || !owner.dependencies().contains(&target_owner)
                        {
                            return Err(crate::Diagnostic::Invalid);
                        }
                        let site_index = attempt!(number(sites.len()));
                        site = Some(attempt!(named_site(
                            self.closure, self.uses, self.registry, self.compiler, self.work,
                            *node, span, target, &checked.interface,
                        )));
                        let identity = site.as_ref().ok_or(crate::Diagnostic::Defective)?.target_identity;
                        plan::Action::UniversalCall { site: site_index, identity }
                    } else if def.0 == 26 && env.text_cursor
                        && env.kind(*def) == Some(Behavior::TextByte)
                    {
                        plan::Action::TextByte
                    } else if def.0 == 27 && env.text_cursor && env.declared_modules
                        && env.kind(*def) == Some(Behavior::I64Less)
                    {
                        plan::Action::I64Less
                    } else if def.0 < 22 && def.0 < super::builtin_count(env) {
                        plan::Action::Word(def.0)
                    } else {
                        return Err(crate::Diagnostic::Unsupported);
                    }
                }
            };
            if let Some(site) = site { sites.push(site); }
            operations.push(plan::Operation {
                node: *node, action,
                input: attempt!(self.compiler.signature(&checked.interface.stack_in, self.work)),
                output: attempt!(self.compiler.signature(&checked.interface.stack_out, self.work)),
                effects: attempt!(plan::effect_mask(&checked.interface.effects)),
            });
        }
        Ok(operations)
    }
}

fn named_site(
    closure: &Closure<'_>, uses: &[UsePrograms], registry: &[usize],
    compiler: &mut Compiler, work: &mut Work,
    node: NodeId, source_span: noble_contracts::Span,
    target: usize, site: &noble_kernel::untrusted::Interface,
) -> Result<UniversalSiteMetadata, crate::Diagnostic> {
    let use_entry = closure.uses().get(target).ok_or(crate::Diagnostic::Invalid)?;
    if site.stack_in != use_entry.checked().interface.stack_in
        || site.stack_out != use_entry.checked().interface.stack_out
        || site.effects != use_entry.checked().interface.effects
    {
        return Err(crate::Diagnostic::Invalid);
    }
    let owner_index = use_entry.owner_index();
    let code = &compiler.universal_code[registry[owner_index]];
    let identity = code.definition_identity;
    let definition = code.definition;
    let entry = code.body_entry;
    Ok(UniversalSiteMetadata {
        node, source_span, action: 2, target_use: Some(target),
        target_program_index: Some(uses[target].root), target_slot: 0,
        input_signature: attempt!(compiler.signature(
            &use_entry.checked().interface.stack_in, work)),
        output_signature: attempt!(compiler.signature(
            &use_entry.checked().interface.stack_out, work)),
        effect_or_witness: 0, target_identity: identity,
        target_definition: definition, target_entry: entry,
    })
}

fn fill(
    closure: &Closure<'_>, registry: &[usize], use_programs: &[UsePrograms],
    compiler: &mut Compiler, programs: &mut [plan::Program], work: &mut Work,
) -> Result<Vec<Vec<UniversalSiteMetadata>>, crate::Diagnostic> {
    let mut sites = vec![Vec::new(); programs.len()];
    let mut lower = Lower { closure, uses: use_programs, registry, compiler, work };
    for (use_index, use_entry) in closure.uses().iter().enumerate() {
        let instance = &use_programs[use_index];
        let candidate = closure.owners()[use_entry.owner_index()].seal().candidate();
        programs[instance.root].operations = attempt!(lower.sequence(
            use_index, &candidate.body, &mut sites[instance.root]));
        for (index, node) in candidate.nodes.iter().enumerate() {
            if let SymbolicNode::Quotation { body, .. } = node {
                let child = instance.quotations[index].ok_or(crate::Diagnostic::Invalid)?;
                programs[child].operations = attempt!(lower.sequence(
                    use_index, body, &mut sites[child]));
            }
        }
    }
    Ok(sites)
}

fn descriptor_data(
    compiler: &mut Compiler, layout: &mut plan::Layout, sites: &mut [Vec<UniversalSiteMetadata>],
    use_programs: &[UsePrograms], closure: &Closure<'_>, registry: &[usize],
    work: &mut Work,
) -> Result<Vec<UniversalProgramMetadata>, crate::Diagnostic> {
    let aligned = compiler.text_end.checked_add(3).ok_or(crate::Diagnostic::Exhausted)? & !3;
    let padding = aligned - compiler.text_end;
    let instance_count = layout.programs.len() - 1;
    let records_count = sites.iter().take(instance_count).try_fold(0usize, |sum, part|
        sum.checked_add(part.len())).ok_or(crate::Diagnostic::Exhausted)?;
    let slots_bytes = instance_count.checked_mul(4).ok_or(crate::Diagnostic::Exhausted)?;
    let records_bytes = records_count.checked_mul(40).ok_or(crate::Diagnostic::Exhausted)?;
    let length = (padding as usize).checked_add(slots_bytes)
        .and_then(|sum| sum.checked_add(records_bytes)).ok_or(crate::Diagnostic::Exhausted)?;
    attempt!(work.entries(length));
    if length > super::MEMORY_BYTES as usize {
        return Err(crate::Diagnostic::Exhausted);
    }
    let mut data = vec![0u8; length];
    let mut records_offset = padding as usize + slots_bytes;
    let mut metadata = Vec::with_capacity(instance_count);
    for (use_index, instance) in use_programs.iter().enumerate() {
        let owner_index = closure.uses()[use_index].owner_index();
        let code = &compiler.universal_code[registry[owner_index]];
        let mut entries = Vec::with_capacity(instance.quotations.len() + 1);
        entries.push((instance.root, None));
        for (node, child) in instance.quotations.iter().enumerate() {
            if let Some(child) = child { entries.push((*child, Some(NodeId(attempt!(number(node)))))); }
        }
        for (program_index, quotation_node) in entries {
            let slot = aligned.checked_add(attempt!(number(program_index.checked_mul(4)
                .ok_or(crate::Diagnostic::Exhausted)?)))
                .ok_or(crate::Diagnostic::Exhausted)?;
            let records = compiler.text_end.checked_add(attempt!(number(records_offset)))
                .ok_or(crate::Diagnostic::Exhausted)?;
            let count = attempt!(number(sites[program_index].len()));
            layout.programs[program_index].universal = Some(plan::UniversalProgram {
                slot, records, count,
                source_identity: code.definition_identity, definition: code.definition.0,
                epoch: compiler.source_epoch_generation,
            });
            for (site_index, site) in sites[program_index].iter_mut().enumerate() {
                if site.action != 3 {
                    let target_index = site.target_program_index.ok_or(crate::Diagnostic::Invalid)?;
                    site.target_slot = aligned.checked_add(attempt!(number(target_index.checked_mul(4)
                        .ok_or(crate::Diagnostic::Exhausted)?)))
                        .ok_or(crate::Diagnostic::Exhausted)?;
                    if site.target_slot == slot && site.action == 1 {
                        return Err(crate::Diagnostic::Invalid);
                    }
                }
                let offset = records_offset.checked_add(site_index.checked_mul(40)
                    .ok_or(crate::Diagnostic::Exhausted)?)
                    .ok_or(crate::Diagnostic::Exhausted)?;
                let row = data.get_mut(offset..offset + 40).ok_or(crate::Diagnostic::Exhausted)?;
                row[0..4].copy_from_slice(&site.node.0.to_le_bytes());
                row[4..8].copy_from_slice(&site.action.to_le_bytes());
                row[8..12].copy_from_slice(&site.target_slot.to_le_bytes());
                row[12..16].copy_from_slice(&site.input_signature.to_le_bytes());
                row[16..20].copy_from_slice(&site.output_signature.to_le_bytes());
                row[20..24].copy_from_slice(&site.effect_or_witness.to_le_bytes());
                row[24..32].copy_from_slice(&site.target_identity.to_le_bytes());
                row[32..36].copy_from_slice(&site.target_definition.0.to_le_bytes());
                row[36..40].copy_from_slice(&site.target_entry.to_le_bytes());
            }
            records_offset = records_offset.checked_add(sites[program_index].len().checked_mul(40)
                .ok_or(crate::Diagnostic::Exhausted)?)
                .ok_or(crate::Diagnostic::Exhausted)?;
            attempt!(work.entries(layout.programs[program_index].operations.len()));
            let atoms = layout.programs[program_index].operations.iter()
                .map(|operation| plan::atom_metadata(operation, 0)).collect();
            let quotation_source_span = match quotation_node {
                Some(node) => Some(closure.owners()[owner_index].lexical_body()
                    .node_span(node).ok_or(crate::Diagnostic::Invalid)?),
                None => None,
            };
            metadata.push(UniversalProgramMetadata {
                use_index, owner_index, program_index, quotation_node,
                quotation_source_span,
                entry: layout.programs[program_index].entry, slot, records,
                input_signature: layout.programs[program_index].input,
                output_signature: layout.programs[program_index].output,
                effect_mask: 0,
                recipe_depth: layout.programs[program_index].depth,
                recipe_leaves: layout.programs[program_index].leaves, atoms,
                sites: core::mem::take(&mut sites[program_index]),
            });
        }
    }
    if records_offset != data.len() { return Err(crate::Diagnostic::Defective); }
    let end = compiler.text_end.checked_add(attempt!(number(data.len())))
        .ok_or(crate::Diagnostic::Exhausted)?;
    if end > super::MEMORY_BYTES { return Err(crate::Diagnostic::Exhausted); }
    layout.data.push((compiler.text_end, data));
    compiler.text_end = end;
    Ok(metadata)
}

fn layout(
    closure: &Closure<'_>, compiler: &mut Compiler, work: &mut Work,
) -> Result<(plan::Layout, UniversalMetadata, CodeSpan), crate::Diagnostic> {
    attempt!(verify_source(closure, work));
    let first_function = compiler.functions;
    let (mut programs, use_programs) = attempt!(allocate_uses(closure, compiler, work));
    let (registry, owners) = attempt!(owner_code(closure, compiler, &use_programs, work));
    attempt!(assign_entries(closure, &registry, compiler, &use_programs, &mut programs));
    let mut sites = attempt!(fill(closure, &registry, &use_programs,
        compiler, &mut programs, work));
    let selected = closure.selected_submission();
    let selected_use = closure.selected_use();
    let [root_node] = selected.body.candidate.body.as_slice() else {
        return Err(crate::Diagnostic::Invalid);
    };
    let wrapper_entry = attempt!(compiler.reserve_code(1)).start;
    let root_index = programs.len();
    let selected_instance = use_programs[selected_use].root;
    let mut wrapper = attempt!(program(compiler,
        &selected.request.expected.stack_in,
        &selected.request.expected.stack_out, work));
    wrapper.entry = wrapper_entry;
    wrapper.emit_code = true;
    wrapper.operations.push(plan::Operation {
        node: *root_node,
        action: plan::Action::Call(selected_instance,
            closure.owners()[closure.uses()[selected_use].owner_index()]
                .program_owner().definition_identity()),
        input: wrapper.input, output: wrapper.output, effects: 0,
    });
    programs.push(wrapper);
    sites.push(Vec::new());
    let mut result = plan::Layout {
        universal: true, programs, definition_roots: Vec::new(), order: Vec::new(),
        first_function, functions: compiler.functions, root: root_index,
        data: Vec::new(), descriptors: Vec::new(), types: super::types::Registry::new(),
        input_types: Vec::new(), output_types: Vec::new(),
        borrowed_input_positions: Vec::new(), text_witness: 0,
        declared_modules: true, declared_text: true, worker_inputs: false,
        live: false, core_live: false, live_slots: false, live_test_hosts: false,
        live_sites: Vec::new(), has_nominals: false,
        has_core_emit: false, has_core_abort: false,
        has_text_byte: false, has_i64_less: false,
        has_bound_emit: false, has_bound_clock: false,
        has_bound_code_export: false, has_live_propose: false,
        has_live_generation: false,
    };
    for program in &result.programs {
        for operation in &program.operations {
            if matches!(operation.action, plan::Action::TextByte) { result.has_text_byte = true; }
            if matches!(operation.action, plan::Action::I64Less) { result.has_i64_less = true; }
        }
    }
    let ordering_cost = result.programs.len().checked_mul(result.programs.len())
        .ok_or(crate::Diagnostic::Exhausted)?;
    attempt!(work.entries(ordering_cost));
    attempt!(plan::order_universal(&mut result));
    let program_metadata = attempt!(descriptor_data(compiler, &mut result, &mut sites,
        &use_programs, closure, &registry, work));
    result.input_types = attempt!(result.types.stack(
        &selected.request.expected.stack_in, compiler, work));
    result.output_types = attempt!(result.types.stack(
        &selected.request.expected.stack_out, compiler, work));
    result.text_witness = attempt!(compiler.signature(&[Ty::Text], work));
    let mut index = compiler.descriptors.len();
    while index < compiler.signatures.keys.len() {
        let bytes = &compiler.signatures.keys[index];
        let descriptor = attempt!(plan::allocation::bytes(&mut compiler.text_end,
            &mut result.data, bytes));
        compiler.descriptors.push(descriptor);
        index += 1;
    }
    result.descriptors = compiler.descriptors.clone();
    let span = CodeSpan {
        start: first_function,
        length: compiler.functions.checked_sub(first_function).ok_or(crate::Diagnostic::Defective)?,
        generation: compiler.generation,
    };
    Ok((result, UniversalMetadata {
        source_epoch: compiler.source_epoch_generation,
        source_generation: closure.source_generation(),
        selected_use, wrapper_program_index: root_index, wrapper_entry,
        owners, programs: program_metadata,
    }, span))
}

impl Compiler {
    /// Only a rederived source owner can authorize reusable code or a concrete
    /// descriptor. Raw submissions keep the historical exact concrete guard.
    pub fn prepare_declared_text_universal(
        &self, source: &noble_contracts::source::DeclaredTextSourceOwner<'_>,
    ) -> Result<Prepared, crate::Diagnostic> {
        if !self.declared_text || !self.text_cursor || self.live || self.declared_live
            || self.live_slots || self.worker_inputs
            || !source.selected_signed_universal_intent()
        { return Err(crate::Diagnostic::Unsupported); }
        let closure = source.rederive_universal().map_err(|_| crate::Diagnostic::Invalid)?;
        let selected = closure.selected_submission();
        let mut work = Work { remaining: u64::from(selected.request.limits.work) };
        attempt!(work.entries(closure.owners().len().saturating_add(closure.uses().len())));
        let mut next = Self {
            live: self.live, declared_live: self.declared_live,
            live_slots: self.live_slots, text_cursor: self.text_cursor,
            declared_text: self.declared_text, worker_inputs: self.worker_inputs,
            generation: self.generation.checked_add(1).ok_or(crate::Diagnostic::Exhausted)?,
            source_epoch_generation: self.source_epoch_generation,
            functions: self.functions, retention_revision: self.retention_revision,
            free_code: self.free_code.clone(), installed_code: self.installed_code.clone(),
            text_end: self.text_end, signatures: self.signatures.clone(),
            descriptors: self.descriptors.clone(), identities: self.identities.clone(),
            universal_code: self.universal_code.clone(),
        };
        let (plan, metadata, code_span) = attempt!(layout(&closure, &mut next, &mut work));
        let wat = attempt!(super::emit::module(&plan, self.generation));
        Ok(Prepared {
            base: super::transaction::Base::capture(self),
            base_live_slots: self.live_slots,
            base_retention_revision: self.retention_revision, next, code_span,
            live_sites: Vec::new(), target_metadata: None, program_metadata: Vec::new(),
            named_targets: Vec::new(), selected_target: None, selected_origin: None,
            resource_catalog: Vec::new(), universal_metadata: Some(metadata), wat,
        })
    }
}
