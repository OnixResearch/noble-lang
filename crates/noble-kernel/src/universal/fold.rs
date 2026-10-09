use alloc::vec;
use alloc::vec::Vec;
use super::{Dependency, Derivation, SchemeError, SymStack, SymTy, SymbolicCandidate,
    SymbolicInterface, SymbolicNode, SourceStamp};
use super::{project, snapshot};
use crate::contracts::{Behavior, Definition, Env};
use crate::types::EffSet;
use crate::untrusted::{Limits, NodeId};

struct Frame<'a> {
    body: &'a [NodeId],
    index: usize,
    current: SymStack,
    effects: EffSet,
    expected: SymStack,
    origin: Option<(NodeId, SymStack, EffSet)>,
}

pub(super) struct Checker<'a> {
    env: &'a Env,
    limits: Limits,
    owner: Definition,
    candidate: &'a SymbolicCandidate,
    dependencies: &'a [Dependency<'a>],
    pub(super) work: u32,
    used: Vec<(Definition, SourceStamp)>,
    seen: Vec<bool>,
    derivations: Vec<Option<Derivation>>,
}

impl<'a> Checker<'a> {
    pub(super) fn new(
        env: &'a Env, limits: Limits, owner: Definition,
        candidate: &'a SymbolicCandidate, dependencies: &'a [Dependency<'a>], work: u32,
    ) -> Self {
        Self { env, limits, owner, candidate, dependencies, work,
            used: Vec::new(), seen: vec![false; candidate.nodes.len()],
            derivations: vec![None; candidate.nodes.len()] }
    }

    pub(super) fn used_dependencies(&self) -> &[(Definition, SourceStamp)] { &self.used }

    pub(super) fn run(&mut self, signature: &SymbolicInterface)
        -> Result<Vec<Derivation>, SchemeError>
    {
        project::validate_stack(&signature.input,self.env,self.limits,&mut self.work)?;
        project::validate_stack(&signature.output,self.env,self.limits,&mut self.work)?;
        let mut frames = vec![Frame {
            body: &self.candidate.body, index: 0,
            current: signature.input.clone(), effects: EffSet::empty(),
            expected: signature.output.clone(), origin: None,
        }];
        while !frames.is_empty() {
            if frames.last().is_some_and(|frame| frame.index == frame.body.len()) {
                let finished = frames.pop().ok_or(SchemeError::UnsupportedShape)?;
                if finished.current != finished.expected { return Err(SchemeError::OutputJoin); }
                if !finished.effects.as_slice().is_empty() { return Err(SchemeError::EffectMismatch); }
                if let Some((id,before,latent)) = finished.origin {
                    let parent = frames.last_mut().ok_or(SchemeError::UnsupportedShape)?;
                    let program = SymTy::Program {
                        input: project::normalized_stack(super::quote_input(&self.candidate.nodes,id)?),
                        output: project::normalized_stack(&finished.expected), effects: latent,
                    };
                    let interface = SymbolicInterface {
                        input: SymStack {prefix:false,suffix:Vec::new()},
                        output: SymStack {prefix:false,suffix:vec![project::normalized_ty(&program)]},
                        effects: EffSet::empty(),
                    };
                    let after = join(parent,&interface,self.env,self.limits,&mut self.work)?;
                    record(&mut self.derivations,id,before,after,interface)?;
                }
                continue;
            }
            let id = {
                let frame = frames.last_mut().ok_or(SchemeError::UnsupportedShape)?;
                let id = frame.body[frame.index];
                frame.index += 1;
                id
            };
            let index = usize::try_from(id.0).map_err(|_| SchemeError::MalformedReference(id))?;
            let node = self.candidate.nodes.get(index).ok_or(SchemeError::MalformedReference(id))?;
            if self.seen[index] { return Err(SchemeError::DuplicateNode(id)); }
            self.seen[index] = true;
            // Body occurrences, nested quotations and structural projections
            // all consume the one remaining request meter.
            project::charge(&mut self.work,1)?;
            match node {
                SymbolicNode::Literal(lit) => {
                    let before = frames.last().ok_or(SchemeError::UnsupportedShape)?.current.clone();
                    let interface = SymbolicInterface {
                        input: SymStack {prefix:false,suffix:Vec::new()},
                        output: SymStack {prefix:false,suffix:vec![SymTy::Concrete(lit.ty())]},
                        effects: EffSet::empty(),
                    };
                    let frame = frames.last_mut().ok_or(SchemeError::UnsupportedShape)?;
                    let after = join(frame,&interface,self.env,self.limits,&mut self.work)?;
                    record(&mut self.derivations,id,before,after,interface)?;
                }
                SymbolicNode::Invocation {def,inst} => {
                    let before = frames.last().ok_or(SchemeError::UnsupportedShape)?.current.clone();
                    let kind = self.env.kind(*def).ok_or(SchemeError::InvalidWitness)?;
                    if kind == Behavior::Reflect { return Err(SchemeError::UnsupportedShape); }
                    if *def == self.owner
                        || !crate::acceptance::nodes::visibility::visible_behavior(self.env,*def,kind)
                    { return Err(SchemeError::InvalidOwner); }
                    if kind == Behavior::Named {
                        let owner_index = usize::try_from(self.owner.0).map_err(|_| SchemeError::InvalidOwner)?;
                        if !self.env.deps.get(owner_index).is_some_and(|ids| ids.contains(def)) {
                            return Err(SchemeError::InvalidDependency(*def));
                        }
                        if !self.used.iter().any(|(used,_)| used == def) {
                            let seal = snapshot::dependency(*def,self.env,self.dependencies,&mut self.work)?;
                            self.used.push((*def,seal.stamp()));
                        }
                    }
                    let scheme = self.env.scheme(*def).ok_or(SchemeError::InvalidWitness)?;
                    let needs_data = matches!(kind,Behavior::Dup | Behavior::Drop | Behavior::Quote);
                    let interface = project::project_invocation(
                        scheme,inst,self.env,self.limits,&mut self.work,needs_data)?;
                    if !interface.effects.as_slice().is_empty() { return Err(SchemeError::EffectMismatch); }
                    let frame = frames.last_mut().ok_or(SchemeError::UnsupportedShape)?;
                    let after = join(frame,&interface,self.env,self.limits,&mut self.work)?;
                    record(&mut self.derivations,id,before,after,interface)?;
                }
                SymbolicNode::SignedSelf {def} => {
                    if *def != self.owner { return Err(SchemeError::InvalidOwner); }
                    let frame = frames.last_mut().ok_or(SchemeError::UnsupportedShape)?;
                    let before = frame.current.clone();
                    // A recursive call must use the SAME S, not silently
                    // instantiate the owner's S to S plus a hidden suffix.
                    if before != signature.input { return Err(SchemeError::StackJoin); }
                    project::charge(&mut self.work,
                        signature.input.suffix.len().saturating_add(signature.output.suffix.len()).saturating_add(1))?;
                    frame.current = signature.output.clone();
                    project::validate_stack(&frame.current,self.env,self.limits,&mut self.work)?;
                    record(&mut self.derivations,id,before,signature.output.clone(),signature.clone())?;
                }
                SymbolicNode::Quotation {body,input,output,effects} => {
                    if frames.len() > self.limits.depth as usize { return Err(SchemeError::DepthExhausted); }
                    if !effects.as_slice().is_empty() { return Err(SchemeError::EffectMismatch); }
                    project::validate_stack(input,self.env,self.limits,&mut self.work)?;
                    project::validate_stack(output,self.env,self.limits,&mut self.work)?;
                    project::charge(&mut self.work,body.len().saturating_add(4))?;
                    let before = frames.last().ok_or(SchemeError::UnsupportedShape)?.current.clone();
                    frames.push(Frame {body,index:0,current:project::normalized_stack(input),
                        effects:EffSet::empty(),expected:project::normalized_stack(output),
                        origin:Some((id,before,effects.clone()))});
                }
            }
        }
        // Disallow dormant arena entries carrying a different unverified
        // definition, effect, source body or lexical node identity.
        let mut out = Vec::with_capacity(self.derivations.len());
        for (index,site) in self.derivations.iter_mut().enumerate() {
            out.push(site.take().ok_or(SchemeError::UnreachableNode(NodeId(index as u32)))?);
        }
        Ok(out)
    }

}

fn record(
    derivations:&mut [Option<Derivation>],id:NodeId,before:SymStack,after:SymStack,
    interface:SymbolicInterface,
) -> Result<(),SchemeError> {
    let index = usize::try_from(id.0).map_err(|_| SchemeError::MalformedReference(id))?;
    let slot = derivations.get_mut(index).ok_or(SchemeError::MalformedReference(id))?;
    if slot.is_some() { return Err(SchemeError::DuplicateNode(id)); }
    *slot = Some(Derivation {id,before,after,interface});
    Ok(())
}

fn join(frame:&mut Frame<'_>, interface:&SymbolicInterface,env:&Env,limits:Limits,work:&mut u32)
    -> Result<SymStack,SchemeError>
{
    project::charge(work, interface.input.suffix.len().saturating_add(
        interface.output.suffix.len()).saturating_add(1))?;
    let current = &mut frame.current;
    if interface.input.prefix {
        if current != &interface.input || !interface.output.prefix { return Err(SchemeError::StackJoin); }
        *current = interface.output.clone();
    } else {
        let len = interface.input.suffix.len();
        if current.suffix.len() < len || interface.output.prefix
            || current.suffix[current.suffix.len()-len..] != interface.input.suffix
        { return Err(SchemeError::StackJoin); }
        current.suffix.truncate(current.suffix.len()-len);
        current.suffix.extend_from_slice(&interface.output.suffix);
    }
    frame.effects = frame.effects.union(&interface.effects);
    project::validate_stack(current,env,limits,work)?;
    Ok(current.clone())
}
