//! Acceptance controls for the fragment checker.
// Names the fragment acceptance contract this crate implements.
// r[impl VT-M2-01]

use noble_kernel::candidate::{
    Candidate, Checked, Constraint, Expected, LimitKind, Limits, Lit, Node, NodeId, Outcome,
    Request, UnsupportedKind,
};
use noble_kernel::check::check_candidate;
use noble_kernel::env::{bootstrap_environment, DefId, Env, FIXTURE_RESOURCE};
use noble_kernel::scheme::{Binding, Inst, PItem, PTy, Scheme, VarId, VarKind};
use noble_kernel::types::{EffId, EffSet, Ty};

fn ids(list: &[u32]) -> EffSet {
    EffSet::from_ids(&list.iter().map(|id| EffId(*id)).collect::<Vec<_>>())
}

fn stack_binding(tys: Vec<Ty>) -> Binding {
    Binding::Stack(tys)
}

fn value_binding(ty: Ty) -> Binding {
    Binding::Value(ty)
}

fn effect_binding(list: &[u32]) -> Binding {
    Binding::Effect(ids(list))
}

fn inst(bindings: Vec<Binding>) -> Inst {
    Inst { bindings }
}

fn limits() -> Limits {
    Limits {
        bytes: 1 << 16,
        nodes: 256,
        depth: 32,
        type_size: 64,
        stack_height: 16,
        work: 10_000,
        diagnostics: 64,
    }
}

fn request(stack_in: Vec<Ty>, stack_out: Vec<Ty>, allowed: &[u32]) -> Request {
    Request {
        input_bytes: 64,
        expected: Expected {
            stack_in,
            stack_out,
            allowed_effects: ids(allowed),
        },
        limits: limits(),
    }
}

fn candidate(nodes: Vec<Node>, body: Vec<u32>) -> Candidate {
    Candidate {
        format: 0,
        revision: 0,
        nodes,
        body: body.into_iter().map(NodeId).collect(),
    }
}

fn check(env: &Env, request: &Request, candidate: &Candidate) -> Outcome {
    check_candidate(env, request, candidate)
}

fn accept(env: &Env, request: &Request, candidate: &Candidate) -> Checked {
    match check(env, request, candidate) {
        Outcome::Accepted(checked) => checked,
        other => panic!("expected acceptance, got {other:?}"),
    }
}

fn reject(env: &Env, request: &Request, candidate: &Candidate) -> (Constraint, Vec<Ty>, Vec<Ty>) {
    match check(env, request, candidate) {
        Outcome::Invalid(diagnostic) => (
            diagnostic.constraint,
            diagnostic.expected,
            diagnostic.actual,
        ),
        other => panic!("expected rejection, got {other:?}"),
    }
}

const DUP: DefId = DefId(0);
const ADD: DefId = DefId(4);
const RUN: DefId = DefId(9);
const TEST_EMIT: DefId = DefId(21);

fn lit_node(lit: Lit, stack: Vec<Ty>) -> Node {
    Node::Literal {
        lit,
        inst: inst(vec![stack_binding(stack)]),
    }
}

fn quote_node(
    body: Vec<u32>,
    surrounding: Vec<Ty>,
    item_in: Vec<Ty>,
    item_out: Vec<Ty>,
    effects: &[u32],
) -> Node {
    Node::Quotation {
        body: body.into_iter().map(NodeId).collect(),
        inst: inst(vec![
            stack_binding(surrounding),
            stack_binding(item_in),
            stack_binding(item_out),
            effect_binding(effects),
        ]),
    }
}

fn forty_two_program() -> (Request, Candidate) {
    let program = candidate(
        vec![
            lit_node(Lit::I64(41), vec![]),
            lit_node(Lit::I64(1), vec![Ty::I64]),
            Node::Invocation {
                def: ADD,
                inst: inst(vec![stack_binding(vec![])]),
            },
            quote_node(vec![1, 2], vec![Ty::I64], vec![Ty::I64], vec![Ty::I64], &[]),
            Node::Invocation {
                def: RUN,
                inst: inst(vec![
                    stack_binding(vec![Ty::I64]),
                    stack_binding(vec![Ty::I64]),
                    effect_binding(&[]),
                ]),
            },
        ],
        vec![0, 3, 4],
    );
    (request(vec![], vec![Ty::I64], &[]), program)
}

#[test]
fn frag_seq_and_lit_accept_arithmetic() {
    let env = bootstrap_environment().unwrap();
    let program = candidate(
        vec![
            lit_node(Lit::I64(41), vec![]),
            lit_node(Lit::I64(1), vec![Ty::I64]),
            Node::Invocation {
                def: ADD,
                inst: inst(vec![stack_binding(vec![])]),
            },
        ],
        vec![0, 1, 2],
    );
    let request = request(vec![], vec![Ty::I64], &[]);
    let checked = accept(&env, &request, &program);
    assert_eq!(checked.derivations.len(), 3);
    assert_eq!(checked.interface.stack_out, vec![Ty::I64]);
}

#[test]
fn frag_quote_run_accepts_and_checks_body() {
    let env = bootstrap_environment().unwrap();
    let (request, program) = forty_two_program();
    let checked = accept(&env, &request, &program);
    // Body nodes 1 and 2 plus the three entry nodes.
    assert_eq!(checked.derivations.len(), 5);
}

#[test]
fn frag_quote_run_rejects_body_join_mismatch() {
    let env = bootstrap_environment().unwrap();
    // The quotation claims C = [I64] but its body derives [I64, I64].
    let program = candidate(
        vec![
            lit_node(Lit::I64(1), vec![]),
            lit_node(Lit::I64(2), vec![Ty::I64]),
            quote_node(vec![0, 1], vec![], vec![], vec![Ty::I64], &[]),
            Node::Invocation {
                def: RUN,
                inst: inst(vec![
                    stack_binding(vec![]),
                    stack_binding(vec![Ty::I64]),
                    effect_binding(&[]),
                ]),
            },
        ],
        vec![2, 3],
    );
    let request = request(vec![], vec![Ty::I64], &[]);
    let (constraint, expected, actual) = reject(&env, &request, &program);
    assert_eq!(constraint, Constraint::StackJoin);
    assert_eq!(expected, vec![Ty::I64]);
    assert_eq!(actual, vec![Ty::I64, Ty::I64]);
}

fn env_with_resource_maker() -> (Env, DefId) {
    let mut env = bootstrap_environment().unwrap();
    let id = DefId(env.defs.len() as u32);
    env.defs.push(Scheme {
        var_kinds: vec![VarKind::Stack],
        stack_in: vec![PItem::Stack(VarId(0))],
        stack_out: vec![
            PItem::Stack(VarId(0)),
            PItem::Ty(PTy::Resource(FIXTURE_RESOURCE)),
        ],
        effects: vec![],
    });
    env.kinds.push(noble_kernel::env::WordKind::Named);
    (env, id)
}

#[test]
fn eligibility_rejects_dup_of_resource() {
    let (env, make) = env_with_resource_maker();
    let resource = Ty::Resource(FIXTURE_RESOURCE);
    let program = candidate(
        vec![
            Node::Invocation {
                def: make,
                inst: inst(vec![stack_binding(vec![])]),
            },
            Node::Invocation {
                def: DUP,
                inst: inst(vec![stack_binding(vec![]), value_binding(resource.clone())]),
            },
        ],
        vec![0, 1],
    );
    let request = request(vec![], vec![resource.clone()], &[]);
    let (constraint, _, _) = reject(&env, &request, &program);
    assert_eq!(constraint, Constraint::Eligibility(resource));
}

#[test]
fn effect_inclusion_rejects_hidden_emit_and_accepts_declared_bound() {
    let env = bootstrap_environment().unwrap();
    let emit_node = || Node::Invocation {
        def: TEST_EMIT,
        inst: inst(vec![stack_binding(vec![])]),
    };
    // `[ test.emit ]` derives [Text] -- [Unit] and claims its latent bound.
    let pushed = Ty::program(vec![Ty::Text], vec![Ty::Unit], ids(&[0]));

    // Hidden: the quotation claims an empty latent bound but its body emits.
    let hidden = candidate(
        vec![
            emit_node(),
            quote_node(vec![0], vec![], vec![Ty::Text], vec![Ty::Unit], &[]),
        ],
        vec![1],
    );
    let request = request(vec![], vec![pushed.clone()], &[0]);
    let (constraint, _, _) = reject(&env, &request, &hidden);
    assert_eq!(constraint, Constraint::EffectInclusion(EffId(0)));

    // Declared: the same body with a truthful bound is accepted, and the
    // construction remains effect-free at the entry interface.
    let declared = candidate(
        vec![
            emit_node(),
            quote_node(vec![0], vec![], vec![Ty::Text], vec![Ty::Unit], &[0]),
        ],
        vec![1],
    );
    let checked = accept(&env, &request, &declared);
    assert!(checked.interface.effects.is_empty());
}

#[test]
fn word_instantiation_negative_controls() {
    let env = bootstrap_environment().unwrap();
    let request = request(vec![], vec![Ty::I64], &[]);
    let arity = candidate(
        vec![Node::Invocation {
            def: ADD,
            inst: inst(vec![]),
        }],
        vec![0],
    );
    assert_eq!(
        reject(&env, &request, &arity).0,
        Constraint::InstantiationArity
    );
    let kind = candidate(
        vec![Node::Invocation {
            def: ADD,
            inst: inst(vec![value_binding(Ty::I64)]),
        }],
        vec![0],
    );
    assert_eq!(
        reject(&env, &request, &kind).0,
        Constraint::InstantiationKind
    );
}

#[test]
fn unknown_definition_and_malformed_reference_reject() {
    let env = bootstrap_environment().unwrap();
    let request = request(vec![], vec![], &[]);
    let unknown = candidate(
        vec![Node::Invocation {
            def: DefId(99),
            inst: inst(vec![stack_binding(vec![])]),
        }],
        vec![0],
    );
    assert_eq!(
        reject(&env, &request, &unknown).0,
        Constraint::UnknownDefinition(DefId(99))
    );
    let malformed = candidate(vec![lit_node(Lit::Unit, vec![])], vec![7]);
    assert_eq!(
        reject(&env, &request, &malformed).0,
        Constraint::MalformedReference(NodeId(7))
    );
}

#[test]
fn foreign_allowed_effect_and_wrong_revision_reject() {
    let env = bootstrap_environment().unwrap();
    let foreign = request(vec![], vec![], &[9]);
    let empty = candidate(vec![], vec![]);
    match check(&env, &foreign, &empty) {
        Outcome::Invalid(diagnostic) => {
            assert_eq!(diagnostic.constraint, Constraint::UnknownEffect(EffId(9)))
        }
        other => panic!("expected rejection, got {other:?}"),
    }
    let mut wrong_revision = candidate(vec![], vec![]);
    wrong_revision.revision = 7;
    match check(&env, &request(vec![], vec![], &[]), &wrong_revision) {
        Outcome::Unsupported(UnsupportedKind::FormatRevision) => {}
        other => panic!("expected unsupported revision, got {other:?}"),
    }
}

#[test]
fn limits_boundary_and_exhaustion() {
    let env = bootstrap_environment().unwrap();
    let (base_request, program) = forty_two_program();

    // Nodes: five nodes exactly.
    let mut exact = base_request.clone();
    exact.limits.nodes = 5;
    accept(&env, &exact, &program);
    let mut over = base_request.clone();
    over.limits.nodes = 4;
    assert!(matches!(
        check(&env, &over, &program),
        Outcome::Exhausted(LimitKind::Nodes)
    ));

    // Bytes.
    let mut exact = base_request.clone();
    exact.limits.bytes = 64;
    accept(&env, &exact, &program);
    let mut over = base_request.clone();
    over.limits.bytes = 63;
    assert!(matches!(
        check(&env, &over, &program),
        Outcome::Exhausted(LimitKind::Bytes)
    ));

    // Depth: the quotation body adds one level.
    let mut exact = base_request.clone();
    exact.limits.depth = 1;
    accept(&env, &exact, &program);
    let mut over = base_request.clone();
    over.limits.depth = 0;
    assert!(matches!(
        check(&env, &over, &program),
        Outcome::Exhausted(LimitKind::Depth)
    ));

    // Stack height at the expected input.
    let mut height_request = request(vec![Ty::I64, Ty::I64], vec![Ty::I64, Ty::I64], &[]);
    height_request.limits.stack_height = 2;
    let empty = candidate(vec![], vec![]);
    accept(&env, &height_request, &empty);
    height_request.limits.stack_height = 1;
    assert!(matches!(
        check(&env, &height_request, &empty),
        Outcome::Exhausted(LimitKind::StackHeight)
    ));

    // Type size: Pair(I64, I64) has size three.
    let pair = Ty::Pair(Box::new(Ty::I64), Box::new(Ty::I64));
    let mut type_request = request(vec![pair.clone()], vec![pair], &[]);
    type_request.limits.type_size = 3;
    let empty = candidate(vec![], vec![]);
    accept(&env, &type_request, &empty);
    type_request.limits.type_size = 2;
    assert!(matches!(
        check(&env, &type_request, &empty),
        Outcome::Exhausted(LimitKind::TypeSize)
    ));

    // Work: one literal costs node(1) + instantiation(4) + join(2).
    let program = candidate(vec![lit_node(Lit::Unit, vec![])], vec![0]);
    let mut request = request(vec![], vec![Ty::Unit], &[]);
    request.limits.work = 7;
    accept(&env, &request, &program);
    request.limits.work = 6;
    assert!(matches!(
        check(&env, &request, &program),
        Outcome::Exhausted(LimitKind::Work)
    ));
}

#[test]
fn diagnostics_distinguish_order_shape_and_truncation() {
    let env = bootstrap_environment().unwrap();
    let order_request = request(vec![], vec![Ty::Bool, Ty::I64], &[]);
    let program = candidate(
        vec![
            lit_node(Lit::I64(7), vec![]),
            lit_node(Lit::Bool(true), vec![Ty::I64]),
        ],
        vec![0, 1],
    );
    let (constraint, expected, actual) = reject(&env, &order_request, &program);
    assert_eq!(constraint, Constraint::StackOrder);
    assert_eq!(expected, vec![Ty::Bool, Ty::I64]);
    assert_eq!(actual, vec![Ty::I64, Ty::Bool]);

    let shape_request = request(vec![], vec![Ty::Bool, Ty::Bool], &[]);
    let (constraint, _, _) = reject(&env, &shape_request, &program);
    assert_eq!(constraint, Constraint::StackJoin);

    // The diagnostic budget cuts the recorded stacks and keeps the outcome.
    let mut tiny = order_request.clone();
    tiny.limits.diagnostics = 1;
    match check(&env, &tiny, &program) {
        Outcome::Invalid(diagnostic) => {
            assert!(diagnostic.truncated);
            assert!(!diagnostic.provenance_available);
            assert!(diagnostic.expected.len() + diagnostic.actual.len() <= 1);
        }
        other => panic!("expected rejection, got {other:?}"),
    }
}

#[test]
fn join_failure_names_the_failing_word() {
    let env = bootstrap_environment().unwrap();
    let request = request(vec![], vec![Ty::I64], &[]);
    // `+` needs two I64s; only one is present.
    let program = candidate(
        vec![
            lit_node(Lit::I64(1), vec![]),
            Node::Invocation {
                def: ADD,
                inst: inst(vec![stack_binding(vec![])]),
            },
        ],
        vec![0, 1],
    );
    match check(&env, &request, &program) {
        Outcome::Invalid(diagnostic) => {
            assert_eq!(diagnostic.constraint, Constraint::StackJoin);
            assert_eq!(diagnostic.def, Some(ADD));
            assert_eq!(diagnostic.node, Some(NodeId(1)));
            assert_eq!(diagnostic.expected, vec![Ty::I64, Ty::I64]);
            assert_eq!(diagnostic.actual, vec![Ty::I64]);
        }
        other => panic!("expected rejection, got {other:?}"),
    }
}
