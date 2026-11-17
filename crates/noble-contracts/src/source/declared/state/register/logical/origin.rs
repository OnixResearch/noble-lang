//! Tie each accepted named specialization to its resolved original source occurrence.
//! Dynamic definition slots are checked graph edges, never lexical identities.

struct PendingUse {
    source_index: u32,
    definition: noble_kernel::contracts::Definition,
    caller_definition: Option<noble_kernel::contracts::Definition>,
    source_node: Option<u32>,
    source_span: Option<crate::Span>,
    candidate_node: noble_kernel::untrusted::NodeId,
}

/// The fixed inputs of one named-use trace, copied into each step.
#[derive(Clone, Copy)]
struct Tracer<'a> {
    request: super::Request<'a>,
    submission: &'a noble_kernel::execution::Submission,
    dependencies: &'a [crate::intrinsic::ResolvedSourceModule],
    span: crate::Span,
}

/// One accepted body checked against its original source body, copied into
/// each node step.
#[derive(Clone, Copy)]
struct BodyTrace<'a> {
    original: super::SourceOrigin<'a>,
    tree: &'a crate::source::Tree,
    candidate: &'a noble_kernel::untrusted::Candidate,
    caller: noble_kernel::contracts::Definition,
}

fn mismatch(span: crate::Span, reason: &str) -> crate::source::Error {
    super::at(crate::source::Stage::Check, span, reason)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
/// Freeze only imported aliases whose original modules occur in this subject
/// graph. Rebinding a session alias later must not rewrite an earlier goal.
pub(super) fn imports(
    request: super::Request<'_>,
    original_owner: u64,
    dependencies: &[crate::intrinsic::ResolvedSourceModule],
) -> Result<alloc::vec::Vec<crate::intrinsic::NamedImportBinding>, crate::source::Error> {
    if original_owner == request.identity {
        let mut alias_at = 0;
        let mut outcome = Ok(alloc::vec::Vec::new());
        while outcome.is_ok() && alias_at < request.session.aliases.len() {
            outcome = match outcome {
                Ok(saved) => session_import(request.session, alias_at, dependencies, saved),
                Err(problem) => Err(problem),
            };
            alias_at += 1;
        }
        return outcome;
    }
    let original = crate::scan::find_first(&request.session.modules, |module| {
        module.identity == original_owner
    })
    .ok_or_else(|| {
        mismatch(
            crate::Span { start: 0, end: 0 },
            "original named caller module is absent",
        )
    })?;
    let mut saved = alloc::vec::Vec::new();
    for alias in original.import_aliases.iter() {
        saved = retained_import(saved, dependencies, &alias.alias, alias.owner);
    }
    Ok(saved)
}

/// `saved` followed by the binding of the session alias at `alias_at` when it
/// is settled and its module is retained; an alias without a module fails.
fn session_import(
    session: &crate::source::declared::ModuleSession,
    alias_at: usize,
    dependencies: &[crate::intrinsic::ResolvedSourceModule],
    saved: alloc::vec::Vec<crate::intrinsic::NamedImportBinding>,
) -> Result<alloc::vec::Vec<crate::intrinsic::NamedImportBinding>, crate::source::Error> {
    let alias = &session.aliases[alias_at];
    if alias.pending {
        return Ok(saved);
    }
    match session.modules.get(alias.module) {
        Some(imported) => Ok(retained_import(
            saved,
            dependencies,
            &alias.spelling,
            imported.identity,
        )),
        None => Err(mismatch(
            crate::Span { start: 0, end: 0 },
            "original imported alias has no retained module",
        )),
    }
}

/// `saved` followed by the binding of `alias` when its module is retained.
fn retained_import(
    mut saved: alloc::vec::Vec<crate::intrinsic::NamedImportBinding>,
    dependencies: &[crate::intrinsic::ResolvedSourceModule],
    alias: &str,
    owner: u64,
) -> alloc::vec::Vec<crate::intrinsic::NamedImportBinding> {
    if let Some(module_index) = dependency_position(dependencies, owner) {
        saved.push(crate::intrinsic::NamedImportBinding {
            alias: alloc::string::String::from(alias),
            owner,
            module_index,
        });
    }
    saved
}

/// The index of the first retained source module owned by `owner`.
fn dependency_position(
    dependencies: &[crate::intrinsic::ResolvedSourceModule],
    owner: u64,
) -> Option<usize> {
    let mut index = 0;
    let mut found = None;
    while found.is_none() && index < dependencies.len() {
        if dependencies[index].owner == owner {
            found = Some(index);
        }
        index += 1;
    }
    found
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
pub(super) fn trace(
    request: super::Request<'_>,
    root_source_index: u32,
    submission: &noble_kernel::execution::Submission,
    dependencies: &[crate::intrinsic::ResolvedSourceModule],
) -> Result<alloc::vec::Vec<crate::intrinsic::NamedUseOrigin>, crate::source::Error> {
    let span = root_span(request.session, root_source_index);
    let (root, def) = attempt!(root_invocation(submission, span));
    if !has_ordered_slots(&submission.definitions) {
        return Err(mismatch(
            span,
            "named specialization slots are not unique and ordered",
        ));
    }
    let tracer = Tracer {
        request,
        submission,
        dependencies,
        span,
    };
    let mut pending = alloc::vec![PendingUse {
        source_index: root_source_index,
        definition: def,
        caller_definition: None,
        source_node: None,
        source_span: None,
        candidate_node: root,
    }];
    let mut seen = alloc::vec![false; submission.definitions.len()];
    let mut result = alloc::vec::Vec::with_capacity(submission.definitions.len());
    let mut failure = None;
    while failure.is_none() && !pending.is_empty() {
        if let Some(use_site) = pending.pop() {
            match trace_use(tracer, &seen, use_site, core::mem::take(&mut pending)) {
                Ok((position, origin, next)) => {
                    seen[position] = true;
                    result.push(origin);
                    pending = next;
                }
                Err(problem) => failure = Some(problem),
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    if crate::scan::find_first(&seen, |visited| !*visited).is_some() {
        return Err(mismatch(
            span,
            "accepted named graph contains an untraced specialization",
        ));
    }
    Ok(result)
}

/// The span of the subject definition's source tree, or an empty span when
/// the index resolves to no definition.
fn root_span(session: &crate::source::declared::ModuleSession, index: u32) -> crate::Span {
    match super::resolved_definition(session, index) {
        Some(definition) => definition.tree.span,
        None => crate::Span { start: 0, end: 0 },
    }
}

/// The single body node of a named contract root and the subject it invokes.
fn root_invocation(
    submission: &noble_kernel::execution::Submission,
    span: crate::Span,
) -> Result<
    (
        noble_kernel::untrusted::NodeId,
        noble_kernel::contracts::Definition,
    ),
    crate::source::Error,
> {
    let candidate = &submission.body.candidate;
    if candidate.body.len() != 1 {
        return Err(mismatch(
            span,
            "named contract requires one selected subject invocation",
        ));
    }
    let root = candidate.body[0];
    if candidate.nodes.len() != 1 || !submission.body.texts.is_empty() {
        return Err(mismatch(
            span,
            "named contract root is not a single source invocation",
        ));
    }
    let Some(noble_kernel::untrusted::Node::Invocation { def, .. }) =
        node_at(&candidate.nodes, root)
    else {
        return Err(mismatch(
            span,
            "named contract root does not invoke its subject",
        ));
    };
    Ok((root, *def))
}

/// The accepted candidate node `id` names, if any.
fn node_at(
    nodes: &[noble_kernel::untrusted::Node],
    id: noble_kernel::untrusted::NodeId,
) -> Option<&noble_kernel::untrusted::Node> {
    if let Ok(index) = usize::try_from(id.0) {
        nodes.get(index)
    } else {
        None
    }
}

/// Whether the accepted specialization slots strictly increase.
fn has_ordered_slots(definitions: &[noble_kernel::execution::Definition]) -> bool {
    let mut preceding = None;
    let mut index = 0;
    let mut is_ordered = true;
    while is_ordered && index < definitions.len() {
        let slot = definitions[index].definition.0;
        is_ordered = preceding.is_none_or(|prior| prior < slot);
        preceding = Some(slot);
        index += 1;
    }
    is_ordered
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
/// Checks one named use against its accepted specialization and original
/// source body. Returns the specialization's position, its origin and the
/// stack with the body's named calls pushed.
fn trace_use(
    tracer: Tracer<'_>,
    seen: &[bool],
    use_site: PendingUse,
    pending: alloc::vec::Vec<PendingUse>,
) -> Result<
    (
        usize,
        crate::intrinsic::NamedUseOrigin,
        alloc::vec::Vec<PendingUse>,
    ),
    crate::source::Error,
> {
    let Some(named) = super::resolved_definition(tracer.request.session, use_site.source_index)
    else {
        return Err(mismatch(
            tracer.span,
            "named invocation has no resolved source definition",
        ));
    };
    let original = attempt!(super::source_origin(tracer.request, named, tracer.span));
    let module_index =
        dependency_position(tracer.dependencies, original.owner).ok_or_else(|| {
            mismatch(
                original.definition.span,
                "named invocation owner has no retained source module",
            )
        })?;
    let module = &tracer.dependencies[module_index];
    if module.module != original.module_name
        || module.version != original.version
        || module.full_source.as_slice() != original.module_source
    {
        return Err(mismatch(
            original.definition.span,
            "named invocation source module differs from immutable owner",
        ));
    }
    let position = tracer
        .submission
        .definitions
        .binary_search_by_key(&use_site.definition.0, |body| body.definition.0)
        .map_err(|_| {
            mismatch(
                original.definition.span,
                "named invocation has no accepted specialization",
            )
        })?;
    if seen[position] {
        return Err(mismatch(
            original.definition.span,
            "named invocation reuses an accepted specialization",
        ));
    }
    let body = &tracer.submission.definitions[position];
    let slot = usize::try_from(body.definition.0).map_err(|_| {
        mismatch(
            original.definition.span,
            "named specialization slot exceeds address space",
        )
    })?;
    if body.identity != named.identity
        || tracer.submission.environment.definition_owners.get(slot) != Some(&Some(original.owner))
        || tracer.submission.environment.kind(body.definition)
            != Some(noble_kernel::contracts::Behavior::Named)
    {
        return Err(mismatch(
            original.definition.span,
            "named specialization differs from resolved source owner or identity",
        ));
    }
    let tree = &named.tree;
    let candidate = &body.body.candidate;
    if tree.body.len() != candidate.body.len()
        || candidate.nodes.len() != candidate.body.len()
        || !body.body.texts.is_empty()
    {
        return Err(mismatch(
            original.definition.span,
            "named body is not a flat original source body",
        ));
    }
    let origin = crate::intrinsic::NamedUseOrigin {
        definition: use_site.definition,
        definition_identity: named.identity,
        definition_owner: original.owner,
        definition_name: named.name.clone(),
        definition_ordinal: original.ordinal,
        definition_span: original.definition.span,
        body_span: original.definition.body_span,
        module_index,
        caller_definition: use_site.caller_definition,
        source_node: use_site.source_node,
        source_span: use_site.source_span,
        candidate_node: use_site.candidate_node,
    };
    let walk = BodyTrace {
        original,
        tree,
        candidate,
        caller: use_site.definition,
    };
    let pending = attempt!(trace_body(walk, pending));
    Ok((position, origin, pending))
}

/// Checks the body nodes last first and pushes each named call, so the stack
/// is LIFO: later lexical calls are pushed first and each use is visited once
/// in the same order as the original, separately parsed body.
fn trace_body(
    walk: BodyTrace<'_>,
    pending: alloc::vec::Vec<PendingUse>,
) -> Result<alloc::vec::Vec<PendingUse>, crate::source::Error> {
    let mut remaining = walk.tree.body.len();
    let mut outcome = Ok(pending);
    while outcome.is_ok() && remaining != 0 {
        remaining -= 1;
        outcome = match outcome {
            Ok(pending) => trace_node(walk, remaining, pending),
            Err(problem) => Err(problem),
        };
    }
    outcome
}

/// Checks body node `position` against its accepted node and pushes the use
/// a named call makes.
fn trace_node(
    walk: BodyTrace<'_>,
    position: usize,
    mut pending: alloc::vec::Vec<PendingUse>,
) -> Result<alloc::vec::Vec<PendingUse>, crate::source::Error> {
    let span = walk.original.definition.span;
    let source_id = walk.tree.body[position];
    let accepted_id = walk.candidate.body[position];
    if source_id != accepted_id.0 {
        return Err(mismatch(
            span,
            "named candidate node order differs from original source body",
        ));
    }
    let source_node = attempt!(walk.tree.node(source_id).map_err(|problem| {
        crate::source::declared::diagnostic(crate::source::Stage::Check, problem)
    }));
    let Some(accepted) = node_at(&walk.candidate.nodes, accepted_id) else {
        return Err(mismatch(span, "named body has an invalid accepted node"));
    };
    match (&source_node.kind, accepted) {
        (
            crate::source::Kind::Literal(expected),
            noble_kernel::untrusted::Node::Literal { lit, .. },
        ) => same_operation(*expected == *lit, span, pending),
        (
            crate::source::Kind::Call(crate::source::Target::Builtin(expected)),
            noble_kernel::untrusted::Node::Invocation { def, .. },
        ) => same_operation(*expected == def.0, span, pending),
        (
            crate::source::Kind::Call(crate::source::Target::Named(next)),
            noble_kernel::untrusted::Node::Invocation { def, .. },
        ) => {
            let source_span = attempt!(occurrence_span(walk.original, source_node.span));
            pending.push(PendingUse {
                source_index: *next,
                definition: *def,
                caller_definition: Some(walk.caller),
                source_node: Some(source_id),
                source_span: Some(source_span),
                candidate_node: accepted_id,
            });
            Ok(pending)
        }
        _ => same_operation(false, span, pending),
    }
}

/// `pending` unchanged when the accepted operation is the original one.
fn same_operation(
    is_same: bool,
    span: crate::Span,
    pending: alloc::vec::Vec<PendingUse>,
) -> Result<alloc::vec::Vec<PendingUse>, crate::source::Error> {
    if is_same {
        Ok(pending)
    } else {
        Err(mismatch(
            span,
            "accepted named operation differs from original source node",
        ))
    }
}

/// The absolute span of a named call: it must lie inside the original
/// definition body and inside the retained module source.
fn occurrence_span(
    original: super::SourceOrigin<'_>,
    relative: crate::Span,
) -> Result<crate::Span, crate::source::Error> {
    let span = original.definition.span;
    let start = span
        .start
        .checked_add(relative.start)
        .ok_or_else(|| mismatch(span, "named source occurrence span overflows"))?;
    let end = span
        .start
        .checked_add(relative.end)
        .ok_or_else(|| mismatch(span, "named source occurrence span overflows"))?;
    if start < original.definition.body_span.start
        || end > original.definition.body_span.end
        || !matches!(
            (usize::try_from(start), usize::try_from(end)),
            (Ok(start), Ok(end)) if original.module_source.get(start..end).is_some()
        )
    {
        return Err(mismatch(
            span,
            "named invocation is outside its original body",
        ));
    }
    Ok(crate::Span { start, end })
}
