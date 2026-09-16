//! The finite acceptance rules for the M2 fragment.
//!
//! Acceptance derives each conclusion from the candidate's premises: node
//! interfaces come from environment schemes through validated instantiation
//! witnesses, joins check complete ordered stack segments, quotation bodies
//! are checked even when unused, and the derived effect bound must fit inside
//! the allowed bound. Every stage charges work before its next bounded step.

use crate::candidate::{
    Candidate, Checked, Constraint, Derivation, Diagnostic, Interface, LimitKind, Lit, Node,
    NodeId, Outcome, Request, UnsupportedKind, CANDIDATE_FORMAT, SEMANTIC_REVISION,
};
use crate::env::{DefId, Env, WordKind};
use crate::scheme::{EffSlot, Inst, InstError, PItem, PSig, PTy, Scheme, VarId, VarKind};
use crate::types::{EffId, EffSet, Ty};
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Run the acceptance checker over one candidate and request.
pub fn check_candidate(env: &Env, request: &Request, candidate: &Candidate) -> Outcome {
    match check(env, request, candidate) {
        Ok(checked) => Outcome::Accepted(checked),
        Err(Fail::Invalid(diagnostic)) => Outcome::Invalid(diagnostic),
        Err(Fail::Unsupported(kind)) => Outcome::Unsupported(kind),
        Err(Fail::Exhausted(limit)) => Outcome::Exhausted(limit),
    }
}

enum Fail {
    Invalid(Diagnostic),
    Unsupported(UnsupportedKind),
    Exhausted(LimitKind),
}

struct Ctx<'a> {
    env: &'a Env,
    request: &'a Request,
    work: u32,
    derivations: Vec<Derivation>,
}

fn check(env: &Env, request: &Request, candidate: &Candidate) -> Result<Checked, Fail> {
    if candidate.format != CANDIDATE_FORMAT || candidate.revision != SEMANTIC_REVISION {
        return Err(Fail::Unsupported(UnsupportedKind::FormatRevision));
    }
    if request.input_bytes > request.limits.bytes {
        return Err(Fail::Exhausted(LimitKind::Bytes));
    }
    if candidate.nodes.len() as u64 > u64::from(request.limits.nodes) {
        return Err(Fail::Exhausted(LimitKind::Nodes));
    }
    for scheme in &env.defs {
        if scheme.validate().is_err() {
            return Err(Fail::Unsupported(UnsupportedKind::SchemeForm));
        }
    }
    let mut ctx = Ctx {
        env,
        request,
        work: request.limits.work,
        derivations: Vec::new(),
    };
    check_stack_limits(&ctx, &request.expected.stack_in)?;
    check_stack_limits(&ctx, &request.expected.stack_out)?;
    for id in request.expected.allowed_effects.as_slice() {
        if !env.knows_effect(*id) {
            return Err(invalid(
                &ctx,
                None,
                None,
                Vec::new(),
                Vec::new(),
                Constraint::UnknownEffect(*id),
            ));
        }
    }

    let mut stack = request.expected.stack_in.clone();
    let mut effects = EffSet::empty();
    for node_id in &candidate.body {
        let interface = check_node(&mut ctx, candidate, *node_id, 0)?;
        join(
            &mut ctx,
            &mut stack,
            &interface,
            *node_id,
            node_def(candidate, *node_id),
        )?;
        effects = effects.union(&interface.effects);
        ctx.derivations.push(Derivation {
            node: *node_id,
            interface,
        });
    }
    if stack != request.expected.stack_out {
        return Err(invalid(
            &ctx,
            None,
            None,
            request.expected.stack_out.clone(),
            stack.clone(),
            mismatch_constraint(&request.expected.stack_out, &stack),
        ));
    }
    if !effects.is_subset_of(&request.expected.allowed_effects) {
        let extra = first_extra(&effects, &request.expected.allowed_effects).unwrap_or(EffId(0));
        return Err(invalid(
            &ctx,
            None,
            None,
            Vec::new(),
            Vec::new(),
            Constraint::EffectInclusion(extra),
        ));
    }
    Ok(Checked {
        interface: Interface {
            stack_in: request.expected.stack_in.clone(),
            stack_out: request.expected.stack_out.clone(),
            effects,
        },
        derivations: ctx.derivations,
    })
}

fn check_node(
    ctx: &mut Ctx,
    candidate: &Candidate,
    node_id: NodeId,
    depth: u32,
) -> Result<Interface, Fail> {
    if depth > ctx.request.limits.depth {
        return Err(Fail::Exhausted(LimitKind::Depth));
    }
    charge(ctx, 1)?;
    let node = match candidate.nodes.get(node_id.0 as usize) {
        Some(node) => node,
        None => {
            return Err(invalid(
                ctx,
                Some(node_id),
                None,
                Vec::new(),
                Vec::new(),
                Constraint::MalformedReference(node_id),
            ))
        }
    };
    match node {
        Node::Literal { lit, inst } => {
            let scheme = literal_scheme(*lit);
            instantiate(ctx, &scheme, inst, None, node_id, None)
        }
        Node::Invocation { def, inst } => {
            let scheme = match ctx.env.scheme(*def) {
                Some(scheme) => scheme.clone(),
                None => {
                    return Err(invalid(
                        ctx,
                        Some(node_id),
                        Some(*def),
                        Vec::new(),
                        Vec::new(),
                        Constraint::UnknownDefinition(*def),
                    ))
                }
            };
            let data_var = match ctx.env.kind(*def) {
                Some(WordKind::Dup) | Some(WordKind::Drop) | Some(WordKind::Quote) => {
                    Some(VarId(1))
                }
                _ => None,
            };
            instantiate(ctx, &scheme, inst, data_var, node_id, Some(*def))
        }
        Node::Quotation { body, inst } => {
            let scheme = quotation_scheme();
            let interface = instantiate(ctx, &scheme, inst, None, node_id, None)?;
            let start = inst.stack(VarId(1)).unwrap_or(&[]).to_vec();
            let claimed_out = inst.stack(VarId(2)).unwrap_or(&[]).to_vec();
            let claimed_effects = inst
                .effects(VarId(3))
                .cloned()
                .unwrap_or_else(EffSet::empty);
            let mut stack = start;
            let mut derived = EffSet::empty();
            for inner in body {
                let interface = check_node(ctx, candidate, *inner, depth.saturating_add(1))?;
                join(
                    ctx,
                    &mut stack,
                    &interface,
                    *inner,
                    node_def(candidate, *inner),
                )?;
                derived = derived.union(&interface.effects);
                ctx.derivations.push(Derivation {
                    node: *inner,
                    interface,
                });
            }
            if stack != claimed_out {
                return Err(invalid(
                    ctx,
                    Some(node_id),
                    None,
                    claimed_out,
                    stack,
                    Constraint::StackJoin,
                ));
            }
            if !derived.is_subset_of(&claimed_effects) {
                let extra = first_extra(&derived, &claimed_effects).unwrap_or(EffId(0));
                return Err(invalid(
                    ctx,
                    Some(node_id),
                    None,
                    Vec::new(),
                    Vec::new(),
                    Constraint::EffectInclusion(extra),
                ));
            }
            Ok(interface)
        }
    }
}

fn instantiate(
    ctx: &mut Ctx,
    scheme: &Scheme,
    inst: &Inst,
    data_var: Option<VarId>,
    node_id: NodeId,
    def: Option<DefId>,
) -> Result<Interface, Fail> {
    let max_effects = ctx.env.effects.len() as u32;
    match scheme.check_inst(
        inst,
        ctx.request.limits.stack_height,
        ctx.request.limits.type_size,
        max_effects,
    ) {
        Ok(()) => {}
        Err(InstError::KindMismatch) | Err(InstError::UnknownVariable) => {
            return Err(invalid(
                ctx,
                Some(node_id),
                def,
                Vec::new(),
                Vec::new(),
                Constraint::InstantiationKind,
            ))
        }
        Err(InstError::ArityMismatch) => {
            return Err(invalid(
                ctx,
                Some(node_id),
                def,
                Vec::new(),
                Vec::new(),
                Constraint::InstantiationArity,
            ))
        }
        Err(InstError::OversizedStack) => return Err(Fail::Exhausted(LimitKind::StackHeight)),
        Err(InstError::OversizedType) | Err(InstError::OversizedEffects) => {
            return Err(Fail::Exhausted(LimitKind::TypeSize))
        }
    }
    charge(
        ctx,
        (scheme.stack_in.len() + scheme.stack_out.len() + 1) as u32,
    )?;
    let stack_in = match scheme.subst_stack(&scheme.stack_in, inst) {
        Ok(stack) => stack,
        Err(_) => {
            return Err(invalid(
                ctx,
                Some(node_id),
                def,
                Vec::new(),
                Vec::new(),
                Constraint::InstantiationKind,
            ))
        }
    };
    let stack_out = match scheme.subst_stack(&scheme.stack_out, inst) {
        Ok(stack) => stack,
        Err(_) => {
            return Err(invalid(
                ctx,
                Some(node_id),
                def,
                Vec::new(),
                Vec::new(),
                Constraint::InstantiationKind,
            ))
        }
    };
    let effects = match scheme.subst_effects(&scheme.effects, inst) {
        Ok(effects) => effects,
        Err(_) => {
            return Err(invalid(
                ctx,
                Some(node_id),
                def,
                Vec::new(),
                Vec::new(),
                Constraint::InstantiationKind,
            ))
        }
    };
    check_stack_limits(ctx, &stack_in)?;
    check_stack_limits(ctx, &stack_out)?;
    for id in effects.as_slice() {
        if !ctx.env.knows_effect(*id) {
            return Err(invalid(
                ctx,
                Some(node_id),
                def,
                Vec::new(),
                Vec::new(),
                Constraint::UnknownEffect(*id),
            ));
        }
    }
    if let Some(var) = data_var {
        match inst.value(var) {
            Some(ty) => {
                if !ty.is_data() {
                    let considered = ty.clone();
                    return Err(invalid(
                        ctx,
                        Some(node_id),
                        def,
                        Vec::new(),
                        Vec::new(),
                        Constraint::Eligibility(considered),
                    ));
                }
            }
            None => {
                return Err(invalid(
                    ctx,
                    Some(node_id),
                    def,
                    Vec::new(),
                    Vec::new(),
                    Constraint::InstantiationKind,
                ))
            }
        }
    }
    Ok(Interface {
        stack_in,
        stack_out,
        effects,
    })
}

fn join(
    ctx: &mut Ctx,
    stack: &mut Vec<Ty>,
    interface: &Interface,
    node_id: NodeId,
    def: Option<DefId>,
) -> Result<(), Fail> {
    charge(
        ctx,
        (interface.stack_in.len() + interface.stack_out.len() + 1) as u32,
    )?;
    let needed = interface.stack_in.len();
    let mut matches = stack.len() >= needed;
    if matches {
        let offset = stack.len() - needed;
        for (index, ty) in interface.stack_in.iter().enumerate() {
            if &stack[offset + index] != ty {
                matches = false;
                break;
            }
        }
    }
    if !matches {
        let actual = tail_copy(stack, needed);
        let constraint = mismatch_constraint(&interface.stack_in, &actual);
        return Err(invalid(
            ctx,
            Some(node_id),
            def,
            interface.stack_in.clone(),
            actual,
            constraint,
        ));
    }
    let keep = stack.len() - needed;
    stack.truncate(keep);
    stack.extend_from_slice(&interface.stack_out);
    check_stack_limits(ctx, stack)?;
    Ok(())
}

fn charge(ctx: &mut Ctx, cost: u32) -> Result<(), Fail> {
    ctx.work = match ctx.work.checked_sub(cost) {
        Some(work) => work,
        None => return Err(Fail::Exhausted(LimitKind::Work)),
    };
    Ok(())
}

fn check_stack_limits(ctx: &Ctx, stack: &[Ty]) -> Result<(), Fail> {
    if stack.len() as u64 > u64::from(ctx.request.limits.stack_height) {
        return Err(Fail::Exhausted(LimitKind::StackHeight));
    }
    for ty in stack {
        if ty.size() > ctx.request.limits.type_size {
            return Err(Fail::Exhausted(LimitKind::TypeSize));
        }
    }
    Ok(())
}

fn node_def(candidate: &Candidate, node_id: NodeId) -> Option<DefId> {
    match candidate.nodes.get(node_id.0 as usize) {
        Some(Node::Invocation { def, .. }) => Some(*def),
        _ => None,
    }
}

fn mismatch_constraint(expected: &[Ty], actual: &[Ty]) -> Constraint {
    if expected.len() == actual.len() && same_multiset(expected, actual) {
        Constraint::StackOrder
    } else {
        Constraint::StackJoin
    }
}

fn tail_copy(stack: &[Ty], needed: usize) -> Vec<Ty> {
    if stack.len() >= needed {
        stack[stack.len() - needed..].to_vec()
    } else {
        stack.to_vec()
    }
}

fn same_multiset(a: &[Ty], b: &[Ty]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut used = alloc::vec![false; b.len()];
    for item in a {
        let mut found = false;
        for (index, other) in b.iter().enumerate() {
            if !used[index] && other == item {
                used[index] = true;
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

fn first_extra(derived: &EffSet, allowed: &EffSet) -> Option<EffId> {
    derived
        .as_slice()
        .iter()
        .find(|id| !allowed.contains(**id))
        .copied()
}

fn invalid(
    ctx: &Ctx,
    node: Option<NodeId>,
    def: Option<DefId>,
    expected: Vec<Ty>,
    actual: Vec<Ty>,
    constraint: Constraint,
) -> Fail {
    let budget = ctx.request.limits.diagnostics as usize;
    let mut truncated = false;
    let mut expected = expected;
    let mut actual = actual;
    if expected.len() + actual.len() > budget {
        truncated = true;
        let keep_expected = budget.saturating_sub(actual.len());
        expected.truncate(keep_expected);
        if expected.len() + actual.len() > budget {
            let keep_actual = budget.saturating_sub(expected.len());
            actual.truncate(keep_actual);
        }
    }
    Fail::Invalid(Diagnostic {
        node,
        def,
        expected,
        actual,
        constraint,
        provenance_available: false,
        truncated,
    })
}

fn literal_scheme(lit: Lit) -> Scheme {
    let pty = match lit {
        Lit::I64(_) => PTy::I64,
        Lit::Bool(_) => PTy::Bool,
        Lit::Text => PTy::Text,
        Lit::Unit => PTy::Unit,
    };
    Scheme {
        var_kinds: alloc::vec![VarKind::Stack],
        stack_in: alloc::vec![PItem::Stack(VarId(0))],
        stack_out: alloc::vec![PItem::Stack(VarId(0)), PItem::Ty(pty)],
        effects: alloc::vec![],
    }
}

fn quotation_scheme() -> Scheme {
    Scheme {
        var_kinds: alloc::vec![
            VarKind::Stack,
            VarKind::Stack,
            VarKind::Stack,
            VarKind::Effect
        ],
        stack_in: alloc::vec![PItem::Stack(VarId(0))],
        stack_out: alloc::vec![
            PItem::Stack(VarId(0)),
            PItem::Ty(PTy::Program(Box::new(PSig {
                stack_in: alloc::vec![PItem::Stack(VarId(1))],
                stack_out: alloc::vec![PItem::Stack(VarId(2))],
                effects: alloc::vec![EffSlot::Var(VarId(3))],
            }))),
        ],
        effects: alloc::vec![],
    }
}
