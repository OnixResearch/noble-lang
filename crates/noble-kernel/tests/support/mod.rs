//! Shared builders for the acceptance controls.

pub fn i64_ty() -> noble_kernel::types::Ty {
    noble_kernel::types::Ty::I64
}

pub fn text_ty() -> noble_kernel::types::Ty {
    noble_kernel::types::Ty::Text
}

pub fn unit_ty() -> noble_kernel::types::Ty {
    noble_kernel::types::Ty::Unit
}

pub fn bool_ty() -> noble_kernel::types::Ty {
    noble_kernel::types::Ty::Bool
}

pub fn ids(list: &[u32]) -> noble_kernel::types::EffSet {
    let effects: Vec<noble_kernel::types::EffId> = list
        .iter()
        .map(|id| noble_kernel::types::EffId(*id))
        .collect();
    noble_kernel::types::EffSet::from_ids(&effects)
}

pub fn segment(list: Vec<noble_kernel::types::Ty>) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Stack(list)
}

pub fn value(ty: noble_kernel::types::Ty) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Value(ty)
}

pub fn effect_binding(list: &[u32]) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Effect(ids(list))
}

pub fn inst(list: Vec<noble_kernel::words::Binding>) -> noble_kernel::words::Inst {
    noble_kernel::words::Inst { bindings: list }
}

pub fn limits() -> noble_kernel::untrusted::Limits {
    noble_kernel::untrusted::Limits {
        bytes: 1 << 16,
        nodes: 256,
        depth: 32,
        type_size: 64,
        stack_height: 16,
        work: 10_000,
        diagnostics: 64,
    }
}

pub fn request(
    stack_in: Vec<noble_kernel::types::Ty>,
    stack_out: Vec<noble_kernel::types::Ty>,
    allowed: &[u32],
) -> noble_kernel::untrusted::Request {
    noble_kernel::untrusted::Request {
        input_bytes: 64,
        expected: noble_kernel::untrusted::Expected {
            stack_in,
            stack_out,
            allowed_effects: ids(allowed),
        },
        limits: limits(),
    }
}

pub fn lit_node(
    lit: noble_kernel::untrusted::Lit,
    stack: Vec<noble_kernel::types::Ty>,
) -> noble_kernel::untrusted::Node {
    noble_kernel::untrusted::Node::Literal {
        lit,
        inst: inst(vec![segment(stack)]),
    }
}

pub fn quote_node(
    body: Vec<u32>,
    r: Vec<noble_kernel::types::Ty>,
    a: Vec<noble_kernel::types::Ty>,
    c: Vec<noble_kernel::types::Ty>,
    e: &[u32],
) -> noble_kernel::untrusted::Node {
    noble_kernel::untrusted::Node::Quotation {
        body: body
            .into_iter()
            .map(noble_kernel::untrusted::NodeId)
            .collect(),
        inst: inst(vec![segment(r), segment(a), segment(c), effect_binding(e)]),
    }
}

pub fn invocation(
    def: noble_kernel::contracts::Definition,
    bindings: Vec<noble_kernel::words::Binding>,
) -> noble_kernel::untrusted::Node {
    noble_kernel::untrusted::Node::Invocation {
        def,
        inst: inst(bindings),
    }
}

pub fn candidate(
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

/// A compact view of the checker's five-way outcome domain.
pub enum Seen {
    Accepted(noble_kernel::untrusted::Checked),
    Bad(noble_kernel::untrusted::Diagnostic),
    Exhausted(noble_kernel::untrusted::LimitKind),
}

pub fn seen(
    env: &noble_kernel::contracts::Env,
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
) -> Result<Seen, String> {
    match noble_kernel::acceptance::check(env, request, candidate) {
        noble_kernel::untrusted::Outcome::Accepted(checked) => Ok(Seen::Accepted(checked)),
        noble_kernel::untrusted::Outcome::Invalid(diagnostic) => Ok(Seen::Bad(diagnostic)),
        noble_kernel::untrusted::Outcome::Unsupported(kind) => {
            Err(format!("unsupported: {kind:?}"))
        }
        noble_kernel::untrusted::Outcome::Exhausted(limit) => Ok(Seen::Exhausted(limit)),
        noble_kernel::untrusted::Outcome::InternalFailure => Err("internal failure".to_string()),
    }
}
