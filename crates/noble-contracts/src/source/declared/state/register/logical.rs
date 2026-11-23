//! Resolve inert contracts and proofs against the same staged source namespace.
//! No proof is accepted or published here: the independent host gate owns that step.

mod origin;

pub(super) struct Staged {
    pub contracts: alloc::vec::Vec<crate::source::declared::LogicalContract>,
    pub proof_exports: alloc::vec::Vec<alloc::string::String>,
    pub pending: Option<crate::intrinsic::ProofBatch>,
}

/// Passed by value: the pinned extraction refuses loops that read through a
/// borrowed struct which itself holds references.
#[derive(Clone, Copy)]
pub(super) struct Request<'a> {
    pub session: &'a crate::source::declared::ModuleSession,
    pub collected: &'a super::collection::Collected,
    pub name: &'a str,
    pub version: u32,
    pub identity: u64,
    pub source: &'a [u8],
    pub generation: u64,
    pub limits: crate::Limits,
}

fn at(stage: crate::source::Stage, span: crate::Span, reason: &str) -> crate::source::Error {
    crate::source::declared::diagnostic(stage, crate::invalid(span, reason))
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
pub(super) fn stage(request: Request<'_>) -> Result<Staged, crate::source::Error> {
    let mut contracts = alloc::vec::Vec::with_capacity(request.collected.contracts.len());
    let mut contract_at = 0;
    let mut failure = None;
    while failure.is_none() && contract_at < request.collected.contracts.len() {
        match stage_contract(request, &request.collected.contracts[contract_at]) {
            Ok(contract) => contracts.push(contract),
            Err(problem) => failure = Some(problem),
        }
        contract_at += 1;
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    let mut proof_exports = alloc::vec::Vec::new();
    for proof in request.collected.proofs.iter() {
        proof_exports = exported_proof(proof_exports, &request.collected.exports, proof);
    }
    if request.collected.proofs.is_empty() {
        return Ok(Staged {
            contracts,
            proof_exports,
            pending: None,
        });
    }
    let mut obligations = alloc::vec::Vec::with_capacity(request.collected.proofs.len());
    let mut proof_at = 0;
    let mut staged = Ok(alloc::vec::Vec::new());
    while staged.is_ok() && proof_at < request.collected.proofs.len() {
        staged = match staged {
            Ok(dependencies) => match stage_proof(
                request,
                &contracts,
                &request.collected.proofs[proof_at],
                &obligations,
                dependencies,
            ) {
                Ok((obligation, dependencies)) => {
                    obligations.push(obligation);
                    Ok(dependencies)
                }
                Err(problem) => Err(problem),
            },
            Err(problem) => Err(problem),
        };
        proof_at += 1;
    }
    let dependencies = attempt!(staged);
    #[expect(
        tigerstyle::numeric_units,
        reason = "Owner: noble-contracts; the constant is a count of source elements, and the name states its unit at every use site through the surrounding type and call"
    )]
    let limit = usize::try_from(request.limits.bytes).map_err(|_| {
        crate::source::declared::error(
            crate::source::Stage::Check,
            "proof dependency byte limit exceeds address space",
        )
    })?;
    let mut copied = 0usize;
    let mut dependency_at = 0;
    let mut failure = None;
    while failure.is_none() && dependency_at < dependencies.len() {
        match dependency_bytes(&dependencies[dependency_at], limit) {
            Ok(bytes) => match copied.checked_add(bytes) {
                Some(total) => copied = total,
                None => {
                    failure = Some(crate::source::declared::error(
                        crate::source::Stage::Check,
                        "proof dependency byte count overflow",
                    ));
                }
            },
            Err(problem) => failure = Some(problem),
        }
        dependency_at += 1;
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    if copied
        .checked_mul(obligations.len())
        .is_none_or(|total| total > limit)
    {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check,
            "proof dependency snapshot byte budget exhausted",
        ));
    }
    // Elaboration is untrusted: it rewrites opted-in abbreviations into the
    // explicit core before any proof is checked or published. A refusal here
    // rejects only the opted-in submission; it never accepts or assumes. It
    // runs under the host-selected proof budgets when the host shared them,
    // the same budgets the elaborated explicit terms are checked under.
    let budgets = match request.session.proof_budgets {
        Some(budgets) => budgets,
        None => crate::intrinsic::ProofBudgets::from_limits(request.limits),
    };
    let elaborated = attempt!(crate::intrinsic::elaborate_obligations(
        request.name,
        request.version,
        &obligations,
        &dependencies,
        request.limits,
        budgets,
    )
    .map_err(|problem| crate::source::declared::diagnostic(crate::source::Stage::Check, problem)));
    Ok(Staged {
        contracts,
        proof_exports,
        pending: Some(crate::intrinsic::ProofBatch {
            module: alloc::string::String::from(request.name),
            version: request.version,
            source: request.source.to_vec(),
            generation: request.generation,
            dependencies,
            obligations: elaborated.obligations,
            elaborations: elaborated.records,
            elaboration_work: elaborated.work,
        }),
    })
}

/// Binds and prepares one contract as its staged logical goal.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; binding the subject, cloning names and forms, Vec derefs and the closure map_err calls are not const (E0277/E0015)."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn stage_contract(
    request: Request<'_>,
    contract: &crate::source::declared::parsing::Contract,
) -> Result<crate::source::declared::LogicalContract, crate::source::Error> {
    let subject = attempt!(bind_subject(request, contract));
    let goal = crate::intrinsic::ContractGoal {
        revision: contract.revision,
        contract_name: contract.name.clone(),
        contract_span: contract.span,
        subject,
        inputs: binding_pairs(&contract.inputs),
        outputs: binding_pairs(&contract.outputs),
        requires: contract.requires.clone(),
        ensures: contract.ensures.clone(),
    };
    // A proof hole is only valid in a proof term. A hole in either predicate
    // is refused here, before the contract's expression preparation can
    // report it as an unknown operator at a contract-level span.
    attempt!(refuse_hole_outside_term(&goal.requires));
    attempt!(refuse_hole_outside_term(&goal.ensures));
    let prepared = match contract.revision {
        1 => crate::intrinsic::prepare_contract(&goal, request.limits).map(|_| ()),
        2 => crate::intrinsic::prepare_named_contract(&goal, request.limits).map(|_| ()),
        _ => {
            return Err(at(
                crate::source::Stage::Check,
                contract.span,
                "unsupported contract revision",
            ))
        }
    };
    attempt!(
        prepared.map_err(|problem| crate::source::declared::diagnostic(
            crate::source::Stage::Check,
            problem
        ))
    );
    Ok(crate::source::declared::LogicalContract {
        name: contract.name.clone(),
        goal,
        exported: request.collected.exports.contains(&contract.name),
    })
}

/// The `(name, type)` text of each binding, in order.
fn binding_pairs(
    bindings: &[crate::source::declared::parsing::Binding],
) -> alloc::vec::Vec<(alloc::string::String, alloc::string::String)> {
    let mut pairs = alloc::vec::Vec::with_capacity(bindings.len());
    for binding in bindings.iter() {
        pairs.push((binding.name.clone(), binding.ty.clone()));
    }
    pairs
}

/// `names` followed by the proof's name when the module exports it.
fn exported_proof(
    mut names: alloc::vec::Vec<alloc::string::String>,
    exports: &[alloc::string::String],
    proof: &crate::source::declared::parsing::Proof,
) -> alloc::vec::Vec<alloc::string::String> {
    if exports.contains(&proof.name) {
        names.push(proof.name.clone());
    }
    names
}

/// Resolves one proof's claim and `use` references against the staged
/// contracts and the proofs before it.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn stage_proof(
    request: Request<'_>,
    contracts: &[crate::source::declared::LogicalContract],
    proof: &crate::source::declared::parsing::Proof,
    prior: &[crate::intrinsic::ProofObligation],
    dependencies: alloc::vec::Vec<crate::intrinsic::ProofDependency>,
) -> Result<
    (
        crate::intrinsic::ProofObligation,
        alloc::vec::Vec<crate::intrinsic::ProofDependency>,
    ),
    crate::source::Error,
> {
    let goal = match &proof.claim {
        crate::source::declared::parsing::ProofClaim::Pure(proposition) => {
            attempt!(refuse_hole_outside_term(proposition));
            crate::intrinsic::PendingGoal::Pure {
                proposition: proposition.clone(),
            }
        }
        crate::source::declared::parsing::ProofClaim::For(reference, span) => {
            let contract = attempt!(resolve_contract(request, contracts, reference, *span));
            if proof.revision != contract.revision {
                return Err(at(
                    crate::source::Stage::Check,
                    *span,
                    "proof and contract revisions differ",
                ));
            }
            crate::intrinsic::PendingGoal::Contract {
                contract: alloc::boxed::Box::new(contract),
            }
        }
    };
    let (term, dependencies) = attempt!(resolve_uses(
        request,
        proof.term.clone(),
        prior,
        dependencies
    ));
    // A proof hole is editor transport only. The reserved form is refused as
    // ordinary source before any checking, so no admission path can reach it;
    // only the goal view's proof-editor session admits it for analysis.
    attempt!(check_proof_holes(&term, request.session.proof_editor));
    Ok((
        crate::intrinsic::ProofObligation {
            revision: proof.revision,
            name: proof.name.clone(),
            goal,
            term,
            span: proof.span,
            elaboration: proof.elaboration,
        },
        dependencies,
    ))
}

/// Where a reserved `(editor-hole ...)` form was encountered during staging.
#[octet::sealed_enum]
#[derive(Clone, Copy)]
enum HoleMode {
    ProofTerm { allow: bool },
    OutsideTerm,
}

/// One entered list on the index path of `walk_proof_holes`.
#[derive(Clone, Copy)]
struct FormLevel {
    index: usize,
    len: usize,
}

/// Refuse or admit the reserved `(editor-hole LABEL)` proof form.
///
/// The walk uses an explicit index path instead of recursion or a borrowed work
/// stack so the extracted model stays transparent. It reads only the proof
/// term, so a hole in a contract predicate or an executable body is never
/// mistaken for a proof hole.
fn check_proof_holes(
    term: &crate::intrinsic::Form,
    proof_editor: bool,
) -> Result<(), crate::source::Error> {
    walk_proof_holes(
        term,
        HoleMode::ProofTerm {
            allow: proof_editor,
        },
    )
}

/// Refuse a reserved proof-hole form outside a proof term. The walk is the same
/// bounded index-path walk as `check_proof_holes`; the first hole in source
/// order names itself as misplaced.
fn refuse_hole_outside_term(form: &crate::intrinsic::Form) -> Result<(), crate::source::Error> {
    walk_proof_holes(form, HoleMode::OutsideTerm)
}

/// Visits `root` and every nested list beneath it left-to-right via an index
/// path, checking each `(editor-hole ...)` form under `mode`.
fn walk_proof_holes(
    root: &crate::intrinsic::Form,
    mode: HoleMode,
) -> Result<(), crate::source::Error> {
    let mut levels = alloc::vec::Vec::with_capacity(LIST_DEPTH);
    if let Some(first) = attempt!(visit_hole_node(root, 0, mode)) {
        levels.push(first);
    }
    let mut failure = None;
    while failure.is_none() && !levels.is_empty() {
        match visit_hole_at(root, &levels, mode) {
            Ok(Some(child)) => levels.push(child),
            Ok(None) => levels = advance_hole_level(levels),
            Err(problem) => failure = Some(problem),
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

/// Follows `path` from `root` and checks the reached form under `mode`.
fn visit_hole_at(
    root: &crate::intrinsic::Form,
    path: &[FormLevel],
    mode: HoleMode,
) -> Result<Option<FormLevel>, crate::source::Error> {
    let mut node = root;
    let mut level = 0;
    while level < path.len() {
        match &node.kind {
            crate::intrinsic::FormKind::List(items) => {
                node = &items[path[level].index];
            }
            crate::intrinsic::FormKind::Atom(_) => return Ok(None),
        }
        level += 1;
    }
    visit_hole_node(node, path.len(), mode)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
/// Checks one node of `walk_proof_holes`, returning the child level to descend
/// into when `node` is a non-empty non-hole list.
fn visit_hole_node(
    node: &crate::intrinsic::Form,
    depth: usize,
    mode: HoleMode,
) -> Result<Option<FormLevel>, crate::source::Error> {
    let crate::intrinsic::FormKind::List(items) = &node.kind else {
        return Ok(None);
    };
    if is_editor_hole_list(items) {
        attempt!(check_hole_list(node.span, items, mode));
        return Ok(None);
    }
    if items.is_empty() {
        return Ok(None);
    }
    if depth >= LIST_DEPTH {
        return Err(at(
            crate::source::Stage::Check,
            node.span,
            "logical nesting exceeds supported depth",
        ));
    }
    Ok(Some(FormLevel {
        index: 0,
        len: items.len(),
    }))
}

/// Unwinds finished levels and advances to the next sibling on the index path.
fn advance_hole_level(mut levels: alloc::vec::Vec<FormLevel>) -> alloc::vec::Vec<FormLevel> {
    let mut is_advanced = false;
    while !is_advanced && !levels.is_empty() {
        if let Some(mut top) = levels.pop() {
            let next = top.index.saturating_add(1);
            if next < top.len {
                top.index = next;
                levels.push(top);
                is_advanced = true;
            }
        }
    }
    levels
}

/// Whether a list is an `(editor-hole ...)` form: its first item is the atom
/// `editor-hole`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; comparing the head atom uses str PartialEq, which is not a stable const trait (E0658)."
)]
fn is_editor_hole_list(items: &[crate::intrinsic::Form]) -> bool {
    match items.first() {
        Some(crate::intrinsic::Form {
            kind: crate::intrinsic::FormKind::Atom(head),
            ..
        }) => head.as_str() == "editor-hole",
        Some(_) | None => false,
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; String::as_str and the declared::error diagnostics in `at` are not const (E0015)."
)]
/// Validates one `(editor-hole ...)` form against `mode`.
fn check_hole_list(
    span: crate::Span,
    items: &[crate::intrinsic::Form],
    mode: HoleMode,
) -> Result<(), crate::source::Error> {
    match mode {
        HoleMode::OutsideTerm => Err(at(
            crate::source::Stage::Check,
            span,
            "a proof hole is only valid in a proof term",
        )),
        HoleMode::ProofTerm { allow } => {
            if items.len() != 2 {
                return Err(at(
                    crate::source::Stage::Check,
                    span,
                    "a proof hole carries exactly one lowercase label",
                ));
            }
            let is_label = match &items[1].kind {
                crate::intrinsic::FormKind::Atom(label) => valid_hole_label(label.as_str()),
                crate::intrinsic::FormKind::List(_) => false,
            };
            if !is_label {
                return Err(at(
                    crate::source::Stage::Check,
                    span,
                    "a proof hole carries exactly one lowercase label",
                ));
            }
            if !allow {
                return Err(at(
                    crate::source::Stage::Check,
                    span,
                    "a proof hole is editor-only and never .noble source",
                ));
            }
            Ok(())
        }
    }
}

/// A transport label: 1-32 lowercase ASCII letters, digits or hyphens.
const fn valid_hole_label(label: &str) -> bool {
    let bytes = label.as_bytes();
    if bytes.is_empty() || bytes.len() > 32 {
        return false;
    }
    let mut at = 0;
    let mut is_valid = true;
    while is_valid && at < bytes.len() {
        let byte = bytes[at];
        is_valid = byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-';
        at += 1;
    }
    is_valid
}

#[expect(
    tigerstyle::numeric_units,
    reason = "Owner: noble-contracts; the constant counts source elements, and its unit is stated by the surrounding type and call sites"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
/// The snapshot bytes of one dependency and of every dependency it carries,
/// added in the order the former work stack popped them: the dependency,
/// then each of its dependencies depth first, the last listed first. One
/// loop with an index path keeps the walk free of recursion.
fn dependency_bytes(
    dependency: &crate::intrinsic::ProofDependency,
    limit: usize,
) -> Result<usize, crate::source::Error> {
    let mut total = attempt!(counted_bytes(0, dependency.source.len(), limit));
    // One entry per open level: how many dependencies of that level's
    // dependency are still unvisited, which is also the index of the one
    // entered last, so the entries below the top lead to the current one.
    let mut levels = alloc::vec![dependency.dependencies.len()];
    let mut failure = None;
    while failure.is_none() && !levels.is_empty() {
        if let Some(remaining) = levels.pop() {
            if let Some(entered) = remaining.checked_sub(1) {
                levels.push(entered);
                let (bytes, children) = entered_dependency(dependency, &levels);
                match counted_bytes(total, bytes, limit) {
                    Ok(counted) => {
                        total = counted;
                        levels.push(children);
                    }
                    Err(problem) => failure = Some(problem),
                }
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(total),
    }
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the running byte count, the bytes added and the byte budget are combined in this order at both call sites in dependency_bytes."
)]
#[expect(
    tigerstyle::numeric_units,
    reason = "Owner: noble-contracts; `limit` is the proof dependency snapshot byte budget (Limits::bytes widened to usize), in the same byte unit as `total`."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the declared::error diagnostics are built by non-const calls (E0015)."
)]
/// `total` plus one dependency snapshot's `bytes`, refused once the sum
/// overflows or exceeds `limit`.
fn counted_bytes(total: usize, bytes: usize, limit: usize) -> Result<usize, crate::source::Error> {
    let Some(total) = total.checked_add(bytes) else {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check,
            "proof dependency byte count overflow",
        ));
    };
    if total > limit {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check,
            "proof dependency snapshot byte budget exhausted",
        ));
    }
    Ok(total)
}

/// The snapshot bytes and dependency count of the dependency `path` leads to
/// from `root`: each entry indexes the dependencies of the one before it.
fn entered_dependency(root: &crate::intrinsic::ProofDependency, path: &[usize]) -> (usize, usize) {
    let mut node = root;
    let mut level = 0;
    while level < path.len() {
        node = &node.dependencies[path[level]];
        level += 1;
    }
    (node.source.len(), node.dependencies.len())
}

/// Contract binding types reach `pure_type` only from `types::parse_with_families`.
/// Its reader refuses a type word once 64 frames are pending, so at most 63
/// constructors nest on any path. A left-first walk then holds at most one
/// pending right sibling per ancestor plus the two children just pushed,
/// which is 64 slots.
const TYPE_STACK: usize = 64;

/// Contract stacks admit only Type0 data built from the four base types.
/// The reader also caps a type at 256 words, which bounds the visits.
fn pure_type(ty: &noble_kernel::types::Ty) -> bool {
    pure_subtree(ty, 0, 256).is_some()
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; this stage imports the domain types it operates on directly, matching the module vocabulary"
)]
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; each call visits a child of a parsed type and spends one of at most 256 visits, and plain recursion keeps a borrowed work stack out of the pinned extraction"
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; `pending` (stack slots held below the node) and `remaining` (visits left) are the two counters of the former explicit stack, threaded positionally through pure_type and the three recursive calls."
)]
/// Visits `ty`, then its children left first, as the former explicit stack
/// did: `pending` right siblings wait below it and every visit spends one of
/// `remaining`. Returns the visits left, or `None` where that stack refused.
fn pure_subtree(ty: &noble_kernel::types::Ty, pending: usize, remaining: usize) -> Option<usize> {
    use noble_kernel::types::Ty;
    let remaining = remaining.checked_sub(1)?;
    match ty {
        Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text => Some(remaining),
        // Right waits below left, so left is visited first, as with `&&`.
        Ty::Pair(left, right) | Ty::Sum(left, right) => {
            let above = pending.checked_add(1).filter(|above| *above < TYPE_STACK)?;
            let remaining = pure_subtree(left, above, remaining)?;
            pure_subtree(right, pending, remaining)
        }
        Ty::List(item) => {
            if pending < TYPE_STACK {
                pure_subtree(item, pending, remaining)
            } else {
                None
            }
        }
        Ty::Syntax
        | Ty::Contract
        | Ty::Evidence
        | Ty::Certified
        | Ty::Program(..)
        | Ty::LiveRef(..)
        | Ty::Resource(..)
        | Ty::Nominal(..)
        | Ty::GenericNominal(..) => None,
    }
}

fn types(
    bindings: &[crate::source::declared::parsing::Binding],
    environment: &noble_kernel::contracts::Env,
) -> Result<alloc::vec::Vec<noble_kernel::types::Ty>, crate::source::Error> {
    let mut result = alloc::vec::Vec::with_capacity(bindings.len());
    let mut binding_at = 0;
    let mut failure = None;
    while failure.is_none() && binding_at < bindings.len() {
        match contract_type(&bindings[binding_at], environment) {
            Ok(ty) => result.push(ty),
            Err(problem) => failure = Some(problem),
        }
        binding_at += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(result),
    }
}

/// Parses one contract binding type, which must be pure Type0 data.
fn contract_type(
    binding: &crate::source::declared::parsing::Binding,
    environment: &noble_kernel::contracts::Env,
) -> Result<noble_kernel::types::Ty, crate::source::Error> {
    let parsed = attempt!(crate::source::declared::types::parse_with_families(
        &binding.ty,
        &[],
        &[],
        Some(environment),
        binding.span,
    )
    .map_err(|problem| crate::source::declared::diagnostic(crate::source::Stage::Check, problem)));
    if !pure_type(&parsed.ty) {
        return Err(at(
            crate::source::Stage::Check,
            binding.span,
            "contract stack contains a non-Type0 value",
        ));
    }
    Ok(parsed.ty)
}

#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-contracts; the type name is the domain term at its use sites and renaming would churn the extracted subject"
)]
/// Copied by value, like `Request`, into helpers that read it.
#[derive(Clone, Copy)]
struct SourceOrigin<'a> {
    module_name: &'a str,
    version: u32,
    module_source: &'a [u8],
    definition: &'a crate::source::declared::DefinitionSource,
    ordinal: u32,
    owner: u64,
}

/// The resolved definition at a namespace index, if any.
fn resolved_definition(
    session: &crate::source::declared::ModuleSession,
    index: u32,
) -> Option<&crate::source::Named> {
    if let Ok(index) = usize::try_from(index) {
        session.source.definitions.get(index)
    } else {
        None
    }
}

#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-contracts; the item name reads as the domain term at its use sites; renaming would churn the extracted subject"
)]
fn source_origin<'a>(
    request: Request<'a>,
    definition: &crate::source::Named,
    reference_span: crate::Span,
) -> Result<SourceOrigin<'a>, crate::source::Error> {
    let owner = definition.owner.ok_or_else(|| {
        at(
            crate::source::Stage::Resolve,
            reference_span,
            "contract subject has no immutable module owner",
        )
    })?;
    let module = attempt!(origin_module(request, owner));
    let (located, ordinal) = attempt!(definition_span(module.spans, &definition.name));
    Ok(SourceOrigin {
        module_name: module.name,
        version: module.version,
        module_source: module.source,
        definition: located,
        ordinal,
        owner,
    })
}

/// The retained parts of the module that owns a subject definition.
#[derive(Clone, Copy)]
struct OwnerModule<'a> {
    name: &'a str,
    version: u32,
    /// Renamed in the extraction only: a Lean field named `source` would
    /// shadow the `source.*` namespace of the later `spans` field type.
    #[charon::rename("source_bytes")]
    source: &'a [u8],
    spans: &'a [crate::source::declared::DefinitionSource],
}

/// The module `owner`: the module being registered or an already registered one.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the Vec deref into crate::scan::find_first, its closure predicate and the declared::error diagnostic are not const (E0277/E0015)."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn origin_module(
    request: Request<'_>,
    owner: u64,
) -> Result<OwnerModule<'_>, crate::source::Error> {
    if owner == request.identity {
        return Ok(OwnerModule {
            name: request.name,
            version: request.version,
            source: request.source,
            spans: request.collected.definition_spans.as_slice(),
        });
    }
    match crate::scan::find_first(&request.session.modules, |module| module.identity == owner) {
        Some(module) => Ok(OwnerModule {
            name: module.name.as_str(),
            version: module.version,
            source: module.source.as_slice(),
            spans: module.definition_spans.as_slice(),
        }),
        None => Err(crate::source::declared::error(
            crate::source::Stage::Resolve,
            "unknown contract subject module",
        )),
    }
}

/// The one original definition span named `name`, with its lexical ordinal.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-contracts; `first` is the index of a matching element of `spans`, so `first + 1` is at most `spans.len()` and cannot overflow."
)]
fn definition_span<'a>(
    spans: &'a [crate::source::declared::DefinitionSource],
    name: &str,
) -> Result<(&'a crate::source::declared::DefinitionSource, u32), crate::source::Error> {
    let Some(first) = position_from(spans, 0, |source| source.name == name) else {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check,
            "missing original subject source span",
        ));
    };
    let ordinal = u32::try_from(first).map_err(|_| {
        crate::source::declared::error(
            crate::source::Stage::Check,
            "subject definition ordinal exceeds address space",
        )
    })?;
    if position_from(spans, first + 1, |source| source.name == name).is_some() {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check,
            "ambiguous original subject definition",
        ));
    }
    Ok((&spans[first], ordinal))
}

/// The index of the first element at or after `from` satisfying `predicate`;
/// elements are tested in order and none after the first match.
fn position_from<T>(
    items: &[T],
    from: usize,
    mut predicate: impl FnMut(&T) -> bool,
) -> Option<usize> {
    let mut index = from;
    let mut found = None;
    while found.is_none() && index < items.len() {
        if predicate(&items[index]) {
            found = Some(index);
        }
        index += 1;
    }
    found
}

/// The last element satisfying `predicate`, as `items.iter().rev().find(..)`:
/// elements are tested from the end and none before the first match.
fn find_last<T>(items: &[T], mut predicate: impl FnMut(&T) -> bool) -> Option<&T> {
    let mut remaining = items.len();
    let mut found = None;
    while found.is_none() && remaining != 0 {
        remaining -= 1;
        if predicate(&items[remaining]) {
            found = Some(remaining);
        }
    }
    match found {
        Some(index) => Some(&items[index]),
        None => None,
    }
}

impl SourceOrigin<'_> {
    fn definition_source(&self) -> Result<&[u8], crate::source::Error> {
        let start = usize::try_from(self.definition.span.start).map_err(|_| {
            crate::source::declared::error(
                crate::source::Stage::Check,
                "subject span exceeds address space",
            )
        })?;
        let end = usize::try_from(self.definition.span.end).map_err(|_| {
            crate::source::declared::error(
                crate::source::Stage::Check,
                "subject span exceeds address space",
            )
        })?;
        self.module_source.get(start..end).ok_or_else(|| {
            crate::source::declared::error(
                crate::source::Stage::Check,
                "subject span is outside retained module source",
            )
        })
    }
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn bind_subject(
    request: Request<'_>,
    contract: &crate::source::declared::parsing::Contract,
) -> Result<crate::intrinsic::SubjectBinding, crate::source::Error> {
    let Some(context) = request.session.source.declared.as_ref() else {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check,
            "missing declared source environment",
        ));
    };
    let lookup = &contract.subject[subject_name_start(request, &contract.subject)..];
    let found = crate::scan::find_first(&context.words, |(name, _)| name == lookup);
    let Some((_, crate::source::Target::Named(index))) = found else {
        return Err(at(
            crate::source::Stage::Resolve,
            contract.subject_span,
            "contract subject is not an accepted source definition",
        ));
    };
    let Some(definition) = resolved_definition(request.session, *index) else {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check,
            "missing resolved contract subject",
        ));
    };
    let original = attempt!(source_origin(request, definition, contract.subject_span));
    let definition_source = attempt!(original.definition_source());
    let inputs = attempt!(types(&contract.inputs, &context.environment));
    let outputs = attempt!(types(&contract.outputs, &context.environment));
    let mut prepared = attempt!(request.session.source.prepare_with_retention(
        lookup.as_bytes(),
        &inputs,
        request.limits,
        request.session.retained_source_limit,
    ));
    if prepared.output() != outputs.as_slice() || prepared.submission().is_none() {
        return Err(at(
            crate::source::Stage::Check,
            contract.subject_span,
            "contract subject does not have the full ordered stack interface",
        ));
    }
    let Some(submission) = prepared.submission.take() else {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check,
            "missing accepted contract subject submission",
        ));
    };
    let effects = submission.request.expected.allowed_effects.clone();
    if effects != noble_kernel::types::EffSet::empty() {
        return Err(at(
            crate::source::Stage::Check,
            contract.subject_span,
            "contract subject is not a pure definition",
        ));
    }
    let source_dependencies = attempt!(retained_dependencies(request, &submission));
    let named_uses = if contract.revision == 2 {
        attempt!(origin::trace(
            request,
            *index,
            &submission,
            &source_dependencies
        ))
    } else {
        alloc::vec::Vec::new()
    };
    let named_imports = if contract.revision == 2 {
        attempt!(origin::imports(
            request,
            original.owner,
            &source_dependencies
        ))
    } else {
        alloc::vec::Vec::new()
    };
    Ok(crate::intrinsic::SubjectBinding {
        module: alloc::string::String::from(original.module_name),
        version: original.version,
        module_source: original.module_source.to_vec(),
        definition: definition.name.clone(),
        definition_source: definition_source.to_vec(),
        definition_identity: definition.identity,
        definition_ordinal: original.ordinal,
        definition_owner: original.owner,
        named_uses,
        named_imports,
        accepted_submission: submission,
        input_types: inputs,
        output_types: outputs,
        effects,
        source_span: original.definition.span,
        source_dependencies,
    })
}

/// The byte offset of the definition name in a contract subject: just past a
/// `scope.` qualifier that names the module being registered, otherwise 0.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; crate::scan::split_once_byte and is_current are not const (E0015)."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-contracts; `scope` is the text before a `.` byte of `subject`, so `scope.len() + 1` is at most `subject.len()` and cannot overflow."
)]
fn subject_name_start(request: Request<'_>, subject: &str) -> usize {
    let Some((scope, _)) = crate::scan::split_once_byte(subject, b'.') else {
        return 0;
    };
    if is_current(request, scope) {
        scope.len() + 1
    } else {
        0
    }
}

/// The retained modules that own the accepted definitions, each once, in
/// definition order; the first failure wins.
fn retained_dependencies(
    request: Request<'_>,
    submission: &noble_kernel::execution::Submission,
) -> Result<alloc::vec::Vec<crate::intrinsic::ResolvedSourceModule>, crate::source::Error> {
    let mut body_at = 0;
    let mut retained = Ok(alloc::vec::Vec::new());
    while retained.is_ok() && body_at < submission.definitions.len() {
        retained = match retained {
            Ok(known) => {
                retain_dependency(request, submission, known, &submission.definitions[body_at])
            }
            Err(problem) => Err(problem),
        };
        body_at += 1;
    }
    retained
}

/// `known` followed by the retained module that owns one accepted definition,
/// unless an earlier definition already retained that module.
fn retain_dependency(
    request: Request<'_>,
    submission: &noble_kernel::execution::Submission,
    mut known: alloc::vec::Vec<crate::intrinsic::ResolvedSourceModule>,
    body: &noble_kernel::execution::Definition,
) -> Result<alloc::vec::Vec<crate::intrinsic::ResolvedSourceModule>, crate::source::Error> {
    let Some(dependency) = attempt!(subject_dependency(request, submission, &known, body)) else {
        return Ok(known);
    };
    // At most one retained module per accepted definition.
    if known.len() >= submission.definitions.len() {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check,
            "subject dependency modules exceed accepted definitions",
        ));
    }
    known.push(dependency);
    Ok(known)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
/// The retained module that owns one accepted definition, or `None` when an
/// earlier definition already retained that module.
fn subject_dependency(
    request: Request<'_>,
    submission: &noble_kernel::execution::Submission,
    known: &[crate::intrinsic::ResolvedSourceModule],
    body: &noble_kernel::execution::Definition,
) -> Result<Option<crate::intrinsic::ResolvedSourceModule>, crate::source::Error> {
    let index = usize::try_from(body.definition.0).map_err(|_| {
        crate::source::declared::error(
            crate::source::Stage::Check,
            "subject dependency identity exceeds address space",
        )
    })?;
    let dependency_owner = submission
        .environment
        .definition_owners
        .get(index)
        .copied()
        .flatten()
        .ok_or_else(|| {
            crate::source::declared::error(
                crate::source::Stage::Check,
                "subject dependency has no immutable module owner",
            )
        })?;
    if crate::scan::find_first(known, |module| module.owner == dependency_owner).is_some() {
        return Ok(None);
    }
    if dependency_owner == request.identity {
        return Ok(Some(crate::intrinsic::ResolvedSourceModule {
            owner: dependency_owner,
            module: alloc::string::String::from(request.name),
            version: request.version,
            full_source: request.source.to_vec(),
        }));
    }
    let Some(module) = crate::scan::find_first(&request.session.modules, |module| {
        module.identity == dependency_owner
    }) else {
        return Err(crate::source::declared::error(
            crate::source::Stage::Resolve,
            "subject dependency module is absent",
        ));
    };
    Ok(Some(crate::intrinsic::ResolvedSourceModule {
        owner: dependency_owner,
        module: module.name.clone(),
        version: module.version,
        full_source: module.source.clone(),
    }))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn resolve_contract(
    request: Request<'_>,
    staged: &[crate::source::declared::LogicalContract],
    reference: &str,
    span: crate::Span,
) -> Result<crate::intrinsic::ContractGoal, crate::source::Error> {
    let Some((scope, name)) = crate::scan::split_once_byte(reference, b'.') else {
        return crate::scan::find_first(staged, |contract| contract.name == reference)
            .map(|contract| contract.goal.clone())
            .ok_or_else(|| {
                at(
                    crate::source::Stage::Resolve,
                    span,
                    "unknown local proof contract",
                )
            });
    };
    if is_current(request, scope) {
        return crate::scan::find_first(staged, |contract| contract.name == name)
            .map(|contract| contract.goal.clone())
            .ok_or_else(|| {
                at(
                    crate::source::Stage::Resolve,
                    span,
                    "unknown local proof contract",
                )
            });
    }
    let module = attempt!(imported_module(request.session, scope, span));
    crate::scan::find_first(&module.contracts, |contract| {
        contract.name == name && contract.exported
    })
    .map(|contract| contract.goal.clone())
    .ok_or_else(|| {
        at(
            crate::source::Stage::Resolve,
            span,
            "unknown or private imported contract",
        )
    })
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_current(request: Request<'_>, scope: &str) -> bool {
    crate::scan::split_once_byte(scope, b'@').is_some_and(|(name, version)| {
        name == request.name
            && crate::source::declared::canonical_version(version) == Some(request.version)
    })
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn imported_module<'a>(
    session: &'a crate::source::declared::ModuleSession,
    scope: &str,
    span: crate::Span,
) -> Result<&'a crate::source::declared::Module, crate::source::Error> {
    if let Some((name, version)) = crate::scan::split_once_byte(scope, b'@') {
        let version = crate::source::declared::canonical_version(version);
        return crate::scan::find_first(&session.modules, |module| {
            module.name == name && Some(module.version) == version
        })
        .ok_or_else(|| {
            at(
                crate::source::Stage::Resolve,
                span,
                "missing imported proof module version",
            )
        });
    }
    let alias = find_last(&session.aliases, |alias| {
        !alias.pending && alias.spelling == scope
    });
    let module = match alias {
        Some(alias) => session.modules.get(alias.module),
        None => None,
    };
    module.ok_or_else(|| {
        at(
            crate::source::Stage::Resolve,
            span,
            "missing imported proof module version",
        )
    })
}

/// Logical bodies nest at most 64 lists deep (`lexer::next_logical_body`).
const LIST_DEPTH: usize = 64;

/// One list the use walk has entered: its items, moved out of the term while
/// they are visited, and how many of them are visited so far.
struct UseFrame {
    items: alloc::vec::Vec<crate::intrinsic::Form>,
    visited: usize,
}

/// What visiting one form leaves to the use walk: nothing, or the items of a
/// list it enters, moved out of that list until they are visited.
enum UseVisit {
    Done,
    Enter(alloc::vec::Vec<crate::intrinsic::Form>),
}

/// Resolves every `(use name)` in pre-order, left to right. The term and the
/// module's dependency list are owned and returned; the first error wins.
/// One loop over the stack of entered lists keeps the walk free of recursion.
fn resolve_uses(
    request: Request<'_>,
    mut term: crate::intrinsic::Form,
    prior: &[crate::intrinsic::ProofObligation],
    mut dependencies: alloc::vec::Vec<crate::intrinsic::ProofDependency>,
) -> Result<
    (
        crate::intrinsic::Form,
        alloc::vec::Vec<crate::intrinsic::ProofDependency>,
    ),
    crate::source::Error,
> {
    let mut frames = alloc::vec::Vec::new();
    let root = attempt!(visit_use(request, &mut term, 0, prior, &mut dependencies));
    if let UseVisit::Enter(items) = root {
        frames.push(UseFrame { items, visited: 0 });
    }
    let mut failure = None;
    while failure.is_none() && !frames.is_empty() {
        if let Err(problem) = use_step(request, &mut term, &mut frames, prior, &mut dependencies) {
            failure = Some(problem);
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok((term, dependencies)),
    }
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the term, the stack of entered lists and the dependency list are the use walk's own state, advanced one form per step"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; entered lists are pushed and finished ones popped, and imported proof dependencies are appended with Vec::push, so the walk needs the growable vectors."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; Vec indexing, push and pop of the entered lists and the calls into visit_use and restore_items are not const (E0277/E0015)."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-contracts; an entered list's enclosing list counts the item it entered as visited before entering it, so that `visited` is at least 1 when the entered list finishes."
)]
/// Advances the use walk by one form: visits the next item of the innermost
/// entered list, or moves the items of a finished list back into the form
/// they were taken from, the enclosing list's last visited item or the term.
fn use_step(
    request: Request<'_>,
    term: &mut crate::intrinsic::Form,
    frames: &mut alloc::vec::Vec<UseFrame>,
    prior: &[crate::intrinsic::ProofObligation],
    dependencies: &mut alloc::vec::Vec<crate::intrinsic::ProofDependency>,
) -> Result<(), crate::source::Error> {
    // The items of the innermost entered list are `depth` lists deep.
    let depth = frames.len();
    let Some(top) = depth.checked_sub(1) else {
        return Ok(());
    };
    let index = frames[top].visited;
    if index < frames[top].items.len() {
        frames[top].visited += 1;
        let visited = attempt!(visit_use(
            request,
            &mut frames[top].items[index],
            depth,
            prior,
            dependencies
        ));
        if let UseVisit::Enter(items) = visited {
            frames.push(UseFrame { items, visited: 0 });
        }
        return Ok(());
    }
    let Some(finished) = frames.pop() else {
        return Ok(());
    };
    match top.checked_sub(1) {
        None => restore_items(term, finished.items),
        Some(parent) => {
            let entered = frames[parent].visited - 1;
            restore_items(&mut frames[parent].items[entered], finished.items);
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the form belongs to the use walk's own term, and its entered list gets its items back"
)]
/// Moves a finished list's items back into the list form they were taken
/// from; the walk takes items only out of list forms.
fn restore_items(
    form: &mut crate::intrinsic::Form,
    items: alloc::vec::Vec<crate::intrinsic::Form>,
) {
    if let crate::intrinsic::FormKind::List(slot) = &mut form.kind {
        *slot = items;
    }
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the form and the dependency list are the walk's own term and accumulator, rewritten in place"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; imported proof dependencies are appended with Vec::push, so the walk needs the growable vector."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; Vec deref and indexing of the form items, core::mem::take and resolve_reference are not const (E0277/E0015)."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
/// Visits one form `depth` lists deep: resolves a `(use name)` in place, or
/// enters any other list by moving its items out for the walk to visit.
fn visit_use(
    request: Request<'_>,
    form: &mut crate::intrinsic::Form,
    depth: usize,
    prior: &[crate::intrinsic::ProofObligation],
    dependencies: &mut alloc::vec::Vec<crate::intrinsic::ProofDependency>,
) -> Result<UseVisit, crate::source::Error> {
    let span = form.span;
    let crate::intrinsic::FormKind::List(items) = &mut form.kind else {
        return Ok(UseVisit::Done);
    };
    if !is_use_list(items) {
        // Its items would be `depth + 1` lists deep, at most LIST_DEPTH.
        if depth >= LIST_DEPTH {
            return Err(at(
                crate::source::Stage::Resolve,
                span,
                "logical nesting exceeds supported depth",
            ));
        }
        return Ok(UseVisit::Enter(core::mem::take(items)));
    }
    if items.len() != 2 {
        return Err(at(
            crate::source::Stage::Resolve,
            span,
            "use requires one resolved proof name",
        ));
    }
    let reference_span = items[1].span;
    let crate::intrinsic::FormKind::Atom(reference) = &mut items[1].kind else {
        return Err(at(
            crate::source::Stage::Resolve,
            reference_span,
            "use requires a proof identifier",
        ));
    };
    attempt!(resolve_reference(
        request,
        reference,
        reference_span,
        prior,
        dependencies
    ));
    Ok(UseVisit::Done)
}

/// Whether a list is a `(use ...)` form: its first item is the atom `use`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; comparing the head atom uses str PartialEq, which is not a stable const trait (E0658)."
)]
fn is_use_list(items: &[crate::intrinsic::Form]) -> bool {
    match items.first() {
        Some(crate::intrinsic::Form {
            kind: crate::intrinsic::FormKind::Atom(head),
            ..
        }) => head.as_str() == "use",
        Some(_) | None => false,
    }
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the reference and the dependency list are the walk's own term and accumulator, rewritten in place"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the qualifier is removed from the owned atom in place and dependencies are appended with Vec::push, so both need the growable types."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; String deref and Clone, the Vec derefs into crate::scan::find_first and String::replace_range are not const (E0277/E0015)."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
/// Resolves one `use` reference: a current-module proof keeps only its local
/// name, in place, and an imported proof records its dependency once.
fn resolve_reference(
    request: Request<'_>,
    reference: &mut alloc::string::String,
    reference_span: crate::Span,
    prior: &[crate::intrinsic::ProofObligation],
    dependencies: &mut alloc::vec::Vec<crate::intrinsic::ProofDependency>,
) -> Result<(), crate::source::Error> {
    let Some((scope, name)) = crate::scan::split_once_byte(reference, b'.') else {
        if crate::scan::find_first(prior, |proof| proof.name == *reference).is_none() {
            return Err(at(
                crate::source::Stage::Resolve,
                reference_span,
                "unknown, forward or cyclic proof use",
            ));
        }
        return Ok(());
    };
    if is_current(request, scope) {
        if crate::scan::find_first(prior, |proof| proof.name == name).is_none() {
            return Err(at(
                crate::source::Stage::Resolve,
                reference_span,
                "forward or cyclic local proof use",
            ));
        }
        // Keep only the local name, in place, as the recursive form did.
        let qualifier = scope.len();
        reference.replace_range(..=qualifier, "");
        return Ok(());
    }
    let module = attempt!(imported_module(request.session, scope, reference_span));
    let Some(published) = crate::scan::find_first(&module.proofs, |proof| {
        proof.reference.name == name && proof.reference.exported
    }) else {
        return Err(at(
            crate::source::Stage::Resolve,
            reference_span,
            "unknown or private imported proof",
        ));
    };
    if crate::scan::find_first(dependencies, |proof| proof.name == *reference).is_none() {
        let mut dependency = published.reference.clone();
        dependency.name = reference.clone();
        dependencies.push(dependency);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// The explicit-stack walk `pure_subtree` replaced, kept as the oracle.
    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; test oracle: the former explicit-stack walk kept as it was, checked against pure_type by the test that calls it."
    )]
    fn former_pure_type(ty: &noble_kernel::types::Ty) -> bool {
        let mut pending: [Option<&noble_kernel::types::Ty>; super::TYPE_STACK] =
            [None; super::TYPE_STACK];
        pending[0] = Some(ty);
        let mut open = 1usize;
        let mut remaining = 256usize;
        while let Some(top) = open.checked_sub(1) {
            let Some(next) = pending.get_mut(top).and_then(Option::take) else {
                return false;
            };
            open = top;
            let Some(after) = remaining.checked_sub(1) else {
                return false;
            };
            remaining = after;
            let children = match next {
                noble_kernel::types::Ty::Unit
                | noble_kernel::types::Ty::Bool
                | noble_kernel::types::Ty::I64
                | noble_kernel::types::Ty::Text => [None, None],
                noble_kernel::types::Ty::Pair(left, right)
                | noble_kernel::types::Ty::Sum(left, right) => [Some(&**right), Some(&**left)],
                noble_kernel::types::Ty::List(item) => [Some(&**item), None],
                noble_kernel::types::Ty::Syntax
                | noble_kernel::types::Ty::Contract
                | noble_kernel::types::Ty::Evidence
                | noble_kernel::types::Ty::Certified
                | noble_kernel::types::Ty::Program(..)
                | noble_kernel::types::Ty::LiveRef(..)
                | noble_kernel::types::Ty::Resource(..)
                | noble_kernel::types::Ty::Nominal(..)
                | noble_kernel::types::Ty::GenericNominal(..) => return false,
            };
            for child in children.into_iter().flatten() {
                let Some(slot) = pending.get_mut(open) else {
                    return false;
                };
                *slot = Some(child);
                open += 1;
            }
        }
        true
    }

    /// `depth` pairs nested on the left (`shape` 0), on the right (1), as sums
    /// on the left (2) or as lists (3) around `leaf`.
    fn nested(shape: u8, depth: usize, leaf: noble_kernel::types::Ty) -> noble_kernel::types::Ty {
        let mut ty = leaf;
        for _ in 0..depth {
            let inner = alloc::boxed::Box::new(ty);
            let other = alloc::boxed::Box::new(noble_kernel::types::Ty::I64);
            ty = match shape {
                0 => noble_kernel::types::Ty::Pair(inner, other),
                1 => noble_kernel::types::Ty::Pair(other, inner),
                2 => noble_kernel::types::Ty::Sum(inner, other),
                _ => noble_kernel::types::Ty::List(inner),
            };
        }
        ty
    }

    #[test]
    fn pure_subtree_matches_the_former_explicit_stack() {
        let leaves = [
            noble_kernel::types::Ty::Unit,
            noble_kernel::types::Ty::Text,
            noble_kernel::types::Ty::Syntax,
            noble_kernel::types::Ty::Contract,
        ];
        let depths = [
            0, 1, 2, 61, 62, 63, 64, 65, 126, 127, 128, 254, 255, 256, 300,
        ];
        let mut outcomes = [0usize; 2];
        for shape in 0..4u8 {
            for depth in depths {
                for leaf in &leaves {
                    let ty = nested(shape, depth, leaf.clone());
                    let is_pure = former_pure_type(&ty);
                    assert_eq!(super::pure_type(&ty), is_pure, "{shape} {depth} {leaf:?}");
                    outcomes[usize::from(is_pure)] += 1;
                }
            }
        }
        // Both outcomes occur, including stack and visit-budget refusals.
        assert!(outcomes[0] > 0);
        assert!(outcomes[1] > 0);
        let leaf = noble_kernel::types::Ty::I64;
        assert!(former_pure_type(&nested(0, 62, leaf.clone())));
        assert!(!former_pure_type(&nested(0, 65, leaf.clone())));
        assert!(!former_pure_type(&nested(3, 256, leaf)));
    }

    /// The work-stack byte count `dependency_bytes` replaced, kept as the oracle.
    #[expect(
        tigerstyle::numeric_units,
        reason = "Owner: noble-contracts; test oracle: `limit` is the proof dependency snapshot byte budget, in the same byte unit as dependency_bytes."
    )]
    fn former_dependency_bytes(
        dependency: &crate::intrinsic::ProofDependency,
        limit: usize,
    ) -> Result<usize, &'static str> {
        let mut pending = alloc::vec![dependency];
        let mut total = 0usize;
        while let Some(next) = pending.pop() {
            total = total
                .checked_add(next.source.len())
                .ok_or("proof dependency byte count overflow")?;
            if total > limit {
                return Err("proof dependency snapshot byte budget exhausted");
            }
            pending.extend(next.dependencies.iter());
        }
        Ok(total)
    }

    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; test fixture constructor: the built dependency is data, checked by the tests that consume it."
    )]
    fn dependency(
        bytes: usize,
        dependencies: alloc::vec::Vec<crate::intrinsic::ProofDependency>,
    ) -> crate::intrinsic::ProofDependency {
        let span = crate::Span { start: 0, end: 0 };
        let form = crate::intrinsic::Form {
            kind: crate::intrinsic::FormKind::Atom(alloc::string::String::from("p")),
            span,
        };
        crate::intrinsic::ProofDependency {
            name: alloc::string::String::from("p"),
            module: alloc::string::String::from("m"),
            version: 1,
            source: alloc::vec![0; bytes],
            declaration: crate::intrinsic::ProofObligation {
                name: alloc::string::String::from("p"),
                revision: 1,
                goal: crate::intrinsic::PendingGoal::Pure {
                    proposition: form.clone(),
                },
                term: form,
                span,
                elaboration: false,
            },
            exported: true,
            dependencies,
            model_revision: alloc::string::String::new(),
            checker_revision: alloc::string::String::new(),
        }
    }

    #[test]
    fn dependency_bytes_match_the_former_work_stack() {
        // Distinct powers of two: a skipped or repeated dependency changes the total.
        let tree = dependency(
            1,
            alloc::vec![
                dependency(
                    2,
                    alloc::vec![
                        dependency(4, alloc::vec::Vec::new()),
                        dependency(8, alloc::vec::Vec::new()),
                    ],
                ),
                dependency(
                    16,
                    alloc::vec![dependency(
                        32,
                        alloc::vec![dependency(64, alloc::vec::Vec::new())]
                    )],
                ),
                dependency(128, alloc::vec::Vec::new()),
            ],
        );
        for limit in 0..=260 {
            let expected = former_dependency_bytes(&tree, limit);
            let actual = super::dependency_bytes(&tree, limit)
                .map_err(|problem| problem.diagnostic().message.clone());
            assert_eq!(
                actual,
                expected.map_err(alloc::string::String::from),
                "{limit}"
            );
        }
        assert_eq!(former_dependency_bytes(&tree, 255), Ok(255));
        assert!(former_dependency_bytes(&tree, 254).is_err());
    }

    /// The recursive use walk `resolve_uses` replaced, kept as the oracle.
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; test oracle: the former recursive walk, bounded by LIST_DEPTH like the walk it checks"
    )]
    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; test oracle: the former recursive walk kept as it was, checked against resolve_uses by the test that calls it."
    )]
    fn former_visit_use(
        request: super::Request<'_>,
        form: &mut crate::intrinsic::Form,
        depth: usize,
        prior: &[crate::intrinsic::ProofObligation],
        dependencies: &mut alloc::vec::Vec<crate::intrinsic::ProofDependency>,
    ) -> Result<(), crate::source::Error> {
        let span = form.span;
        let crate::intrinsic::FormKind::List(items) = &mut form.kind else {
            return Ok(());
        };
        if !super::is_use_list(items) {
            let Some(inner) = depth
                .checked_add(1)
                .filter(|inner| *inner <= super::LIST_DEPTH)
            else {
                return Err(super::at(
                    crate::source::Stage::Resolve,
                    span,
                    "logical nesting exceeds supported depth",
                ));
            };
            for item in items.iter_mut() {
                former_visit_use(request, item, inner, prior, dependencies)?;
            }
            return Ok(());
        }
        if items.len() != 2 {
            return Err(super::at(
                crate::source::Stage::Resolve,
                span,
                "use requires one resolved proof name",
            ));
        }
        let reference_span = items[1].span;
        let crate::intrinsic::FormKind::Atom(reference) = &mut items[1].kind else {
            return Err(super::at(
                crate::source::Stage::Resolve,
                reference_span,
                "use requires a proof identifier",
            ));
        };
        super::resolve_reference(request, reference, reference_span, prior, dependencies)
    }

    fn form(kind: crate::intrinsic::FormKind, at: u32) -> crate::intrinsic::Form {
        crate::intrinsic::Form {
            kind,
            span: crate::Span { start: at, end: at },
        }
    }

    fn atom(text: &str, at: u32) -> crate::intrinsic::Form {
        form(
            crate::intrinsic::FormKind::Atom(alloc::string::String::from(text)),
            at,
        )
    }

    fn list(items: alloc::vec::Vec<crate::intrinsic::Form>, at: u32) -> crate::intrinsic::Form {
        form(crate::intrinsic::FormKind::List(items), at)
    }

    /// `(use name)`: the list and its `use` atom at `spans[0]`, the name at `spans[1]`.
    fn use_of(name: &str, spans: [u32; 2]) -> crate::intrinsic::Form {
        list(
            alloc::vec![atom("use", spans[0]), atom(name, spans[1])],
            spans[0],
        )
    }

    /// `inner` enclosed in `depth` lists.
    fn nest(depth: usize, inner: crate::intrinsic::Form) -> crate::intrinsic::Form {
        let mut nested = inner;
        for _ in 0..depth {
            nested = list(alloc::vec![nested], 0);
        }
        nested
    }

    fn obligation(name: &str) -> crate::intrinsic::ProofObligation {
        crate::intrinsic::ProofObligation {
            name: alloc::string::String::from(name),
            revision: 1,
            goal: crate::intrinsic::PendingGoal::Pure {
                proposition: atom("p", 0),
            },
            term: atom("p", 0),
            span: crate::Span { start: 0, end: 0 },
            elaboration: false,
        }
    }

    type Walked = Result<
        (
            crate::intrinsic::Form,
            alloc::vec::Vec<crate::intrinsic::ProofDependency>,
        ),
        crate::source::Error,
    >;

    /// The rewritten term and dependency names, or the refusal's message and span.
    fn outcome(
        walked: Walked,
    ) -> Result<
        (
            crate::intrinsic::Form,
            alloc::vec::Vec<alloc::string::String>,
        ),
        (alloc::string::String, crate::Span),
    > {
        match walked {
            Ok((term, dependencies)) => {
                let mut names = alloc::vec::Vec::with_capacity(dependencies.len());
                for dependency in &dependencies {
                    names.push(dependency.name.clone());
                }
                Ok((term, names))
            }
            Err(problem) => Err((
                problem.diagnostic().message.clone(),
                problem.diagnostic().span,
            )),
        }
    }

    #[expect(
        tigerstyle::function_length,
        reason = "Owner: noble-contracts; test: the body is a case table compared case by case against the former recursive walk in one loop; splitting it would only scatter the cases"
    )]
    #[test]
    fn use_walk_matches_the_former_recursive_walk() {
        let session =
            crate::source::declared::ModuleSession::new_calculator().expect("calculator session");
        let collected = super::super::collection::Collected::collect(
            alloc::vec::Vec::new(),
            crate::Limits::default(),
            false,
            None,
        )
        .expect("empty module");
        let request = super::Request {
            session: &session,
            collected: &collected,
            name: "m",
            version: 1,
            identity: 0,
            source: &[],
            generation: 0,
            limits: crate::Limits::default(),
        };
        let prior = [obligation("p"), obligation("q")];
        let cases = alloc::vec![
            atom("p", 1),
            list(alloc::vec::Vec::new(), 2),
            list(
                alloc::vec![
                    atom("a", 3),
                    list(alloc::vec![atom("b", 4)], 5),
                    atom("c", 6)
                ],
                7,
            ),
            use_of("p", [8, 9]),
            use_of("m@1.p", [10, 11]),
            list(
                alloc::vec![
                    use_of("m@1.p", [12, 13]),
                    list(
                        alloc::vec![use_of("q", [14, 15]), use_of("m@1.q", [16, 17])],
                        18,
                    ),
                    list(alloc::vec::Vec::new(), 19),
                    use_of("p", [20, 21]),
                ],
                22,
            ),
            list(
                alloc::vec![
                    list(alloc::vec![atom("use", 23), atom("p", 24)], 25),
                    atom("use", 26)
                ],
                27,
            ),
            list(
                alloc::vec![atom("x", 28), atom("use", 29), atom("p", 30)],
                31
            ),
            use_of("zz", [32, 33]),
            list(alloc::vec![use_of("m@1.zz", [34, 35])], 36),
            list(alloc::vec![atom("use", 37)], 38),
            list(
                alloc::vec![atom("use", 39), list(alloc::vec::Vec::new(), 40)],
                41
            ),
            list(
                alloc::vec![atom("use", 42), atom("p", 43), atom("x", 44)],
                45
            ),
            list(
                alloc::vec![
                    list(alloc::vec![use_of("zz", [46, 47])], 48),
                    list(alloc::vec![atom("use", 49)], 50),
                ],
                51,
            ),
            list(
                alloc::vec![
                    list(alloc::vec![atom("use", 52)], 53),
                    use_of("zz", [54, 55])
                ],
                56,
            ),
            use_of("other@2.p", [57, 58]),
            nest(63, list(alloc::vec![atom("x", 59)], 60)),
            nest(64, list(alloc::vec::Vec::new(), 61)),
            nest(64, use_of("m@1.p", [62, 63])),
            nest(64, atom("x", 64)),
            nest(65, atom("x", 65)),
            list(
                alloc::vec![
                    nest(64, list(alloc::vec::Vec::new(), 66)),
                    use_of("zz", [67, 68])
                ],
                69,
            ),
            list(
                alloc::vec![
                    use_of("zz", [70, 71]),
                    nest(64, list(alloc::vec::Vec::new(), 72))
                ],
                73,
            ),
        ];
        let mut refusals = 0usize;
        for case in cases {
            let mut former_term = case.clone();
            let mut former_dependencies = alloc::vec::Vec::new();
            let former = former_visit_use(
                request,
                &mut former_term,
                0,
                &prior,
                &mut former_dependencies,
            )
            .map(|()| (former_term, former_dependencies));
            let expected = outcome(former);
            if expected.is_err() {
                refusals += 1;
            }
            let walked = super::resolve_uses(request, case, &prior, alloc::vec::Vec::new());
            assert_eq!(outcome(walked), expected);
        }
        // Both accepted and refused walks are compared, and the qualifier of
        // a current-module use is removed in place.
        assert!(refusals > 0);
        let local = super::resolve_uses(
            request,
            use_of("m@1.p", [1, 2]),
            &prior,
            alloc::vec::Vec::new(),
        );
        assert_eq!(
            outcome(local),
            Ok((use_of("p", [1, 2]), alloc::vec::Vec::new()))
        );
        let deep = super::resolve_uses(
            request,
            nest(64, list(alloc::vec::Vec::new(), 7)),
            &prior,
            alloc::vec::Vec::new(),
        );
        assert_eq!(
            outcome(deep).err(),
            Some((
                alloc::string::String::from("logical nesting exceeds supported depth"),
                crate::Span { start: 7, end: 7 },
            ))
        );
    }

    #[test]
    fn definition_span_refuses_missing_and_ambiguous_names() {
        let source = |name: &str, start: u32| crate::source::declared::DefinitionSource {
            name: alloc::string::String::from(name),
            span: crate::Span { start, end: start },
            body_span: crate::Span { start, end: start },
        };
        let spans = [
            source("a", 0),
            source("b", 1),
            source("c", 2),
            source("b", 3),
        ];
        let found =
            super::definition_span(&spans, "c").map(|(found, ordinal)| (found.span, ordinal));
        assert_eq!(found.ok(), Some((spans[2].span, 2)));
        let first =
            super::definition_span(&spans, "a").map(|(found, ordinal)| (found.span, ordinal));
        assert_eq!(first.ok(), Some((spans[0].span, 0)));
        for (name, reason) in [
            ("z", "missing original subject source span"),
            ("b", "ambiguous original subject definition"),
        ] {
            let refused = super::definition_span(&spans, name).map(|(found, _)| found.span);
            assert_eq!(
                refused.map_err(|problem| problem.diagnostic().message.clone()),
                Err(alloc::string::String::from(reason))
            );
        }
    }

    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-contracts; `at` is a position in `items[from..]`, so `at + from` is below `items.len()` and cannot overflow."
    )]
    #[test]
    fn find_last_and_position_from_match_the_standard_searches() {
        let items = [3u32, 5, 4, 5, 6];
        for target in [3, 4, 5, 6, 9] {
            assert_eq!(
                super::find_last(&items, |item| *item == target),
                items.iter().rev().find(|item| **item == target)
            );
            for from in 0..=items.len() {
                assert_eq!(
                    super::position_from(&items, from, |item| *item == target),
                    items[from..]
                        .iter()
                        .position(|item| *item == target)
                        .map(|at| at + from)
                );
            }
        }
        let last = super::find_last(&items, |item| *item == 5);
        assert!(last.is_some_and(|item| core::ptr::eq(item, &items[3])));
    }
}
