//! Structural corruption for the separate malformed-candidate lane
//! (DX-PROPERTY-02). Every corruption here produces input that a correct
//! kernel must reject or bound — never accept, never panic, never undefined
//! behavior. The lane runs under the debug profile, which the workspace pins
//! to `overflow-checks = true`.

use noble_kernel::types::Ty;
use noble_kernel::untrusted::{Candidate, Node, NodeId};
use noble_kernel::words::{Binding, Inst};

use super::gen::Case;
use super::rng::Rng;

/// The name of each corruption, reported with the lane summary.
pub fn label(choice: u64) -> &'static str {
    match choice {
        0 => "format-revision",
        1 => "semantic-revision",
        2 => "body-reference-out-of-range",
        3 => "reference-u32-max",
        4 => "unknown-definition",
        5 => "instantiation-arity-minus",
        6 => "instantiation-arity-plus",
        7 => "instantiation-kind",
        8 => "oversized-stack-binding",
        9 => "depth-chain",
        10 => "truncated-arena",
        _ => "raw-identifier-bits",
    }
}

pub const KINDS: u64 = 12;

/// Corrupt one freshly generated case in exactly one structural way.
pub fn corrupt(rng: &mut Rng, mut case: Case) -> (Case, &'static str) {
    let choice = rng.below(KINDS);
    let label = label(choice);
    match choice {
        0 => case.candidate.format = 1 + rng.below(1000) as u32,
        1 => case.candidate.revision = 1 + rng.below(1000) as u32,
        2 => {
            let last = case.candidate.body.len() - 1;
            case.candidate.body[last] = NodeId(2000 + rng.below(400) as u32);
        }
        3 => {
            let quotes: Vec<usize> = case
                .candidate
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| matches!(node, Node::Quotation { .. }))
                .map(|(index, _)| index)
                .collect();
            if quotes.is_empty() {
                case.candidate.body[0] = NodeId(u32::MAX);
            } else {
                let target = quotes[rng.below(quotes.len() as u64) as usize];
                if let Node::Quotation { body, .. } = &mut case.candidate.nodes[target] {
                    if body.is_empty() {
                        body.push(NodeId(u32::MAX));
                    } else {
                        body[0] = NodeId(u32::MAX);
                    }
                }
            }
        }
        4 => {
            if let Some(node) = case
                .candidate
                .nodes
                .iter_mut()
                .find(|node| matches!(node, Node::Invocation { .. }))
            {
                if let Node::Invocation { def, .. } = node {
                    *def = noble_kernel::contracts::Definition(100 + rng.below(200) as u32);
                }
            } else {
                case.candidate.nodes.push(Node::Invocation {
                    def: noble_kernel::contracts::Definition(u32::MAX),
                    inst: Inst {
                        bindings: vec![Binding::Stack(Vec::new())],
                    },
                });
                let last = NodeId(case.candidate.nodes.len() as u32 - 1);
                case.candidate.body.push(last);
            }
        }
        5 | 6 => {
            let first = first_with_bindings(&mut case.candidate);
            if let Some(node) = first {
                let inst = inst_of(node);
                if choice == 5 && inst.bindings.len() > 1 {
                    inst.bindings.pop();
                } else {
                    inst.bindings.push(Binding::Value(Ty::Unit));
                }
            }
        }
        7 => {
            if let Some(node) = first_with_bindings(&mut case.candidate) {
                let inst = inst_of(node);
                if let Some(binding) = inst.bindings.first_mut() {
                    *binding = match binding {
                        Binding::Stack(_) => Binding::Value(Ty::Unit),
                        _ => Binding::Stack(Vec::new()),
                    };
                }
            }
        }
        8 => {
            let oversized: Vec<Ty> = (0..40).map(|_| Ty::I64).collect();
            if let Some(node) = case.candidate.nodes.first_mut() {
                let inst = inst_of(node);
                if let Some(binding) = inst.bindings.first_mut() {
                    *binding = Binding::Stack(oversized);
                }
            }
        }
        9 => {
            // A chain of nested quotations deeper than the declared depth
            // limit (8): the chain must stop at `Exhausted(Depth)`.
            let depth = 12u32;
            let mut nodes: Vec<Node> = Vec::new();
            let mut level = 0;
            while level < depth {
                let body = if level + 1 < depth {
                    vec![NodeId(level + 1)]
                } else {
                    Vec::new()
                };
                nodes.push(Node::Quotation {
                    body,
                    inst: Inst {
                        bindings: vec![
                            Binding::Stack(Vec::new()),
                            Binding::Stack(Vec::new()),
                            Binding::Stack(Vec::new()),
                            Binding::Effect(noble_kernel::types::EffSet::empty()),
                        ],
                    },
                });
                level += 1;
            }
            case.candidate.nodes = nodes;
            case.candidate.body = vec![NodeId(0)];
        }
        10 => {
            // A truncated arena whose body still references dropped nodes.
            let keep = case.candidate.nodes.len() / 2;
            let dangling = NodeId(case.candidate.nodes.len() as u32 - 1);
            case.candidate.nodes.truncate(keep.max(1));
            case.candidate.body = vec![dangling];
        }
        _ => {
            // Random identifier bits splatted across every identifier field.
            // The high bit stays set so the revision stays foreign.
            case.candidate.format = rng.next_u64() as u32 | 0x8000_0000;
            case.candidate.revision = rng.next_u64() as u32;
            for node in case.candidate.nodes.iter_mut() {
                if let Node::Invocation { def, .. } = node {
                    *def = noble_kernel::contracts::Definition(rng.next_u64() as u32 | 0x8000_0000);
                }
            }
            let last = case.candidate.body.len() - 1;
            case.candidate.body[last] = NodeId(rng.next_u64() as u32);
        }
    }
    (case, label)
}

fn first_with_bindings(candidate: &mut Candidate) -> Option<&mut Node> {
    candidate.nodes.iter_mut().next()
}

fn inst_of(node: &mut Node) -> &mut Inst {
    match node {
        Node::Literal { inst, .. }
        | Node::Invocation { inst, .. }
        | Node::Quotation { inst, .. } => inst,
    }
}
