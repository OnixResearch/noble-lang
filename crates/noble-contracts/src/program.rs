#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; program construction mutates only preparation-owned node-metered inference and draft arenas, depth-checked traversal scratch, and work accounting; diagnostic spelling appends at most 10 bytes to an owned scratch String, while borrowed source, syntax, environment, and published candidates remain unchanged."
)]

mod bindings;
mod forms;
mod traversal;
mod witness;
mod words;

pub(crate) fn bootstrap_word(word: &[u8]) -> Option<noble_kernel::contracts::Definition> {
    words::bootstrap(word)
}

pub(crate) const fn bootstrap_spelling(identity: u32) -> Option<&'static str> {
    words::spelling(identity)
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; spelling appends directly to the caller-owned diagnostic String, requiring growable capacity rather than a borrowed str and avoiding an intermediate allocation."
)]
pub(crate) fn append_bootstrap_spelling(
    definition: noble_kernel::contracts::Definition,
    output: &mut alloc::string::String,
) {
    words::append_spelling(definition, output);
}

#[derive(Clone, Copy)]
pub(crate) struct Context<'a> {
    pub source: &'a [u8],
    pub tree: &'a crate::syntax::Tree,
    pub env: &'a noble_kernel::contracts::Env,
}

#[derive(Clone, Copy)]
struct Form<'a> {
    children: &'a [u32],
    span: crate::Span,
}

struct Frame {
    syntax: u32,
    position: usize,
    stack: u32,
    expected: u32,
    body: alloc::vec::Vec<noble_kernel::untrusted::NodeId>,
    origin: Option<(u32, u32, u32, crate::Span)>,
}

#[octet::sealed_enum]
enum DraftKind {
    Literal(noble_kernel::untrusted::Lit),
    Invocation(noble_kernel::contracts::Definition),
    Quotation(alloc::vec::Vec<noble_kernel::untrusted::NodeId>),
}

struct Draft {
    kind: DraftKind,
    variables: alloc::vec::Vec<crate::inference::Variable>,
    explicit: Option<noble_kernel::words::Inst>,
    span: crate::Span,
}

struct Resolution {
    span: crate::Span,
    arena: crate::inference::Arena,
    frames: alloc::vec::Vec<Frame>,
    drafts: alloc::vec::Vec<Option<Draft>>,
    body: alloc::vec::Vec<noble_kernel::untrusted::NodeId>,
}

impl Resolution {
    fn add(
        &mut self,
        draft: Draft,
        meter: &mut crate::Meter,
    ) -> Result<noble_kernel::untrusted::NodeId, crate::Diagnostic> {
        attempt!(meter.node(draft.span));
        let id = attempt!(crate::index(self.drafts.len(), draft.span));
        self.drafts.push(Some(draft));
        Ok(noble_kernel::untrusted::NodeId(id))
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; syntax shape, inferred stack equations, and traversal failures are checked through diagnostics before kernel acceptance."
)]
pub(crate) fn resolve(
    context: Context<'_>,
    root: u32,
    inputs: &[noble_kernel::types::Ty],
    outputs: &[noble_kernel::types::Ty],
    meter: &mut crate::Meter,
) -> Result<
    (
        noble_kernel::untrusted::Candidate,
        alloc::vec::Vec<crate::Span>,
    ),
    crate::Diagnostic,
> {
    let span = attempt!(context.tree.node(root)).span;
    attempt!(context.tree.square(root));
    let mut arena = crate::inference::Arena::new();
    let stack = attempt!(arena.stack(inputs, span, meter));
    let expected = attempt!(arena.stack(outputs, span, meter));
    let mut state = Resolution {
        span,
        arena,
        frames: alloc::vec::Vec::with_capacity(1),
        drafts: alloc::vec::Vec::new(),
        body: alloc::vec::Vec::new(),
    };
    state.frames.push(Frame {
        syntax: root,
        position: 0,
        stack,
        expected,
        body: alloc::vec::Vec::new(),
        origin: None,
    });
    let mut failure = None;
    while let Some(frame) = state.frames.pop() {
        if let Err(problem) = traversal::step(context, frame, &mut state, meter) {
            failure = Some(problem);
            break;
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    finish(state, meter)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each draft is materialized fallibly and failure stops candidate construction without asserting over inferred input."
)]
fn finish(
    mut state: Resolution,
    meter: &mut crate::Meter,
) -> Result<
    (
        noble_kernel::untrusted::Candidate,
        alloc::vec::Vec<crate::Span>,
    ),
    crate::Diagnostic,
> {
    let mut nodes = alloc::vec::Vec::with_capacity(state.drafts.len());
    let mut spans = alloc::vec::Vec::with_capacity(state.drafts.len());
    let mut at = 0usize;
    let mut failure = None;
    while at < state.drafts.len() {
        match finish_draft(&mut state.drafts, at, &state.arena, state.span, meter) {
            Ok((node, at_span)) => {
                nodes.push(node);
                spans.push(at_span);
                at += 1;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok((
            noble_kernel::untrusted::Candidate {
                format: noble_kernel::untrusted::CANDIDATE_FORMAT,
                revision: noble_kernel::untrusted::SEMANTIC_REVISION,
                nodes,
                body: state.body,
            },
            spans,
        )),
    }
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; consumed draft slots and witness inference have explicit error paths; instantiation materializes owned structural types."
)]
fn finish_draft(
    drafts: &mut [Option<Draft>],
    at: usize,
    arena: &crate::inference::Arena,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(noble_kernel::untrusted::Node, crate::Span), crate::Diagnostic> {
    attempt!(meter.charge(1, span));
    let slot = match drafts.get_mut(at) {
        Some(slot) => slot,
        None => return Err(crate::internal(span)),
    };
    let draft = match slot.take() {
        Some(draft) => draft,
        None => return Err(crate::internal(span)),
    };
    let inst = match draft.explicit {
        Some(inst) => inst,
        None => attempt!(arena.instantiation(&draft.variables, draft.span, meter)),
    };
    let node = match draft.kind {
        DraftKind::Literal(lit) => noble_kernel::untrusted::Node::Literal { lit, inst },
        DraftKind::Invocation(def) => noble_kernel::untrusted::Node::Invocation { def, inst },
        DraftKind::Quotation(body) => noble_kernel::untrusted::Node::Quotation { body, inst },
    };
    Ok((node, draft.span))
}

pub(crate) fn check(
    env: &noble_kernel::contracts::Env,
    candidate: &noble_kernel::untrusted::Candidate,
    request: &noble_kernel::untrusted::Request,
    spans: &[crate::Span],
    span: crate::Span,
) -> Result<noble_kernel::untrusted::Checked, crate::Diagnostic> {
    check_with_work(env, candidate, request, spans, span).0
}

pub(crate) fn check_with_work(
    env: &noble_kernel::contracts::Env,
    candidate: &noble_kernel::untrusted::Candidate,
    request: &noble_kernel::untrusted::Request,
    spans: &[crate::Span],
    span: crate::Span,
) -> (
    Result<noble_kernel::untrusted::Checked, crate::Diagnostic>,
    u32,
) {
    check_with_owner(env, candidate, request, spans, span, None, None)
}

/// Accept one body. When the body applies a named definition, an eligibility
/// refusal names that definition as well as the inner word, so a library
/// specialization such as `wire@1.fanout` is attributable at its use; with no
/// application this is `check_with_work`.
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
pub(crate) fn check_with_application(
    env: &noble_kernel::contracts::Env,
    candidate: &noble_kernel::untrusted::Candidate,
    request: &noble_kernel::untrusted::Request,
    spans: &[crate::Span],
    span: crate::Span,
    application: Option<&str>,
) -> (
    Result<noble_kernel::untrusted::Checked, crate::Diagnostic>,
    u32,
) {
    check_with_owner(env, candidate, request, spans, span, None, application)
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
pub(crate) fn check_signed_structure_with_work(
    env: &noble_kernel::contracts::Env,
    candidate: &noble_kernel::untrusted::Candidate,
    request: &noble_kernel::untrusted::Request,
    spans: &[crate::Span],
    span: crate::Span,
    owner: noble_kernel::contracts::Definition,
    owner_inst: &noble_kernel::words::Inst,
) -> (
    Result<noble_kernel::untrusted::Checked, crate::Diagnostic>,
    u32,
) {
    check_with_owner(
        env,
        candidate,
        request,
        spans,
        span,
        Some((owner, owner_inst)),
        None,
    )
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the kernel checker and diagnostic construction are non-const; invalid candidates are rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn check_with_owner(
    env: &noble_kernel::contracts::Env,
    candidate: &noble_kernel::untrusted::Candidate,
    request: &noble_kernel::untrusted::Request,
    spans: &[crate::Span],
    span: crate::Span,
    owner: Option<(
        noble_kernel::contracts::Definition,
        &noble_kernel::words::Inst,
    )>,
    application: Option<&str>,
) -> (
    Result<noble_kernel::untrusted::Checked, crate::Diagnostic>,
    u32,
) {
    let (outcome, work_used) = kernel_outcome(env, candidate, request, owner);
    let result = match outcome {
        noble_kernel::untrusted::Outcome::Accepted(checked) => Ok(checked),
        noble_kernel::untrusted::Outcome::Invalid(diagnostic) => Err(invalid_rejection(
            &diagnostic,
            request,
            spans,
            span,
            application,
        )),
        noble_kernel::untrusted::Outcome::Unsupported(_) => Err(crate::Diagnostic::new(
            crate::DiagnosticKind::Unsupported,
            span,
            "ordinary typing is outside the kernel fragment",
        )),
        noble_kernel::untrusted::Outcome::Exhausted(_) => Err(crate::Diagnostic::new(
            crate::DiagnosticKind::Exhausted,
            span,
            "ordinary typing exhausted a declared kernel limit",
        )),
        noble_kernel::untrusted::Outcome::InternalFailure => Err(crate::internal(span)),
    };
    (result, work_used)
}

/// Run the kernel checker, with the signed-structure owner when one is given.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; both kernel acceptance entry points are non-const runtime checkers."
)]
fn kernel_outcome(
    env: &noble_kernel::contracts::Env,
    candidate: &noble_kernel::untrusted::Candidate,
    request: &noble_kernel::untrusted::Request,
    owner: Option<(
        noble_kernel::contracts::Definition,
        &noble_kernel::words::Inst,
    )>,
) -> (noble_kernel::untrusted::Outcome, u32) {
    match owner {
        Some((owner, owner_inst)) => noble_kernel::acceptance::check_signed_structure_with_work(
            env, request, candidate, owner, owner_inst,
        ),
        None => noble_kernel::acceptance::check_with_work(env, request, candidate),
    }
}

/// The frontend diagnostic for one kernel rejection: its node's source span,
/// the constraint's message, and either the eligibility detail or the
/// rendered stack shapes.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; building the diagnostic allocates owned messages and renders stack shapes through non-const calls."
)]
fn invalid_rejection(
    diagnostic: &noble_kernel::untrusted::Diagnostic,
    request: &noble_kernel::untrusted::Request,
    spans: &[crate::Span],
    span: crate::Span,
    application: Option<&str>,
) -> crate::Diagnostic {
    let at = match rejection_span(diagnostic.node, spans, span) {
        Ok(at) => at,
        Err(problem) => return problem,
    };
    let problem = crate::invalid(at, rejection_message(&diagnostic.constraint));
    if let Some(definition) = diagnostic.def {
        if let noble_kernel::untrusted::Constraint::Eligibility(ty) = &diagnostic.constraint {
            return with_eligibility(problem, definition, ty, application);
        }
    }
    with_shapes(problem, diagnostic, request, at)
}

/// The source span of the rejected node, or `span` when it has none.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the checked offset conversion and its diagnostic are non-const."
)]
fn rejection_span(
    node: Option<noble_kernel::untrusted::NodeId>,
    spans: &[crate::Span],
    span: crate::Span,
) -> Result<crate::Span, crate::Diagnostic> {
    let node = match node {
        Some(node) => node,
        None => return Ok(span),
    };
    let index = attempt!(crate::offset(node.0, span));
    match spans.get(index) {
        Some(at) => Ok(*at),
        None => Ok(span),
    }
}

/// The frontend message for one kernel constraint.
const fn rejection_message(constraint: &noble_kernel::untrusted::Constraint) -> &'static str {
    match constraint {
        noble_kernel::untrusted::Constraint::StackJoin
        | noble_kernel::untrusted::Constraint::StackOrder => {
            "kernel rejected a program stack join or output interface"
        }
        noble_kernel::untrusted::Constraint::Eligibility(_) => {
            "kernel rejected a non-capturable value"
        }
        noble_kernel::untrusted::Constraint::CyclicWitness => "kernel rejected a cyclic witness",
        noble_kernel::untrusted::Constraint::InvalidType => {
            "kernel rejected an unregistered or mismatched nominal type"
        }
        noble_kernel::untrusted::Constraint::InvalidContract => {
            "kernel rejected a changed module operation or adapter contract"
        }
        noble_kernel::untrusted::Constraint::PrivateDefinition(_) => {
            "kernel rejected a private module operation"
        }
        noble_kernel::untrusted::Constraint::BorrowProvenance => {
            "kernel rejected a missing, mistyped, or incorrectly forwarded borrowed live input"
        }
        noble_kernel::untrusted::Constraint::SlotSite => {
            "kernel rejected a live slot site that differs from its checked source node"
        }
        noble_kernel::untrusted::Constraint::EffectInclusion(_)
        | noble_kernel::untrusted::Constraint::UnknownEffect(_)
        | noble_kernel::untrusted::Constraint::InstantiationKind
        | noble_kernel::untrusted::Constraint::InstantiationArity
        | noble_kernel::untrusted::Constraint::MalformedReference(_)
        | noble_kernel::untrusted::Constraint::UnknownDefinition(_) => {
            "kernel rejected the ordinary typing witness"
        }
    }
}

/// Attach the word, required and actual stacks of a non-capturable value.
/// When the body applies a named definition, the reported word is that
/// application followed by the inner word it instantiated (`wire@1.fanout via
/// dup`), so a library refusal is attributable to the library word.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; this total formatter appends the fixed eligibility detail to an already constructed diagnostic and has no precondition to assert."
)]
fn with_eligibility(
    mut problem: crate::Diagnostic,
    definition: noble_kernel::contracts::Definition,
    ty: &noble_kernel::types::Ty,
    application: Option<&str>,
) -> crate::Diagnostic {
    let mut word = alloc::string::String::new();
    if let Some(application) = application {
        word.push_str(application);
        word.push_str(" via ");
    }
    append_bootstrap_spelling(definition, &mut word);
    let required = alloc::string::String::from("S Data");
    let mut actual = alloc::string::String::from("S ");
    debug_ty(&mut actual, ty);
    problem.message.push_str("; word ");
    problem.message.push_str(&word);
    problem.message.push_str("; required ");
    problem.message.push_str(&required);
    problem.message.push_str("; actual ");
    problem.message.push_str(&actual);
    problem.join_with(crate::JoinDiagnostic {
        word,
        expected_stack: required,
        actual_stack: actual,
        constraint: alloc::string::String::from("eligibility:Data"),
        value_origin: None,
        difference: None,
    })
}

/// Append `ty` exactly as its `{:?}` renders it: the kernel's constructor
/// form, with the derived renderings of the effect bound, resource kind and
/// nominal identity, built by `String` pushes instead of `core::fmt`.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appending a type rendering requires the caller's growable String, not a fixed str slice."
)]
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; each call descends one constructor of the borrowed type, exactly as the kernel's structural Debug recursion this replaces; direct recursion preserves pinned Aeneas extraction without a borrowed work stack."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; type rendering grows the caller's String through non-const push and push_str."
)]
fn debug_ty(out: &mut alloc::string::String, ty: &noble_kernel::types::Ty) {
    match ty {
        noble_kernel::types::Ty::Unit => out.push_str("Unit"),
        noble_kernel::types::Ty::Bool => out.push_str("Bool"),
        noble_kernel::types::Ty::I64 => out.push_str("I64"),
        noble_kernel::types::Ty::Text => out.push_str("Text"),
        noble_kernel::types::Ty::Syntax => out.push_str("Syntax"),
        noble_kernel::types::Ty::Contract => out.push_str("Contract"),
        noble_kernel::types::Ty::Evidence => out.push_str("Evidence"),
        noble_kernel::types::Ty::Certified => out.push_str("Certified"),
        noble_kernel::types::Ty::Pair(left, right) => debug_pair(out, "Pair(", left, right),
        noble_kernel::types::Ty::Sum(left, right) => debug_pair(out, "Sum(", left, right),
        noble_kernel::types::Ty::List(item) => {
            out.push_str("List(");
            debug_ty(out, item);
            out.push(')');
        }
        noble_kernel::types::Ty::Program(stack_in, stack_out, effects) => {
            debug_callable(out, "Program(", stack_in, stack_out, effects);
        }
        noble_kernel::types::Ty::LiveRef(stack_in, stack_out, effects) => {
            debug_callable(out, "LiveRef(", stack_in, stack_out, effects);
        }
        noble_kernel::types::Ty::Resource(kind) => {
            out.push_str("Resource(ResourceKind(");
            crate::rendering::number(out, u64::from(kind.0));
            out.push_str("))");
        }
        noble_kernel::types::Ty::Nominal(id, shape) => {
            out.push_str("Nominal(");
            debug_nominal_id(out, *id);
            out.push_str(", ");
            debug_shape(out, shape);
            out.push(')');
        }
        noble_kernel::types::Ty::GenericNominal(id, args, shape) => {
            out.push_str("GenericNominal(");
            debug_nominal_id(out, *id);
            out.push_str(", ");
            debug_stack(out, &**args);
            out.push_str(", ");
            debug_shape(out, shape);
            out.push(')');
        }
    }
}

/// Append `{opening}{left:?}, {right:?})`.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appending a type rendering requires the caller's growable String, not a fixed str slice."
)]
fn debug_pair(
    out: &mut alloc::string::String,
    opening: &str,
    left: &noble_kernel::types::Ty,
    right: &noble_kernel::types::Ty,
) {
    out.push_str(opening);
    debug_ty(out, left);
    out.push_str(", ");
    debug_ty(out, right);
    out.push(')');
}

/// Append one nominal shape in the kernel's `Opaque(..)`/`Variant(.., ..)` form.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appending a type rendering requires the caller's growable String, not a fixed str slice."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; type rendering grows the caller's String through non-const push and push_str."
)]
fn debug_shape(out: &mut alloc::string::String, shape: &noble_kernel::types::NominalShape) {
    match shape {
        noble_kernel::types::NominalShape::Opaque(representation) => {
            out.push_str("Opaque(");
            debug_ty(out, representation);
            out.push(')');
        }
        noble_kernel::types::NominalShape::Variant(left, right) => {
            debug_pair(out, "Variant(", left, right);
        }
    }
}

/// Append `{opening}[in], [out], EffSet([..]))` for a program or a borrowed
/// live reference.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appending a type rendering requires the caller's growable String, not a fixed str slice."
)]
fn debug_callable(
    out: &mut alloc::string::String,
    opening: &str,
    stack_in: &[noble_kernel::types::Ty],
    stack_out: &[noble_kernel::types::Ty],
    effects: &noble_kernel::types::EffSet,
) {
    out.push_str(opening);
    debug_stack(out, stack_in);
    out.push_str(", ");
    debug_stack(out, stack_out);
    out.push_str(", ");
    debug_effects(out, effects.as_slice());
    out.push(')');
}

/// Append a type stack in the kernel's list form `[a, b]`.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appending a type rendering requires the caller's growable String, not a fixed str slice."
)]
fn debug_stack(out: &mut alloc::string::String, entries: &[noble_kernel::types::Ty]) {
    out.push('[');
    debug_entries(out, entries, 0);
    out.push(']');
}

/// Append the stack entries from `index` on, separated by `, `.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appending a type rendering requires the caller's growable String, not a fixed str slice."
)]
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; each call advances one entry of a finite borrowed stack; a direct tail call keeps the type recursion outside Aeneas's higher-order loop combinator, as in rendering::types."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-contracts; index < entries.len() implies index < usize::MAX, so the immediate recursive successor cannot overflow."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; type rendering grows the caller's String through non-const push and push_str."
)]
fn debug_entries(
    out: &mut alloc::string::String,
    entries: &[noble_kernel::types::Ty],
    index: usize,
) {
    if index < entries.len() {
        if index != 0 {
            out.push_str(", ");
        }
        debug_ty(out, &entries[index]);
        debug_entries(out, entries, index + 1);
    }
}

/// Append the derived rendering `EffSet([EffId(a), EffId(b)])` of the
/// identities `ids` of one effect bound.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appending a type rendering requires the caller's growable String, not a fixed str slice."
)]
fn debug_effects(out: &mut alloc::string::String, ids: &[noble_kernel::types::EffId]) {
    out.push_str("EffSet([");
    let mut index = 0;
    while index < ids.len() {
        if index != 0 {
            out.push_str(", ");
        }
        out.push_str("EffId(");
        crate::rendering::number(out, u64::from(ids[index].0));
        out.push(')');
        index += 1;
    }
    out.push_str("])");
}

/// Append the derived rendering `NominalTypeId { module: m, ordinal: o }`.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appending a type rendering requires the caller's growable String, not a fixed str slice."
)]
fn debug_nominal_id(out: &mut alloc::string::String, id: noble_kernel::types::NominalTypeId) {
    out.push_str("NominalTypeId { module: ");
    crate::rendering::number(out, id.module);
    out.push_str(", ordinal: ");
    crate::rendering::number(out, u64::from(id.ordinal));
    out.push_str(" }");
}

/// Append the rendered expected and actual stacks when they render.
fn with_shapes(
    mut problem: crate::Diagnostic,
    diagnostic: &noble_kernel::untrusted::Diagnostic,
    request: &noble_kernel::untrusted::Request,
    at: crate::Span,
) -> crate::Diagnostic {
    if let Ok(shapes) = rejection_shapes(diagnostic, request, at) {
        problem.message.push_str("; ");
        problem.message.push_str(&shapes);
    }
    problem
}

fn rejection_shapes(
    diagnostic: &noble_kernel::untrusted::Diagnostic,
    request: &noble_kernel::untrusted::Request,
    span: crate::Span,
) -> Result<alloc::string::String, crate::Diagnostic> {
    let mut meter = crate::Meter::new(crate::Limits {
        bytes: 1024,
        nodes: request.limits.nodes.min(1024),
        depth: request.limits.depth,
        work: request.limits.work.min(4096),
    });
    let mut arena = crate::inference::Arena::source(u64::MAX, true);
    let expected = attempt!(arena.stack(&diagnostic.expected, span, &mut meter));
    let actual = attempt!(arena.stack(&diagnostic.actual, span, &mut meter));
    arena.join_message(expected, actual, span, &mut meter)
}

#[cfg(test)]
mod tests {
    //! The format-free type rendering must equal the `format!("S {:?}")` text
    //! it replaced, byte for byte.

    fn id(module: u64, ordinal: u32) -> noble_kernel::types::NominalTypeId {
        noble_kernel::types::NominalTypeId { module, ordinal }
    }

    fn boxed(ty: noble_kernel::types::Ty) -> alloc::boxed::Box<noble_kernel::types::Ty> {
        alloc::boxed::Box::new(ty)
    }

    /// Every constructor, both shapes, empty and nonempty stacks and effect
    /// bounds, extreme identities and a deep nesting.
    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; the sample table is data, checked by the tests that consume it"
    )]
    fn samples() -> alloc::vec::Vec<noble_kernel::types::Ty> {
        #[expect(
            tigerstyle::non_trait_imports,
            reason = "Owner: noble-contracts; the sample table names these kernel types on every line, and the qualified form would bury the shapes under owner paths"
        )]
        use noble_kernel::types::{EffId, EffSet, NominalShape, ResourceKind, Ty};
        let effects = EffSet::from_ids(&[EffId(9), EffId(0), EffId(u32::MAX)]);
        let program = Ty::program(
            alloc::vec![Ty::I64, Ty::List(boxed(Ty::Text))],
            alloc::vec![Ty::Bool],
            effects.clone(),
        );
        let opaque = NominalShape::Opaque(boxed(Ty::I64));
        let variant = NominalShape::Variant(boxed(Ty::Unit), boxed(program.clone()));
        let mut deep = Ty::Bool;
        for depth in 0..64 {
            deep = if depth % 2 == 0 {
                Ty::List(boxed(deep))
            } else {
                Ty::Pair(boxed(Ty::Unit), boxed(deep))
            };
        }
        alloc::vec![
            Ty::Unit,
            Ty::Bool,
            Ty::I64,
            Ty::Text,
            Ty::Syntax,
            Ty::Contract,
            Ty::Evidence,
            Ty::Certified,
            Ty::Pair(boxed(Ty::I64), boxed(Ty::Text)),
            Ty::Sum(
                boxed(Ty::Bool),
                boxed(Ty::Pair(boxed(Ty::Unit), boxed(Ty::I64)))
            ),
            Ty::List(boxed(Ty::List(boxed(Ty::Syntax)))),
            Ty::program(
                alloc::vec::Vec::new(),
                alloc::vec::Vec::new(),
                EffSet::empty()
            ),
            program.clone(),
            Ty::live_ref(alloc::vec![program.clone()], alloc::vec![Ty::I64], effects),
            Ty::live_ref(
                alloc::vec::Vec::new(),
                alloc::vec::Vec::new(),
                EffSet::empty()
            ),
            Ty::Resource(ResourceKind(0)),
            Ty::Resource(ResourceKind(u32::MAX)),
            Ty::Nominal(id(0, 0), alloc::boxed::Box::new(opaque.clone())),
            Ty::Nominal(
                id(u64::MAX, u32::MAX),
                alloc::boxed::Box::new(variant.clone())
            ),
            Ty::GenericNominal(
                id(17, 3),
                alloc::boxed::Box::new([Ty::I64, Ty::List(boxed(Ty::Text))]),
                alloc::boxed::Box::new(variant),
            ),
            Ty::List(boxed(Ty::GenericNominal(
                id(1, 1),
                alloc::boxed::Box::new([program, Ty::Unit]),
                alloc::boxed::Box::new(opaque),
            ))),
            deep,
        ]
    }

    #[test]
    fn debug_ty_matches_the_formatted_rendering() {
        for ty in samples() {
            let mut rendered = alloc::string::String::from("S ");
            super::debug_ty(&mut rendered, &ty);
            assert_eq!(rendered, alloc::format!("S {ty:?}"));
        }
    }

    /// `with_eligibility` as it was written with `format!`, kept as the oracle.
    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; the oracle is the replaced implementation verbatim, checked by the test that compares against it"
    )]
    fn formatted_with_eligibility(
        mut problem: crate::Diagnostic,
        definition: noble_kernel::contracts::Definition,
        ty: &noble_kernel::types::Ty,
        application: Option<&str>,
    ) -> crate::Diagnostic {
        let mut word = alloc::string::String::new();
        super::append_bootstrap_spelling(definition, &mut word);
        let word = match application {
            Some(application) => alloc::format!("{application} via {word}"),
            None => word,
        };
        let required = alloc::string::String::from("S Data");
        let actual = alloc::format!("S {:?}", *ty);
        problem.message.push_str("; word ");
        problem.message.push_str(&word);
        problem.message.push_str("; required ");
        problem.message.push_str(&required);
        problem.message.push_str("; actual ");
        problem.message.push_str(&actual);
        problem.join_with(crate::JoinDiagnostic {
            word,
            expected_stack: required,
            actual_stack: actual,
            constraint: alloc::string::String::from("eligibility:Data"),
            value_origin: None,
            difference: None,
        })
    }

    #[test]
    fn eligibility_detail_matches_the_formatted_detail() {
        let span = crate::Span { start: 3, end: 7 };
        for definition in [0, 1, 4, 999] {
            let definition = noble_kernel::contracts::Definition(definition);
            for ty in samples() {
                for application in [None, Some("wire@1.fanout")] {
                    let problem = || crate::invalid(span, "kernel rejected a non-capturable value");
                    let actual = super::with_eligibility(problem(), definition, &ty, application);
                    let expected =
                        formatted_with_eligibility(problem(), definition, &ty, application);
                    assert_eq!(alloc::format!("{actual:?}"), alloc::format!("{expected:?}"));
                }
            }
        }
    }
}
