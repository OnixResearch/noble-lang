//! B-CHECK-02/05 rejection controls: recursive definition dependencies,
//! user-declared recursive schemas, and cyclic substitution witnesses
//! (fragment v1).
//!
//! Every rejection here names its identity (the definition the dependency
//! cycle closes on, the schema declaration, or the cyclic-witness
//! constraint), issues no candidate-body host request, and leaves the
//! outcome domain untouched. Each walk carries a boundary companion (a
//! walk exactly within its declared work limit succeeds) and an exhausted
//! companion (a limit below the walk's charge fails closed). The walks
//! charge before they traverse, and the environment validation runs in
//! preflight, before any candidate body is checked.

use noble_kernel::contracts::{Behavior, Definition, SchemaDecl, SchemaId};
use noble_kernel::types::Ty;
use noble_kernel::untrusted::{
    Candidate, Constraint, Limits, Lit, Node, NodeId, Outcome, Request, UnsupportedKind,
};
use noble_kernel::words::{Binding, Inst, Scheme, VariableKind};

/// Local builders (the shared `support` module belongs to the acceptance
/// suite; this module keeps its own so unused helpers cannot warn).
fn segment(list: Vec<Ty>) -> Binding {
    Binding::Stack(list)
}

fn value(ty: Ty) -> Binding {
    Binding::Value(ty)
}

fn reference(index: u32) -> Binding {
    Binding::Ref(noble_kernel::words::Variable(index))
}

fn lit_node(lit: Lit, under: Vec<Ty>) -> Node {
    Node::Literal {
        lit,
        inst: Inst {
            bindings: vec![segment(under)],
        },
    }
}

fn invocation(def: Definition, bindings: Vec<Binding>) -> Node {
    Node::Invocation {
        def,
        inst: Inst { bindings },
    }
}

fn candidate(nodes: Vec<Node>, body: Vec<u32>) -> Candidate {
    Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision: noble_kernel::untrusted::SEMANTIC_REVISION,
        nodes,
        body: body.into_iter().map(NodeId).collect(),
    }
}

fn request(stack_in: Vec<Ty>, stack_out: Vec<Ty>) -> Request {
    Request {
        input_bytes: 64,
        expected: noble_kernel::untrusted::Expected {
            stack_in,
            stack_out,
            allowed_effects: noble_kernel::types::EffSet::empty(),
        },
        limits: Limits {
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

/// A compact view of the outcome domain this module asserts on.
enum Seen {
    Accepted,
    Bad(noble_kernel::untrusted::Diagnostic),
    Exhausted(noble_kernel::untrusted::LimitKind),
}

fn seen(env: &noble_kernel::contracts::Env, request: &Request, cand: &Candidate) -> Seen {
    match noble_kernel::acceptance::check(env, request, cand) {
        Outcome::Accepted(_) => Seen::Accepted,
        Outcome::Invalid(diagnostic) => Seen::Bad(diagnostic),
        Outcome::Exhausted(limit) => Seen::Exhausted(limit),
        Outcome::Unsupported(kind) => panic!("unexpected unsupported outcome: {kind:?}"),
        Outcome::InternalFailure => panic!("unexpected internal failure"),
    }
}

/// One extra named definition: `S -- S Unit`.
fn unit_like() -> Scheme {
    Scheme {
        var_kinds: vec![VariableKind::Stack],
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

/// The bootstrap environment plus `extra` named definitions, each with its
/// declared dependency list.
fn env_with(deps: &[Vec<u32>]) -> noble_kernel::contracts::Env {
    let mut env = noble_kernel::contracts::environment().expect("table validates");
    let mut index = 0;
    while index < deps.len() {
        env.defs.push(unit_like());
        env.kinds.push(Behavior::Named);
        env.deps.push(
            deps[index]
                .iter()
                .map(|id| Definition(*id))
                .collect::<Vec<Definition>>(),
        );
        index += 1;
    }
    env
}

/// An empty-body program: `[] -- []` with no allowed effects.
fn empty_program() -> noble_kernel::untrusted::Candidate {
    candidate(vec![], vec![])
}

/// The unsupported kind of one outcome, for identity-naming assertions.
fn unsupported_of(outcome: noble_kernel::untrusted::Outcome) -> UnsupportedKind {
    match outcome {
        Outcome::Unsupported(kind) => kind,
        other => panic!("expected the unsupported outcome, got {other:?}"),
    }
}

#[test]
fn recursive_dependency_self_names_identity() {
    let env = env_with(&[vec![23]]);
    let outcome = noble_kernel::acceptance::check(&env, &request(vec![], vec![]), &empty_program());
    assert_eq!(
        unsupported_of(outcome),
        UnsupportedKind::RecursiveDependency(Definition(23)),
        "the rejection names the definition the cycle closes on"
    );
}

#[test]
fn recursive_dependency_mutual_names_identity() {
    // Definitions 23 and 23+1 depend on each other.
    let env = env_with(&[vec![24], vec![23]]);
    let outcome = noble_kernel::acceptance::check(&env, &request(vec![], vec![]), &empty_program());
    assert_eq!(
        unsupported_of(outcome),
        UnsupportedKind::RecursiveDependency(Definition(23)),
        "the mutual cycle closes on the definition the walk reaches on-path"
    );
}

#[test]
fn acyclic_dependencies_accept_at_the_walk_boundary() {
    // One edge, and exactly one unit of declared work: the walk charges the
    // edge and succeeds; the empty body charges nothing.
    let env = env_with(&[vec![24], vec![]]);
    let mut request = request(vec![], vec![]);
    request.limits.work = 1;
    let Seen::Accepted = seen(&env, &request, &empty_program()) else {
        panic!("an acyclic dependency chain must check at the work boundary");
    };
    // The empty body records no derivations.;
    // One below the boundary: the walk's single charge fails closed.
    request.limits.work = 0;
    let Seen::Exhausted(limit) = seen(&env, &request, &empty_program()) else {
        panic!("a work limit below the walk's charge must fail closed");
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::Work);
}

#[test]
fn external_dependency_data_is_validated_before_any_body_check() {
    // The environment carries a recursive dependency; the candidate also
    // carries a dangling node reference that would reject as invalid. The
    // outcome is the unsupported environment rejection, proving external
    // environment data never enters checking unvalidated.
    let env = env_with(&[vec![23]]);
    let dangling = candidate(vec![], vec![9]);
    let outcome = noble_kernel::acceptance::check(&env, &request(vec![], vec![]), &dangling);
    assert_eq!(
        unsupported_of(outcome),
        UnsupportedKind::RecursiveDependency(Definition(23))
    );
}

#[test]
fn recursive_schema_rejects_unsupported_naming_the_declaration() {
    let mut env = noble_kernel::contracts::environment().expect("table validates");
    env.schemas.push(SchemaDecl {
        id: SchemaId(7),
        scheme: unit_like(),
        recursive: true,
    });
    let outcome = noble_kernel::acceptance::check(&env, &request(vec![], vec![]), &empty_program());
    assert_eq!(
        unsupported_of(outcome),
        UnsupportedKind::RecursiveSchema(SchemaId(7)),
        "a user-declared recursive schema is unsupported and named"
    );
}

#[test]
fn nonrecursive_schemas_scan_at_the_boundary_and_exhaust_below_it() {
    let mut env = noble_kernel::contracts::environment().expect("table validates");
    env.schemas.push(SchemaDecl {
        id: SchemaId(1),
        scheme: unit_like(),
        recursive: false,
    });
    env.schemas.push(SchemaDecl {
        id: SchemaId(2),
        scheme: unit_like(),
        recursive: false,
    });
    let mut request = request(vec![], vec![]);
    request.limits.work = 2;
    let Seen::Accepted = seen(&env, &request, &empty_program()) else {
        panic!("two non-recursive schema entries scan within two work units");
    };
    request.limits.work = 1;
    let Seen::Exhausted(limit) = seen(&env, &request, &empty_program()) else {
        panic!("the schema scan must fail closed below its charge");
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::Work);
}

#[test]
fn cyclic_witness_self_reference_rejects_invalid() {
    // `dup` with its value variable bound to itself.
    let env = noble_kernel::contracts::environment().expect("table validates");
    let program = candidate(
        vec![invocation(
            Definition(0),
            vec![segment(vec![]), reference(1)],
        )],
        vec![0],
    );
    let Seen::Bad(diagnostic) = seen(&env, &request(vec![], vec![]), &program) else {
        panic!("a self-referential witness must reject as invalid");
    };
    assert_eq!(diagnostic.constraint, Constraint::CyclicWitness);
    assert!(!diagnostic.provenance_available);
}

#[test]
fn cyclic_witness_mutual_references_reject_invalid() {
    // `swap` with its two value variables bound to each other.
    let env = noble_kernel::contracts::environment().expect("table validates");
    let program = candidate(
        vec![invocation(
            Definition(2),
            vec![segment(vec![]), reference(2), reference(1)],
        )],
        vec![0],
    );
    let Seen::Bad(diagnostic) = seen(&env, &request(vec![], vec![]), &program) else {
        panic!("mutually referential witnesses must reject as invalid");
    };
    assert_eq!(diagnostic.constraint, Constraint::CyclicWitness);
}

#[test]
fn resolvable_witness_chains_accept_and_resolve() {
    // `swap` with its first value bound by reference to the second: the
    // chain resolves and the interface is exactly the resolved one.
    let env = noble_kernel::contracts::environment().expect("table validates");
    let program = candidate(
        vec![
            lit_node(Lit::Bool(true), vec![]),
            lit_node(Lit::Bool(false), vec![Ty::Bool]),
            invocation(
                Definition(2),
                vec![segment(vec![]), reference(2), value(Ty::Bool)],
            ),
        ],
        vec![0, 1, 2],
    );
    let Seen::Accepted = seen(&env, &request(vec![], vec![Ty::Bool, Ty::Bool]), &program) else {
        panic!("a resolvable reference chain must accept");
    };
    // The swap of two identical bools leaves the resolved stack intact.
}

#[test]
fn resolvable_chain_at_the_walk_boundary_and_exhausted_below_it() {
    // `pair` with a two-hop reference chain under its declared value
    // variable: the resolution walk charges one unit per hop inside `apply`
    // — before the node's fold charge — so a work limit below the hop count
    // fails closed at the walk, and an ample budget resolves and accepts.
    let env = noble_kernel::contracts::environment().expect("table validates");
    let program = candidate(
        vec![
            lit_node(Lit::Bool(true), vec![]),
            lit_node(Lit::Bool(false), vec![Ty::Bool]),
            invocation(
                Definition(13),
                vec![segment(vec![]), reference(2), value(Ty::Bool)],
            ),
        ],
        vec![0, 1, 2],
    );
    let request = request(
        vec![],
        vec![Ty::Pair(Box::new(Ty::Bool), Box::new(Ty::Bool))],
    );
    let Seen::Accepted = seen(&env, &request, &program) else {
        panic!("an ample budget resolves the chain and accepts");
    };
    let mut tight = request.clone();
    tight.limits.work = 1;
    let Seen::Exhausted(limit) = seen(&env, &tight, &program) else {
        panic!("a work limit below the chain's hops must fail closed");
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::Work);
}

#[test]
fn kind_crossed_reference_rejects_as_instantiation_kind() {
    // A stack variable bound by reference to a value variable: the chain
    // resolves, but the terminal binding's kind does not match the
    // referring variable's declared kind.
    let env = noble_kernel::contracts::environment().expect("table validates");
    let program = candidate(
        vec![invocation(
            Definition(0),
            vec![reference(1), value(Ty::Bool)],
        )],
        vec![0],
    );
    let Seen::Bad(diagnostic) = seen(&env, &request(vec![], vec![]), &program) else {
        panic!("a kind-crossed reference must reject as invalid");
    };
    assert_eq!(diagnostic.constraint, Constraint::InstantiationKind);
}
