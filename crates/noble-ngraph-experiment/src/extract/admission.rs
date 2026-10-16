//! D7 stage 1: admission of an exact, independently kernel-accepted subject.

use noble_kernel::contracts::Behavior;
use noble_kernel::execution::Submission;
use noble_kernel::types::Ty;
use noble_kernel::untrusted::{Candidate, Checked, Interface, Node, NodeId, Outcome};

use crate::diag::{Code, Diagnostic, Site};
use crate::ir::{ScalarType, SourceRef, SubProfile, Subject, SubjectKind};

use super::Budget;

/// One admission request (D7 stage 1 inputs besides the subject).
#[derive(Clone, Debug)]
pub struct Request<'a> {
    /// Requested sub-profile name; only `ngraph-i64/0` has source extraction
    /// (approval P2, option B0(b)).
    pub profile: &'a str,
    /// The caller's claimed exact closure: every definition identity
    /// reachable from the root through named calls (any order, no repeats).
    pub claimed_closure: &'a [u64],
    /// Budgets; each may only be lowered from the `/0` defaults.
    pub budget: Budget,
}

/// One kernel-accepted body: the root (`identity == None`) or one definition
/// specialization.
#[derive(Debug)]
pub(super) struct Body<'s> {
    pub(super) candidate: &'s Candidate,
    pub(super) identity: Option<u64>,
    /// Per-node derived interface, or `None` when no consistent derivation
    /// exists (refused with `E-INTERFACE` if the node is ever processed).
    pub(super) interfaces: Vec<Option<Interface>>,
}

/// An admitted subject. Only [`admit`] constructs one, after the kernel
/// accepted every body of the exact submission it borrows.
#[derive(Debug)]
pub struct Admitted<'s> {
    pub(super) submission: &'s Submission,
    /// `bodies[0]` is the root; `bodies[i + 1]` is `submission.definitions[i]`.
    pub(super) bodies: Vec<Body<'s>>,
    pub(super) closure: Vec<u64>,
    pub(super) subject: Subject,
    pub(super) budget: Budget,
}

impl Admitted<'_> {
    /// Recomputed exact closure (sorted definition identities).
    #[must_use]
    pub fn closure(&self) -> &[u64] {
        &self.closure
    }

    /// The subject bound into the graph identity.
    #[must_use]
    pub const fn subject(&self) -> &Subject {
        &self.subject
    }

    /// The root's ordered (bottom-first) input and output stack types.
    #[must_use]
    pub fn root_interface(&self) -> (&[Ty], &[Ty]) {
        let e = &self.submission.request.expected;
        (&e.stack_in, &e.stack_out)
    }

    /// Body index of a definition specialization.
    pub(super) fn definition_body(
        &self,
        def: noble_kernel::contracts::Definition,
    ) -> Option<usize> {
        self.submission
            .definitions
            .iter()
            .position(|d| d.definition == def)
            .map(|i| i + 1)
    }
}

fn refuse(code: Code, detail: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, Site::Graph, detail)
}

fn at(code: Code, body: Option<u64>, node: NodeId, detail: impl Into<String>) -> Diagnostic {
    Diagnostic::new(
        code,
        Site::Source(SourceRef {
            definition: body,
            node: node.0,
        }),
        detail,
    )
}

/// D7 stage 1 (a)–(d).
///
/// # Errors
/// `E-PROFILE`, `E-SUBJECT`, `E-LIVE-SLOT`, `E-RECURSION`, `E-CLOSURE`, `E-RESOURCE`,
/// `E-DYNAMIC-RUN`, `E-PROGRAM-OUTPUT`, `E-AGGREGATE` or `E-EFFECT`, in that
/// stage order; nothing is extracted or evaluated.
pub fn admit<'s>(
    submission: &'s Submission,
    request: &Request<'_>,
) -> Result<Admitted<'s>, Diagnostic> {
    match SubProfile::from_name(request.profile) {
        Some(SubProfile::I64) => {}
        Some(SubProfile::F32) => {
            return Err(refuse(
                Code::Profile,
                "ngraph-f32/0 graphs are authored directly in ngraph-ir/0; source extraction is I64-only (B0(b))",
            ))
        }
        None => return Err(refuse(Code::Profile, format!("unknown sub-profile {:?}", request.profile))),
    }
    let limit = Budget::default();
    if request.budget.work_units > limit.work_units
        || request.budget.expanded_nodes > limit.expanded_nodes
        || request.budget.inline_depth > limit.inline_depth
    {
        return Err(refuse(
            Code::Profile,
            "raising a /0 budget requires a new profile policy revision",
        ));
    }

    // (a) Independent kernel acceptance of every body of the exact subject.
    let bodies = accept_all(submission)?;

    // (b) Exact closure, recomputed from resolved identities.
    let closure = recompute_closure(submission, &bodies)?;
    let mut claimed = request.claimed_closure.to_vec();
    claimed.sort_unstable();
    let claimed_len = claimed.len();
    claimed.dedup();
    if claimed.len() != claimed_len || claimed != closure {
        return Err(refuse(
            Code::Closure,
            format!(
                "claimed closure {claimed:?} differs from the recomputed exact closure {closure:?}"
            ),
        ));
    }

    // (c) The root interface: P scalars, Unit and pairs of them.
    let expected = &submission.request.expected;
    for (k, ty) in expected
        .stack_in
        .iter()
        .chain(&expected.stack_out)
        .enumerate()
    {
        if has_resource(ty) {
            return Err(Diagnostic::new(
                Code::Resource,
                Site::Slot(u32::try_from(k).unwrap_or(u32::MAX)),
                format!("Resource or LiveRef in the root interface: {ty:?}"),
            ));
        }
    }
    for ty in &expected.stack_in {
        classify(ty, Code::DynamicRun)?;
    }
    for ty in &expected.stack_out {
        classify(ty, Code::ProgramOutput)?;
    }

    // (d) The root effect bound must be empty.
    if !bodies[0].1.interface.effects.is_empty() {
        return Err(refuse(
            Code::Effect,
            format!(
                "root effect bound {:?} is not empty",
                bodies[0].1.interface.effects
            ),
        ));
    }

    let subject = subject_of(submission, &closure);
    let bodies = bodies
        .into_iter()
        .map(|(body, checked)| index_interfaces(body, &checked))
        .collect();
    Ok(Admitted {
        submission,
        bodies,
        closure,
        subject,
        budget: request.budget,
    })
}

type Accepted<'s> = (Body<'s>, Checked);

fn accept_all(submission: &Submission) -> Result<Vec<Accepted<'_>>, Diagnostic> {
    let env = &submission.environment;
    let mut out = Vec::with_capacity(submission.definitions.len() + 1);
    out.push((
        Body {
            candidate: &submission.body.candidate,
            identity: None,
            interfaces: Vec::new(),
        },
        accept_one(env, &submission.request, &submission.body.candidate, None)?,
    ));
    for (i, d) in submission.definitions.iter().enumerate() {
        if env.kind(d.definition) != Some(Behavior::Named)
            || submission.definitions[..i]
                .iter()
                .any(|e| e.definition == d.definition)
        {
            return Err(refuse(
                Code::Subject,
                format!(
                    "definition {:?} is not one unique named specialization",
                    d.definition
                ),
            ));
        }
        let request = noble_kernel::untrusted::Request {
            input_bytes: submission.request.input_bytes,
            expected: d.expected.clone(),
            limits: submission.request.limits,
        };
        // As noble-wasm's definition admission: a module-owned body is
        // checked from its owner's module context.
        let owner = usize::try_from(d.definition.0)
            .ok()
            .and_then(|slot| env.definition_owners.get(slot).copied())
            .flatten();
        let checked = if owner == env.caller_module {
            accept_one(env, &request, &d.body.candidate, Some(d.identity))?
        } else {
            let mut owned = env.clone();
            owned.caller_module = owner;
            accept_one(&owned, &request, &d.body.candidate, Some(d.identity))?
        };
        out.push((
            Body {
                candidate: &d.body.candidate,
                identity: Some(d.identity),
                interfaces: Vec::new(),
            },
            checked,
        ));
    }
    Ok(out)
}

fn accept_one(
    env: &noble_kernel::contracts::Env,
    request: &noble_kernel::untrusted::Request,
    candidate: &Candidate,
    identity: Option<u64>,
) -> Result<Checked, Diagnostic> {
    let checked = match noble_kernel::acceptance::check(env, request, candidate) {
        Outcome::Accepted(checked) => checked,
        other => {
            return Err(refuse(
                Code::Subject,
                format!("body of {identity:?} is not kernel-accepted: {other:?}"),
            ))
        }
    };
    // No node may hide outside the checked derivations.
    let mut covered = vec![false; candidate.nodes.len()];
    for d in &checked.derivations {
        match covered.get_mut(d.node.0 as usize) {
            Some(slot) => *slot = true,
            None => {
                return Err(refuse(
                    Code::Subject,
                    "derivation names a node outside the arena",
                ))
            }
        }
    }
    if covered.iter().any(|c| !c) {
        return Err(refuse(
            Code::Subject,
            format!("body of {identity:?} has an unchecked arena node"),
        ));
    }
    if checked.interface.stack_in != request.expected.stack_in
        || checked.interface.stack_out != request.expected.stack_out
        || !checked
            .interface
            .effects
            .is_subset_of(&request.expected.allowed_effects)
    {
        return Err(refuse(
            Code::Subject,
            format!("body of {identity:?} was not accepted at its expected interface"),
        ));
    }
    Ok(checked)
}

/// The Wasm planner's `derivation()` rule: every derivation recorded for a
/// node must agree on `stack_in`, `stack_out` and `effects`.
fn index_interfaces<'s>(mut body: Body<'s>, checked: &Checked) -> Body<'s> {
    let mut slots: Vec<Option<Result<Interface, ()>>> = vec![None; body.candidate.nodes.len()];
    for d in &checked.derivations {
        let slot = &mut slots[d.node.0 as usize];
        *slot = match slot.take() {
            None => Some(Ok(d.interface.clone())),
            Some(Ok(prev))
                if prev.stack_in == d.interface.stack_in
                    && prev.stack_out == d.interface.stack_out
                    && prev.effects == d.interface.effects =>
            {
                Some(Ok(prev))
            }
            Some(_) => Some(Err(())),
        };
    }
    body.interfaces = slots.into_iter().map(|s| s.and_then(Result::ok)).collect();
    body
}

/// Named-call targets reachable from the root, by exact resolved identity.
/// A `Named` invocation that is no submission definition is a canonical
/// host row (`test.abort`, live operations); it is not part of the closure
/// and is refused during extraction as a host operation.
///
/// In order: live-slot dispatch has no static closure (`E-LIVE-SLOT`); a
/// definition identity reached again below itself is recursion
/// (`E-RECURSION`, direct or mutual); each reached definition row's supplied
/// dependency list (`Env::deps`) must match its body exactly, as in the Wasm
/// admission — every listed row is invoked and every invoked submission
/// definition is listed (`E-CLOSURE`).
fn recompute_closure(
    submission: &Submission,
    bodies: &[Accepted<'_>],
) -> Result<Vec<u64>, Diagnostic> {
    let env = &submission.environment;
    let target_of = |def: noble_kernel::contracts::Definition| {
        if env.kind(def) == Some(Behavior::Named) {
            submission
                .definitions
                .iter()
                .position(|d| d.definition == def)
                .map(|t| t + 1)
        } else {
            None
        }
    };
    // Reachable bodies and their named-call children, in node order.
    let mut children: Vec<Option<Vec<usize>>> = vec![None; bodies.len()];
    let mut work = vec![0usize];
    while let Some(b) = work.pop() {
        if children[b].is_some() {
            continue;
        }
        let body = &bodies[b].0;
        let mut out = Vec::new();
        for (n, node) in body.candidate.nodes.iter().enumerate() {
            match node {
                Node::SlotInvoke { .. } => return Err(at(
                    Code::LiveSlot,
                    body.identity,
                    NodeId(u32::try_from(n).unwrap_or(u32::MAX)),
                    "live-slot dispatch selects its target at run time; no static closure exists",
                )),
                Node::Invocation { def, .. } => {
                    if let Some(t) = target_of(*def) {
                        out.push(t);
                        work.push(t);
                    }
                }
                Node::Literal { .. } | Node::Quotation { .. } => {}
            }
        }
        children[b] = Some(out);
    }
    // Recursion by definition identity along any call path (three-colour
    // DFS over bodies; a gray ancestor with the same identity or a back edge
    // is recursion).
    let mut color = vec![0u8; bodies.len()];
    let mut path: Vec<(usize, usize)> = vec![(0, 0)];
    color[0] = 1;
    while let Some(top) = path.last_mut() {
        let (b, next) = *top;
        let kids = children[b].as_deref().unwrap_or(&[]);
        if let Some(&c) = kids.get(next) {
            top.1 += 1;
            let identity = bodies[c].0.identity;
            if color[c] == 1 || path.iter().any(|&(p, _)| bodies[p].0.identity == identity) {
                return Err(refuse(
                    Code::Recursion,
                    format!("definition identity {identity:?} is reached again below itself"),
                ));
            }
            if color[c] == 0 {
                color[c] = 1;
                path.push((c, 0));
            }
        } else {
            color[b] = 2;
            path.pop();
        }
    }
    let mut closure = Vec::new();
    for (b, kids) in children.iter().enumerate() {
        if b == 0 || kids.is_none() {
            continue;
        }
        closure.push(bodies[b].0.identity.unwrap_or_default());
        let row = submission.definitions[b - 1].definition;
        let listed = usize::try_from(row.0)
            .ok()
            .and_then(|r| env.deps.get(r))
            .map_or(&[][..], Vec::as_slice);
        let invoked: Vec<_> = bodies[b]
            .0
            .candidate
            .nodes
            .iter()
            .filter_map(|n| match n {
                Node::Invocation { def, .. } => Some(*def),
                _ => None,
            })
            .collect();
        let named_unlisted = invoked
            .iter()
            .any(|d| target_of(*d).is_some() && !listed.contains(d));
        if listed.iter().any(|l| !invoked.contains(l)) || named_unlisted {
            return Err(refuse(
                Code::Closure,
                format!(
                    "supplied dependency list {listed:?} of {:?} differs from its body's invocations",
                    bodies[b].0.identity
                ),
            ));
        }
    }
    closure.sort_unstable();
    closure.dedup();
    Ok(closure)
}

fn has_resource(ty: &Ty) -> bool {
    let mut work = vec![ty];
    while let Some(t) = work.pop() {
        match t {
            Ty::Resource(_) | Ty::LiveRef(..) => return true,
            Ty::Pair(a, b) | Ty::Sum(a, b) => {
                work.push(a);
                work.push(b);
            }
            Ty::List(a) => work.push(a),
            Ty::Program(i, o, _) => work.extend(i.iter().chain(o.iter())),
            Ty::Nominal(_, shape) => push_shape(&mut work, shape),
            Ty::GenericNominal(_, args, shape) => {
                work.extend(args.iter());
                push_shape(&mut work, shape);
            }
            Ty::Unit
            | Ty::Bool
            | Ty::I64
            | Ty::Text
            | Ty::Syntax
            | Ty::Contract
            | Ty::Evidence
            | Ty::Certified => {}
        }
    }
    false
}

fn push_shape<'a>(work: &mut Vec<&'a Ty>, shape: &'a noble_kernel::types::NominalShape) {
    match shape {
        noble_kernel::types::NominalShape::Opaque(a) => work.push(a),
        noble_kernel::types::NominalShape::Variant(a, b) => {
            work.push(a);
            work.push(b);
        }
    }
}

/// Root interface item: `I64`/`Bool` scalars, `Unit` or pairs of them.
/// A Program input is never statically known (`E-DYNAMIC-RUN`); a Program
/// output exposes its recipe (`E-PROGRAM-OUTPUT`, V-OBS-01).
fn classify(ty: &Ty, program: Code) -> Result<(), Diagnostic> {
    let mut work = vec![ty];
    while let Some(t) = work.pop() {
        match t {
            Ty::I64 | Ty::Bool | Ty::Unit => {}
            Ty::Pair(a, b) => {
                work.push(a);
                work.push(b);
            }
            Ty::Program(..) => {
                return Err(refuse(
                    program,
                    format!("Program-valued root interface item {ty:?}"),
                ));
            }
            other => {
                return Err(refuse(
                    Code::Aggregate,
                    format!("{other:?} is not an ngraph-i64/0 interface type"),
                ))
            }
        }
    }
    Ok(())
}

/// `I64`/`Bool` scalar type of a kernel type.
pub(super) const fn scalar(ty: &Ty) -> Option<ScalarType> {
    match ty {
        Ty::I64 => Some(ScalarType::I64),
        Ty::Bool => Some(ScalarType::Bool),
        _ => None,
    }
}

/// Subject identity: the exact root request and body plus every closure
/// definition specialization (identity, contract entry, body, expected
/// interface), encoded with the kernel's derived `Debug` rendering under a
/// domain tag. The encoding is deterministic for one kernel revision; it is
/// a producer digest, not independent validation.
fn subject_of(submission: &Submission, closure: &[u64]) -> Subject {
    let mut parts: Vec<Vec<u8>> = vec![
        b"noble.ngraph.subject.kernel-debug.v0".to_vec(),
        format!("{:?}", submission.request.expected).into_bytes(),
        format!("{:?}", submission.body.candidate).into_bytes(),
    ];
    let mut defs: Vec<_> = submission
        .definitions
        .iter()
        .filter(|d| closure.binary_search(&d.identity).is_ok())
        .collect();
    defs.sort_by_key(|d| (d.identity, d.definition.0));
    for d in defs {
        parts.push(d.identity.to_le_bytes().to_vec());
        parts.push(
            format!("{:?}|{:?}|{:?}", d.definition, d.expected, d.body.candidate).into_bytes(),
        );
    }
    let refs: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
    Subject::derive(SubjectKind::Extracted, &refs, closure.to_vec())
}
