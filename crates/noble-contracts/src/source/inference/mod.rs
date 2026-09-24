#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; inference mutates only preparation-owned arena/draft bodies, bounded frame/validation worklists, and the shared preparation meter; source trees, supplied types, the environment, and the committed namespace remain borrowed unchanged."
)]

mod completion;
mod operations;
mod traversal;
mod validation;

pub(super) enum DraftKind {
    Literal(noble_kernel::untrusted::Lit),
    Invocation(noble_kernel::contracts::Definition),
    Quotation(alloc::vec::Vec<noble_kernel::untrusted::NodeId>),
}

pub(super) struct Draft {
    pub kind: DraftKind,
    pub variables: alloc::vec::Vec<crate::inference::Variable>,
    pub text: Option<alloc::vec::Vec<u8>>,
    pub span: crate::Span,
}

pub(super) struct Body {
    pub nodes: alloc::vec::Vec<Draft>,
    pub root: alloc::vec::Vec<noble_kernel::untrusted::NodeId>,
    pub input: u32,
    pub output: u32,
    pub effect: u32,
    pub identity: Option<u64>,
    pub span: crate::Span,
}

pub(super) struct State {
    pub arena: crate::inference::Arena,
    pub bodies: alloc::vec::Vec<Body>,
    pub span: crate::Span,
    text_bytes: u32,
    pub(super) definition_base: usize,
}

#[derive(Clone, Copy)]
enum Origin {
    Root,
    Quotation(u32),
    Named,
}

#[derive(Clone, Copy)]
enum TreeKey {
    Root,
    Named(u32),
}

#[derive(Clone, Copy)]
enum BodyKey {
    Root,
    Quotation(u32),
}

pub(super) enum Mode {
    Declaration,
    Submission,
}

struct Frame {
    tree: TreeKey,
    items: BodyKey,
    at: usize,
    input: u32,
    stack: u32,
    effect: u32,
    body: usize,
    sequence: alloc::vec::Vec<noble_kernel::untrusted::NodeId>,
    origin: Origin,
    span: crate::Span,
    caller: Option<crate::Span>,
}

pub(super) struct Scope<'a> {
    pub(super) root: &'a super::Tree,
    pub(super) session: &'a super::Session,
    pub(super) environment: &'a noble_kernel::contracts::Env,
}

impl Scope<'_> {
    #[expect(
        tigerstyle::missing_const_fn,
        tigerstyle::fragile_exhaustive_enum_match,
        reason = "Owner: noble-maintainers; each closed tree selector resolves through checked non-const namespace indexing, with an owned diagnostic on invalid IDs; new selector kinds must receive an explicit resolution rule."
    )]
    fn tree(
        &self,
        tree_key: TreeKey,
        span: crate::Span,
    ) -> Result<&super::Tree, crate::Diagnostic> {
        match tree_key {
            TreeKey::Root => Ok(self.root),
            TreeKey::Named(id) => {
                match self
                    .session
                    .definitions
                    .get(attempt!(crate::offset(id, span)))
                {
                    Some(definition) => Ok(&definition.tree),
                    None => Err(crate::internal(span)),
                }
            }
        }
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; each closed body selector resolves through checked non-const arena access without cloning; invalid quotation IDs return owned diagnostics and new selector kinds require an explicit rule."
)]
fn items(tree: &super::Tree, body_key: BodyKey) -> Result<&[u32], crate::Diagnostic> {
    match body_key {
        BodyKey::Root => Ok(&tree.body),
        BodyKey::Quotation(id) => {
            let node = attempt!(tree.node(id));
            match &node.kind {
                super::Kind::Quotation(items) => Ok(items),
                _ => Err(crate::internal(node.span)),
            }
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; frame traversal charges work and checks depth/node limits, then solves effects and validates every open interface without choosing values for quantified holes."
)]
pub(super) fn infer(
    scope: Scope<'_>,
    mode: Mode,
    inputs: &[noble_kernel::types::Ty],
    meter: &mut crate::Meter,
) -> Result<State, crate::Diagnostic> {
    let tree = scope.root;
    let mut state = attempt!(initial(tree, scope.session, mode, inputs, meter));
    state.definition_base = scope.environment.defs.len();
    attempt!(meter.node(tree.span));
    let mut frames = alloc::vec::Vec::with_capacity(1);
    let body = match state.bodies.first() {
        Some(body) => body,
        None => return Err(crate::internal(tree.span)),
    };
    frames.push(Frame {
        tree: TreeKey::Root,
        items: BodyKey::Root,
        at: 0,
        input: body.input,
        stack: body.input,
        effect: body.effect,
        body: 0,
        sequence: alloc::vec::Vec::new(),
        origin: Origin::Root,
        span: tree.span,
        caller: None,
    });
    let mut failure = None;
    while let Some(frame) = frames.pop() {
        if let Err(problem) = traversal::visit(frame, &mut frames, &mut state, &scope, meter) {
            failure = Some(problem);
            break;
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    attempt!(state.arena.solve_effects(tree.span, meter));
    attempt!(validation::open(&state, meter));
    Ok(state)
}

#[expect(
    clippy::vec_init_then_push,
    reason = "Owner: noble-maintainers; explicit owned push avoids the pinned Aeneas erased-region failure in vec! array conversion; reassess when the translator supports it."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; input-stack construction, the fresh effect term, and the sole root body are charged before allocation; supplied inputs and declaration holes follow separate checked paths."
)]
fn initial(
    tree: &super::Tree,
    session: &super::Session,
    mode: Mode,
    inputs: &[noble_kernel::types::Ty],
    meter: &mut crate::Meter,
) -> Result<State, crate::Diagnostic> {
    let mut arena =
        crate::inference::Arena::source(session.effect_universe(), session.bindings.is_some());
    #[expect(
        tigerstyle::fragile_exhaustive_enum_match,
        reason = "Owner: noble-maintainers; declarations must retain quantified stack holes while executable submissions use their supplied stack; any new inference mode must make this choice explicitly."
    )]
    let input = match mode {
        Mode::Submission => attempt!(arena.stack(inputs, tree.span, meter)),
        Mode::Declaration => attempt!(arena.add(
            crate::inference::Term::Hole(crate::inference::Sort::Stack),
            tree.span,
            meter
        )),
    };
    let effect = attempt!(arena.effect_empty(tree.span, meter));
    attempt!(meter.node(tree.span));
    let mut bodies = alloc::vec::Vec::with_capacity(1);
    bodies.push(Body {
        nodes: alloc::vec::Vec::new(),
        root: alloc::vec::Vec::new(),
        input,
        output: input,
        effect,
        identity: None,
        span: tree.span,
    });
    Ok(State {
        arena,
        bodies,
        span: tree.span,
        text_bytes: 0,
        definition_base: 24,
    })
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; appending an owned draft allocates after node charging and checked body/ID lookup, whose failure paths construct diagnostics."
)]
fn add(
    body: usize,
    draft: Draft,
    state: &mut State,
    meter: &mut crate::Meter,
) -> Result<noble_kernel::untrusted::NodeId, crate::Diagnostic> {
    attempt!(meter.node(draft.span));
    let body = match state.bodies.get_mut(body) {
        Some(body) => body,
        None => return Err(crate::internal(draft.span)),
    };
    let id = noble_kernel::untrusted::NodeId(attempt!(crate::index(body.nodes.len(), draft.span)));
    body.nodes.push(draft);
    Ok(id)
}
