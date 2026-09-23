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
    match noble_kernel::acceptance::check(env, request, candidate) {
        noble_kernel::untrusted::Outcome::Accepted(checked) => Ok(checked),
        noble_kernel::untrusted::Outcome::Invalid(diagnostic) => {
            let at = match diagnostic.node {
                Some(node) => {
                    let index = attempt!(crate::offset(node.0, span));
                    match spans.get(index) {
                        Some(at) => *at,
                        None => span,
                    }
                }
                None => span,
            };
            let message = match diagnostic.constraint {
                noble_kernel::untrusted::Constraint::StackJoin
                | noble_kernel::untrusted::Constraint::StackOrder => {
                    "kernel rejected a program stack join or output interface"
                }
                noble_kernel::untrusted::Constraint::Eligibility(_) => {
                    "kernel rejected a non-capturable value"
                }
                noble_kernel::untrusted::Constraint::CyclicWitness => {
                    "kernel rejected a cyclic witness"
                }
                noble_kernel::untrusted::Constraint::EffectInclusion(_)
                | noble_kernel::untrusted::Constraint::UnknownEffect(_)
                | noble_kernel::untrusted::Constraint::InstantiationKind
                | noble_kernel::untrusted::Constraint::InstantiationArity
                | noble_kernel::untrusted::Constraint::MalformedReference(_)
                | noble_kernel::untrusted::Constraint::UnknownDefinition(_) => {
                    "kernel rejected the ordinary typing witness"
                }
            };
            let mut problem = crate::invalid(at, message);
            if let Ok(shapes) = rejection_shapes(&diagnostic, request, at) {
                problem.message.push_str("; ");
                problem.message.push_str(&shapes);
            }
            Err(problem)
        }
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
    }
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
    let mut arena = crate::inference::Arena::source(true);
    let expected = attempt!(arena.stack(&diagnostic.expected, span, &mut meter));
    let actual = attempt!(arena.stack(&diagnostic.actual, span, &mut meter));
    arena.join_message(expected, actual, span, &mut meter)
}
