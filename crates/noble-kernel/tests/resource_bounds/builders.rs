//! Shared builders for the resource-bound controls.
//!
//! Every control builds its request here so the limits under test are visible
//! beside the scenario, and reads the outcome through one exhaustive view.

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

/// A compact view of the outcome domain these controls assert on.
#[derive(Debug)]
pub(super) enum Seen {
    Accepted,
    Bad(noble_kernel::untrusted::Diagnostic),
    Exhausted(noble_kernel::untrusted::LimitKind),
}

pub(super) fn segment(list: Vec<noble_kernel::types::Ty>) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Stack(list)
}

pub(super) fn value(ty: noble_kernel::types::Ty) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Value(ty)
}

pub(super) fn reference(index: u32) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Ref(noble_kernel::words::Variable(index))
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

pub(super) fn limits(
    work: u32,
    stack_height: u32,
    type_size: u32,
) -> noble_kernel::untrusted::Limits {
    noble_kernel::untrusted::Limits {
        bytes: 1 << 20,
        nodes: 1_024,
        depth: 32,
        type_size,
        stack_height,
        work,
        diagnostics: 64,
    }
}

pub(super) fn request(
    stack_in: Vec<noble_kernel::types::Ty>,
    stack_out: Vec<noble_kernel::types::Ty>,
    limits: noble_kernel::untrusted::Limits,
) -> noble_kernel::untrusted::Request {
    noble_kernel::untrusted::Request {
        input_bytes: 64,
        expected: noble_kernel::untrusted::Expected {
            stack_in,
            stack_out,
            allowed_effects: noble_kernel::types::EffSet::empty(),
        },
        limits,
    }
}

/// A right-nested `List` value: `List^k(I64)`.
///
/// The value is built by one loop, so the control itself never needs a deep
/// call stack to state a deep type.
pub(super) fn deep_list(depth: u32) -> noble_kernel::types::Ty {
    let mut ty = noble_kernel::types::Ty::I64;
    let mut index = 0;
    while index < depth {
        ty = noble_kernel::types::Ty::List(Box::new(ty));
        index += 1;
    }
    ty
}

/// The number of nodes `deep_list(depth)` holds.
pub(super) fn deep_list_nodes(depth: u32) -> u32 {
    depth.saturating_add(1)
}
