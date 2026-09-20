//! Candidate and environment builders shared by the DX-01 diagnostic controls.

pub(super) fn stack(segment: Vec<noble_kernel::types::Ty>) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Stack(segment)
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
            bindings: vec![stack(under)],
        },
    }
}

pub(super) fn quote_node(
    body: Vec<u32>,
    around: Vec<noble_kernel::types::Ty>,
    start: Vec<noble_kernel::types::Ty>,
    end: Vec<noble_kernel::types::Ty>,
) -> noble_kernel::untrusted::Node {
    noble_kernel::untrusted::Node::Quotation {
        body: body
            .into_iter()
            .map(noble_kernel::untrusted::NodeId)
            .collect(),
        inst: noble_kernel::words::Inst {
            bindings: vec![
                stack(around),
                stack(start),
                stack(end),
                noble_kernel::words::Binding::Effect(noble_kernel::types::EffSet::empty()),
            ],
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

pub(super) fn reject(
    candidate: &noble_kernel::untrusted::Candidate,
    request: &noble_kernel::untrusted::Request,
) -> Result<noble_kernel::untrusted::Diagnostic, String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    match noble_kernel::acceptance::check(&env, request, candidate) {
        noble_kernel::untrusted::Outcome::Invalid(diagnostic) => Ok(diagnostic),
        other => Err(format!("expected a rejection, got {other:?}")),
    }
}

/// A candidate over the given nodes and body; an empty candidate still
/// drives environment preflight before any body check.
pub(super) fn candidate(
    nodes: Vec<noble_kernel::untrusted::Node>,
    body: Vec<u32>,
) -> noble_kernel::untrusted::Candidate {
    noble_kernel::untrusted::Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision: 0,
        nodes,
        body: body
            .into_iter()
            .map(noble_kernel::untrusted::NodeId)
            .collect(),
    }
}

pub(super) fn program(
    stack_in: Vec<noble_kernel::types::Ty>,
    stack_out: Vec<noble_kernel::types::Ty>,
) -> noble_kernel::types::Ty {
    noble_kernel::types::Ty::program(stack_in, stack_out, noble_kernel::types::EffSet::empty())
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

/// The bootstrap environment plus named definitions and their dependency
/// lists; the extra definitions start after the 23 table entries.
pub(super) fn env_with_deps(deps: &[Vec<u32>]) -> Result<noble_kernel::contracts::Env, String> {
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

pub(super) fn unsupported(
    outcome: noble_kernel::untrusted::Outcome,
) -> Result<noble_kernel::untrusted::UnsupportedKind, String> {
    match outcome {
        noble_kernel::untrusted::Outcome::Unsupported(kind) => Ok(kind),
        other => Err(format!("expected the unsupported outcome, got {other:?}")),
    }
}
