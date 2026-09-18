//! Witness-corruption knobs for the property lane (DX-PROPERTY-01).
//!
//! Each knob is a small, decision-preserving-with-respect-to-both-sides
//! mutation of a generated case: it changes what the candidate claims, not
//! whether the structures stay decodable. Both the kernel checker and the
//! independent oracle must re-decide the corrupted candidate; the harness
//! asserts they agree.

use noble_kernel::types::Ty;
use noble_kernel::untrusted::{Candidate, Node, Request};
use noble_kernel::words::Binding;

use super::rng::Rng;

/// Mutate one type: flip a base constructor or wrap the type once.
fn mutate_type(ty: &mut Ty) {
    match ty {
        Ty::Unit => *ty = Ty::Bool,
        Ty::Bool => *ty = Ty::I64,
        Ty::I64 => *ty = Ty::Text,
        Ty::Text => *ty = Ty::Syntax,
        Ty::Syntax => *ty = Ty::Unit,
        Ty::Pair(left, _) => mutate_type(left),
        Ty::Sum(left, _) => mutate_type(left),
        Ty::List(inner) => mutate_type(inner),
        Ty::Program(_, _, _) => *ty = Ty::List(Box::new(Ty::I64)),
        Ty::Resource(_) => *ty = Ty::I64,
    }
}

/// Collect every (node index, binding index) whose binding is a stack of at
/// least one type, plus every node's binding count.
struct Sites {
    stacks: Vec<(usize, usize)>,
    bindings: Vec<(usize, usize)>,
    quotes: Vec<usize>,
}

fn sites(candidate: &Candidate) -> Sites {
    let mut found = Sites {
        stacks: Vec::new(),
        bindings: Vec::new(),
        quotes: Vec::new(),
    };
    let mut index = 0;
    while index < candidate.nodes.len() {
        let node = &candidate.nodes[index];
        if matches!(node, Node::Quotation { .. }) {
            found.quotes.push(index);
        }
        let inst = match node {
            Node::Literal { inst, .. }
            | Node::Invocation { inst, .. }
            | Node::Quotation { inst, .. } => inst,
        };
        if !inst.bindings.is_empty() {
            found.bindings.push((index, inst.bindings.len()));
            let mut slot = 0;
            while slot < inst.bindings.len() {
                if let Binding::Stack(segment) = &inst.bindings[slot] {
                    if !segment.is_empty() {
                        found.stacks.push((index, slot));
                    }
                }
                slot += 1;
            }
        }
        index += 1;
    }
    found
}

/// Apply exactly one knob, chosen by the seed.
pub fn apply(rng: &mut Rng, request: &mut Request, candidate: &mut Candidate) {
    let found = sites(candidate);
    let choice = rng.below(9);
    match choice {
        0 => {
            // Kind corruption: the first binding of a random node gets the
            // opposite variable kind.
            if found.bindings.is_empty() {
                return;
            }
            let (node_index, _) = found.bindings[rng.below(found.bindings.len() as u64) as usize];
            {
                let inst = inst_of(&mut candidate.nodes[node_index]);
                let first = inst.bindings.first_mut();
                if let Some(binding) = first {
                    *binding = match binding {
                        Binding::Stack(_) => Binding::Value(Ty::Unit),
                        _ => Binding::Stack(Vec::new()),
                    };
                }
            }
        }
        1 | 2 => {
            // Arity corruption: drop the last binding, or append one extra.
            if found.bindings.is_empty() {
                return;
            }
            let (node_index, count) =
                found.bindings[rng.below(found.bindings.len() as u64) as usize];
            {
                let inst = inst_of(&mut candidate.nodes[node_index]);
                if choice == 1 {
                    inst.bindings.truncate(count - 1);
                } else {
                    inst.bindings.push(Binding::Stack(Vec::new()));
                }
            }
        }
        3 | 4 => {
            // Stack corruption: mutate one entry, or drop the first entry of
            // one stack binding.
            if found.stacks.is_empty() {
                return;
            }
            let (node_index, slot) = found.stacks[rng.below(found.stacks.len() as u64) as usize];
            {
                let inst = inst_of(&mut candidate.nodes[node_index]);
                if let Binding::Stack(segment) = &mut inst.bindings[slot] {
                    if choice == 3 {
                        mutate_type(&mut segment[0]);
                    } else if segment.len() > 1 {
                        segment.remove(0);
                    } else {
                        segment.clear();
                    }
                }
            }
        }
        5 => {
            // Expected-output corruption.
            if let Some(last) = request.expected.stack_out.last_mut() {
                mutate_type(last);
            } else {
                request.expected.stack_out.push(Ty::I64);
            }
        }
        6 => {
            // Narrow the allowed effect bound to empty.
            request.expected.allowed_effects = noble_kernel::types::EffSet::empty();
        }
        7 => {
            // Quotation claim corruption: mutate the claimed output stack.
            if !found.quotes.is_empty() {
                let node_index = found.quotes[rng.below(found.quotes.len() as u64) as usize];
                let inst = inst_of(&mut candidate.nodes[node_index]);
                if let Binding::Stack(segment) = &mut inst.bindings[2] {
                    match segment.last_mut() {
                        Some(last) => mutate_type(last),
                        None => segment.push(Ty::Bool),
                    }
                }
            }
        }
        _ => {
            // Repoint one body entry at another arena node.
            let arena = candidate.nodes.len();
            if arena > 1 && !candidate.body.is_empty() {
                let position = rng.below(candidate.body.len() as u64) as usize;
                let mut target = rng.below(arena as u64) as u32;
                if target == candidate.body[position].0 {
                    target = (target + 1) % arena as u32;
                }
                candidate.body[position] = noble_kernel::untrusted::NodeId(target);
            }
        }
    }
}

fn inst_of(node: &mut Node) -> &mut noble_kernel::words::Inst {
    match node {
        Node::Literal { inst, .. }
        | Node::Invocation { inst, .. }
        | Node::Quotation { inst, .. } => inst,
    }
}
