use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use super::{SchemeError, SymBinding, SymStack, SymTy, SymbolicInst, SymbolicInterface};
use crate::contracts::{Definition, Env};
use crate::shapes::{EffectSlot, Pattern};
use crate::types::{EffId, EffSet, Ty};
use crate::untrusted::Limits;
use crate::words::{Binding, Inst, Scheme, Variable, VariableKind};

pub(super) fn charge(work: &mut u32, cost: usize) -> Result<(), SchemeError> {
    let count = u32::try_from(cost).map_err(|_| SchemeError::WorkExhausted)?;
    *work = work.checked_sub(count).ok_or(SchemeError::WorkExhausted)?;
    Ok(())
}

pub(super) fn owner_signature<'a>(env: &'a Env, owner: Definition, work: &mut u32)
    -> Result<(SymbolicInterface, &'a Scheme), SchemeError>
{
    let scheme = env.scheme(owner).ok_or(SchemeError::InvalidOwner)?;
    charge(work,scheme.var_kinds.len().saturating_add(scheme.stack_in.len())
        .saturating_add(scheme.stack_out.len()).saturating_add(scheme.effects.len()).saturating_add(1))?;
    if scheme.validate().is_err() || scheme.var_kinds != [VariableKind::Stack]
        || !scheme.effects.is_empty()
        || !matches!(scheme.stack_in.first(), Some(Pattern::StackVar(Variable(0))))
        || !matches!(scheme.stack_out.first(), Some(Pattern::StackVar(Variable(0))))
    {
        return Err(SchemeError::UnsupportedShape);
    }
    for pattern in scheme.stack_in[1..].iter().chain(scheme.stack_out[1..].iter()) {
        if !closed_pattern(pattern) { return Err(SchemeError::UnsupportedShape); }
        charge(work,pattern_size(pattern))?;
    }
    let empty = Inst { bindings: vec![Binding::Stack(Vec::new())] };
    let input = scheme.subst_stack(&scheme.stack_in[1..], &empty)
        .map_err(|_| SchemeError::InvalidType)?;
    let output = scheme.subst_stack(&scheme.stack_out[1..], &empty)
        .map_err(|_| SchemeError::InvalidType)?;
    Ok((SymbolicInterface {
        input: SymStack { prefix: true, suffix: input.into_iter().map(SymTy::Concrete).collect() },
        output: SymStack { prefix: true, suffix: output.into_iter().map(SymTy::Concrete).collect() },
        effects: EffSet::empty(),
    }, scheme))
}

/// Only the first stack slot of an owner signature may mention S.
fn closed_pattern(root: &Pattern) -> bool {
    let mut pending = vec![root];
    let mut seen = 0;
    while let Some(next) = pending.pop() {
        seen += 1;
        if seen > 512 { return false; }
        match next {
            Pattern::StackVar(_) | Pattern::Var(_) => return false,
            Pattern::Pair(a, b) | Pattern::Sum(a, b) => { pending.push(a); pending.push(b); }
            Pattern::List(a) => pending.push(a),
            Pattern::Program(input, output, effects)
            | Pattern::LiveRef(input, output, effects) => {
                if !effects.iter().all(|e| matches!(e, EffectSlot::Effect(_))) { return false; }
                pending.extend(input.iter());
                pending.extend(output.iter());
            }
            Pattern::GenericNominal(_, args, _) => pending.extend(args.iter()),
            Pattern::Unit | Pattern::Bool | Pattern::I64 | Pattern::Text
            | Pattern::Syntax | Pattern::Contract | Pattern::Evidence | Pattern::Certified
            | Pattern::Resource(_) | Pattern::Nominal(_, _) => {}
        }
    }
    true
}

pub(super) fn pattern_size(root: &Pattern) -> usize {
    let mut pending = vec![root];
    let mut size = 0usize;
    while let Some(next) = pending.pop() {
        size = size.saturating_add(1);
        match next {
            Pattern::Pair(a,b) | Pattern::Sum(a,b) => {pending.push(a);pending.push(b);}
            Pattern::List(a) => pending.push(a),
            Pattern::Program(input,output,_) | Pattern::LiveRef(input,output,_) => {
                pending.extend(input.iter());pending.extend(output.iter());
            }
            Pattern::GenericNominal(_,args,_) => pending.extend(args.iter()),
            Pattern::Nominal(_,shape) => {
                let payload = match &**shape {
                    crate::types::NominalShape::Opaque(ty) =>
                        ty.size().map(|n| n as usize),
                    crate::types::NominalShape::Variant(left,right) =>
                        left.size().and_then(|a| a.checked_add(right.size()?))
                            .map(|n| n as usize),
                }.unwrap_or(usize::MAX);
                size = size.saturating_add(payload);
            }
            Pattern::Unit | Pattern::Bool | Pattern::I64 | Pattern::Text
            | Pattern::Syntax | Pattern::Contract | Pattern::Evidence | Pattern::Certified
            | Pattern::Resource(_) | Pattern::Var(_) | Pattern::StackVar(_) => {}
        }
    }
    size
}

pub(super) fn validate_stack(
    stack: &SymStack, env: &Env, limits: Limits, work: &mut u32,
) -> Result<(), SchemeError> {
    charge(work, stack.suffix.len().saturating_add(1))?;
    if stack.suffix.len().saturating_add(usize::from(stack.prefix)) > limits.stack_height as usize {
        return Err(SchemeError::StackHeightExhausted);
    }
    for ty in &stack.suffix { validate_ty(ty, env, limits, work, 0)?; }
    Ok(())
}

fn validate_ty(ty: &SymTy, env: &Env, limits: Limits, work: &mut u32, depth: usize)
    -> Result<u32, SchemeError>
{
    charge(work, 1)?;
    if depth >= 512 { return Err(SchemeError::TypeSizeExhausted); }
    let size = match ty {
        SymTy::Concrete(value) => {
            if !env.valid_type(value, 512) { return Err(SchemeError::InvalidType); }
            let size = value.size().ok_or(SchemeError::TypeSizeExhausted)?;
            charge(work,size as usize)?;
            size
        }
        SymTy::Pair(a,b) | SymTy::Sum(a,b) => {
            let left = validate_ty(a,env,limits,work,depth+1)?;
            let right = validate_ty(b,env,limits,work,depth+1)?;
            1u32.checked_add(left).and_then(|n| n.checked_add(right))
                .ok_or(SchemeError::TypeSizeExhausted)?
        }
        SymTy::List(a) => 1u32.checked_add(validate_ty(a,env,limits,work,depth+1)?)
            .ok_or(SchemeError::TypeSizeExhausted)?,
        SymTy::Program { input, output, effects } => {
            for id in effects.as_slice() { if !env.knows_effect(*id) { return Err(SchemeError::UnknownEffect); } }
            let mut total = 1u32;
            for stack in [input,output] {
                if stack.suffix.len().saturating_add(usize::from(stack.prefix)) > limits.stack_height as usize {
                    return Err(SchemeError::StackHeightExhausted);
                }
                charge(work, stack.suffix.len().saturating_add(1))?;
                total = total.checked_add(u32::from(stack.prefix)).ok_or(SchemeError::TypeSizeExhausted)?;
                for child in &stack.suffix {
                    total = total.checked_add(validate_ty(child,env,limits,work,depth+1)?)
                        .ok_or(SchemeError::TypeSizeExhausted)?;
                }
            }
            charge(work, effects.as_slice().len())?;
            total
        }
    };
    if size > limits.type_size || size > 512 { return Err(SchemeError::TypeSizeExhausted); }
    Ok(size)
}

pub(super) fn data_eligible(ty: &SymTy) -> bool {
    match ty {
        SymTy::Concrete(value) => value.is_data(),
        SymTy::Pair(a,b) | SymTy::Sum(a,b) => data_eligible(a) && data_eligible(b),
        SymTy::List(a) => data_eligible(a),
        SymTy::Program { .. } => true,
    }
}

pub(super) fn project_invocation(
    scheme: &Scheme, inst: &SymbolicInst, env: &Env, limits: Limits, work: &mut u32,
    needs_data: bool,
) -> Result<SymbolicInterface, SchemeError> {
    if scheme.validate().is_err() || inst.bindings.len() != scheme.var_kinds.len() {
        return Err(SchemeError::InvalidWitness);
    }
    charge(work, scheme.var_kinds.len().saturating_add(
        scheme.stack_in.len()).saturating_add(scheme.stack_out.len()).saturating_add(1))?;
    for (kind,binding) in scheme.var_kinds.iter().zip(&inst.bindings) {
        match (kind,binding) {
            (VariableKind::Stack,SymBinding::Stack(stack)) => validate_stack(stack,env,limits,work)?,
            (VariableKind::Value,SymBinding::Value(ty)) => { validate_ty(ty,env,limits,work,0)?; }
            (VariableKind::Effect,SymBinding::Effect(set)) => {
                charge(work, set.as_slice().len().saturating_add(1))?;
                for id in set.as_slice() { if !env.knows_effect(*id) {return Err(SchemeError::UnknownEffect);} }
            }
            _ => return Err(SchemeError::InvalidWitness),
        }
    }
    if needs_data && !matches!(inst.bindings.get(1),Some(SymBinding::Value(ty)) if data_eligible(ty)) {
        return Err(SchemeError::InvalidType);
    }
    let input = project_stack(&scheme.stack_in,scheme,inst,env,limits,work)?;
    let output = project_stack(&scheme.stack_out,scheme,inst,env,limits,work)?;
    let effects = project_effects(&scheme.effects,inst,env,work)?;
    Ok(SymbolicInterface { input, output, effects })
}

fn project_stack(
    parts: &[Pattern], scheme: &Scheme, inst: &SymbolicInst,
    env: &Env, limits: Limits, work: &mut u32,
) -> Result<SymStack, SchemeError> {
    let mut out = SymStack { prefix: false, suffix: Vec::new() };
    for part in parts {
        match part {
            Pattern::StackVar(var) => {
                charge(work,1)?;
                let Some(SymBinding::Stack(segment)) = inst.bindings.get(var.0 as usize) else {
                    return Err(SchemeError::InvalidWitness);
                };
                // S must stay the *bottom* row: it cannot be appended after
                // finite values, occur twice, or migrate into a value slot.
                if segment.prefix {
                    if out.prefix || !out.suffix.is_empty() { return Err(SchemeError::UnsupportedShape); }
                    out.prefix = true;
                }
                out.suffix.extend(segment.suffix.iter().map(normalized_ty));
            }
            _ => out.suffix.push(normalized_ty(
                &project_pattern(part,scheme,inst,env,limits,work,0)?)),
        }
    }
    validate_stack(&out,env,limits,work)?;
    Ok(out)
}

fn project_pattern(
    pattern: &Pattern, scheme: &Scheme, inst: &SymbolicInst, env: &Env,
    limits: Limits, work: &mut u32, depth: usize,
) -> Result<SymTy, SchemeError> {
    charge(work,1)?;
    if depth >= 512 { return Err(SchemeError::TypeSizeExhausted); }
    match pattern {
        Pattern::Var(var) => match inst.bindings.get(var.0 as usize) {
            Some(SymBinding::Value(value)) => Ok(value.clone()),
            _ => Err(SchemeError::InvalidWitness),
        },
        Pattern::StackVar(_) => Err(SchemeError::InvalidWitness),
        Pattern::Pair(a,b) => Ok(SymTy::Pair(
            Box::new(project_pattern(a,scheme,inst,env,limits,work,depth+1)?),
            Box::new(project_pattern(b,scheme,inst,env,limits,work,depth+1)?))),
        Pattern::Sum(a,b) => Ok(SymTy::Sum(
            Box::new(project_pattern(a,scheme,inst,env,limits,work,depth+1)?),
            Box::new(project_pattern(b,scheme,inst,env,limits,work,depth+1)?))),
        Pattern::List(a) => Ok(SymTy::List(Box::new(
            project_pattern(a,scheme,inst,env,limits,work,depth+1)?))),
        Pattern::Program(input,output,effects) => {
            let input = project_stack(input,scheme,inst,env,limits,work)?;
            let output = project_stack(output,scheme,inst,env,limits,work)?;
            let effects = project_effects(effects,inst,env,work)?;
            Ok(SymTy::Program { input, output, effects })
        }
        Pattern::LiveRef(_,_,_) => Err(SchemeError::UnsupportedShape),
        Pattern::GenericNominal(id,args,payload_params) if !closed_pattern(pattern) => {
            // Generic payload substitution is only admitted when every
            // involved argument is concrete; a Program<S> nominal payload
            // would require a distinct symbolic nominal representation.
            let decl=env.generic_variant(*id).ok_or(SchemeError::InvalidType)?;
            if decl.payload_params != *payload_params {
                return Err(SchemeError::InvalidType);
            }
            let first=project_pattern(&args[0],scheme,inst,env,limits,work,depth+1)?;
            let second=project_pattern(&args[1],scheme,inst,env,limits,work,depth+1)?;
            if contains_rigid_row(&first) || contains_rigid_row(&second) {
                return Err(SchemeError::UnsupportedShape);
            }
            let first=concrete_ty(&first,&[]).ok_or(SchemeError::InvalidType)?;
            let second=concrete_ty(&second,&[]).ok_or(SchemeError::InvalidType)?;
            let ty=env.generic_instance(*id,[first,second]).ok_or(SchemeError::InvalidType)?;
            Ok(SymTy::Concrete(ty))
        }
        _ => {
            if !closed_pattern(pattern) { return Err(SchemeError::UnsupportedShape); }
            let dummy = Inst { bindings: scheme.var_kinds.iter().map(|kind| match kind {
                VariableKind::Stack => Binding::Stack(Vec::new()),
                VariableKind::Value => Binding::Value(Ty::Unit),
                VariableKind::Effect => Binding::Effect(EffSet::empty()),
            }).collect() };
            let ty = scheme.subst_pattern(pattern,&dummy)
                .map_err(|_| SchemeError::InvalidWitness)?;
            if !env.valid_type(&ty,512) { return Err(SchemeError::InvalidType); }
            Ok(SymTy::Concrete(ty))
        }
    }
}

fn project_effects(slots: &[EffectSlot], inst: &SymbolicInst, env: &Env, work: &mut u32)
    -> Result<EffSet,SchemeError> {
    let mut ids = Vec::<EffId>::new();
    for slot in slots {
        charge(work,1)?;
        match slot {
            EffectSlot::Effect(id) => ids.push(*id),
            EffectSlot::Var(var) => match inst.bindings.get(var.0 as usize) {
                Some(SymBinding::Effect(set)) => ids.extend_from_slice(set.as_slice()),
                _ => return Err(SchemeError::InvalidWitness),
            },
        }
    }
    charge(work,ids.len())?;
    let out = EffSet::from_ids(&ids);
    if out.as_slice().iter().any(|id| !env.knows_effect(*id)) { return Err(SchemeError::UnknownEffect); }
    Ok(out)
}

pub(super) fn concrete_stack(stack: &SymStack, row: &[Ty]) -> Option<Vec<Ty>> {
    let mut out = Vec::with_capacity((if stack.prefix {row.len()} else {0}) + stack.suffix.len());
    if stack.prefix { out.extend_from_slice(row); }
    for ty in &stack.suffix { out.push(concrete_ty(ty,row)?); }
    Some(out)
}

/// Closed aggregates have one canonical representation (`Concrete`), while
/// programs containing the rigid S remain structural at every enclosing
/// level. This is a structural normalization, not finite row sampling.
pub(super) fn normalized_ty(ty: &SymTy) -> SymTy {
    let structure = match ty {
        SymTy::Concrete(value) => return SymTy::Concrete(value.clone()),
        SymTy::Pair(a,b) => SymTy::Pair(Box::new(normalized_ty(a)),Box::new(normalized_ty(b))),
        SymTy::Sum(a,b) => SymTy::Sum(Box::new(normalized_ty(a)),Box::new(normalized_ty(b))),
        SymTy::List(a) => SymTy::List(Box::new(normalized_ty(a))),
        SymTy::Program {input,output,effects} => SymTy::Program {
            input:normalized_stack(input), output:normalized_stack(output), effects:effects.clone(),
        },
    };
    // Substituting an *empty* S proves nothing: it erases the row's
    // occurrence while constructing a plausible closed Program. Inspect
    // the syntax itself, including programs nested under data constructors.
    if contains_rigid_row(&structure) {
        structure
    } else {
        match concrete_ty(&structure,&[]) {
            Some(closed) => SymTy::Concrete(closed),
            None => structure,
        }
    }
}

fn contains_rigid_row(ty: &SymTy) -> bool {
    match ty {
        SymTy::Concrete(_) => false,
        SymTy::Pair(left,right) | SymTy::Sum(left,right) =>
            contains_rigid_row(left) || contains_rigid_row(right),
        SymTy::List(item) => contains_rigid_row(item),
        SymTy::Program {input,output,..} =>
            input.prefix || output.prefix
                || input.suffix.iter().any(contains_rigid_row)
                || output.suffix.iter().any(contains_rigid_row),
    }
}

pub(super) fn normalized_stack(stack: &SymStack) -> SymStack {
    SymStack {prefix:stack.prefix,suffix:stack.suffix.iter().map(normalized_ty).collect()}
}

pub(super) fn concrete_ty(ty: &SymTy, row: &[Ty]) -> Option<Ty> {
    match ty {
        SymTy::Concrete(value) => Some(value.clone()),
        SymTy::Pair(a,b) => Some(Ty::Pair(Box::new(concrete_ty(a,row)?),Box::new(concrete_ty(b,row)?))),
        SymTy::Sum(a,b) => Some(Ty::Sum(Box::new(concrete_ty(a,row)?),Box::new(concrete_ty(b,row)?))),
        SymTy::List(a) => Some(Ty::List(Box::new(concrete_ty(a,row)?))),
        SymTy::Program {input,output,effects} => Some(Ty::program(
            concrete_stack(input,row)?,concrete_stack(output,row)?,effects.clone())),
    }
}
