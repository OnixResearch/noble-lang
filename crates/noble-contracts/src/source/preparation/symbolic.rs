//! The rank-1 owner's original lexical graph is checked against the rigid
//! inferred graph before its definition can enter a staged module. This is a
//! contracts-private source certificate, not a concrete kernel Checked value.

use alloc::{collections::BTreeMap, vec, vec::Vec};
use noble_kernel::{contracts::Definition, words::{Scheme, VariableKind}};

use crate::source::{Kind, SignedRelation, SymbolicNode, Target, Tree};

fn refused(span: crate::Span) -> crate::Diagnostic {
    crate::invalid(span, "signed symbolic owner source and inferred body differ")
}

fn same_variable(left: crate::inference::Variable, right: crate::inference::Variable) -> bool {
    use crate::inference::Variable;
    match (left, right) {
        (Variable::Value(a), Variable::Value(b))
        | (Variable::Stack(a), Variable::Stack(b))
        | (Variable::EffectValue(a), Variable::EffectValue(b)) => a == b,
        (Variable::Effect, Variable::Effect) => true,
        _ => false,
    }
}

fn witness_shape(kinds: &[VariableKind], witnesses: &[crate::inference::Variable]) -> bool {
    use crate::inference::Variable;
    kinds.len() == witnesses.len() && kinds.iter().zip(witnesses).all(|(kind, witness)| {
        matches!((kind, witness),
            (VariableKind::Value, Variable::Value(_))
            | (VariableKind::Stack, Variable::Stack(_))
            | (VariableKind::Effect, Variable::Effect | Variable::EffectValue(_)))
    })
}

fn rigid_owner(
    state: &crate::source::inference::State,
    scheme: &Scheme,
    variables: &[crate::inference::Variable],
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    use crate::inference::{Term, Variable};
    if !witness_shape(&scheme.var_kinds, variables) {
        return Err(refused(span));
    }
    for (ordinal, (kind, variable)) in scheme.var_kinds.iter().zip(variables).enumerate() {
        attempt!(meter.charge(1, span));
        let ordinal = attempt!(crate::index(ordinal, span));
        let sound = match (kind, variable) {
            (VariableKind::Value, Variable::Value(id)) =>
                matches!(state.arena.get(attempt!(state.arena.root(*id, span, meter)), span)?,
                    Term::RigidValue(identity) if identity == ordinal),
            (VariableKind::Stack, Variable::Stack(id)) =>
                matches!(state.arena.get(attempt!(state.arena.root(*id, span, meter)), span)?,
                    Term::RigidStack(identity) if identity == ordinal),
            (VariableKind::Effect, Variable::EffectValue(id)) =>
                state.arena.is_rigid_effect(*id, ordinal),
            _ => false,
        };
        if !sound { return Err(refused(span)); }
    }
    Ok(())
}

fn semantic_count(tree: &Tree) -> Result<usize, crate::Diagnostic> {
    let Some(outer) = tree.nodes.last() else { return Err(refused(tree.span)); };
    // The declared `def name [ ... ]` parser retains its final syntactic
    // quotation node after moving its children into Tree.body, leaving its
    // quotation children empty. It has no executable inference Draft.
    if !matches!(&outer.kind, Kind::Quotation(children) if children.is_empty())
        || outer.span.start == 0 || outer.span.end > tree.span.end
    {
        return Err(refused(tree.span));
    }
    Ok(tree.nodes.len() - 1)
}

fn reachable(tree: &Tree, meter: &mut crate::Meter) -> Result<Vec<u32>, crate::Diagnostic> {
    let mut seen = vec![false; attempt!(semantic_count(tree))];
    let mut pending = Vec::with_capacity(tree.body.len());
    for node in &tree.body { pending.push((*node, 0u32)); }
    while let Some((id, depth)) = pending.pop() {
        attempt!(meter.charge(1, tree.span));
        attempt!(meter.depth(depth, tree.span));
        let index = attempt!(crate::offset(id, tree.span));
        let node = attempt!(tree.node(id));
        let Some(visited) = seen.get_mut(index) else { return Err(refused(tree.span)); };
        if *visited { return Err(refused(node.span)); }
        *visited = true;
        match &node.kind {
            Kind::Quotation(children) => {
                let child_depth = depth.checked_add(1).ok_or_else(|| refused(node.span))?;
                for child in children { pending.push((*child, child_depth)); }
            }
            Kind::Call(Target::SlotInvoke(_)) | Kind::Word(_) | Kind::EditorHole =>
                return Err(refused(node.span)),
            _ => {}
        }
    }
    if seen.iter().any(|visited| !visited) {
        return Err(refused(tree.span));
    }
    let mut self_nodes = Vec::new();
    for (index, node) in tree.nodes.iter().enumerate() {
        attempt!(meter.charge(1, node.span));
        if matches!(node.kind, Kind::Call(Target::SelfOwner)) {
            self_nodes.push(attempt!(crate::index(index, node.span)));
        }
    }
    if self_nodes.is_empty() { return Err(refused(tree.span)); }
    Ok(self_nodes)
}

/// Every source occurrence, including quotation children, has exactly one
/// symbolic draft. A recursion call reuses the owner's rigid variables; no
/// representative concrete instantiation is chosen or materialized here.
pub(super) fn check(
    tree: &Tree,
    state: &crate::source::inference::State,
    scheme: &Scheme,
    source_bytes: &[u8],
    owner: Definition,
    session: &crate::source::Session,
    environment: &noble_kernel::contracts::Env,
    meter: &mut crate::Meter,
) -> Result<SignedRelation, crate::Diagnostic> {
    let body = state.bodies.first().ok_or_else(|| refused(tree.span))?;
    let variables = body.owner_variables.as_ref().ok_or_else(|| refused(tree.span))?;
    let semantic_count = attempt!(semantic_count(tree));
    if body.owner != session.declared.as_ref().and_then(|context| context.owner)
        || body.owner.is_none()
        || state.definition_base != attempt!(crate::offset(owner.0, tree.span))
        || environment.scheme(owner).is_none()
        || body.nodes.len() != semantic_count
        || tree.nodes.len() > usize::try_from(meter.limits.nodes).unwrap_or(usize::MAX)
    {
        return Err(refused(tree.span));
    }
    attempt!(rigid_owner(state, scheme, variables, tree.span, meter));
    let (original, parsed_name) = attempt!(crate::source::parsing::parse_declared(source_bytes, meter));
    if parsed_name.is_none() || original.body != tree.body || original.nodes.len() != tree.nodes.len()
        || original.nodes.last().map(|node| node.span) != tree.nodes.last().map(|node| node.span)
    {
        return Err(refused(tree.span));
    }
    let lexical_nodes = attempt!(reachable(tree, meter));
    let mut spans = BTreeMap::new();
    for (index, (source, raw)) in tree.nodes.iter().zip(&original.nodes)
        .take(semantic_count).enumerate() {
        attempt!(meter.charge(16, source.span));
        if source.span != raw.span || source.span.start >= source.span.end
            || spans.insert((source.span.start, source.span.end), index).is_some()
            || !lexical_kind(source, raw, parsed_name.as_deref(), source_bytes)
            || !attempt!(named_token_matches(source, raw, session, meter))
        {
            return Err(refused(source.span));
        }
    }
    let mut source_to_draft = vec![None; semantic_count];
    for (draft_index, draft) in body.nodes.iter().enumerate() {
        attempt!(meter.charge(16, draft.span));
        let Some(source_index) = spans.remove(&(draft.span.start, draft.span.end)) else {
            return Err(refused(draft.span));
        };
        let source = &tree.nodes[source_index];
        if !draft_matches(source, draft, state, scheme, variables, owner, session, environment, meter)? {
            return Err(refused(source.span));
        }
        source_to_draft[source_index] = Some(attempt!(crate::index(draft_index, draft.span)));
    }
    if !spans.is_empty() { return Err(refused(tree.span)); }
    let mapped = |id: u32| -> Option<u32> {
        source_to_draft.get(usize::try_from(id).ok()?).copied().flatten()
    };
    if !same_edges(&tree.body, &body.root, &mapped) {
        return Err(refused(tree.span));
    }
    let mut symbolic_nodes = Vec::with_capacity(semantic_count);
    for (index, source) in tree.nodes.iter().take(semantic_count).enumerate() {
        attempt!(meter.charge(1, source.span));
        let draft_id = source_to_draft[index].ok_or_else(|| refused(source.span))?;
        if let Kind::Quotation(children) = &source.kind {
            let Some(crate::source::inference::DraftKind::Quotation(drafts)) =
                body.nodes.get(attempt!(crate::offset(draft_id, source.span)))
                    .map(|draft| &draft.kind) else { return Err(refused(source.span)); };
            if !same_edges(children, drafts, &mapped) {
                return Err(refused(source.span));
            }
        }
        symbolic_nodes.push(SymbolicNode { draft: draft_id, span: source.span,
            target: match &source.kind { Kind::Call(target) => Some(*target), _ => None } });
    }
    Ok(SignedRelation { source_slot: attempt!(crate::index(session.definitions.len(), tree.span)),
        symbolic_nodes, lexical_nodes })
}

fn same_edges(source: &[u32], drafts: &[noble_kernel::untrusted::NodeId],
    mapped: &impl Fn(u32) -> Option<u32>) -> bool {
    source.len() == drafts.len() && source.iter().zip(drafts)
        .all(|(original, inferred)| mapped(*original) == Some(inferred.0))
}

fn named_token_matches(source: &crate::source::Node, raw: &crate::source::Node,
    session: &crate::source::Session, meter: &mut crate::Meter
) -> Result<bool, crate::Diagnostic> {
    // A named word can be an imported alias. Bind it against this transaction's
    // declared namespace, not the canonical name of its target definition.
    let (Kind::Call(Target::Named(index)), Kind::Word(word)) = (&source.kind, &raw.kind)
        else { return Ok(true); };
    let Some(context) = session.declared.as_ref() else { return Ok(false); };
    for (name, target) in context.words.iter().rev() {
        attempt!(meter.charge(attempt!(crate::index(
            name.len().saturating_add(word.len()).saturating_add(1), source.span
        )), source.span));
        if name.as_bytes() == word.as_slice() {
            return Ok(*target == Target::Named(*index));
        }
    }
    Ok(false)
}

fn lexical_kind(source: &crate::source::Node, raw: &crate::source::Node,
    name: Option<&str>, bytes: &[u8]) -> bool {
    match (&source.kind, &raw.kind) {
        (Kind::Literal(a), Kind::Literal(b)) => a == b,
        (Kind::Text(a), Kind::Text(b)) => a == b,
        (Kind::Quotation(a), Kind::Quotation(b)) => a == b,
        (Kind::Call(target), Kind::Word(word)) => {
            let start = usize::try_from(raw.span.start).ok();
            let end = usize::try_from(raw.span.end).ok();
            if start.zip(end).and_then(|(a,b)| bytes.get(a..b)) != Some(word.as_slice()) {
                return false;
            }
            match target {
                Target::SelfOwner => name.is_some_and(|n| word.as_slice() == n.as_bytes()),
                Target::Named(_) => true,
                Target::Builtin(_) => true,
                Target::SlotInvoke(_) => false,
            }
        }
        _ => false,
    }
}

fn draft_matches(
    source: &crate::source::Node,
    draft: &crate::source::inference::Draft,
    state: &crate::source::inference::State,
    scheme: &Scheme,
    owner_variables: &[crate::inference::Variable],
    owner: Definition,
    session: &crate::source::Session,
    environment: &noble_kernel::contracts::Env,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    use crate::source::inference::DraftKind;
    let same = match (&source.kind, &draft.kind) {
        (Kind::Literal(a), DraftKind::Literal(b)) =>
            a == b && draft.text.is_none() && draft.variables.len() == 1
                && matches!(draft.variables[0], crate::inference::Variable::Stack(_)),
        (Kind::Text(bytes), DraftKind::Literal(noble_kernel::untrusted::Lit::Text)) =>
            draft.text.as_deref() == Some(bytes.as_slice()) && draft.variables.len() == 1
                && matches!(draft.variables[0], crate::inference::Variable::Stack(_)),
        (Kind::Call(Target::Builtin(id)), DraftKind::Invocation(def)) => {
            let expected = Definition(*id);
            *def == expected && environment.scheme(expected).is_some_and(|scheme|
                witness_shape(&scheme.var_kinds, &draft.variables)) && draft.text.is_none()
        }
        (Kind::Call(Target::Named(index)), DraftKind::Invocation(def)) => {
            let target = session.definitions.get(attempt!(crate::offset(*index, source.span)));
            let body = usize::try_from(def.0).ok()
                .and_then(|id| id.checked_sub(state.definition_base))
                .and_then(|offset| offset.checked_add(1))
                .and_then(|position| state.bodies.get(position));
            match (target, body) {
                (Some(target), Some(body)) => {
                    let exact_variables = if body.nodes.iter().any(|node| {
                        matches!(node.kind, DraftKind::SignedSelf(_))
                    }) {
                        target.signed.is_some()
                            && target.signature.as_ref().is_some_and(|scheme|
                                witness_shape(&scheme.var_kinds, &draft.variables))
                            && body.owner_variables.as_ref().is_some_and(|variables|
                                draft.variables.len() == variables.len()
                                    && draft.variables.iter().copied().zip(variables.iter().copied())
                                        .all(|(left, right)| same_variable(left, right)))
                    } else {
                        draft.variables.is_empty()
                    };
                    body.span == source.span && body.owner_slot == Some(*index)
                        && body.identity == Some(target.identity)
                        && body.owner == target.owner && exact_variables
                        && draft.text.is_none()
                }
                _ => false,
            }
        }
        (Kind::Call(Target::SelfOwner), DraftKind::SignedSelf(def)) =>
            *def == owner && draft.variables.len() == scheme.var_kinds.len()
                && draft.variables.iter().copied().zip(owner_variables.iter().copied())
                    .all(|(a,b)| same_variable(a,b)) && draft.text.is_none(),
        (Kind::Quotation(_), DraftKind::Quotation(_)) =>
            draft.text.is_none() && matches!(draft.variables.as_slice(),
                [crate::inference::Variable::Stack(_), crate::inference::Variable::Stack(_),
                 crate::inference::Variable::Stack(_), crate::inference::Variable::EffectValue(_)]),
        _ => false,
    };
    attempt!(meter.charge(1, source.span));
    Ok(same)
}

/// Check the private certificate against the retained original lexical bytes
/// before a signed-only compiler may rederive any concrete specialization.
pub(in crate::source) fn verify(
    tree: &Tree,
    original: &Tree,
    relation: &SignedRelation,
    name: &str,
    source_bytes: &[u8],
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let n = attempt!(semantic_count(tree));
    if relation.symbolic_nodes.len() != n || original.nodes.len() != tree.nodes.len()
        || original.nodes.last().map(|node| node.span) != tree.nodes.last().map(|node| node.span)
        || original.body != tree.body || relation.lexical_nodes.is_empty() {
        return Err(refused(tree.span));
    }
    let mut drafts = vec![false; n];
    let mut self_nodes = Vec::new();
    for (index, ((node, raw), binding)) in tree.nodes.iter().zip(&original.nodes)
        .zip(&relation.symbolic_nodes).enumerate() {
        attempt!(meter.charge(1, node.span));
        if node.span != raw.span || binding.span != node.span
            || !lexical_kind(node, raw, Some(name), source_bytes)
            || binding.target != (match &node.kind { Kind::Call(target) => Some(*target), _ => None }) {
            return Err(refused(node.span));
        }
        let draft = attempt!(crate::offset(binding.draft, node.span));
        let Some(visited) = drafts.get_mut(draft) else { return Err(refused(node.span)); };
        if *visited { return Err(refused(node.span)); }
        *visited = true;
        if matches!(node.kind, Kind::Call(Target::SelfOwner)) {
            self_nodes.push(attempt!(crate::index(index, node.span)));
        }
    }
    if drafts.iter().any(|seen| !seen) || self_nodes != relation.lexical_nodes
        || attempt!(reachable(tree, meter)) != relation.lexical_nodes {
        return Err(refused(tree.span));
    }
    Ok(())
}
