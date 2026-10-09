//! Reify one rigid, pure source body without choosing an instance of its stack row.
//! This is candidate construction only: source authentication and the independent
//! kernel derivation are separate obligations of the caller.

use alloc::{boxed::Box, vec, vec::Vec};

use noble_kernel::{
    shapes::Pattern,
    types::{EffSet, Ty},
    universal::{SymBinding, SymStack, SymTy, SymbolicCandidate, SymbolicInst, SymbolicNode},
    untrusted::{Lit, NodeId},
    words::{Variable as SchemeVariable, VariableKind},
};

use crate::{inference::{Arena, Term, Variable}, source::inference::{Draft, DraftKind, State}};

fn refused(span: crate::Span) -> crate::Diagnostic {
    crate::invalid(span, "unresolved or unsupported universal source candidate")
}

/// The only accepted binder is the same opaque bottom row on both sides.
/// Every visited pattern, including rejected shapes, is charged to the
/// registration's original meter; there is no secondary traversal budget.
pub(super) fn eligible(
    scheme: &noble_kernel::words::Scheme,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    attempt!(meter.charge(attempt!(crate::index(
        scheme.var_kinds.len().saturating_add(scheme.effects.len()).saturating_add(1), span
    )), span));
    if scheme.var_kinds != [VariableKind::Stack] || !scheme.effects.is_empty() {
        return Ok(false);
    }
    let mut visits = 0u32;
    for parts in [&scheme.stack_in, &scheme.stack_out] {
        attempt!(meter.charge(1, span));
        if !matches!(parts.first(), Some(Pattern::StackVar(SchemeVariable(0)))) {
            return Ok(false);
        }
        for pattern in parts.iter().skip(1) {
            if !closed(pattern, span, meter, 0, &mut visits)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn closed(
    pattern: &Pattern,
    span: crate::Span,
    meter: &mut crate::Meter,
    depth: u32,
    visits: &mut u32,
) -> Result<bool, crate::Diagnostic> {
    attempt!(meter.depth(depth, span));
    attempt!(meter.charge(1, span));
    *visits = match visits.checked_add(1) {
        Some(count) if count <= 512 => count,
        _ => return Ok(false),
    };
    match pattern {
        Pattern::Unit | Pattern::Bool | Pattern::I64 | Pattern::Text
        | Pattern::Nominal(..)
        | Pattern::Syntax | Pattern::Contract | Pattern::Evidence | Pattern::Certified =>
            Ok(true),
        Pattern::Pair(left, right) | Pattern::Sum(left, right) => {
            let next = depth.checked_add(1).ok_or_else(|| refused(span))?;
            Ok(closed(left, span, meter, next, visits)?
                && closed(right, span, meter, next, visits)?)
        }
        Pattern::List(item) => {
            let next = depth.checked_add(1).ok_or_else(|| refused(span))?;
            closed(item, span, meter, next, visits)
        }
        Pattern::GenericNominal(_, arguments, _) => {
            let next = depth.checked_add(1).ok_or_else(|| refused(span))?;
            for argument in arguments.iter() {
                if !closed(argument, span, meter, next, visits)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Pattern::Program(input, output, effects) if effects.is_empty() => {
            let next = depth.checked_add(1).ok_or_else(|| refused(span))?;
            for child in input.iter().chain(output.iter()) {
                if !closed(child, span, meter, next, visits)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Pattern::Program(..) | Pattern::LiveRef(..) | Pattern::Resource(_)
        | Pattern::Var(_) | Pattern::StackVar(_) => Ok(false),
    }
}

fn pure_effect(arena: &Arena, id: u32, span: crate::Span, meter: &mut crate::Meter)
    -> Result<EffSet, crate::Diagnostic>
{
    // Solved finite effect bounds alone cannot distinguish an unknown effect
    // hole or an arbitrary rigid set from an actually empty effect expression.
    let (known, unknown) = attempt!(arena.editor_effect(id, span, meter));
    if !known.is_empty() { return Err(refused(span)); }
    if unknown && !attempt!(arena.proven_empty_effect(id, span, meter)) {
        return Err(crate::Diagnostic::new(
            crate::DiagnosticKind::Unsupported, span,
            "unresolved effect cannot establish a universal pure quotation",
        ));
    }
    Ok(EffSet::empty())
}

#[derive(Clone, Copy)]
enum SortToRead { Stack, Value }

enum Step {
    Visit { id: u32, sort: SortToRead, depth: u32, height: u32 },
    Push,
    Pair,
    Sum,
    List,
    GenericNominal(noble_kernel::types::NominalTypeId),
    Program(u32),
}

enum Material { Stack(SymStack), Value(SymTy) }

fn value(values: &mut Vec<Material>, span: crate::Span) -> Result<SymTy, crate::Diagnostic> {
    match values.pop() {
        Some(Material::Value(ty)) => Ok(ty),
        Some(Material::Stack(_)) | None => Err(refused(span)),
    }
}

fn stack(values: &mut Vec<Material>, span: crate::Span) -> Result<SymStack, crate::Diagnostic> {
    match values.pop() {
        Some(Material::Stack(stack)) => Ok(stack),
        Some(Material::Value(_)) | None => Err(refused(span)),
    }
}

fn closed_stack(stack: SymStack, span: crate::Span) -> Result<Vec<Ty>, crate::Diagnostic> {
    if stack.prefix { return Err(refused(span)); }
    let mut values = Vec::with_capacity(stack.suffix.len());
    for ty in stack.suffix {
        let SymTy::Concrete(ty) = ty else { return Err(refused(span)); };
        values.push(ty);
    }
    Ok(values)
}

/// Explicit continuations avoid recursion through either stacks or nested
/// Program types. Each visit, link resolution, continuation, and depth check
/// spends the original preparation meter; no default witness fills a hole.
fn material(
    arena: &Arena, id: u32, sort: SortToRead, span: crate::Span, meter: &mut crate::Meter,
) -> Result<Material, crate::Diagnostic> {
    let mut steps = vec![Step::Visit { id, sort, depth: 0, height: 0 }];
    let mut values = Vec::new();
    while let Some(step) = steps.pop() {
        attempt!(meter.charge(1, span));
        match step {
            Step::Visit { id, sort, depth, height } => {
                attempt!(meter.depth(depth, span));
                let id = attempt!(arena.root(id, span, meter));
                let term = attempt!(arena.get(id, span));
                match (sort, term) {
                    (SortToRead::Stack, Term::RigidStack(0)) => {
                        values.push(Material::Stack(SymStack { prefix: true, suffix: Vec::new() }));
                    }
                    (SortToRead::Stack, Term::Empty) => {
                        values.push(Material::Stack(SymStack { prefix: false, suffix: Vec::new() }));
                    }
                    (SortToRead::Stack, Term::Push(prefix, item)) => {
                        if height >= crate::inference::STACK_CAP { return Err(refused(span)); }
                        steps.push(Step::Push);
                        steps.push(Step::Visit { id: item, sort: SortToRead::Value,
                            depth, height: 0 });
                        steps.push(Step::Visit { id: prefix, sort: SortToRead::Stack,
                            depth, height: height + 1 });
                    }
                    (SortToRead::Value, Term::Unit) => values.push(Material::Value(SymTy::Concrete(Ty::Unit))),
                    (SortToRead::Value, Term::Bool) => values.push(Material::Value(SymTy::Concrete(Ty::Bool))),
                    (SortToRead::Value, Term::I64) => values.push(Material::Value(SymTy::Concrete(Ty::I64))),
                    (SortToRead::Value, Term::Text) => values.push(Material::Value(SymTy::Concrete(Ty::Text))),
                    (SortToRead::Value, Term::Syntax) => values.push(Material::Value(SymTy::Concrete(Ty::Syntax))),
                    (SortToRead::Value, Term::Contract) => values.push(Material::Value(SymTy::Concrete(Ty::Contract))),
                    (SortToRead::Value, Term::Evidence) => values.push(Material::Value(SymTy::Concrete(Ty::Evidence))),
                    (SortToRead::Value, Term::Certified) => values.push(Material::Value(SymTy::Concrete(Ty::Certified))),
                    (SortToRead::Value, Term::Nominal(id)) => {
                        let mut found = None;
                        for (known, shape) in &arena.nominals {
                            attempt!(meter.charge(1, span));
                            if *known == id {
                                let size = match shape {
                                    noble_kernel::types::NominalShape::Opaque(ty) =>
                                        ty.size().ok_or_else(|| refused(span))?,
                                    noble_kernel::types::NominalShape::Variant(a, b) =>
                                        a.size().and_then(|a| a.checked_add(b.size()?))
                                            .ok_or_else(|| refused(span))?,
                                };
                                attempt!(meter.charge(size, span));
                                found = Some(shape.clone());
                                break;
                            }
                        }
                        let shape = found.ok_or_else(|| refused(span))?;
                        values.push(Material::Value(SymTy::Concrete(
                            Ty::Nominal(id, Box::new(shape)),
                        )));
                    }
                    (SortToRead::Value, Term::GenericNominal(id, first, second)) => {
                        let next = depth.checked_add(1).ok_or_else(|| refused(span))?;
                        steps.push(Step::GenericNominal(id));
                        steps.push(Step::Visit { id: second, sort: SortToRead::Value,
                            depth: next, height: 0 });
                        steps.push(Step::Visit { id: first, sort: SortToRead::Value,
                            depth: next, height: 0 });
                    }
                    (SortToRead::Value, Term::Pair(a, b) | Term::Sum(a, b)) => {
                        let next = depth.checked_add(1).ok_or_else(|| refused(span))?;
                        steps.push(if matches!(term, Term::Pair(..)) { Step::Pair } else { Step::Sum });
                        steps.push(Step::Visit { id: b, sort: SortToRead::Value, depth: next, height: 0 });
                        steps.push(Step::Visit { id: a, sort: SortToRead::Value, depth: next, height: 0 });
                    }
                    (SortToRead::Value, Term::List(item)) => {
                        let next = depth.checked_add(1).ok_or_else(|| refused(span))?;
                        steps.push(Step::List);
                        steps.push(Step::Visit { id: item, sort: SortToRead::Value, depth: next, height: 0 });
                    }
                    (SortToRead::Value, Term::Program(input, output)) => {
                        let next = depth.checked_add(1).ok_or_else(|| refused(span))?;
                        steps.push(Step::Program(id));
                        steps.push(Step::Visit { id: output, sort: SortToRead::Stack,
                            depth: next, height: 0 });
                        steps.push(Step::Visit { id: input, sort: SortToRead::Stack,
                            depth: next, height: 0 });
                    }
                    // A different rigid binder, existential hole, host
                    // resource, or sort mismatch is not a proof of S.
                    (_, _) => return Err(refused(span)),
                }
            }
            Step::Push => {
                let item = attempt!(value(&mut values, span));
                let mut prefix = attempt!(stack(&mut values, span));
                prefix.suffix.push(item);
                values.push(Material::Stack(prefix));
            }
            Step::Pair | Step::Sum => {
                let b = attempt!(value(&mut values, span));
                let a = attempt!(value(&mut values, span));
                let ty = match (step, a, b) {
                    (Step::Pair, SymTy::Concrete(a), SymTy::Concrete(b)) =>
                        SymTy::Concrete(Ty::Pair(Box::new(a), Box::new(b))),
                    (Step::Sum, SymTy::Concrete(a), SymTy::Concrete(b)) =>
                        SymTy::Concrete(Ty::Sum(Box::new(a), Box::new(b))),
                    (Step::Pair, a, b) => SymTy::Pair(Box::new(a), Box::new(b)),
                    (Step::Sum, a, b) => SymTy::Sum(Box::new(a), Box::new(b)),
                    _ => return Err(refused(span)),
                };
                values.push(Material::Value(ty));
            }
            Step::List => {
                let item = attempt!(value(&mut values, span));
                values.push(Material::Value(match item {
                    SymTy::Concrete(value) => SymTy::Concrete(Ty::List(Box::new(value))),
                    item => SymTy::List(Box::new(item)),
                }));
            }
            Step::GenericNominal(id) => {
                let second = attempt!(value(&mut values, span));
                let first = attempt!(value(&mut values, span));
                let (SymTy::Concrete(first), SymTy::Concrete(second)) = (first, second)
                    else { return Err(refused(span)); };
                let mut found = None;
                for decl in &arena.generic_nominals {
                    attempt!(meter.charge(1, span));
                    if decl.id == id {
                        found = Some(decl);
                        break;
                    }
                }
                let decl = found.ok_or_else(|| refused(span))?;
                let arguments = [first, second];
                let copied = arguments[0].size().and_then(|a|
                    a.checked_add(arguments[1].size()?))
                    .ok_or_else(|| refused(span))?;
                attempt!(meter.charge(copied, span));
                let left = arguments.get(usize::from(decl.payload_params[0]))
                    .ok_or_else(|| refused(span))?.clone();
                let right = arguments.get(usize::from(decl.payload_params[1]))
                    .ok_or_else(|| refused(span))?.clone();
                let shape = noble_kernel::types::NominalShape::Variant(
                    Box::new(left), Box::new(right),
                );
                values.push(Material::Value(SymTy::Concrete(Ty::GenericNominal(
                    id, Box::new(arguments), Box::new(shape),
                ))));
            }
            Step::Program(id) => {
                let output = attempt!(stack(&mut values, span));
                let input = attempt!(stack(&mut values, span));
                let effect = attempt!(arena.program_effect(id, span, meter));
                let effects = attempt!(pure_effect(arena, effect, span, meter));
                let closed = if input.prefix || output.prefix {
                    false
                } else {
                    let size = input.suffix.len().saturating_add(output.suffix.len());
                    attempt!(meter.charge(attempt!(crate::index(size, span)), span));
                    input.suffix.iter().chain(&output.suffix)
                        .all(|ty| matches!(ty, SymTy::Concrete(_)))
                };
                values.push(Material::Value(if closed {
                    SymTy::Concrete(Ty::program(
                        attempt!(closed_stack(input, span)),
                        attempt!(closed_stack(output, span)),
                        effects,
                    ))
                } else {
                    SymTy::Program { input, output, effects }
                }));
            }
        }
    }
    if values.len() != 1 { return Err(refused(span)); }
    values.pop().ok_or_else(|| refused(span))
}

fn material_stack(arena: &Arena, id: u32, span: crate::Span, meter: &mut crate::Meter)
    -> Result<SymStack, crate::Diagnostic>
{
    match attempt!(material(arena, id, SortToRead::Stack, span, meter)) {
        Material::Stack(stack) => Ok(stack),
        Material::Value(_) => Err(refused(span)),
    }
}

fn material_value(arena: &Arena, id: u32, span: crate::Span, meter: &mut crate::Meter)
    -> Result<SymTy, crate::Diagnostic>
{
    match attempt!(material(arena, id, SortToRead::Value, span, meter)) {
        Material::Value(ty) => Ok(ty),
        Material::Stack(_) => Err(refused(span)),
    }
}

fn instantiation(
    arena: &Arena, variables: &[Variable], span: crate::Span, meter: &mut crate::Meter,
) -> Result<SymbolicInst, crate::Diagnostic> {
    let mut bindings = Vec::with_capacity(variables.len());
    for variable in variables {
        attempt!(meter.charge(1, span));
        bindings.push(match variable {
            Variable::Stack(id) => SymBinding::Stack(attempt!(material_stack(arena, *id, span, meter))),
            Variable::Value(id) => SymBinding::Value(attempt!(material_value(arena, *id, span, meter))),
            Variable::EffectValue(id) => SymBinding::Effect(attempt!(pure_effect(arena, *id, span, meter))),
            Variable::Effect => return Err(refused(span)),
        });
    }
    Ok(SymbolicInst { bindings })
}

fn references(
    original: &[NodeId], count: usize, span: crate::Span, meter: &mut crate::Meter,
) -> Result<Vec<NodeId>, crate::Diagnostic> {
    let mut refs = Vec::with_capacity(original.len());
    for id in original {
        attempt!(meter.charge(1, span));
        if attempt!(crate::offset(id.0, span)) >= count { return Err(refused(span)); }
        // Draft index == candidate index: quotation and entry edges retain
        // the exact occurrence identity, rather than a traversal-order ID.
        refs.push(*id);
    }
    Ok(refs)
}

fn node(
    draft: &Draft, state: &State, owner_stack: u32, count: usize, meter: &mut crate::Meter,
) -> Result<SymbolicNode, crate::Diagnostic> {
    let span = draft.span;
    if draft.text.is_some() { return Err(refused(span)); }
    match &draft.kind {
        DraftKind::Literal(Lit::Text) => Err(refused(span)),
        DraftKind::Literal(lit) => {
            if draft.variables.len() != 1 { return Err(refused(span)); }
            let Some(Variable::Stack(id)) = draft.variables.first() else { return Err(refused(span)); };
            let _ = attempt!(material_stack(&state.arena, *id, span, meter));
            Ok(SymbolicNode::Literal(*lit))
        }
        DraftKind::Invocation(def) => {
            if usize::try_from(def.0).ok().is_none_or(|id| id >= state.definition_base) {
                return Err(refused(span));
            }
            Ok(SymbolicNode::Invocation {
                def: *def,
                inst: attempt!(instantiation(&state.arena, &draft.variables, span, meter)),
            })
        }
        DraftKind::SignedSelf(def) => {
            if def.0 != attempt!(crate::index(state.definition_base, span))
                || !matches!(draft.variables.as_slice(), [Variable::Stack(id)] if *id == owner_stack)
            {
                return Err(refused(span));
            }
            Ok(SymbolicNode::SignedSelf { def: *def })
        }
        DraftKind::Quotation(children) => {
            let [Variable::Stack(surrounding), Variable::Stack(input),
                Variable::Stack(output), Variable::EffectValue(effect)] = draft.variables.as_slice()
                else { return Err(refused(span)); };
            let _ = attempt!(material_stack(&state.arena, *surrounding, span, meter));
            let input = attempt!(material_stack(&state.arena, *input, span, meter));
            let output = attempt!(material_stack(&state.arena, *output, span, meter));
            let effects = attempt!(pure_effect(&state.arena, *effect, span, meter));
            Ok(SymbolicNode::Quotation {
                body: attempt!(references(children, count, span, meter)),
                input, output, effects,
            })
        }
        DraftKind::SlotInvoke { .. } => Err(refused(span)),
    }
}

/// Reify all and only the entry body's reachable occurrences. Source identity,
/// lexical bytes, owner grants, and the resulting kernel check belong to the
/// caller; expanded named bodies cannot masquerade as a sealed dependency.
pub(super) fn candidate(
    state: &crate::source::inference::State, meter: &mut crate::Meter,
) -> Result<noble_kernel::universal::SymbolicCandidate, crate::Diagnostic> {
    let span = state.span;
    let [body] = state.bodies.as_slice() else { return Err(refused(span)); };
    if body.owner.is_none() || !state.holes.is_empty() || !state.borrows.is_empty()
        || state.logical_inputs.is_some() { return Err(refused(span)); }
    let Some([Variable::Stack(owner_stack)]) = body.owner_variables.as_deref() else {
        return Err(refused(span));
    };
    let binder = attempt!(state.arena.root(*owner_stack, span, meter));
    if !matches!(state.arena.get(binder, span), Ok(Term::RigidStack(0))) {
        return Err(refused(span));
    }
    let input = attempt!(material_stack(&state.arena, body.input, body.span, meter));
    let output = attempt!(material_stack(&state.arena, body.output, body.span, meter));
    if !input.prefix || !output.prefix { return Err(refused(body.span)); }
    let _ = attempt!(pure_effect(&state.arena, body.effect, body.span, meter));

    let count = body.nodes.len();
    let root = attempt!(references(&body.root, count, body.span, meter));
    let mut nodes = Vec::with_capacity(count);
    for draft in &body.nodes {
        attempt!(meter.node(draft.span));
        nodes.push(attempt!(node(draft, state, *owner_stack, count, meter)));
    }

    // A draft disconnected from the lexical entry, an edge repeated twice,
    // or a cyclic reference cannot be reinterpreted as a valid source node.
    attempt!(meter.charge(attempt!(crate::index(count, span)), span));
    let mut seen = vec![false; count];
    let mut pending = Vec::with_capacity(root.len());
    for id in &root {
        attempt!(meter.charge(1, span));
        pending.push((*id, 0u32));
    }
    while let Some((id, depth)) = pending.pop() {
        attempt!(meter.depth(depth, span));
        let index = attempt!(crate::offset(id.0, span));
        let Some(visited) = seen.get_mut(index) else { return Err(refused(span)); };
        if *visited { return Err(refused(span)); }
        *visited = true;
        let Some(next) = nodes.get(index) else { return Err(refused(span)); };
        if let SymbolicNode::Quotation { body: children, .. } = next {
            let child_depth = depth.checked_add(1).ok_or_else(|| refused(span))?;
            for child in children {
                attempt!(meter.charge(1, span));
                pending.push((*child, child_depth));
            }
        }
    }
    for visited in seen {
        attempt!(meter.charge(1, span));
        if !visited { return Err(refused(span)); }
    }
    Ok(SymbolicCandidate { nodes, body: root })
}
