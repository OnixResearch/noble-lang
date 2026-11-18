//! Exact, additive named-call proof profile. These consistency checks do not
//! turn public binding fields into retained namespace authority.
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use super::{
    atom, operation, reject, CheckedProof, ContractGoal, Form, Meter, NamedUseOrigin, ProofKind,
    ProofObligation, Transition,
};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use crate::{Diagnostic, Limits, Span};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use alloc::{string::String, vec::Vec};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use noble_kernel::{
    acceptance,
    contracts::{Behavior, Definition},
    execution,
    shapes::Pattern,
    types::{EffSet, Ty},
    untrusted::{self, Lit, Node, Outcome},
    words::Inst,
};

struct Graph<'a> {
    first_use: &'a NamedUseOrigin,
    second_use: &'a NamedUseOrigin,
}

fn invalid(span: Span, message: &str) -> Diagnostic {
    reject(span, message)
}
fn at(span: Span, source: &[u8]) -> Result<&[u8], Diagnostic> {
    let start = usize::try_from(span.start)
        .map_err(|_| invalid(span, "source span exceeds address space"))?;
    let end = usize::try_from(span.end)
        .map_err(|_| invalid(span, "source span exceeds address space"))?;
    source
        .get(start..end)
        .ok_or_else(|| invalid(span, "source span exceeds retained module"))
}
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn unary(expected: &untrusted::Expected) -> bool {
    expected.stack_in == [Ty::I64]
        && expected.stack_out == [Ty::I64]
        && expected.allowed_effects == EffSet::empty()
}
#[expect(
    tigerstyle::compound_condition,
    reason = "Owner: noble-contracts; the guard is the admission rule verbatim; splitting it would obscure the predicate it states"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn checked(
    body: &execution::Body,
    request: &untrusted::Request,
    environment: &noble_kernel::contracts::Env,
    span: Span,
) -> Result<untrusted::Checked, Diagnostic> {
    let count = body.candidate.nodes.len();
    if !body.texts.is_empty()
        || count != body.candidate.body.len()
        || count == 0
        || count > 2
        || u32::try_from(count).map_or(true, |n| n > request.limits.nodes)
        || body.candidate.format != untrusted::CANDIDATE_FORMAT
        || body.candidate.revision != untrusted::SEMANTIC_REVISION
    {
        return Err(invalid(
            span,
            "named proof has orphan nodes, text or wrong recipe revision",
        ));
    }
    let mut seen = 0u8;
    let mut position = 0;
    let mut occurrences = Ok(());
    while occurrences.is_ok() && position < body.candidate.body.len() {
        occurrences = record_occurrence(body, body.candidate.body[position], &mut seen, span);
        position += 1;
    }
    occurrences?;
    if Some(seen) != (1u8 << count).checked_sub(1) {
        return Err(invalid(span, "named proof has unreachable candidate node"));
    }
    let Outcome::Accepted(result) = acceptance::check(environment, request, &body.candidate) else {
        return Err(invalid(
            span,
            "named recipe is not independently kernel accepted",
        ));
    };
    if result.derivations.len() != body.candidate.body.len()
        || result.interface.stack_in != request.expected.stack_in
        || result.interface.stack_out != request.expected.stack_out
        || result.interface.effects != request.expected.allowed_effects
        || derivation_differs(&result.derivations, &body.candidate.body)
    {
        return Err(invalid(
            span,
            "named recipe derivations differ from complete accepted body",
        ));
    }
    Ok(result)
}
/// Record one body occurrence in the `seen` node mask, refusing a node outside
/// the recipe, a repeated node or a quotation.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the occurrence mask is the caller's locally owned state, borrowed only for one loop-free step"
)]
fn record_occurrence(
    body: &execution::Body,
    id: untrusted::NodeId,
    seen: &mut u8,
    span: Span,
) -> Result<(), Diagnostic> {
    let count = body.candidate.nodes.len();
    let index = usize::try_from(id.0).map_err(|_| invalid(span, "invalid named node"))?;
    if index >= count {
        return Err(invalid(
            span,
            "named node lies outside selected flat recipe",
        ));
    }
    let bit = 1u8 << index;
    if *seen & bit != 0 {
        return Err(invalid(
            span,
            "named proof has duplicate or missing node occurrence",
        ));
    }
    *seen |= bit;
    if matches!(body.candidate.nodes[index], Node::Quotation { .. }) {
        return Err(invalid(
            span,
            "quoted named proof is outside selected v2 rule",
        ));
    }
    Ok(())
}
/// Whether some derivation names another node than the body occurrence at its
/// position, as `derivations.iter().zip(body).any(..)`.
fn derivation_differs(derivations: &[untrusted::Derivation], body: &[untrusted::NodeId]) -> bool {
    let mut index = 0;
    let mut is_different = false;
    while !is_different && index < derivations.len() && index < body.len() {
        is_different = derives_other_node(&derivations[index], body[index]);
        index += 1;
    }
    is_different
}
/// Whether `derived` names another node than `id`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn derives_other_node(derived: &untrusted::Derivation, id: untrusted::NodeId) -> bool {
    derived.node != id
}
fn node(body: &execution::Body, index: usize, span: Span) -> Result<&Node, Diagnostic> {
    let id = body
        .candidate
        .body
        .get(index)
        .ok_or_else(|| invalid(span, "missing named body occurrence"))?;
    body.candidate
        .nodes
        .get(usize::try_from(id.0).map_err(|_| invalid(span, "invalid named node ID"))?)
        .ok_or_else(|| invalid(span, "missing named node"))
}
fn empty_inst(inst: &Inst) -> bool {
    inst.bindings.is_empty()
}
#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-contracts; the item name reads as the domain term at its use sites; renaming would churn the extracted subject"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn named_scheme(env: &noble_kernel::contracts::Env, slot: Definition, owner: u64) -> bool {
    env.kind(slot) == Some(Behavior::Named)
        && is_owned_by(env, slot, owner)
        && env.scheme(slot).is_some_and(|scheme| {
            scheme.var_kinds.is_empty()
                && scheme.stack_in == [Pattern::I64]
                && scheme.stack_out == [Pattern::I64]
                && scheme.effects.is_empty()
        })
}
/// Whether `slot` records `owner`, as
/// `usize::try_from(slot.0).ok().and_then(|index| owners.get(index)) == Some(&Some(owner))`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_owned_by(env: &noble_kernel::contracts::Env, slot: Definition, owner: u64) -> bool {
    match usize::try_from(slot.0) {
        Ok(index) => env.definition_owners.get(index) == Some(&Some(owner)),
        Err(_) => false,
    }
}
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn source_origin(
    goal: &ContractGoal,
    use_record: &NamedUseOrigin,
    limits: Limits,
) -> Result<Span, Diagnostic> {
    let span = use_record.definition_span;
    let module = goal
        .subject
        .source_dependencies
        .get(use_record.module_index)
        .ok_or_else(|| invalid(span, "named original module is not retained"))?;
    if module.owner != use_record.definition_owner
        || module.full_source.is_empty()
        || use_record.definition_name.is_empty()
    {
        return Err(invalid(span, "named original owner is not retained"));
    }
    let definition_source = at(span, &module.full_source)?;
    let body = crate::source::verify_definition_origin(
        crate::source::DefinitionOrigin {
            module_source: &module.full_source,
            module: &module.module,
            version: module.version,
            definition: &use_record.definition_name,
            ordinal: use_record.definition_ordinal,
            span,
        },
        definition_source,
        limits,
    )?;
    if body != use_record.body_span {
        return Err(invalid(span, "named original body span differs"));
    }
    Ok(body)
}
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn source_call(
    goal: &ContractGoal,
    caller: &NamedUseOrigin,
    callee: &NamedUseOrigin,
    limits: Limits,
) -> Result<(), Diagnostic> {
    let span = callee.source_span.ok_or_else(|| {
        invalid(
            callee.definition_span,
            "named caller has no original token span",
        )
    })?;
    let node = callee
        .source_node
        .ok_or_else(|| invalid(span, "named caller has no original source node"))?;
    let subject = &goal.subject;
    let original = crate::source::verify_named_call_origin(
        crate::source::DefinitionOrigin {
            module_source: &subject.module_source,
            module: &subject.module,
            version: subject.version,
            definition: &caller.definition_name,
            ordinal: caller.definition_ordinal,
            span: caller.definition_span,
        },
        node,
        span,
        limits,
    )?;
    let module = subject
        .source_dependencies
        .get(callee.module_index)
        .ok_or_else(|| invalid(span, "callee module source is absent"))?;
    match crate::scan::rsplit_once_byte(original, b'.') {
        Some((scope, word)) => {
            callee_word(word, callee, span)?;
            scoped_call(subject, module, callee, scope, span)
        }
        None => {
            callee_word(original, callee, span)?;
            if module.owner == caller.definition_owner && callee.module_index == caller.module_index
            {
                Ok(())
            } else {
                Err(invalid(
                    span,
                    "unqualified named word has another original owner",
                ))
            }
        }
    }
}
/// Refuse an original word other than the source-resolved callee's name.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn callee_word(word: &str, callee: &NamedUseOrigin, span: Span) -> Result<(), Diagnostic> {
    if word != callee.definition_name {
        return Err(invalid(
            span,
            "original named word differs from source-resolved callee",
        ));
    }
    Ok(())
}
/// Resolve a qualified call scope: either `module@version` naming the callee's
/// module, or one retained import alias bound to exactly that module.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn scoped_call(
    subject: &super::SubjectBinding,
    module: &super::ResolvedSourceModule,
    callee: &NamedUseOrigin,
    scope: &str,
    span: Span,
) -> Result<(), Diagnostic> {
    if names_module(scope, module) {
        return Ok(());
    }
    let Some(position) = alias_position(&subject.named_imports, scope, 0) else {
        return Err(invalid(
            span,
            "original named word has no retained import alias",
        ));
    };
    let binding = &subject.named_imports[position];
    if alias_position(&subject.named_imports, scope, position.saturating_add(1)).is_some()
        || binding.owner != module.owner
        || binding.module_index != callee.module_index
    {
        return Err(invalid(
            span,
            "original import alias was rebound to another module",
        ));
    }
    Ok(())
}
/// Whether `qualified` is `name@version` naming exactly `module`, as
/// `qualified.rsplit_once('@').is_some_and(..)`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn names_module(qualified: &str, module: &super::ResolvedSourceModule) -> bool {
    match crate::scan::rsplit_once_byte(qualified, b'@') {
        Some((name, version)) => {
            name == module.module && version.parse::<u32>() == Ok(module.version)
        }
        None => false,
    }
}
/// The first import at or after `from` bound to `alias`, as the next item of
/// `imports.iter().filter(|binding| binding.alias == alias)`.
fn alias_position(
    imports: &[super::NamedImportBinding],
    alias: &str,
    from: usize,
) -> Option<usize> {
    let mut index = from;
    let mut found = None;
    while found.is_none() && index < imports.len() {
        if is_bound_to(&imports[index], alias) {
            found = Some(index);
        }
        index += 1;
    }
    found
}
/// Whether `binding` is bound to `alias`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_bound_to(binding: &super::NamedImportBinding, alias: &str) -> bool {
    binding.alias == alias
}
fn body_request(
    submission: &execution::Submission,
    definition: &execution::Definition,
) -> untrusted::Request {
    untrusted::Request {
        input_bytes: submission.request.input_bytes,
        expected: definition.expected.clone(),
        limits: submission.request.limits,
    }
}
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn same_derived(left: &untrusted::Checked, right: &untrusted::Checked) -> bool {
    left.interface.stack_in == right.interface.stack_in
        && left.interface.stack_out == right.interface.stack_out
        && left.interface.effects == right.interface.effects
        && left.derivations.len() == right.derivations.len()
        && super::derivations_agree(&left.derivations, &right.derivations)
}
#[expect(
    tigerstyle::compound_condition,
    reason = "Owner: noble-contracts; the guard is the admission rule verbatim; splitting it would obscure the predicate it states"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn source_step(
    goal: &ContractGoal,
    origin: &NamedUseOrigin,
    definition: &execution::Definition,
    checked_definition: &untrusted::Checked,
    limits: Limits,
) -> Result<(), Diagnostic> {
    // Re-parse and re-accept the actual original lexical definition, never a
    // manufactured MC1 program or a source-independent stand-in.
    let module = &goal.subject.source_dependencies[origin.module_index];
    let source = at(origin.definition_span, &module.full_source)?;
    let mut session = crate::source::Session::without_test_hosts();
    let prepared = session.prepare(source, &[], limits).map_err(|_| {
        invalid(
            origin.definition_span,
            "original step cannot be independently prepared",
        )
    })?;
    session
        .commit(prepared)
        .map_err(|_| invalid(origin.definition_span, "original step cannot be committed"))?;
    let replay = session
        .prepare(origin.definition_name.as_bytes(), &[Ty::I64], limits)
        .map_err(|_| {
            invalid(
                origin.definition_span,
                "original step cannot be independently resolved",
            )
        })?;
    let replay = replay
        .submission()
        .ok_or_else(|| invalid(origin.definition_span, "missing reaccepted original step"))?;
    if replay.definitions.len() != 1 {
        return Err(invalid(
            origin.definition_span,
            "original step is not a closed builtin definition",
        ));
    }
    let original = &replay.definitions[0];
    let candidate = &definition.body.candidate;
    let original_first = node(&original.body, 0, origin.definition_span)?;
    let original_second = node(&original.body, 1, origin.definition_span)?;
    let actual_first = node(&definition.body, 0, origin.definition_span)?;
    let actual_second = node(&definition.body, 1, origin.definition_span)?;
    if !matches!(
        (original_first, original_second),
        (
            Node::Literal {
                lit: Lit::I64(1),
                ..
            },
            Node::Invocation {
                def: Definition(4),
                ..
            }
        )
    ) || !matches!(
        (actual_first, actual_second),
        (
            Node::Literal {
                lit: Lit::I64(1),
                ..
            },
            Node::Invocation {
                def: Definition(4),
                ..
            }
        )
    ) || candidate.nodes.len() != original.body.candidate.nodes.len()
        || node_differs(actual_first, original_first)
        || node_differs(actual_second, original_second)
    {
        return Err(invalid(
            origin.definition_span,
            "named step differs from original lexical recipe or Inst",
        ));
    }
    let checked_original = checked(
        &original.body,
        &body_request(replay, original),
        &replay.environment,
        origin.definition_span,
    )?;
    if !same_derived(checked_definition, &checked_original) {
        return Err(invalid(
            origin.definition_span,
            "named step derivations differ from original",
        ));
    }
    Ok(())
}
/// Verify the named v2 source graph in stage order: the subject, then its
/// selected definition and call occurrences, then the two named steps.
fn verify(goal: &ContractGoal, limits: Limits) -> Result<Graph<'_>, Diagnostic> {
    let subject_body_span = verify_subject(goal, limits)?;
    let (selected, selected_check) = verify_selected(goal, subject_body_span)?;
    verify_steps(goal, selected, &selected_check, limits)?;
    Ok(Graph {
        first_use: &goal.subject.named_uses[1],
        second_use: &goal.subject.named_uses[2],
    })
}
/// The subject's shape, accepted submission and original source binding;
/// returns the subject body's span in its original module.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn verify_subject(goal: &ContractGoal, limits: Limits) -> Result<Span, Diagnostic> {
    let subject = &goal.subject;
    let span = subject.source_span;
    if subject_shape_differs(goal) {
        return Err(invalid(
            span,
            "named v2 needs a pure unary subject and one original imported module",
        ));
    }
    if subject.named_uses.len() != 3 {
        return Err(invalid(span, "named v2 use graph is incomplete"));
    }
    let root = &subject.named_uses[0];
    let first_use = &subject.named_uses[1];
    if root.definition_owner == first_use.definition_owner
        || root.module_index == first_use.module_index
    {
        return Err(invalid(
            span,
            "named v2 requires an original imported named definition",
        ));
    }
    if has_repeated_owner(&subject.source_dependencies) {
        return Err(invalid(span, "duplicate named source module"));
    }
    if has_unbound_alias(subject) {
        return Err(invalid(
            span,
            "retained import alias lacks unique original module",
        ));
    }
    let accepted = &subject.accepted_submission;
    let env = &accepted.environment;
    if submission_differs(accepted) {
        return Err(invalid(
            span,
            "named submission or canonical wrapping add differs",
        ));
    }
    checked(&accepted.body, &accepted.request, env, span)?;
    if root_binding_differs(subject, root)? {
        return Err(invalid(
            span,
            "subject source binding differs from original module",
        ));
    }
    source_origin(goal, root, limits)?;
    let subject_body_span = crate::source::verify_definition_origin(
        crate::source::DefinitionOrigin {
            module_source: &subject.module_source,
            module: &subject.module,
            version: subject.version,
            definition: &subject.definition,
            ordinal: subject.definition_ordinal,
            span,
        },
        &subject.definition_source,
        limits,
    )?;
    if subject_body_span != root.body_span {
        return Err(invalid(
            span,
            "selected subject body differs from original source",
        ));
    }
    Ok(subject_body_span)
}
/// The selected subject definition, the accepted root invocation and the two
/// ordered call occurrences inside the subject body at `subject_body_span`;
/// returns the selected definition with its checked body.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn verify_selected(
    goal: &ContractGoal,
    subject_body_span: Span,
) -> Result<(&execution::Definition, untrusted::Checked), Diagnostic> {
    let subject = &goal.subject;
    let span = subject.source_span;
    let root = &subject.named_uses[0];
    let first_use = &subject.named_uses[1];
    let second_use = &subject.named_uses[2];
    let accepted = &subject.accepted_submission;
    let env = &accepted.environment;
    let Some(selected) = crate::scan::find_first(&accepted.definitions, |item| {
        item.definition == root.definition && item.identity == subject.definition_identity
    }) else {
        return Err(invalid(span, "unique selected named subject is absent"));
    };
    if selected_graph_differs(subject, selected) {
        return Err(invalid(
            span,
            "selected subject definition, owner or closed graph differs",
        ));
    }
    let root_node = node(&accepted.body, 0, span)?;
    if !is_plain_invocation(root_node, selected.definition)
        || accepted.body.candidate.body != [root.candidate_node]
    {
        return Err(invalid(
            span,
            "accepted submission root is not the exact subject invocation",
        ));
    }
    let selected_check = checked(&selected.body, &body_request(accepted, selected), env, span)?;
    let first_node = node(&selected.body, 0, span)?;
    let second_node = node(&selected.body, 1, span)?;
    if calls_differ(subject, selected, first_node, second_node) {
        return Err(invalid(
            span,
            "subject body does not contain two distinct ordered source calls",
        ));
    }
    let (Some(a_span), Some(b_span)) = (first_use.source_span, second_use.source_span) else {
        return Err(invalid(
            span,
            "named calls have no lexical source occurrences",
        ));
    };
    if occurrences_outside(subject, subject_body_span, a_span, b_span)? {
        return Err(invalid(
            span,
            "named call occurrence is outside ordered original subject body",
        ));
    }
    Ok((selected, selected_check))
}
/// The two named steps: their source calls and typed occurrences, their fresh
/// specializations of one original step, and that step's export.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn verify_steps(
    goal: &ContractGoal,
    selected: &execution::Definition,
    selected_check: &untrusted::Checked,
    limits: Limits,
) -> Result<(), Diagnostic> {
    let subject = &goal.subject;
    let span = subject.source_span;
    let root = &subject.named_uses[0];
    let first_use = &subject.named_uses[1];
    let second_use = &subject.named_uses[2];
    let accepted = &subject.accepted_submission;
    let env = &accepted.environment;
    source_call(goal, root, first_use, limits)?;
    source_call(goal, root, second_use, limits)?;
    unary_occurrence(first_use, selected_check.derivations.first(), span)?;
    unary_occurrence(second_use, selected_check.derivations.get(1), span)?;
    let first = crate::scan::find_first(&accepted.definitions, |d| {
        d.definition == first_use.definition
    })
    .ok_or_else(|| invalid(span, "first named specialization is absent"))?;
    let second = crate::scan::find_first(&accepted.definitions, |d| {
        d.definition == second_use.definition
    })
    .ok_or_else(|| invalid(span, "second named specialization is absent"))?;
    step_specialization(goal, first_use, first, limits)?;
    step_specialization(goal, second_use, second, limits)?;
    let checked_first = checked(
        &first.body,
        &body_request(accepted, first),
        env,
        first_use.definition_span,
    )?;
    let checked_second = checked(
        &second.body,
        &body_request(accepted, second),
        env,
        second_use.definition_span,
    )?;
    if specializations_differ(first, second, &checked_first, &checked_second) {
        return Err(invalid(
            span,
            "fresh named specializations differ from one original step",
        ));
    }
    source_step(goal, first_use, first, &checked_first, limits)?;
    if step_graph_differs(subject, selected, first, second) {
        return Err(invalid(
            span,
            "named graph has orphan entries or inconsistent original step",
        ));
    }
    if first_use.definition_owner != root.definition_owner {
        let module = &subject.source_dependencies[first_use.module_index];
        crate::source::verify_named_export(
            &module.full_source,
            &module.module,
            module.version,
            &first_use.definition_name,
            first_use.definition_ordinal,
            first_use.definition_span,
            limits,
        )?;
    }
    Ok(())
}
/// Whether the subject is not a pure unary revision-2 subject with three
/// named uses over two source modules.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn subject_shape_differs(goal: &ContractGoal) -> bool {
    let subject = &goal.subject;
    goal.revision != 2
        || !subject.effects.is_empty()
        || subject.input_types != [Ty::I64]
        || subject.output_types != [Ty::I64]
        || subject.named_uses.len() != 3
        || subject.source_dependencies.len() != 2
}
/// Whether the accepted submission is not the unary three-definition
/// canonical wrapping-add environment.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn submission_differs(accepted: &execution::Submission) -> bool {
    let env = &accepted.environment;
    !unary(&accepted.request.expected)
        || accepted.definitions.len() != 3
        || env.defs.len() != env.kinds.len()
        || env.defs.len() != env.deps.len()
        || env.defs.len() != env.definition_owners.len()
        || !super::canonical_wrapping_add(env)
}
/// Whether the root use differs from the subject's own original source
/// binding; refuses a subject span outside its retained module.
fn root_binding_differs(
    subject: &super::SubjectBinding,
    root: &NamedUseOrigin,
) -> Result<bool, Diagnostic> {
    let span = subject.source_span;
    Ok(root.caller_definition.is_some()
        || root.source_node.is_some()
        || root.source_span.is_some()
        || root.definition_identity != subject.definition_identity
        || root.definition_owner != subject.definition_owner
        || root.definition_name != subject.definition
        || root.definition_ordinal != subject.definition_ordinal
        || root.definition_span != span
        || at(span, &subject.module_source)? != subject.definition_source
        || !is_subject_module(subject, root.module_index))
}
/// Whether `selected` is not the unique subject definition whose closed graph
/// is the two named uses that directly follow it.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn selected_graph_differs(
    subject: &super::SubjectBinding,
    selected: &execution::Definition,
) -> bool {
    let accepted = &subject.accepted_submission;
    let env = &accepted.environment;
    let root = &subject.named_uses[0];
    let first_use = &subject.named_uses[1];
    let second_use = &subject.named_uses[2];
    count_root_matches(
        &accepted.definitions,
        root.definition,
        subject.definition_identity,
    ) != 1
        || !named_scheme(env, selected.definition, subject.definition_owner)
        || !unary(&selected.expected)
        || selected.definition.0.checked_add(1) != Some(first_use.definition.0)
        || first_use.definition.0.checked_add(1) != Some(second_use.definition.0)
        || usize::try_from(second_use.definition.0)
            .ok()
            .and_then(|last| last.checked_add(1))
            != Some(env.defs.len())
        || !definition_deps_are(
            &env.deps,
            selected.definition,
            &[first_use.definition, second_use.definition],
        )
}
/// Whether the selected body is not two distinct, ordered, plain source calls
/// of the subject's first and second named uses.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn calls_differ(
    subject: &super::SubjectBinding,
    selected: &execution::Definition,
    first_node: &Node,
    second_node: &Node,
) -> bool {
    let first_use = &subject.named_uses[1];
    let second_use = &subject.named_uses[2];
    !(is_plain_invocation(first_node, first_use.definition)
        && is_plain_invocation(second_node, second_use.definition))
        || selected.body.candidate.body != [first_use.candidate_node, second_use.candidate_node]
        || first_use.candidate_node == second_use.candidate_node
        || first_use.source_node == second_use.source_node
        || first_use.source_node.is_none()
        || second_use.source_node.is_none()
        || first_use.source_node != Some(first_use.candidate_node.0)
        || second_use.source_node != Some(second_use.candidate_node.0)
        || first_use.caller_definition != Some(selected.definition)
        || second_use.caller_definition != Some(selected.definition)
        || first_use.definition == second_use.definition
        || first_use.definition == selected.definition
        || second_use.definition == selected.definition
}
/// Whether the call occurrences `a_span` then `b_span` are not ordered inside
/// the subject body or name empty source; refuses a span outside the module.
fn occurrences_outside(
    subject: &super::SubjectBinding,
    body: Span,
    a_span: Span,
    b_span: Span,
) -> Result<bool, Diagnostic> {
    Ok(a_span.start < body.start
        || a_span.end > body.end
        || b_span.start < a_span.end
        || b_span.end > body.end
        || at(a_span, &subject.module_source)?.is_empty()
        || at(b_span, &subject.module_source)?.is_empty())
}
/// Whether the two fresh named specializations differ as candidates or in
/// their checked derivations.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn specializations_differ(
    first: &execution::Definition,
    second: &execution::Definition,
    checked_first: &untrusted::Checked,
    checked_second: &untrusted::Checked,
) -> bool {
    let a = &first.body.candidate;
    let b = &second.body.candidate;
    a.format != b.format
        || a.revision != b.revision
        || a.body != b.body
        || a.nodes.len() != b.nodes.len()
        || nodes_differ(&a.nodes, &b.nodes)
        || !same_derived(checked_first, checked_second)
}
/// Whether the graph holds a definition outside the subject and its two
/// steps, or the two uses do not name one original step in another module.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn step_graph_differs(
    subject: &super::SubjectBinding,
    selected: &execution::Definition,
    first: &execution::Definition,
    second: &execution::Definition,
) -> bool {
    let root = &subject.named_uses[0];
    let first_use = &subject.named_uses[1];
    let second_use = &subject.named_uses[2];
    has_orphan_definition(
        &subject.accepted_submission.definitions,
        selected.definition,
        first.definition,
        second.definition,
    ) || first_use.definition_owner != second_use.definition_owner
        || first_use.definition_name != second_use.definition_name
        || first_use.definition_ordinal != second_use.definition_ordinal
        || first_use.definition_span != second_use.definition_span
        || first_use.body_span != second_use.body_span
        || first_use.module_index != second_use.module_index
        || has_other_module(&subject.source_dependencies, root, first_use)
}
/// Whether `node` invokes `definition` with an empty `Inst`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_plain_invocation(node: &Node, definition: Definition) -> bool {
    matches!(node, Node::Invocation { def, inst } if *def == definition && empty_inst(inst))
}
/// Whether the module at `index` is the subject's own original module, as
/// the negation of `get(index).is_none_or(..)` over its differences.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_subject_module(subject: &super::SubjectBinding, index: usize) -> bool {
    match subject.source_dependencies.get(index) {
        Some(module) => {
            module.owner == subject.definition_owner
                && module.module == subject.module
                && module.version == subject.version
                && module.full_source == subject.module_source
        }
        None => false,
    }
}
/// Whether some module repeats the owner of an earlier one.
fn has_repeated_owner(modules: &[super::ResolvedSourceModule]) -> bool {
    let mut index = 0;
    let mut is_repeated = false;
    while !is_repeated && index < modules.len() {
        is_repeated = owner_seen_before(modules, index);
        index += 1;
    }
    is_repeated
}
/// Whether a module before `index` has the owner of the module at `index`.
fn owner_seen_before(modules: &[super::ResolvedSourceModule], index: usize) -> bool {
    let owner = modules[index].owner;
    let mut prior = 0;
    let mut is_seen = false;
    while !is_seen && prior < index {
        is_seen = modules[prior].owner == owner;
        prior += 1;
    }
    is_seen
}
/// Whether some retained import alias lacks a unique original module.
fn has_unbound_alias(subject: &super::SubjectBinding) -> bool {
    let mut index = 0;
    let mut is_unbound = false;
    while !is_unbound && index < subject.named_imports.len() {
        is_unbound = is_unbound_alias(subject, index);
        index += 1;
    }
    is_unbound
}
/// Whether the import at `index` has an empty alias, repeats an earlier alias,
/// or names a module index whose retained module has another owner.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_unbound_alias(subject: &super::SubjectBinding, index: usize) -> bool {
    let binding = &subject.named_imports[index];
    binding.alias.is_empty()
        || alias_position(&subject.named_imports, &binding.alias, 0)
            .is_some_and(|first| first < index)
        || match subject.source_dependencies.get(binding.module_index) {
            Some(module) => module.owner != binding.owner,
            None => true,
        }
}
/// The number of definitions in `definition`'s slot or with `identity`, as
/// `definitions.iter().filter(..).count()`.
fn count_root_matches(
    definitions: &[execution::Definition],
    definition: Definition,
    identity: u64,
) -> usize {
    let mut index = 0;
    let mut count = 0usize;
    while index < definitions.len() {
        if is_root_match(&definitions[index], definition, identity) {
            count += 1;
        }
        index += 1;
    }
    count
}
/// Whether `item` occupies `definition`'s slot or carries `identity`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_root_match(item: &execution::Definition, definition: Definition, identity: u64) -> bool {
    item.definition == definition || item.identity == identity
}
/// Whether `definition` depends on exactly `expected`, as
/// `usize::try_from(definition.0).ok().and_then(|index| deps.get(index)).map(Vec::as_slice) == Some(expected)`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn definition_deps_are(
    deps: &[Vec<Definition>],
    definition: Definition,
    expected: &[Definition],
) -> bool {
    match usize::try_from(definition.0) {
        Ok(index) => super::deps_are(deps, index, expected),
        Err(_) => false,
    }
}
/// Refuse a named occurrence whose derivation at the same position, when
/// there is one, is not the pure unary I64 typing of that occurrence's node.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn unary_occurrence(
    use_record: &NamedUseOrigin,
    derived: Option<&untrusted::Derivation>,
    span: Span,
) -> Result<(), Diagnostic> {
    match derived {
        Some(derived)
            if derived.node != use_record.candidate_node
                || derived.interface.stack_in != [Ty::I64]
                || derived.interface.stack_out != [Ty::I64]
                || derived.interface.effects != EffSet::empty() =>
        {
            Err(invalid(span, "named occurrence has wrong typed derivation"))
        }
        Some(_) | None => Ok(()),
    }
}
/// Check one named step: its original source, then its identity, scheme,
/// unary interface and sole builtin dependency.
#[expect(
    tigerstyle::compound_condition,
    reason = "Owner: noble-contracts; the guard is the admission rule verbatim; splitting it would obscure the predicate it states"
)]
fn step_specialization(
    goal: &ContractGoal,
    use_record: &NamedUseOrigin,
    definition: &execution::Definition,
    limits: Limits,
) -> Result<(), Diagnostic> {
    source_origin(goal, use_record, limits)?;
    let env = &goal.subject.accepted_submission.environment;
    if definition.identity != use_record.definition_identity
        || !named_scheme(env, definition.definition, use_record.definition_owner)
        || !unary(&definition.expected)
        || !definition_deps_are(&env.deps, definition.definition, &[Definition(4)])
    {
        return Err(invalid(
            use_record.definition_span,
            "named step identity, scheme or dependencies differ",
        ));
    }
    Ok(())
}
/// Whether two nodes differ in kind, literal, definition or `Inst`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn node_differs(a: &Node, b: &Node) -> bool {
    match (a, b) {
        (Node::Literal { lit: la, inst: ia }, Node::Literal { lit: lb, inst: ib }) => {
            la != lb || !super::same_inst(ia, ib)
        }
        (Node::Invocation { def: da, inst: ia }, Node::Invocation { def: db, inst: ib }) => {
            da != db || !super::same_inst(ia, ib)
        }
        _ => true,
    }
}
/// Whether some pair of nodes at the same position differs, as
/// `left.iter().zip(right).any(..)` over `node_differs`.
fn nodes_differ(left: &[Node], right: &[Node]) -> bool {
    let mut index = 0;
    let mut is_different = false;
    while !is_different && index < left.len() && index < right.len() {
        is_different = node_differs(&left[index], &right[index]);
        index += 1;
    }
    is_different
}
/// Whether some definition is none of the selected subject and its two steps.
fn has_orphan_definition(
    definitions: &[execution::Definition],
    selected: Definition,
    first: Definition,
    second: Definition,
) -> bool {
    let mut index = 0;
    let mut is_orphan = false;
    while !is_orphan && index < definitions.len() {
        is_orphan = is_other_definition(&definitions[index], selected, first, second);
        index += 1;
    }
    is_orphan
}
/// Whether `item` is none of the three given definitions.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_other_definition(
    item: &execution::Definition,
    selected: Definition,
    first: Definition,
    second: Definition,
) -> bool {
    item.definition != selected && item.definition != first && item.definition != second
}
/// Whether some retained module index is neither the root's nor the step's.
fn has_other_module(
    modules: &[super::ResolvedSourceModule],
    root: &NamedUseOrigin,
    step: &NamedUseOrigin,
) -> bool {
    let mut index = 0;
    let mut is_other = false;
    while !is_other && index < modules.len() {
        is_other = index != root.module_index && index != step.module_index;
        index += 1;
    }
    is_other
}

fn bytes(mut out: String, source: &[u8]) -> String {
    out.push('[');
    for (index, byte) in source.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        out.push_str("(UInt8.ofNat ");
        crate::rendering::number(&mut out, u64::from(*byte));
        out.push(')');
    }
    out.push(']');
    out
}
/// `out` with `text` appended exactly as `format!("{text:?}")` renders it.
/// Escaping a non-ASCII character depends on Unicode printability tables, so
/// non-ASCII text is refused: `verify` has already reparsed every module and
/// definition name rendered here as an ASCII identifier.
fn debug_name(mut out: String, text: &str, span: Span) -> Result<String, Diagnostic> {
    let bytes = text.as_bytes();
    let mut ascii = 0;
    while ascii < bytes.len() && bytes[ascii] < 0x80 {
        ascii += 1;
    }
    if ascii != bytes.len() {
        return Err(invalid(span, "named provenance name is not ASCII"));
    }
    out.push('"');
    let mut index = 0;
    while index < bytes.len() {
        out = debug_ascii(out, bytes[index]);
        index += 1;
    }
    out.push('"');
    Ok(out)
}
/// `out` with one ASCII byte appended as `str`'s `Debug` writes it: the six
/// backslash escapes, `\u{..}` in lowercase hexadecimal for every other
/// control byte and DEL, and the byte itself otherwise, a single quote included.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper appends through non-const String pushes, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; this total escape table accepts every byte its only caller passes, ASCII checked by debug_name, and the unit tests pin all 128 renderings"
)]
fn debug_ascii(mut out: String, byte: u8) -> String {
    match byte {
        0 => out.push_str("\\0"),
        b'\t' => out.push_str("\\t"),
        b'\n' => out.push_str("\\n"),
        b'\r' => out.push_str("\\r"),
        b'"' => out.push_str("\\\""),
        b'\\' => out.push_str("\\\\"),
        _ => {
            if byte < 0x20 || byte == 0x7f {
                out.push_str("\\u{");
                if byte >= 0x10 {
                    out.push(hex_digit(byte / 16));
                }
                out.push(hex_digit(byte % 16));
                out.push('}');
            } else {
                out.push(char::from(byte));
            }
        }
    }
    out
}
/// The lowercase hexadecimal digit of `value`, which is below sixteen; the
/// last arm is fifteen.
const fn hex_digit(value: u8) -> char {
    match value {
        0 => '0',
        1 => '1',
        2 => '2',
        3 => '3',
        4 => '4',
        5 => '5',
        6 => '6',
        7 => '7',
        8 => '8',
        9 => '9',
        10 => 'a',
        11 => 'b',
        12 => 'c',
        13 => 'd',
        14 => 'e',
        _ => 'f',
    }
}
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn provenance(
    mut out: String,
    goal: &ContractGoal,
    origin: &NamedUseOrigin,
) -> Result<String, Diagnostic> {
    let module = goal
        .subject
        .source_dependencies
        .get(origin.module_index)
        .ok_or_else(|| invalid(origin.definition_span, "named module is absent"))?;
    out.push_str("{ moduleName := ");
    out = debug_name(out, &module.module, origin.definition_span)?;
    out.push_str(", moduleVersion := ");
    crate::rendering::number(&mut out, u64::from(module.version));
    out.push_str(", moduleSource := ");
    out = bytes(out, &module.full_source);
    out.push_str(", definitionSlot := ");
    crate::rendering::number(&mut out, u64::from(origin.definition_ordinal));
    out.push_str(", definitionName := ");
    out = debug_name(out, &origin.definition_name, origin.definition_span)?;
    out.push_str(", definitionSource := ");
    out = bytes(out, at(origin.definition_span, &module.full_source)?);
    out.push_str(" }");
    Ok(out)
}
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn statement(goal: &ContractGoal, graph: &Graph<'_>, limits: Limits) -> Result<String, Diagnostic> {
    let mut out = String::from("import NobleContracts.NamedV2\nopen NobleContracts\nnamespace NamedV2Obligation\nopen NobleContracts.NamedV1\n");
    out.push_str("private def caller : Provenance := ");
    out = provenance(out, goal, &goal.subject.named_uses[0])?;
    out.push_str("\nprivate def callee : Provenance := ");
    out = provenance(out, goal, graph.first_use)?;
    out = use_entry(out, "A", graph.first_use)?;
    out = use_entry(out, "B", graph.second_use)?;
    out.push_str("\nprivate def env : Environment := { entries := [entryA, entryB] }\n");
    out.push_str("private def root : List NamedV1.Op := [.call useA, .call useB]\n");
    out.push_str("private theorem closure : ExactClosure env caller root [useA.slot, useB.slot] 1 := by\n  simp [ExactClosure, env, entryA, entryB, root, useA, useB, usesCode, usesOp, usesValue, Environment.lookup, Entry.matches]\n");
    out.push_str("private theorem typedA : NamedV1.CodeTyped env entryA.body [.i64] [.i64] := by\n  exact .cons (.lit .i64) (.cons (NamedV1.OpTyped.add (s := [])) .nil)\n");
    out.push_str("private theorem typedB : NamedV1.CodeTyped env entryB.body [.i64] [.i64] := by\n  exact .cons (.lit .i64) (.cons (NamedV1.OpTyped.add (s := [])) .nil)\n");
    out.push_str("private theorem typedRoot : NamedV1.CodeTyped env root [.i64] [.i64] := by\n  exact .cons (NamedV1.OpTyped.call (entry := entryA) (s := []) (by rfl) (by simp [Entry.matches, entryA, useA]) typedA) (.cons (NamedV1.OpTyped.call (entry := entryB) (s := []) (by rfl) (by simp [Entry.matches, entryB, useB]) typedB) .nil)\n");
    out = subject_and_claim(out);
    let is_within_work = u32::try_from(out.len()).is_ok_and(|len| len <= limits.work);
    let is_within_bytes = u64::try_from(out.len())
        .ok()
        .zip(u64::from(limits.bytes).checked_mul(128))
        .is_some_and(|(len, cap)| len <= cap);
    if !is_within_work || !is_within_bytes {
        return Err(invalid(
            goal.contract_span,
            "named proof statement exceeds bounded source budget",
        ));
    }
    Ok(out)
}
/// `out` with the obligation's `subject` and `claim` definitions appended.
/// Their non-ASCII Lean symbols (U+00B7 and U+2082) are appended as code
/// points, because the extraction cannot print non-ASCII string literals.
fn subject_and_claim(mut out: String) -> String {
    out.push_str("def subject : Subject :=\n  { provenance := caller, env, root, dependencies := [useA.slot, useB.slot], rank := 1,\n    closure, checkedEntries := by\n      intro entry he\n      simp only [env, List.mem_cons, List.mem_nil_iff, or_false] at he\n      rcases he with rfl | rfl\n      ");
    out = super::push_code_point(out, 0x00B7);
    out.push_str(" exact typedA\n      ");
    out = super::push_code_point(out, 0x00B7);
    out.push_str(" exact typedB,\n    input := [.i64], output := [.i64], typed := typedRoot }\n");
    out.push_str("def claim : Prop := NobleContracts.NamedV2.exportedNamedClaim subject []\n  (fun before after params => NobleContracts.NamedV2.Holds");
    out = super::push_code_point(out, 0x2082);
    out.push_str(" subject.env (.bool true) before after params)\n  (fun before after params => NobleContracts.NamedV2.Holds");
    out = super::push_code_point(out, 0x2082);
    out.push_str(" subject.env (.eq (.output 0) (.add (.input 0) (.i64 2))) before after params)\nend NamedV2Obligation\n");
    out
}
/// Append the Lean `use` and `entry` definitions of one named occurrence.
fn use_entry(mut out: String, name: &str, record: &NamedUseOrigin) -> Result<String, Diagnostic> {
    let occurrence = record
        .source_node
        .ok_or_else(|| invalid(record.definition_span, "named source occurrence is absent"))?;
    out.push_str("\ndef use");
    out.push_str(name);
    out.push_str(" : Use := { slot := ");
    crate::rendering::number(&mut out, u64::from(record.definition.0));
    out.push_str(", occurrence := ");
    crate::rendering::number(&mut out, u64::from(occurrence));
    out.push_str(", caller, origin := callee, input := [.i64], output := [.i64] }");
    out.push_str("\ndef entry");
    out.push_str(name);
    out.push_str(" : Entry := { slot := ");
    crate::rendering::number(&mut out, u64::from(record.definition.0));
    out.push_str(", origin := callee, input := [.i64], output := [.i64], body := [.lit (.i64 1), .word 4], dependencies := [], rank := 0 }");
    Ok(out)
}
#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-contracts; the item name reads as the domain term at its use sites; renaming would churn the extracted subject"
)]
/// Build the complete, separate Lean v2 obligation from a checked source-bound
/// subject BODY, never from the enclosing invocation or a named v1 promise.
pub fn prepare_named_contract(goal: &ContractGoal, limits: Limits) -> Result<String, Diagnostic> {
    selected_assertion(goal)?;
    let graph = verify(goal, limits)?;
    statement(goal, &graph, limits)
}
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
fn exact_eq_add(form: &Form, input: &str, output: &str) -> bool {
    let Ok(eq) = operation(form, "eq", 2) else {
        return false;
    };
    let Ok(out) = operation(&eq[0], "out", 1) else {
        return false;
    };
    let Ok(add) = operation(&eq[1], "add", 2) else {
        return false;
    };
    let Ok(input_form) = operation(&add[0], "in", 1) else {
        return false;
    };
    atom(&out[0]).ok() == Some(output)
        && atom(&input_form[0]).ok() == Some(input)
        && atom(&add[1]).ok() == Some("2")
}
#[expect(
    tigerstyle::compound_condition,
    reason = "Owner: noble-contracts; the guard is the admission rule verbatim; splitting it would obscure the predicate it states"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn selected_assertion(goal: &ContractGoal) -> Result<(), Diagnostic> {
    if goal.inputs.len() != 1
        || goal.outputs.len() != 1
        || goal.inputs[0].1 != "I64"
        || goal.outputs[0].1 != "I64"
        || atom(&goal.requires).ok() != Some("true")
        || !exact_eq_add(&goal.ensures, &goal.inputs[0].0, &goal.outputs[0].0)
    {
        return Err(invalid(
            goal.contract_span,
            "selected named rule requires true and exact wrapping +2 postcondition",
        ));
    }
    Ok(())
}
pub(super) fn check_contract<'m>(
    obligation: &ProofObligation,
    goal: &ContractGoal,
    mut meter: Meter<'m>,
) -> Transition<'m, CheckedProof> {
    let result = two_call_proof(obligation, goal, &mut meter);
    (meter, result)
}
/// Check the selected two-call named rule of a source proof against its contract.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the meter is the caller's owned transition state, written back through this borrow on every path"
)]
fn two_call_proof(
    obligation: &ProofObligation,
    goal: &ContractGoal,
    meter: &mut Meter<'_>,
) -> Result<CheckedProof, Diagnostic> {
    selected_assertion(goal)?;
    let _ = verify(goal, meter.limits)?;
    // A source proof explicitly supplies both ordered call slices and their
    // literal/add expansions. No unchecked Lean source is accepted as a term.
    let rule = operation(&obligation.term, "export-named-unary-I64", 4)?;
    call_slice(&rule[0], 0)?;
    call_slice(&rule[1], 1)?;
    operation(&rule[2], "by-exact-tail", 0)?;
    operation(&rule[3], "named-true-eq-wrap", 0)?;
    let lean_term = String::from("(NobleContracts.NamedV2.twoStepClaim NamedV2Obligation.subject NamedV2Obligation.useA NamedV2Obligation.useB NamedV2Obligation.entryA NamedV2Obligation.entryB (by rfl) (by rfl) (by rfl) (by decide) (by decide) (by rfl) (by rfl) (by simp [NamedV1.Entry.matches, NamedV2Obligation.entryA, NamedV2Obligation.useA]) (by simp [NamedV1.Entry.matches, NamedV2Obligation.entryB, NamedV2Obligation.useB]) (by rfl) (by rfl))");
    let (next, charged) = meter.charge(lean_term.len(), obligation.span);
    *meter = next;
    charged?;
    Ok(CheckedProof {
        name: obligation.name.clone(),
        kind: ProofKind::NamedContract,
        lean_term,
        claim: String::from("NamedV2Obligation.claim"),
        model_revision: String::new(),
        checker_revision: String::new(),
    })
}
/// Check one ordered named call slice: its source position, literal and add.
fn call_slice(slice: &Form, index: usize) -> Result<(), Diagnostic> {
    let call = operation(slice, "pc-named-call", 3)?;
    if atom(&call[0])?.parse::<usize>().ok() != Some(index) {
        return Err(invalid(
            call[0].span,
            "named call slice does not match source order",
        ));
    }
    operation(&call[1], "exec-literal-1", 0)?;
    operation(&call[2], "exec-add", 0)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    /// The `subject` and `claim` templates as literals, before their non-ASCII
    /// symbols moved to code points.
    const SUBJECT: &str = "def subject : Subject :=\n  { provenance := caller, env, root, dependencies := [useA.slot, useB.slot], rank := 1,\n    closure, checkedEntries := by\n      intro entry he\n      simp only [env, List.mem_cons, List.mem_nil_iff, or_false] at he\n      rcases he with rfl | rfl\n      · exact typedA\n      · exact typedB,\n    input := [.i64], output := [.i64], typed := typedRoot }\n";
    const CLAIM: &str = "def claim : Prop := NobleContracts.NamedV2.exportedNamedClaim subject []\n  (fun before after params => NobleContracts.NamedV2.Holds₂ subject.env (.bool true) before after params)\n  (fun before after params => NobleContracts.NamedV2.Holds₂ subject.env (.eq (.output 0) (.add (.input 0) (.i64 2))) before after params)\nend NamedV2Obligation\n";

    #[test]
    fn subject_and_claim_match_the_literal_templates() {
        let emitted = super::subject_and_claim(alloc::string::String::from("prefix\n"));
        assert_eq!(emitted, alloc::format!("prefix\n{SUBJECT}{CLAIM}"));
    }

    const LIMITS: crate::Limits = crate::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        work: 2_000_000,
    };

    /// The named v2 contract goal of the `tests/named_v2.rs` fixture: a
    /// subject calling one imported step twice.
    fn goal() -> Option<super::ContractGoal> {
        let limit = crate::source::RetainedSourceLimit::new(65_536)?;
        let mut session = crate::source::ModuleSession::new(&[], limit).ok()?;
        for source in [
            b"module Definitions@5 [ export step def step [ 1 + ] ]".as_slice(),
            b"import Definitions@5 as d".as_slice(),
        ] {
            let prepared = session.prepare(source, &[], LIMITS).ok()?;
            let (next, committed) = session.commit(prepared);
            committed.ok()?;
            session = next;
        }
        let prepared = session.prepare(b"module Subject@3 [ def twice [ d.step d.step ] contract 2 twice-law [ subject twice input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 2)) ] ] proof 2 twice-correct for twice-law [ (export-named-unary-I64 (pc-named-call 0 (exec-literal-1) (exec-add)) (pc-named-call 1 (exec-literal-1) (exec-add)) (by-exact-tail) (named-true-eq-wrap)) ] ]", &[], LIMITS).ok()?;
        match &prepared.proof_obligations()?.obligations.first()?.goal {
            super::super::PendingGoal::Contract { contract } => Some(contract.as_ref().clone()),
            super::super::PendingGoal::Pure { .. } => None,
        }
    }

    /// `bytes` as it was written with `to_string`, kept as the oracle.
    fn formatted_bytes(mut out: alloc::string::String, source: &[u8]) -> alloc::string::String {
        out.push('[');
        for (index, byte) in source.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            out.push_str("(UInt8.ofNat ");
            out.push_str(&alloc::string::ToString::to_string(byte));
            out.push(')');
        }
        out.push(']');
        out
    }

    /// `provenance` as it was written with `format!`, kept as the oracle.
    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; the oracle is the replaced implementation verbatim, checked by the tests that compare against it"
    )]
    fn formatted_provenance(
        mut out: alloc::string::String,
        goal: &super::ContractGoal,
        origin: &super::NamedUseOrigin,
    ) -> Result<alloc::string::String, crate::Diagnostic> {
        let module = goal
            .subject
            .source_dependencies
            .get(origin.module_index)
            .ok_or_else(|| super::invalid(origin.definition_span, "named module is absent"))?;
        out.push_str("{ moduleName := ");
        out.push_str(&alloc::format!("{:?}", module.module));
        out.push_str(&alloc::format!(
            ", moduleVersion := {}, moduleSource := ",
            module.version
        ));
        out = formatted_bytes(out, &module.full_source);
        out.push_str(&alloc::format!(
            ", definitionSlot := {}, definitionName := {:?}, definitionSource := ",
            origin.definition_ordinal,
            origin.definition_name
        ));
        out = formatted_bytes(out, super::at(origin.definition_span, &module.full_source)?);
        out.push_str(" }");
        Ok(out)
    }

    /// `use_entry` as it was written with `format!`, kept as the oracle.
    fn formatted_use_entry(
        mut out: alloc::string::String,
        name: &str,
        record: &super::NamedUseOrigin,
    ) -> Result<alloc::string::String, crate::Diagnostic> {
        let occurrence = record.source_node.ok_or_else(|| {
            super::invalid(record.definition_span, "named source occurrence is absent")
        })?;
        out.push_str(&alloc::format!("\ndef use{name} : Use := {{ slot := {}, occurrence := {}, caller, origin := callee, input := [.i64], output := [.i64] }}",
            record.definition.0, occurrence));
        out.push_str(&alloc::format!("\ndef entry{name} : Entry := {{ slot := {}, origin := callee, input := [.i64], output := [.i64], body := [.lit (.i64 1), .word 4], dependencies := [], rank := 0 }}",record.definition.0));
        Ok(out)
    }

    /// Every ASCII character alone, all of them together, and the names the
    /// fixture's reparse admits.
    fn ascii_texts() -> alloc::vec::Vec<alloc::string::String> {
        let mut texts: alloc::vec::Vec<_> = (0u8..0x80)
            .map(|byte| alloc::string::String::from(char::from(byte)))
            .collect();
        texts.push((0u8..0x80).map(char::from).collect());
        texts.push(alloc::string::String::new());
        texts.push(alloc::string::String::from("Definitions_9"));
        texts.push(alloc::string::String::from(
            "it's \"quoted\"\\\t\r\n\u{1b}\u{7f}",
        ));
        texts
    }

    #[test]
    fn debug_name_matches_the_formatted_ascii_rendering() {
        let span = crate::Span { start: 0, end: 0 };
        for text in ascii_texts() {
            assert_eq!(
                super::debug_name(alloc::string::String::from("<"), &text, span).ok(),
                Some(alloc::format!("<{text:?}"))
            );
        }
        for text in ["\u{e9}", "a\u{301}", "\u{200b}", "ok\u{80}"] {
            assert!(super::debug_name(alloc::string::String::new(), text, span).is_err());
        }
    }

    #[test]
    fn bytes_matches_the_formatted_rendering() {
        let every: alloc::vec::Vec<u8> = (0..=u8::MAX).collect();
        for source in [&[][..], &[0][..], &[255, 10, 9][..], every.as_slice()] {
            assert_eq!(
                super::bytes(alloc::string::String::from("x"), source),
                formatted_bytes(alloc::string::String::from("x"), source)
            );
        }
    }

    #[test]
    fn provenance_and_uses_match_the_formatted_rendering() {
        let goal = goal();
        assert!(goal.is_some(), "named v2 fixture");
        let Some(goal) = goal else { return };
        assert!(super::prepare_named_contract(&goal, LIMITS).is_ok());
        let mut origins = goal.subject.named_uses.clone();
        for (index, text) in ascii_texts().into_iter().enumerate() {
            let mut origin = goal.subject.named_uses[index % 3].clone();
            origin.definition_name = text;
            origin.definition_ordinal = [0, 7, u32::MAX][index % 3];
            origin.definition = noble_kernel::contracts::Definition([0, 12, u32::MAX][index % 3]);
            origin.source_node = [Some(0), Some(u32::MAX), None][index % 3];
            origins.push(origin);
        }
        let mut goals = alloc::vec![goal.clone()];
        for (version, name) in [(0, "M"), (u32::MAX, "it's \"q\"\\\n")] {
            let mut changed = goal.clone();
            for module in &mut changed.subject.source_dependencies {
                module.version = version;
                module.module = alloc::string::String::from(name);
            }
            goals.push(changed);
        }
        for goal in &goals {
            for origin in &origins {
                let prefix = || alloc::string::String::from("prefix ");
                assert_eq!(
                    super::provenance(prefix(), goal, origin).ok(),
                    formatted_provenance(prefix(), goal, origin).ok()
                );
                for name in ["A", "B"] {
                    assert_eq!(
                        super::use_entry(prefix(), name, origin).ok(),
                        formatted_use_entry(prefix(), name, origin).ok()
                    );
                }
            }
        }
        let mut foreign = goal.subject.named_uses[0].clone();
        foreign.definition_name = alloc::string::String::from("\u{e9}");
        assert!(formatted_provenance(alloc::string::String::new(), &goal, &foreign).is_ok());
        assert!(super::provenance(alloc::string::String::new(), &goal, &foreign).is_err());
    }

    /// The MC1 contract text of `prepare_checked_contract` as it was written
    /// with `format!`, kept as the oracle.
    #[expect(
        tigerstyle::ambiguous_params,
        reason = "Owner: noble-contracts; the oracle keeps the replaced builder's parameter order, checked by the test that compares against it"
    )]
    fn formatted_contract_text(
        contract: &super::ContractGoal,
        body: &str,
        requires: &str,
        ensures: &str,
    ) -> alloc::string::String {
        let mut text = alloc::format!("(contract 1 {} (input", contract.contract_name);
        for (name, ty) in &contract.inputs {
            text.push_str(&alloc::format!(" ({name} {ty})"));
        }
        text.push_str(") (output");
        for (name, ty) in &contract.outputs {
            text.push_str(&alloc::format!(" ({name} {ty})"));
        }
        text.push_str(&alloc::format!(
            ") (program {body}) (requires {requires}) (ensures {ensures}))"
        ));
        text
    }

    #[test]
    fn contract_text_matches_the_formatted_rendering() {
        let goal = goal();
        assert!(goal.is_some(), "named v2 fixture");
        let Some(goal) = goal else { return };
        let binding = |name: &str, ty: &str| {
            (
                alloc::string::String::from(name),
                alloc::string::String::from(ty),
            )
        };
        let mut contracts = alloc::vec![goal.clone()];
        let mut empty = goal.clone();
        empty.inputs.clear();
        empty.outputs.clear();
        empty.contract_name.clear();
        contracts.push(empty);
        let mut wide = goal;
        wide.contract_name = alloc::string::String::from("law-\u{e9} {x}");
        wide.inputs = alloc::vec![
            binding("x", "I64"),
            binding("flag", "Bool"),
            binding("", "")
        ];
        wide.outputs = alloc::vec![binding("y", "List<I64>"), binding("{z}", "\"Text\"")];
        contracts.push(wide);
        for contract in &contracts {
            for (body, requires, ensures) in [
                ("1 +", "(true)", "(eq (out y) (add (in x) 1))"),
                ("", "", ""),
                ("{} {{", "\u{2227}", "\\n"),
            ] {
                assert_eq!(
                    super::super::contract_text(contract, body, requires, ensures),
                    formatted_contract_text(contract, body, requires, ensures)
                );
            }
        }
    }
}
