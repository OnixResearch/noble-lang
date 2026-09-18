//! Seeded generator of fragment candidates for the bounded property harness
//! (DX-PROPERTY-01). Deterministic from its seed; no time or environment
//! input. Candidates stay inside generous declared limits, so a correct
//! checker decides each one `accepted` or `invalid` — never exhausted.

use noble_kernel::contracts::{Behavior, Definition, Env};
use noble_kernel::shapes::Pattern;
use noble_kernel::types::{EffId, EffSet, Ty};
use noble_kernel::untrusted::{Candidate, Limits, Lit, Node, NodeId, Request};
use noble_kernel::words::{Inst, Scheme, Variable, VariableKind};

use super::fit::{effects, fit_word, stack, WORDS};
use super::rng::Rng;

/// One generated acceptance case.
pub struct Case {
    pub request: Request,
    pub candidate: Candidate,
}

/// The bootstrap environment plus one named fixture definition (id 22) that
/// produces the opaque resource from any stack, so eligibility negatives are
/// reachable.
pub fn environment() -> Result<Env, String> {
    let mut env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    env.defs.push(Scheme {
        var_kinds: vec![VariableKind::Stack],
        stack_in: vec![Pattern::StackVar(Variable(0))],
        stack_out: vec![
            Pattern::StackVar(Variable(0)),
            Pattern::Resource(noble_kernel::contracts::FIXTURE_RESOURCE),
        ],
        effects: vec![],
    });
    env.kinds.push(Behavior::Named);
    Ok(env)
}

/// A random type of the given nesting depth.
pub fn ty(rng: &mut Rng, depth: u64) -> Ty {
    let shallow = depth == 0;
    match rng.below(if shallow { 5 } else { 11 }) {
        0 => Ty::Unit,
        1 => Ty::Bool,
        2 => Ty::I64,
        3 => Ty::Text,
        4 => Ty::Syntax,
        5 => Ty::Pair(Box::new(ty(rng, depth - 1)), Box::new(ty(rng, depth - 1))),
        6 => Ty::Sum(Box::new(ty(rng, depth - 1)), Box::new(ty(rng, depth - 1))),
        7 => Ty::List(Box::new(ty(rng, depth - 1))),
        _ => Ty::program(small_stack(rng), small_stack(rng), empty_or_emit(rng)),
    }
}

fn empty_or_emit(rng: &mut Rng) -> EffSet {
    if rng.bit() {
        EffSet::from_ids(&[EffId(0)])
    } else {
        EffSet::empty()
    }
}

pub fn small_stack(rng: &mut Rng) -> Vec<Ty> {
    let count = rng.below(3);
    (0..count).map(|_| ty(rng, 0)).collect()
}

fn lit_of(rng: &mut Rng) -> Lit {
    match rng.below(4) {
        0 => Lit::I64(if rng.bit() {
            -(rng.below(1000) as i64)
        } else {
            rng.below(1000) as i64
        }),
        1 => Lit::Bool(rng.bit()),
        2 => Lit::Text,
        _ => Lit::Unit,
    }
}

/// Build one body: append nodes to the arena and fold the simulated stack.
/// Returns the body's node ids, the final stack, and its latent identities.
fn build_body(
    rng: &mut Rng,
    arena: &mut Vec<Node>,
    start: Vec<Ty>,
    depth: u64,
    steps: u64,
) -> (Vec<NodeId>, Vec<Ty>, Vec<u32>) {
    let mut now = start;
    let mut latent: Vec<u32> = Vec::new();
    let mut body: Vec<NodeId> = Vec::new();
    let mut step = 0;
    while step < steps {
        if depth > 0 && rng.below(5) == 0 {
            let around = now.clone();
            let inner_start = small_stack(rng);
            let inner_steps = rng.below(4);
            let (child_body, child_final, child_latent) =
                build_body(rng, arena, inner_start.clone(), depth - 1, inner_steps);
            let claimed = claimed_effects(rng, &child_latent);
            let claimed_set =
                EffSet::from_ids(&claimed.iter().map(|id| EffId(*id)).collect::<Vec<_>>());
            arena.push(Node::Quotation {
                body: child_body,
                inst: Inst {
                    bindings: vec![
                        stack(around.clone()),
                        stack(inner_start.clone()),
                        stack(child_final.clone()),
                        effects(&claimed),
                    ],
                },
            });
            now.push(Ty::program(inner_start, child_final, claimed_set));
            body.push(NodeId(index_of_last(arena)));
        } else {
            let mut emitted = false;
            let mut attempt = 0;
            while !emitted && attempt < 5 {
                attempt += 1;
                let def = WORDS[rng.below(WORDS.len() as u64) as usize];
                if let Some((bindings, next, add)) = fit_word(rng, &now, def) {
                    arena.push(Node::Invocation {
                        def: Definition(def),
                        inst: Inst { bindings },
                    });
                    now = next;
                    latent.extend(add);
                    body.push(NodeId(index_of_last(arena)));
                    emitted = true;
                }
            }
            if !emitted {
                let lit = lit_of(rng);
                let under = now.clone();
                arena.push(Node::Literal {
                    lit,
                    inst: Inst {
                        bindings: vec![stack(under.clone())],
                    },
                });
                now.push(lit.ty());
                body.push(NodeId(index_of_last(arena)));
            }
        }
        step += 1;
    }
    (body, now, latent)
}

fn index_of_last(arena: &[Node]) -> u32 {
    (arena.len() - 1) as u32
}

/// The quotation's claimed effect bound: the child's derived latent set, or
/// a narrowed set that makes the candidate rejecting.
fn claimed_effects(rng: &mut Rng, child_latent: &[u32]) -> Vec<u32> {
    if !child_latent.is_empty() && rng.below(4) != 0 {
        child_latent.to_vec()
    } else {
        Vec::new()
    }
}

/// Generate one complete case: request plus candidate. With probability
/// `KNOB_CHANCE` one post-generation knob corrupts a witness or expectation,
/// which keeps both accepting and rejecting candidates in the stream.
pub const KNOB_CHANCE: u64 = 45;

pub fn case(rng: &mut Rng) -> Case {
    let mut arena: Vec<Node> = Vec::new();
    let entry: Vec<Ty> = (0..rng.below(3)).map(|_| ty(rng, 1)).collect();
    let steps = 1 + rng.below(6);
    let (body, final_stack, latent) = build_body(rng, &mut arena, entry.clone(), 2, steps);
    let allowed = allowed_effects(rng, &latent);
    let mut request = Request {
        input_bytes: 64,
        expected: noble_kernel::untrusted::Expected {
            stack_in: entry,
            stack_out: final_stack,
            allowed_effects: EffSet::from_ids(
                &allowed.iter().map(|id| EffId(*id)).collect::<Vec<_>>(),
            ),
        },
        limits: limits(),
    };
    let mut candidate = Candidate {
        format: 0,
        revision: 0,
        nodes: arena,
        body,
    };
    if rng.below(100) < KNOB_CHANCE {
        super::knobs::apply(rng, &mut request, &mut candidate);
    }
    Case { request, candidate }
}

fn allowed_effects(rng: &mut Rng, latent: &[u32]) -> Vec<u32> {
    if latent.is_empty() {
        if rng.bit() {
            vec![0]
        } else {
            Vec::new()
        }
    } else if rng.below(10) < 7 {
        latent.to_vec()
    } else {
        Vec::new()
    }
}

pub fn limits() -> Limits {
    Limits {
        bytes: 1 << 16,
        nodes: 256,
        depth: 8,
        type_size: 64,
        stack_height: 16,
        work: 100_000,
        diagnostics: 64,
    }
}
