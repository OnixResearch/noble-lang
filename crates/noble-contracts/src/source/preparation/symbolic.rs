//! The rank-1 owner's original lexical graph is checked against the rigid
//! inferred graph before its definition can enter a staged module. This is a
//! contracts-private source certificate, not a concrete kernel Checked value.

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
)]
use alloc::{vec, vec::Vec};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
)]
use noble_kernel::{
    contracts::Definition,
    words::{Scheme, VariableKind},
};

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
)]
use crate::source::{Kind, SignedRelation, SymbolicNode, Target, Tree};

fn refused(span: crate::Span) -> crate::Diagnostic {
    crate::invalid(
        span,
        "signed symbolic owner source and inferred body differ",
    )
}

const fn same_variable(
    left: crate::inference::Variable,
    right: crate::inference::Variable,
) -> bool {
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
    )]
    use crate::inference::Variable;
    match (left, right) {
        (Variable::Value(a), Variable::Value(b))
        | (Variable::Stack(a), Variable::Stack(b))
        | (Variable::EffectValue(a), Variable::EffectValue(b)) => a == b,
        (Variable::Effect, Variable::Effect) => true,
        _ => false,
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn witness_shape(kinds: &[VariableKind], witnesses: &[crate::inference::Variable]) -> bool {
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
    )]
    use crate::inference::Variable;
    if kinds.len() != witnesses.len() {
        return false;
    }
    let mut at = 0;
    let mut is_shaped = true;
    while is_shaped && at < kinds.len() {
        is_shaped = matches!(
            (&kinds[at], &witnesses[at]),
            (VariableKind::Value, Variable::Value(_))
                | (VariableKind::Stack, Variable::Stack(_))
                | (
                    VariableKind::Effect,
                    Variable::Effect | Variable::EffectValue(_)
                )
        );
        at += 1;
    }
    is_shaped
}

fn rigid_owner(
    state: &crate::source::inference::State,
    scheme: &Scheme,
    variables: &[crate::inference::Variable],
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    if !witness_shape(&scheme.var_kinds, variables) {
        return Err(refused(span));
    }
    let mut at = 0;
    let mut outcome = Ok(());
    while outcome.is_ok() && at < scheme.var_kinds.len() && at < variables.len() {
        outcome = rigid_variable(
            state,
            (&scheme.var_kinds[at], &variables[at]),
            at,
            span,
            meter,
        );
        at += 1;
    }
    outcome
}

/// Check that one owner variable is the rigid witness of its scheme ordinal.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn rigid_variable(
    state: &crate::source::inference::State,
    pair: (&VariableKind, &crate::inference::Variable),
    ordinal: usize,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
    )]
    use crate::inference::{Term, Variable};
    attempt!(meter.charge(1, span));
    let ordinal = attempt!(crate::index(ordinal, span));
    let is_sound = match pair {
        (VariableKind::Value, Variable::Value(id)) => {
            matches!(state.arena.get(attempt!(state.arena.root(*id, span, meter)), span)?,
                Term::RigidValue(identity) if identity == ordinal)
        }
        (VariableKind::Stack, Variable::Stack(id)) => {
            matches!(state.arena.get(attempt!(state.arena.root(*id, span, meter)), span)?,
                Term::RigidStack(identity) if identity == ordinal)
        }
        (VariableKind::Effect, Variable::EffectValue(id)) => {
            state.arena.is_rigid_effect(*id, ordinal)
        }
        _ => false,
    };
    if !is_sound {
        return Err(refused(span));
    }
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the count walks owned tree vectors with iterator adapters, which are not const."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-contracts; the earlier nodes.last() refusal guarantees at least one node, so nodes.len() - 1 cannot underflow."
)]
fn semantic_count(tree: &Tree) -> Result<usize, crate::Diagnostic> {
    let Some(outer) = tree.nodes.last() else {
        return Err(refused(tree.span));
    };
    // The declared `def name [ ... ]` parser retains its final syntactic
    // quotation node after moving its children into Tree.body, leaving its
    // quotation children empty. It has no executable inference Draft.
    if !is_empty_quotation(&outer.kind) || outer.span.start == 0 || outer.span.end > tree.span.end {
        return Err(refused(tree.span));
    }
    Ok(tree.nodes.len() - 1)
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; growth is bounded by the checked input or admission limits that drive the loop."
)]
fn reachable(
    tree: &Tree,
    meter: &mut crate::Meter,
) -> Result<(Vec<bool>, Vec<u32>), crate::Diagnostic> {
    let mut seen = vec![false; attempt!(semantic_count(tree))];
    let mut pending = Vec::with_capacity(tree.body.len());
    for node in &tree.body {
        pending.push((*node, 0u32));
    }
    let mut failure = None;
    while let Some(entry) = pending.pop() {
        if let Err(problem) = reach(tree, entry, &mut seen, &mut pending, meter) {
            failure = Some(problem);
            break;
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    let mut index = 0;
    let mut stray = None;
    while stray.is_none() && index < seen.len() {
        stray = stray_span(seen[index], &tree.nodes[index]);
        index += 1;
    }
    if let Some(span) = stray {
        return Err(refused(span));
    }
    let mut self_nodes = Vec::new();
    let mut index = 0;
    let mut failure = None;
    while failure.is_none() && index < seen.len() && index < tree.nodes.len() {
        match self_node(&tree.nodes[index], index, meter) {
            Ok(Some(position)) => self_nodes.push(position),
            Ok(None) => {}
            Err(problem) => failure = Some(problem),
        }
        index += 1;
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    if self_nodes.is_empty() {
        return Err(refused(tree.span));
    }
    Ok((seen, self_nodes))
}

/// The span of an unreached node, unless it is the retained empty outer quotation.
fn stray_span(is_seen: bool, node: &crate::source::Node) -> Option<crate::Span> {
    if is_seen || is_empty_quotation(&node.kind) {
        None
    } else {
        Some(node.span)
    }
}

/// Charge one node of the self-call scan; its ordinal when it is a signed self call.
fn self_node(
    node: &crate::source::Node,
    index: usize,
    meter: &mut crate::Meter,
) -> Result<Option<u32>, crate::Diagnostic> {
    attempt!(meter.charge(1, node.span));
    if !is_self_call(&node.kind) {
        return Ok(None);
    }
    Ok(Some(attempt!(crate::index(index, node.span))))
}

/// Whether a node kind is a signed self call.
const fn is_self_call(kind: &Kind) -> bool {
    matches!(kind, Kind::Call(Target::SelfOwner))
}

/// Visit one pending source node: mark it reached and queue its quotation children.
#[expect(
    tigerstyle::assertion_density,
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; the step appends to the owned pending worklist, so it needs the Vec rather than a slice."
)]
fn reach(
    tree: &Tree,
    entry: (u32, u32),
    seen: &mut [bool],
    pending: &mut Vec<(u32, u32)>,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let (id, depth) = entry;
    attempt!(meter.charge(1, tree.span));
    attempt!(meter.depth(depth, tree.span));
    let index = attempt!(crate::offset(id, tree.span));
    let node = attempt!(tree.node(id));
    let Some(visited) = seen.get_mut(index) else {
        return Err(refused(tree.span));
    };
    if *visited {
        return Err(refused(node.span));
    }
    *visited = true;
    match &node.kind {
        Kind::Quotation(children) => {
            let child_depth = depth.checked_add(1).ok_or_else(|| refused(node.span))?;
            for child in children {
                pending.push((*child, child_depth));
            }
        }
        Kind::Call(Target::SlotInvoke(_)) | Kind::Word(_) | Kind::EditorHole => {
            return Err(refused(node.span))
        }
        _ => {}
    }
    Ok(())
}

/// The number of reached source nodes.
fn reached_count(live: &[bool]) -> usize {
    let mut count = 0;
    for visited in live {
        if *visited {
            count += 1;
        }
    }
    count
}

/// Whether a node is the empty quotation the declared parser leaves behind.
fn is_empty_quotation(kind: &Kind) -> bool {
    matches!(kind, Kind::Quotation(children) if children.is_empty())
}

/// Every source occurrence, including quotation children, has exactly one
/// symbolic draft. A recursion call reuses the owner's rigid variables; no
/// representative concrete instantiation is chosen or materialized here.
#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-contracts; the symbolic check compares the parsed tree, inference state, owner scheme and exact source bytes together; bundling them would change the extracted subject"
)]
#[expect(
    tigerstyle::assertion_density,
    tigerstyle::compound_condition,
    tigerstyle::function_length,
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation; the function keeps one ordered pass in a single reviewable unit; splitting it would separate each step from its bounds checks; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::sentinel_fallback,
    reason = "Owner: noble-contracts; u32 node limits always fit usize on supported targets, so the usize::MAX fallback is unreachable."
)]
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
    let variables = body
        .owner_variables
        .as_ref()
        .ok_or_else(|| refused(tree.span))?;
    let semantic_count = attempt!(semantic_count(tree));
    let (live, lexical_nodes) = attempt!(reachable(tree, meter));
    if body.owner != session.declared.as_ref().and_then(|context| context.owner)
        || body.owner.is_none()
        || state.definition_base != attempt!(crate::offset(owner.0, tree.span))
        || environment.scheme(owner).is_none()
        || body.nodes.len() != reached_count(&live)
        || tree.nodes.len() > usize::try_from(meter.limits.nodes).unwrap_or(usize::MAX)
    {
        return Err(refused(tree.span));
    }
    attempt!(rigid_owner(state, scheme, variables, tree.span, meter));
    let (mut original, parsed_name) =
        attempt!(crate::source::parsing::parse_declared(source_bytes, meter));
    if session
        .declared
        .as_ref()
        .is_some_and(|context| context.declared_text)
    {
        attempt!(crate::source::locals::lower(&mut original, meter));
    }
    if parsed_name.is_none()
        || original.body != tree.body
        || original.nodes.len() != tree.nodes.len()
        || original.nodes.last().map(|node| node.span) != tree.nodes.last().map(|node| node.span)
    {
        return Err(refused(tree.span));
    }
    let mut index = 0;
    let mut outcome = Ok(());
    while outcome.is_ok()
        && index < semantic_count
        && index < tree.nodes.len()
        && index < original.nodes.len()
    {
        outcome = lexical_node(
            (&tree.nodes[index], &original.nodes[index]),
            live[index],
            (parsed_name.as_deref(), source_bytes),
            session,
            meter,
        );
        index += 1;
    }
    attempt!(outcome);
    let subject = Subject {
        tree,
        body,
        live: &live,
        state,
        scheme,
        variables,
        owner,
        session,
        environment,
    };
    let source_to_draft = attempt!(pair_drafts(subject, semantic_count, meter));
    if !same_edges(&tree.body, &body.root, &source_to_draft) {
        return Err(refused(tree.span));
    }
    let mut symbolic_nodes = Vec::with_capacity(semantic_count);
    let mut index = 0;
    let mut failure = None;
    while failure.is_none() && index < semantic_count && index < tree.nodes.len() {
        match relation_node(
            &tree.nodes[index],
            source_to_draft[index],
            body,
            &source_to_draft,
            meter,
        ) {
            Ok(node) => symbolic_nodes.push(node),
            Err(problem) => failure = Some(problem),
        }
        index += 1;
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    Ok(SignedRelation {
        source_slot: attempt!(crate::index(session.definitions.len(), tree.span)),
        symbolic_nodes,
        lexical_nodes,
    })
}

/// The owner tree, its inferred body and the checked inputs one certificate compares.
#[derive(Clone, Copy)]
struct Subject<'a> {
    tree: &'a Tree,
    body: &'a crate::source::inference::Body,
    live: &'a [bool],
    state: &'a crate::source::inference::State,
    scheme: &'a Scheme,
    variables: &'a [crate::inference::Variable],
    owner: Definition,
    session: &'a crate::source::Session,
    environment: &'a noble_kernel::contracts::Env,
}

/// The source-to-draft pairing built by one breadth of the symbolic walk.
struct Pairing {
    source_to_draft: Vec<Option<u32>>,
    drafts_seen: Vec<bool>,
    pending: Vec<(u32, u32)>,
}

/// Compare one parsed source node with its retained lexical original.
///
/// The checks run in their admission order, each refusing on its own.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the comparison charges the shared meter and binds named words through non-const lookups."
)]
fn lexical_node(
    nodes: (&crate::source::Node, &crate::source::Node),
    is_live: bool,
    lexical: (Option<&str>, &[u8]),
    session: &crate::source::Session,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let (source, raw) = nodes;
    let (parsed_name, source_bytes) = lexical;
    attempt!(meter.charge(16, source.span));
    if source.span != raw.span {
        return Err(refused(source.span));
    }
    if source.span.start >= source.span.end && !both_builtin(&source.kind, &raw.kind) {
        return Err(refused(source.span));
    }
    if !lexical_kind(source, raw, parsed_name, source_bytes) {
        return Err(refused(source.span));
    }
    if !attempt!(named_token_matches(source, raw, session, meter)) {
        return Err(refused(source.span));
    }
    if !is_live && !is_empty_quotation(&source.kind) {
        return Err(refused(source.span));
    }
    Ok(())
}

/// Whether both node kinds are builtin calls.
const fn both_builtin(source: &Kind, raw: &Kind) -> bool {
    matches!(source, Kind::Call(Target::Builtin(_)))
        && matches!(raw, Kind::Call(Target::Builtin(_)))
}

/// Pair every reached source occurrence with exactly one inferred draft.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn pair_drafts(
    subject: Subject<'_>,
    semantic_count: usize,
    meter: &mut crate::Meter,
) -> Result<Vec<Option<u32>>, crate::Diagnostic> {
    let tree = subject.tree;
    let body = subject.body;
    let source_to_draft = vec![None; semantic_count];
    let drafts_seen = vec![false; body.nodes.len()];
    if tree.body.len() != body.root.len() {
        return Err(refused(tree.span));
    }
    let mut pending = Vec::with_capacity(tree.body.len());
    let mut at = 0;
    while at < tree.body.len() && at < body.root.len() {
        pending.push((tree.body[at], body.root[at].0));
        at += 1;
    }
    let mut pairing = Pairing {
        source_to_draft,
        drafts_seen,
        pending,
    };
    let mut failure = None;
    while let Some(entry) = pairing.pending.pop() {
        if let Err(problem) = pair_draft(subject, entry, &mut pairing, meter) {
            failure = Some(problem);
            break;
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    if crate::scan::find_first(&pairing.drafts_seen, |seen| !*seen).is_some()
        || !drafts_match_reach(subject.live, &pairing.source_to_draft)
    {
        return Err(refused(tree.span));
    }
    Ok(pairing.source_to_draft)
}

/// Pair one source occurrence with its inferred draft and queue their children.
#[expect(
    tigerstyle::assertion_density,
    tigerstyle::compound_condition,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation."
)]
fn pair_draft(
    subject: Subject<'_>,
    entry: (u32, u32),
    pairing: &mut Pairing,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let (source_id, draft_id) = entry;
    let tree = subject.tree;
    let source_index = attempt!(crate::offset(source_id, tree.span));
    let draft_index = attempt!(crate::offset(draft_id, tree.span));
    let Some(source) = tree.nodes.get(source_index) else {
        return Err(refused(tree.span));
    };
    let Some(draft) = subject.body.nodes.get(draft_index) else {
        return Err(refused(tree.span));
    };
    attempt!(meter.charge(16, draft.span));
    if !subject.live.get(source_index).copied().unwrap_or(false)
        || pairing.source_to_draft[source_index].is_some()
        || pairing.drafts_seen[draft_index]
        || source.span != draft.span
        || !draft_matches(
            source,
            draft,
            subject.state,
            subject.scheme,
            subject.variables,
            subject.owner,
            subject.session,
            subject.environment,
            meter,
        )?
    {
        return Err(refused(source.span));
    }
    pairing.source_to_draft[source_index] = Some(draft_id);
    pairing.drafts_seen[draft_index] = true;
    if let Kind::Quotation(children) = &source.kind {
        let crate::source::inference::DraftKind::Quotation(inferred) = &draft.kind else {
            return Err(refused(source.span));
        };
        if children.len() != inferred.len() {
            return Err(refused(source.span));
        }
        pairing.pending.reserve(children.len());
        let mut at = 0;
        while at < children.len() && at < inferred.len() {
            pairing.pending.push((children[at], inferred[at].0));
            at += 1;
        }
    }
    Ok(())
}

/// Whether every reached source node, and no other, has a draft.
fn drafts_match_reach(live: &[bool], source_to_draft: &[Option<u32>]) -> bool {
    let mut index = 0;
    let mut is_matched = true;
    while is_matched && index < live.len() {
        is_matched = live[index] == source_to_draft[index].is_some();
        index += 1;
    }
    is_matched
}

/// The certificate entry of one source node, checking a quotation's child edges.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn relation_node(
    source: &crate::source::Node,
    slot: Option<u32>,
    body: &crate::source::inference::Body,
    source_to_draft: &[Option<u32>],
    meter: &mut crate::Meter,
) -> Result<Option<SymbolicNode>, crate::Diagnostic> {
    attempt!(meter.charge(1, source.span));
    let Some(draft_id) = slot else {
        return Ok(None);
    };
    if let Kind::Quotation(children) = &source.kind {
        let Some(crate::source::inference::Draft {
            kind: crate::source::inference::DraftKind::Quotation(drafts),
            ..
        }) = body
            .nodes
            .get(attempt!(crate::offset(draft_id, source.span)))
        else {
            return Err(refused(source.span));
        };
        if !same_edges(children, drafts, source_to_draft) {
            return Err(refused(source.span));
        }
    }
    Ok(Some(SymbolicNode {
        draft: draft_id,
        span: source.span,
        target: call_target(&source.kind),
    }))
}

/// The call target of a node kind, if it is a call.
const fn call_target(kind: &Kind) -> Option<Target> {
    match kind {
        Kind::Call(target) => Some(*target),
        _ => None,
    }
}

/// The draft a source node is paired with, if any.
fn mapped(source_to_draft: &[Option<u32>], id: u32) -> Option<u32> {
    source_to_draft
        .get(usize::try_from(id).ok()?)
        .copied()
        .flatten()
}

fn same_edges(
    source: &[u32],
    drafts: &[noble_kernel::untrusted::NodeId],
    source_to_draft: &[Option<u32>],
) -> bool {
    if source.len() != drafts.len() {
        return false;
    }
    let mut at = 0;
    let mut is_same = true;
    while is_same && at < source.len() {
        is_same = matches!(
            mapped(source_to_draft, source[at]),
            Some(draft) if draft == drafts[at].0
        );
        at += 1;
    }
    is_same
}

fn named_token_matches(
    source: &crate::source::Node,
    raw: &crate::source::Node,
    session: &crate::source::Session,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    // A named word can be an imported alias. Bind it against this transaction's
    // declared namespace, not the canonical name of its target definition.
    let (Kind::Call(Target::Named(index)), Kind::Word(word)) = (&source.kind, &raw.kind) else {
        return Ok(true);
    };
    let Some(context) = session.declared.as_ref() else {
        return Ok(false);
    };
    bound_word(&context.words, word, *index, source.span, meter)
}

/// Whether the last namespace word spelling `word` binds `Named(index)`.
/// Words are matched from the last binding backwards, each one charged.
fn bound_word(
    words: &[(alloc::string::String, Target)],
    word: &[u8],
    index: u32,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    let mut remaining = words.len();
    let mut found = None;
    let mut failure = None;
    while failure.is_none() && found.is_none() && remaining > 0 {
        remaining -= 1;
        match word_binding(&words[remaining], word, index, span, meter) {
            Ok(binding) => found = binding,
            Err(problem) => failure = Some(problem),
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    Ok(found.unwrap_or(false))
}

/// Charge one namespace word; when it spells `word`, whether it binds `Named(index)`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the shared meter charge and byte-slice equality are not const."
)]
fn word_binding(
    entry: &(alloc::string::String, Target),
    word: &[u8],
    index: u32,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<Option<bool>, crate::Diagnostic> {
    let (name, target) = entry;
    attempt!(meter.charge(
        attempt!(crate::index(
            name.len().saturating_add(word.len()).saturating_add(1),
            span
        )),
        span
    ));
    if name.as_bytes() == word {
        return Ok(Some(*target == Target::Named(index)));
    }
    Ok(None)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; str and byte-slice equality are not const."
)]
fn lexical_kind(
    source: &crate::source::Node,
    raw: &crate::source::Node,
    name: Option<&str>,
    bytes: &[u8],
) -> bool {
    match (&source.kind, &raw.kind) {
        (Kind::Literal(a), Kind::Literal(b)) => a == b,
        (Kind::Text(a), Kind::Text(b)) => a == b,
        (Kind::Quotation(a), Kind::Quotation(b)) => a == b,
        (Kind::Call(Target::Builtin(a)), Kind::Call(Target::Builtin(b))) => a == b,
        (Kind::Call(target), Kind::Word(word)) => {
            let spelled = match (
                usize::try_from(raw.span.start).ok(),
                usize::try_from(raw.span.end).ok(),
            ) {
                (Some(start), Some(end)) => bytes.get(start..end),
                _ => None,
            };
            if spelled != Some(word.as_slice()) {
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

#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-contracts; one occurrence comparison needs the source node, draft, inference state, owner scheme and rigid variables at once; bundling them would change the extracted subject"
)]
#[expect(
    tigerstyle::assertion_density,
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the comparison resolves definitions and charges the shared meter through non-const calls."
)]
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
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
    )]
    use crate::source::inference::DraftKind;
    // Each occurrence kind is compared by its own helper, so the arms join only booleans.
    let is_same = match (&source.kind, &draft.kind) {
        (Kind::Literal(a), DraftKind::Literal(b)) => literal_draft(a, b, draft),
        (Kind::Text(bytes), DraftKind::Literal(noble_kernel::untrusted::Lit::Text)) => {
            text_draft(bytes, draft)
        }
        (Kind::Call(Target::Builtin(id)), DraftKind::Invocation(def)) => {
            builtin_draft((Definition(*id), *def), draft, environment)
        }
        (Kind::Call(Target::Named(index)), DraftKind::Invocation(def)) => {
            attempt!(named_draft(
                (*index, *def),
                source.span,
                draft,
                state,
                session
            ))
        }
        (Kind::Call(Target::SelfOwner), DraftKind::SignedSelf(def)) => {
            self_draft(*def == owner, draft, scheme, owner_variables)
        }
        (Kind::Quotation(_), DraftKind::Quotation(_)) => quotation_draft(draft),
        _ => false,
    };
    attempt!(meter.charge(1, source.span));
    Ok(is_same)
}

/// Whether a literal draft instantiates its literal occurrence at one stack variable.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; derived literal equality is a non-const PartialEq call."
)]
fn literal_draft(
    source: &noble_kernel::untrusted::Lit,
    inferred: &noble_kernel::untrusted::Lit,
    draft: &crate::source::inference::Draft,
) -> bool {
    source == inferred && draft.text.is_none() && single_stack(draft)
}

/// Whether a text literal draft carries exactly the occurrence's bytes at one stack variable.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; byte-slice equality is not const."
)]
fn text_draft(bytes: &[u8], draft: &crate::source::inference::Draft) -> bool {
    draft.text.as_deref() == Some(bytes) && single_stack(draft)
}

/// Whether a draft has exactly one variable, a stack variable.
const fn single_stack(draft: &crate::source::inference::Draft) -> bool {
    let variables = draft.variables.as_slice();
    variables.len() == 1 && matches!(variables[0], crate::inference::Variable::Stack(_))
}

/// Whether an invocation draft calls the builtin `expected` at its scheme's variable shape.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the scheme lookup and witness walk are non-const calls."
)]
fn builtin_draft(
    definitions: (Definition, Definition),
    draft: &crate::source::inference::Draft,
    environment: &noble_kernel::contracts::Env,
) -> bool {
    let (expected, def) = definitions;
    def == expected
        && environment
            .scheme(expected)
            .is_some_and(|scheme| witness_shape(&scheme.var_kinds, &draft.variables))
        && draft.text.is_none()
}

/// Whether an invocation draft instantiates the named target's own inferred body.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the comparison resolves definitions and bodies through non-const lookups."
)]
fn named_draft(
    call: (u32, Definition),
    span: crate::Span,
    draft: &crate::source::inference::Draft,
    state: &crate::source::inference::State,
    session: &crate::source::Session,
) -> Result<bool, crate::Diagnostic> {
    let (index, def) = call;
    let target = session
        .definitions
        .get(attempt!(crate::offset(index, span)));
    let position = usize::try_from(def.0)
        .ok()
        .and_then(|id| id.checked_sub(state.definition_base))
        .and_then(|offset| offset.checked_add(1));
    let body = match position {
        Some(position) => state.bodies.get(position),
        None => None,
    };
    let (Some(target), Some(body)) = (target, body) else {
        return Ok(false);
    };
    let is_exact_variables = if crate::scan::find_first(&body.nodes, |node| {
        matches!(
            node.kind,
            crate::source::inference::DraftKind::SignedSelf(_)
        )
    })
    .is_some()
    {
        signed_variables(target, body, draft)
    } else {
        draft.variables.is_empty()
    };
    Ok(body.span == span
        && body.owner_slot == Some(index)
        && body.identity == Some(target.identity)
        && body.owner == target.owner
        && is_exact_variables
        && draft.text.is_none())
}

/// Whether a draft's variables exactly instantiate a signed target's rigid owner body.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the witness and prefix walks are non-const calls."
)]
fn signed_variables(
    target: &crate::source::Named,
    body: &crate::source::inference::Body,
    draft: &crate::source::inference::Draft,
) -> bool {
    if target.signed.is_none() {
        return false;
    }
    let is_shaped = match target.signature.as_ref() {
        Some(scheme) => witness_shape(&scheme.var_kinds, &draft.variables),
        None => false,
    };
    if !is_shaped {
        return false;
    }
    match body.owner_variables.as_ref() {
        Some(variables) => {
            draft.variables.len() == variables.len() && same_prefix(&draft.variables, variables)
        }
        None => false,
    }
}

/// Whether a signed self draft calls its owner at the owner's rigid variables.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the prefix walk is a non-const call."
)]
fn self_draft(
    is_owner: bool,
    draft: &crate::source::inference::Draft,
    scheme: &Scheme,
    owner_variables: &[crate::inference::Variable],
) -> bool {
    is_owner
        && draft.variables.len() == scheme.var_kinds.len()
        && same_prefix(&draft.variables, owner_variables)
        && draft.text.is_none()
}

/// Whether a quotation draft carries three stack variables and an effect variable.
const fn quotation_draft(draft: &crate::source::inference::Draft) -> bool {
    let variables = draft.variables.as_slice();
    draft.text.is_none()
        && variables.len() == 4
        && matches!(variables[0], crate::inference::Variable::Stack(_))
        && matches!(variables[1], crate::inference::Variable::Stack(_))
        && matches!(variables[2], crate::inference::Variable::Stack(_))
        && matches!(variables[3], crate::inference::Variable::EffectValue(_))
}

/// Whether two variable lists agree pairwise over their common prefix.
fn same_prefix(left: &[crate::inference::Variable], right: &[crate::inference::Variable]) -> bool {
    let mut at = 0;
    let mut is_same = true;
    while is_same && at < left.len() && at < right.len() {
        is_same = same_variable(left[at], right[at]);
        at += 1;
    }
    is_same
}

/// Check the private certificate against the retained original lexical bytes
/// before a signed-only compiler may rederive any concrete specialization.
#[expect(
    tigerstyle::assertion_density,
    tigerstyle::compound_condition,
    tigerstyle::too_many_parameters,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields; growth is bounded by the checked input or admission limits that drive the loop."
)]
pub(in crate::source) fn verify(
    tree: &Tree,
    original: &Tree,
    relation: &SignedRelation,
    name: &str,
    source_bytes: &[u8],
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let n = attempt!(semantic_count(tree));
    let (live, lexical_nodes) = attempt!(reachable(tree, meter));
    if relation.symbolic_nodes.len() != n
        || original.nodes.len() != tree.nodes.len()
        || original.nodes.last().map(|node| node.span) != tree.nodes.last().map(|node| node.span)
        || original.body != tree.body
        || relation.lexical_nodes.is_empty()
    {
        return Err(refused(tree.span));
    }
    let mut drafts = vec![false; reached_count(&live)];
    let mut self_nodes = Vec::new();
    let mut index = 0;
    let mut failure = None;
    while failure.is_none()
        && index < tree.nodes.len()
        && index < original.nodes.len()
        && index < relation.symbolic_nodes.len()
    {
        match verified_node(
            (
                &tree.nodes[index],
                &original.nodes[index],
                &relation.symbolic_nodes[index],
            ),
            (index, live[index]),
            (name, source_bytes),
            &mut drafts,
            meter,
        ) {
            Ok(Some(position)) => self_nodes.push(position),
            Ok(None) => {}
            Err(problem) => failure = Some(problem),
        }
        index += 1;
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    if crate::scan::find_first(&drafts, |seen| !*seen).is_some()
        || self_nodes != relation.lexical_nodes
        || lexical_nodes != relation.lexical_nodes
    {
        return Err(refused(tree.span));
    }
    Ok(())
}

/// Check one certificate entry against its source node; a self call yields its index.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the check charges the shared meter and constructs owned diagnostics through non-const calls."
)]
fn verified_node(
    occurrence: (
        &crate::source::Node,
        &crate::source::Node,
        &Option<SymbolicNode>,
    ),
    position: (usize, bool),
    lexical: (&str, &[u8]),
    drafts: &mut [bool],
    meter: &mut crate::Meter,
) -> Result<Option<u32>, crate::Diagnostic> {
    let (node, raw, slot) = occurrence;
    let (index, is_live) = position;
    let (name, source_bytes) = lexical;
    attempt!(meter.charge(1, node.span));
    if node.span != raw.span
        || !lexical_kind(node, raw, Some(name), source_bytes)
        || is_live != slot.is_some()
    {
        return Err(refused(node.span));
    }
    let Some(binding) = slot else {
        return Ok(None);
    };
    if binding.span != node.span || binding.target != call_target(&node.kind) {
        return Err(refused(node.span));
    }
    let draft = attempt!(crate::offset(binding.draft, node.span));
    let Some(visited) = drafts.get_mut(draft) else {
        return Err(refused(node.span));
    };
    if *visited {
        return Err(refused(node.span));
    }
    *visited = true;
    if is_self_call(&node.kind) {
        return Ok(Some(attempt!(crate::index(index, node.span))));
    }
    Ok(None)
}
