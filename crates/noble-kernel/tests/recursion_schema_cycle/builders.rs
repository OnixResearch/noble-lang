//! Local builders and fallible outcome inspection for recursion/schema/cycle controls.

pub(super) fn segment(list: Vec<noble_kernel::types::Ty>) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Stack(list)
}

pub(super) fn value(ty: noble_kernel::types::Ty) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Value(ty)
}

pub(super) fn reference(index: u32) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Ref(noble_kernel::words::Variable(index))
}

pub(super) fn lit_node(
    lit: noble_kernel::untrusted::Lit,
    under: Vec<noble_kernel::types::Ty>,
) -> noble_kernel::untrusted::Node {
    noble_kernel::untrusted::Node::Literal {
        lit,
        inst: noble_kernel::words::Inst {
            bindings: vec![segment(under)],
        },
    }
}

pub(super) fn invocation(
    def: noble_kernel::contracts::Definition,
    bindings: Vec<noble_kernel::words::Binding>,
) -> noble_kernel::untrusted::Node {
    noble_kernel::untrusted::Node::Invocation {
        def,
        inst: noble_kernel::words::Inst { bindings },
    }
}

pub(super) fn candidate(
    nodes: Vec<noble_kernel::untrusted::Node>,
    body: Vec<u32>,
) -> noble_kernel::untrusted::Candidate {
    noble_kernel::untrusted::Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision: noble_kernel::untrusted::SEMANTIC_REVISION,
        nodes,
        body: body
            .into_iter()
            .map(noble_kernel::untrusted::NodeId)
            .collect(),
    }
}

pub(super) fn request(
    stack_in: Vec<noble_kernel::types::Ty>,
    stack_out: Vec<noble_kernel::types::Ty>,
) -> noble_kernel::untrusted::Request {
    noble_kernel::untrusted::Request {
        input_bytes: 64,
        expected: noble_kernel::untrusted::Expected {
            stack_in,
            stack_out,
            allowed_effects: noble_kernel::types::EffSet::empty(),
        },
        limits: noble_kernel::untrusted::Limits {
            bytes: 1 << 16,
            nodes: 256,
            depth: 32,
            type_size: 64,
            stack_height: 16,
            work: 10_000,
            diagnostics: 64,
        },
    }
}

/// A compact view of the outcome domain these controls assert on.
#[derive(Debug)]
pub(super) enum Seen {
    Accepted,
    Bad(noble_kernel::untrusted::Diagnostic),
    Exhausted(noble_kernel::untrusted::LimitKind),
}

pub(super) fn seen(
    env: &noble_kernel::contracts::Env,
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
) -> Result<Seen, String> {
    match noble_kernel::acceptance::check(env, request, candidate) {
        noble_kernel::untrusted::Outcome::Accepted(_) => Ok(Seen::Accepted),
        noble_kernel::untrusted::Outcome::Invalid(diagnostic) => Ok(Seen::Bad(diagnostic)),
        noble_kernel::untrusted::Outcome::Exhausted(limit) => Ok(Seen::Exhausted(limit)),
        noble_kernel::untrusted::Outcome::Unsupported(kind) => {
            Err(format!("unexpected unsupported outcome: {kind:?}"))
        }
        noble_kernel::untrusted::Outcome::InternalFailure => {
            Err("unexpected internal failure".to_string())
        }
    }
}

/// One extra named definition: `S -- S Unit`.
pub(super) fn unit_like() -> noble_kernel::words::Scheme {
    noble_kernel::words::Scheme {
        var_kinds: vec![noble_kernel::words::VariableKind::Stack],
        stack_in: vec![noble_kernel::shapes::Pattern::StackVar(
            noble_kernel::words::Variable(0),
        )],
        stack_out: vec![
            noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0)),
            noble_kernel::shapes::Pattern::Unit,
        ],
        effects: vec![],
    }
}

/// The bootstrap environment plus named definitions and their dependency lists.
pub(super) fn env_with(deps: &[Vec<u32>]) -> Result<noble_kernel::contracts::Env, String> {
    let mut env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    let mut index = 0;
    while index < deps.len() {
        env.defs.push(unit_like());
        env.kinds.push(noble_kernel::contracts::Behavior::Named);
        env.deps.push(
            deps[index]
                .iter()
                .map(|id| noble_kernel::contracts::Definition(*id))
                .collect::<Vec<noble_kernel::contracts::Definition>>(),
        );
        index += 1;
    }
    Ok(env)
}

/// An empty-body program: `[] -- []` with no allowed effects.
pub(super) fn empty_program() -> noble_kernel::untrusted::Candidate {
    candidate(vec![], vec![])
}

/// The unsupported kind of one outcome, for identity-naming assertions.
pub(super) fn unsupported_of(
    outcome: noble_kernel::untrusted::Outcome,
) -> Result<noble_kernel::untrusted::UnsupportedKind, String> {
    match outcome {
        noble_kernel::untrusted::Outcome::Unsupported(kind) => Ok(kind),
        other => Err(format!("expected the unsupported outcome, got {other:?}")),
    }
}
