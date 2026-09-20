//! Independent acceptance oracle for the M2 fragment (DX-PROPERTY-01).
//!
//! This decision function is written separately from the kernel checker,
//! from the fragment's documented rule table ([verification/m2-fragment.md]
//! "Environment contracts" and "Rules"): its own type representation
//! (`otypes`), its own contract table (`table`), its own tail-join, and its
//! own frame machine below. It reads the kernel's public *input* structures
//! only — it calls no kernel decision code. The harness asserts the kernel
//! checker and this oracle agree on every generated candidate.
//!
//! The harness environment provides one effect identity (`test.emit`, id 0)
//! and one resource kind, so the oracle collapses resource kinds to one
//! constructor; no kernel decision distinguishes resource kinds.

/// Literals push their type, quotations expose their claimed program, and
/// invocations go through the independent word table.
fn face(node: &noble_kernel::untrusted::Node) -> Option<super::table::Interface> {
    match node {
        noble_kernel::untrusted::Node::Literal { lit, inst } => {
            super::table::exact_arity(inst, 1)?;
            let under = super::table::stack_at(inst, 0)?;
            let lit_ty = match lit {
                noble_kernel::untrusted::Lit::I64(_) => super::otypes::Type::I64,
                noble_kernel::untrusted::Lit::Bool(_) => super::otypes::Type::Bool,
                noble_kernel::untrusted::Lit::Text => super::otypes::Type::Text,
                noble_kernel::untrusted::Lit::Unit => super::otypes::Type::Unit,
            };
            Some(super::table::Interface {
                input: under.clone(),
                output: super::table::append(under, &[lit_ty]),
                latent: vec![],
            })
        }
        noble_kernel::untrusted::Node::Invocation { def, inst } => {
            super::table::word_face(def.0, inst)
        }
        noble_kernel::untrusted::Node::Quotation { inst, .. } => {
            super::table::exact_arity(inst, 4)?;
            let (around, start, end, claimed) = (
                super::table::stack_at(inst, 0)?,
                super::table::stack_at(inst, 1)?,
                super::table::stack_at(inst, 2)?,
                super::table::effects_at(inst, 3)?,
            );
            let mut output = around.clone();
            output.push(super::otypes::Type::Program(start, end, claimed));
            Some(super::table::Interface {
                input: around,
                output,
                latent: vec![],
            })
        }
    }
}

/// One in-progress body fold. `origin` names the quotation whose body this
/// frame checks; the entry frame has none. Bodies borrow the fixed arena.
struct Frame<'a> {
    body: &'a [noble_kernel::untrusted::NodeId],
    index: usize,
    stack: Vec<super::otypes::Type>,
    effects: Vec<u32>,
    claimed_out: Vec<super::otypes::Type>,
    claimed_effects: Vec<u32>,
    origin: Option<noble_kernel::untrusted::NodeId>,
}

impl<'a> Frame<'a> {
    fn quotation(
        body: &'a [noble_kernel::untrusted::NodeId],
        inst: &noble_kernel::words::Inst,
        origin: noble_kernel::untrusted::NodeId,
    ) -> Option<Self> {
        Some(Self {
            body,
            index: 0,
            stack: super::table::stack_at(inst, 1)?,
            effects: vec![],
            claimed_out: super::table::stack_at(inst, 2)?,
            claimed_effects: super::table::effects_at(inst, 3)?,
            origin: Some(origin),
        })
    }

    fn consume(mut self, interface: super::table::Interface) -> Option<Self> {
        // The harness environment provides only latent identity 0.
        if interface.latent.iter().any(|id| *id != 0) {
            return None;
        }
        let prefix = self.stack.len().checked_sub(interface.input.len())?;
        if self.stack[prefix..] != interface.input[..] {
            return None;
        }
        self.stack.truncate(prefix);
        self.stack.extend(interface.output);
        self.effects = super::otypes::union(&self.effects, &interface.latent);
        self.index += 1;
        Some(self)
    }
}

struct Machine<'a> {
    candidate: &'a noble_kernel::untrusted::Candidate,
    frames: Vec<Frame<'a>>,
}

impl<'a> Machine<'a> {
    fn run(mut self) -> super::otypes::Decision {
        while let Some(frame) = self.frames.pop() {
            if let Some(decision) = self.advance(frame) {
                return decision;
            }
        }
        super::otypes::Decision::Reject
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; advance reserves and mutates the owned frame vector and invokes allocating interface decoders/consumption; those operations and frame destruction are not const."
    )]
    fn advance(&mut self, frame: Frame<'a>) -> Option<super::otypes::Decision> {
        let node_id = match frame.body.get(frame.index) {
            Some(id) => *id,
            None => return self.close(frame),
        };
        let index = match usize::try_from(node_id.0) {
            Ok(index) => index,
            Err(_) => return Some(super::otypes::Decision::Reject),
        };
        let node = match self.candidate.nodes.get(index) {
            Some(node) => node,
            None => return Some(super::otypes::Decision::Reject),
        };
        match node {
            noble_kernel::untrusted::Node::Quotation { body, inst } => {
                let child = match Frame::quotation(body, inst, node_id) {
                    Some(child) => child,
                    None => return Some(super::otypes::Decision::Reject),
                };
                self.frames.reserve(2);
                self.frames.push(frame);
                self.frames.push(child);
            }
            noble_kernel::untrusted::Node::Literal { .. }
            | noble_kernel::untrusted::Node::Invocation { .. } => {
                if !has_eligible_value(node) {
                    return Some(super::otypes::Decision::Reject);
                }
                let next = match face(node).and_then(|interface| frame.consume(interface)) {
                    Some(next) => next,
                    None => return Some(super::otypes::Decision::Reject),
                };
                self.frames.push(next);
            }
        }
        None
    }

    /// Check a completed body's joins, then resume its parent or finish.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; close compares owned type vectors and pops/resumes frames through allocating interface operations and closures; reassess if those runtime frame operations become const."
    )]
    fn close(&mut self, frame: Frame<'a>) -> Option<super::otypes::Decision> {
        if frame.stack != frame.claimed_out
            || !super::otypes::is_subset(&frame.effects, &frame.claimed_effects)
        {
            return Some(super::otypes::Decision::Reject);
        }
        let origin = match frame.origin {
            Some(origin) => origin,
            None => return Some(super::otypes::Decision::Accept),
        };
        let parent = match self.frames.pop() {
            Some(parent) => parent,
            None => return Some(super::otypes::Decision::Reject),
        };
        let index = match usize::try_from(origin.0) {
            Ok(index) => index,
            Err(_) => return Some(super::otypes::Decision::Reject),
        };
        let next = self
            .candidate
            .nodes
            .get(index)
            .and_then(face)
            .and_then(|interface| parent.consume(interface));
        match next {
            Some(parent) => {
                self.frames.push(parent);
                None
            }
            None => Some(super::otypes::Decision::Reject),
        }
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; has_eligible_value mirrors the witness into an owned oracle Type and walks its Data predicate through non-const allocation/Option operations; it is not a const tag-only projection."
)]
fn has_eligible_value(node: &noble_kernel::untrusted::Node) -> bool {
    match node {
        noble_kernel::untrusted::Node::Invocation { def, inst }
            if super::table::requires_data(def.0) =>
        {
            super::table::value_at(inst, 1).is_some_and(|ty| super::otypes::is_data(&ty))
        }
        noble_kernel::untrusted::Node::Literal { .. }
        | noble_kernel::untrusted::Node::Quotation { .. }
        | noble_kernel::untrusted::Node::Invocation { .. } => true,
    }
}

/// Decide one candidate against one request under the oracle's rules.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; decide independently rejects unsupported revisions, oversized arenas, unknown effects and failed frame joins; input validity must produce Reject rather than assertion panics."
)]
pub fn decide(
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
) -> super::otypes::Decision {
    if candidate.format != noble_kernel::untrusted::CANDIDATE_FORMAT
        || candidate.revision != noble_kernel::untrusted::SEMANTIC_REVISION
    {
        return super::otypes::Decision::Reject;
    }
    let is_within_node_limit = u64::try_from(candidate.nodes.len())
        .is_ok_and(|count| count <= u64::from(request.limits.nodes));
    if !is_within_node_limit
        || request
            .expected
            .allowed_effects
            .as_slice()
            .iter()
            .any(|id| id.0 != 0)
    {
        return super::otypes::Decision::Reject;
    }
    Machine {
        candidate,
        frames: vec![Frame {
            body: &candidate.body,
            index: 0,
            stack: super::otypes::oty_stack(&request.expected.stack_in),
            effects: vec![],
            claimed_out: super::otypes::oty_stack(&request.expected.stack_out),
            claimed_effects: request
                .expected
                .allowed_effects
                .as_slice()
                .iter()
                .map(|id| id.0)
                .collect(),
            origin: None,
        }],
    }
    .run()
}
