//! Independent rank-one checking over one rigid, opaque stack prefix.
//!
//! `CheckedScheme` proves a symbolic type/effect derivation, not source identity.
//! The source ingress must authenticate the exact lexical candidate, owner,
//! dependency exports, source bytes, and grant epoch before using this result.
//! `SourceStamp` is a caller-supplied comparison tag, never a credential.

mod fold;
mod instantiate;
mod project;
mod snapshot;

use alloc::boxed::Box;
use alloc::vec::Vec;
use crate::contracts::{Definition, Env};
use crate::types::{EffSet, Ty};
use crate::untrusted::{Interface, Limits, Lit, NodeId, Request};
use crate::words::Inst;

/// A finite stack with either the same abstract bottom row S or no row.
/// No source operation may inspect or manufacture the abstract row.
#[derive(Clone, Debug, PartialEq)]
pub struct SymStack {
    pub prefix: bool,
    pub suffix: Vec<SymTy>,
}

/// A type that may contain a program closed over the same abstract row S.
/// Only the program stacks may mention S, never a value/element/nominal.
#[derive(Clone, Debug, PartialEq)]
pub enum SymTy {
    Concrete(Ty),
    Pair(Box<SymTy>, Box<SymTy>),
    Sum(Box<SymTy>, Box<SymTy>),
    List(Box<SymTy>),
    Program { input: SymStack, output: SymStack, effects: EffSet },
}

#[derive(Clone, Debug, PartialEq)]
pub enum SymBinding {
    Stack(SymStack),
    Value(SymTy),
    Effect(EffSet),
}

/// An explicitly resolved invocation witness, retaining the exact binder order.
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolicInst {
    pub bindings: Vec<SymBinding>,
}

/// One lexical node. The source ingress, not this data, authenticates lexical origin.
#[derive(Clone, Debug)]
pub enum SymbolicNode {
    Literal(Lit),
    Invocation { def: Definition, inst: SymbolicInst },
    SignedSelf { def: Definition },
    Quotation { body: Vec<NodeId>, input: SymStack, output: SymStack, effects: EffSet },
}

#[derive(Clone, Debug)]
pub struct SymbolicCandidate {
    pub nodes: Vec<SymbolicNode>,
    pub body: Vec<NodeId>,
}

/// Caller-supplied freshness tag; possession is NOT authority over source bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceStamp {
    pub source_tag: [u8; 32],
    pub grant_epoch: u64,
    pub generation: u64,
}

/// A dependency previously checked symbolically, with its current source tag.
/// The trusted source ingress must independently authenticate that current tag.
pub struct Dependency<'a> {
    pub seal: &'a CheckedScheme,
    pub current_stamp: SourceStamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemeError {
    UnsupportedShape,
    InvalidOwner,
    InvalidDependency(Definition),
    StaleEnvironment,
    StaleSource,
    InvalidType,
    StackJoin,
    OutputJoin,
    EffectMismatch,
    UnknownEffect,
    InvalidWitness,
    MalformedReference(NodeId),
    DuplicateNode(NodeId),
    UnreachableNode(NodeId),
    BytesExhausted,
    NodesExhausted,
    DepthExhausted,
    TypeSizeExhausted,
    StackHeightExhausted,
    WorkExhausted,
}

#[derive(Clone, Debug, PartialEq)]
struct SymbolicInterface {
    input: SymStack,
    output: SymStack,
    effects: EffSet,
}

#[derive(Clone, Debug)]
struct Derivation {
    id: NodeId,
    before: SymStack,
    after: SymStack,
    interface: SymbolicInterface,
}

/// Opaque witness of an independently folded symbolic body. Cloning does not
/// reauthenticate source or its externally supplied grants.
#[derive(Clone, Debug)]
pub struct CheckedScheme {
    owner: Definition,
    stamp: SourceStamp,
    environment: snapshot::EnvSnapshot,
    dependencies: Vec<(Definition, SourceStamp)>,
    candidate: SymbolicCandidate,
    signature: SymbolicInterface,
    derivations: Vec<Derivation>,
    work_spent: u32,
}

impl CheckedScheme {
    pub fn owner(&self) -> Definition { self.owner }
    pub fn stamp(&self) -> SourceStamp { self.stamp }
    pub fn candidate(&self) -> &SymbolicCandidate { &self.candidate }
    pub fn symbolic_input(&self) -> &SymStack { &self.signature.input }
    pub fn symbolic_output(&self) -> &SymStack { &self.signature.output }
    pub fn effects(&self) -> &EffSet { &self.signature.effects }
    /// Actual request-local work consumed by this successful symbolic check.
    pub fn work_spent(&self) -> u32 { self.work_spent }
}

/// Checked, concrete ABI of a node at exactly one chosen S instantiation.
#[derive(Clone, Debug)]
pub struct CheckedNode {
    pub id: NodeId,
    pub before: Vec<Ty>,
    pub after: Vec<Ty>,
    pub interface: Interface,
    pub kind: CheckedNodeKind,
}

#[derive(Clone, Debug)]
pub enum CheckedNodeKind {
    Literal { lit: Lit, inst: Inst },
    Invocation { def: Definition, inst: Inst },
    SignedSelf { def: Definition, inst: Inst },
    Quotation { body: Vec<NodeId>, inst: Inst },
}

#[derive(Clone, Debug)]
pub struct CheckedInstantiation {
    pub interface: Interface,
    /// One checked record per lexical node, indexed by its `NodeId.0`.
    pub nodes: Vec<CheckedNode>,
    work_spent: u32,
}

impl CheckedInstantiation {
    /// Actual work consumed by this successful concrete ABI derivation.
    pub fn work_spent(&self) -> u32 { self.work_spent }
}

/// Independently fold the actual symbolic body under one request's ORIGINAL
/// work meter. The caller owns lexical/source authentication and its separate
/// frontend per-definition and module node meters.
pub fn check_scheme(
    env: &Env,
    request: &Request,
    candidate: &SymbolicCandidate,
    owner: Definition,
    stamp: SourceStamp,
    dependencies: &[Dependency<'_>],
) -> Result<CheckedScheme, SchemeError> {
    let owner_index = usize::try_from(owner.0).map_err(|_| SchemeError::InvalidOwner)?;
    if env.caller_module.is_none()
        || env.definition_owners.get(owner_index) != Some(&env.caller_module)
        || env.kind(owner) != Some(crate::contracts::Behavior::Named)
        || env.scheme(owner).is_none()
    {
        return Err(SchemeError::InvalidOwner);
    }
    if request.input_bytes > request.limits.bytes { return Err(SchemeError::BytesExhausted); }
    if candidate.nodes.len() > request.limits.nodes as usize { return Err(SchemeError::NodesExhausted); }
    if !request.expected.allowed_effects.as_slice().is_empty() {
        return Err(SchemeError::EffectMismatch);
    }
    // Reuse the original preflight, including environment dependency/schema,
    // nominal and scheme validation. The empty body merely obtains its
    // remaining meter; it never certifies the symbolic candidate.
    let preflight_request = Request {
        input_bytes: request.input_bytes,
        expected: crate::untrusted::Expected {
            stack_in: Vec::new(), stack_out: Vec::new(), allowed_effects: EffSet::empty(),
        },
        limits: request.limits,
    };
    let empty = crate::untrusted::Candidate {
        format: crate::untrusted::CANDIDATE_FORMAT,
        revision: crate::untrusted::SEMANTIC_REVISION,
        nodes: Vec::new(), body: Vec::new(),
    };
    let mut work = crate::acceptance::preflight::check_request(env, &preflight_request, &empty, None)
        .map_err(map_preflight)?;
    let (signature, _) = project::owner_signature(env, owner, &mut work)?;
    let mut checker = fold::Checker::new(env, request.limits, owner, candidate, dependencies, work);
    let derivations = checker.run(&signature)?;
    let used = checker.used_dependencies().to_vec();
    let environment = snapshot::capture(env,owner,candidate,&mut checker.work)?;
    project::charge(&mut checker.work,candidate.nodes.len().saturating_add(
        candidate.body.len()).saturating_add(used.len()))?;
    for node in &candidate.nodes {
        if let SymbolicNode::Quotation {body,..} = node {
            project::charge(&mut checker.work,body.len())?;
        }
    }
    let work_spent=request.limits.work.checked_sub(checker.work)
        .ok_or(SchemeError::WorkExhausted)?;
    Ok(CheckedScheme {
        owner, stamp, environment, dependencies: used,
        candidate: candidate.clone(), signature, derivations, work_spent,
    })
}

/// Recheck current owner/dependency context and instantiate every retained
/// lexical node into concrete ABI. The source ingress MUST authenticate the
/// selected source and grant state independently of caller-supplied stamps.
pub fn instantiate_checked(
    seal: &CheckedScheme,
    env: &Env,
    stamp: SourceStamp,
    owner_inst: &Inst,
    limits: Limits,
    dependencies: &[Dependency<'_>],
) -> Result<CheckedInstantiation, SchemeError> {
    instantiate::instantiate(seal, env, stamp, owner_inst, limits, dependencies)
}

fn map_preflight(fail: crate::acceptance::Fail) -> SchemeError {
    match fail {
        crate::acceptance::Fail::Invalid(_) => SchemeError::InvalidType,
        crate::acceptance::Fail::Unsupported(_) => SchemeError::UnsupportedShape,
        crate::acceptance::Fail::Exhausted(crate::untrusted::LimitKind::Bytes) => SchemeError::BytesExhausted,
        crate::acceptance::Fail::Exhausted(crate::untrusted::LimitKind::Nodes) => SchemeError::NodesExhausted,
        crate::acceptance::Fail::Exhausted(crate::untrusted::LimitKind::Depth) => SchemeError::DepthExhausted,
        crate::acceptance::Fail::Exhausted(crate::untrusted::LimitKind::TypeSize) => SchemeError::TypeSizeExhausted,
        crate::acceptance::Fail::Exhausted(crate::untrusted::LimitKind::StackHeight) => SchemeError::StackHeightExhausted,
        crate::acceptance::Fail::Exhausted(crate::untrusted::LimitKind::Work) => SchemeError::WorkExhausted,
        crate::acceptance::Fail::Exhausted(crate::untrusted::LimitKind::Diagnostics) => SchemeError::UnsupportedShape,
        crate::acceptance::Fail::Internal => SchemeError::UnsupportedShape,
    }
}

fn quote_input(nodes: &[SymbolicNode], id: NodeId) -> Result<&SymStack, SchemeError> {
    let index = usize::try_from(id.0).map_err(|_| SchemeError::MalformedReference(id))?;
    match nodes.get(index) {
        Some(SymbolicNode::Quotation {input,..}) => Ok(input),
        _ => Err(SchemeError::MalformedReference(id)),
    }
}
