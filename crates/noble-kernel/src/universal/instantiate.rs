use alloc::vec;
use alloc::vec::Vec;
use super::{CheckedInstantiation, CheckedNode, CheckedNodeKind, CheckedScheme, Dependency,
    SchemeError, SourceStamp, SymBinding, SymStack, SymbolicNode};
use super::{project, snapshot};
use crate::contracts::Env;
use crate::types::{EffSet, Ty};
use crate::untrusted::{Interface, Limits};
use crate::words::{Binding, Inst};

pub(super) fn instantiate(
    seal: &CheckedScheme, env: &Env, stamp: SourceStamp, owner_inst: &Inst,
    limits: Limits, dependencies: &[Dependency<'_>],
) -> Result<CheckedInstantiation,SchemeError> {
    if seal.stamp != stamp { return Err(SchemeError::StaleSource); }
    if seal.derivations.len() > limits.nodes as usize { return Err(SchemeError::NodesExhausted); }
    let mut work = limits.work;
    snapshot::instantiate_context(seal,env,dependencies,&mut work)?;
    let [Binding::Stack(row)] = owner_inst.bindings.as_slice() else {
        // In particular, no Ref may reopen the historical uncharged witness
        // resolver at the universal ingress.
        return Err(SchemeError::InvalidWitness);
    };
    if row.len() > limits.stack_height as usize { return Err(SchemeError::StackHeightExhausted); }
    project::charge(&mut work,row.len().saturating_add(1))?;
    for ty in row { valid_type(ty,env,limits)?; }
    let owner = env.scheme(seal.owner).ok_or(SchemeError::InvalidOwner)?;
    owner.check_inst(owner_inst,limits.stack_height,limits.type_size,
        env.len().ok_or(SchemeError::InvalidWitness)?)
        .map_err(|_| SchemeError::InvalidWitness)?;
    let input = checked_stack(&seal.signature.input,row,env,limits,&mut work)?;
    let output = checked_stack(&seal.signature.output,row,env,limits,&mut work)?;
    if owner.subst_stack(&owner.stack_in,owner_inst).map_err(|_| SchemeError::InvalidWitness)? != input
        || owner.subst_stack(&owner.stack_out,owner_inst).map_err(|_| SchemeError::InvalidWitness)? != output
        || !owner.subst_effects(&owner.effects,owner_inst).map_err(|_| SchemeError::InvalidWitness)?
            .as_slice().is_empty()
    { return Err(SchemeError::InvalidWitness); }
    let mut nodes = Vec::with_capacity(seal.derivations.len());
    for derivation in &seal.derivations {
        project::charge(&mut work,1)?;
        let index = usize::try_from(derivation.id.0)
            .map_err(|_| SchemeError::MalformedReference(derivation.id))?;
        let symbolic = seal.candidate.nodes.get(index)
            .ok_or(SchemeError::MalformedReference(derivation.id))?;
        let before = checked_stack(&derivation.before,row,env,limits,&mut work)?;
        let after = checked_stack(&derivation.after,row,env,limits,&mut work)?;
        let projected = Interface {
            stack_in: checked_stack(&derivation.interface.input,row,env,limits,&mut work)?,
            stack_out: checked_stack(&derivation.interface.output,row,env,limits,&mut work)?,
            effects:derivation.interface.effects.clone(),
        };
        let (kind,interface) = match symbolic {
            SymbolicNode::Literal(lit) => {
                let inst = Inst {bindings:vec![Binding::Stack(before.clone())]};
                let mut expected = before.clone();
                expected.push(lit.ty());
                if projected.stack_in != [] || projected.stack_out != [lit.ty()]
                    || after != expected || !projected.effects.as_slice().is_empty()
                { return Err(SchemeError::InvalidWitness); }
                (CheckedNodeKind::Literal{lit:*lit,inst},Interface {
                    stack_in:before.clone(),stack_out:expected,effects:EffSet::empty(),
                })
            }
            SymbolicNode::Invocation {def,inst} => {
                let behavior=env.kind(*def).ok_or(SchemeError::StaleEnvironment)?;
                if !crate::acceptance::nodes::visibility::visible_behavior(env,*def,behavior) {
                    return Err(SchemeError::StaleEnvironment);
                }
                let scheme = env.scheme(*def).ok_or(SchemeError::InvalidDependency(*def))?;
                let mut bindings = Vec::with_capacity(inst.bindings.len());
                for binding in &inst.bindings {
                    project::charge(&mut work,1)?;
                    bindings.push(match binding {
                        SymBinding::Stack(stack) => Binding::Stack(
                            checked_stack(stack,row,env,limits,&mut work)?),
                        SymBinding::Value(ty) => {
                            let value = project::concrete_ty(ty,row).ok_or(SchemeError::InvalidType)?;
                            valid_type(&value,env,limits)?;
                            Binding::Value(value)
                        }
                        SymBinding::Effect(set) => {
                            if set.as_slice().iter().any(|id| !env.knows_effect(*id)) {
                                return Err(SchemeError::UnknownEffect);
                            }
                            project::charge(&mut work,set.as_slice().len())?;
                            Binding::Effect(set.clone())
                        }
                    });
                }
                let inst = Inst {bindings};
                scheme.check_inst(&inst,limits.stack_height,limits.type_size,
                    env.len().ok_or(SchemeError::InvalidWitness)?)
                    .map_err(|_| SchemeError::InvalidWitness)?;
                let from_scheme = Interface {
                    stack_in: scheme.subst_stack(&scheme.stack_in,&inst)
                        .map_err(|_| SchemeError::InvalidWitness)?,
                    stack_out: scheme.subst_stack(&scheme.stack_out,&inst)
                        .map_err(|_| SchemeError::InvalidWitness)?,
                    effects: scheme.subst_effects(&scheme.effects,&inst)
                        .map_err(|_| SchemeError::InvalidWitness)?,
                };
                if from_scheme.stack_in != projected.stack_in
                    || from_scheme.stack_out != projected.stack_out
                    || from_scheme.effects != projected.effects
                    || !from_scheme.effects.as_slice().is_empty()
                { return Err(SchemeError::InvalidWitness); }
                (CheckedNodeKind::Invocation{def:*def,inst},from_scheme)
            }
            SymbolicNode::SignedSelf {def} => {
                if *def != seal.owner || projected.stack_in != input
                    || projected.stack_out != output || before != input || after != output
                { return Err(SchemeError::InvalidWitness); }
                (CheckedNodeKind::SignedSelf{def:*def,inst:owner_inst.clone()},projected)
            }
            SymbolicNode::Quotation {body,input,output,effects} => {
                if !effects.as_slice().is_empty() { return Err(SchemeError::EffectMismatch); }
                project::charge(&mut work,body.len().saturating_add(4))?;
                let quote_in = checked_stack(input,row,env,limits,&mut work)?;
                let quote_out = checked_stack(output,row,env,limits,&mut work)?;
                let program = Ty::program(quote_in.clone(),quote_out.clone(),effects.clone());
                valid_type(&program,env,limits)?;
                let mut produced = before.clone();
                produced.push(program);
                if projected.stack_in != [] || projected.stack_out.len() != 1
                    || projected.stack_out[0] != *produced.last().ok_or(SchemeError::InvalidWitness)?
                    || projected.effects != EffSet::empty() || after != produced
                { return Err(SchemeError::InvalidWitness); }
                let inst = Inst { bindings:vec![
                    Binding::Stack(before.clone()),Binding::Stack(quote_in),
                    Binding::Stack(quote_out),Binding::Effect(effects.clone()),
                ] };
                (CheckedNodeKind::Quotation {body:body.clone(),inst}, Interface {
                    stack_in:before.clone(),stack_out:produced,effects:EffSet::empty(),
                })
            }
        };
        valid_stack(&interface.stack_in,env,limits)?;
        valid_stack(&interface.stack_out,env,limits)?;
        nodes.push(CheckedNode {id:derivation.id,before,after,interface,kind});
    }
    let work_spent=limits.work.checked_sub(work).ok_or(SchemeError::WorkExhausted)?;
    Ok(CheckedInstantiation {
        interface: Interface {stack_in:input,stack_out:output,effects:EffSet::empty()},nodes,
        work_spent,
    })
}

fn checked_stack(
    symbolic:&SymStack,row:&[Ty],env:&Env,limits:Limits,work:&mut u32,
) -> Result<Vec<Ty>,SchemeError> {
    project::charge(work,symbolic.suffix.len().saturating_add(1)
        .saturating_add(if symbolic.prefix {row.len()} else {0}))?;
    let values = project::concrete_stack(symbolic,row).ok_or(SchemeError::InvalidType)?;
    valid_stack(&values,env,limits)?;
    Ok(values)
}

fn valid_stack(stack:&[Ty],env:&Env,limits:Limits) -> Result<(),SchemeError> {
    if stack.len() > limits.stack_height as usize { return Err(SchemeError::StackHeightExhausted); }
    for ty in stack { valid_type(ty,env,limits)?; }
    Ok(())
}

fn valid_type(ty:&Ty,env:&Env,limits:Limits) -> Result<(),SchemeError> {
    if !env.valid_type(ty,512) { return Err(SchemeError::InvalidType); }
    if ty.size().is_none_or(|size| size > limits.type_size) {
        return Err(SchemeError::TypeSizeExhausted);
    }
    Ok(())
}
