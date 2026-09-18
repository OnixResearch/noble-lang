//! DX-01 extension to the fragment-v1 rejection families (DX-DIAG-01,
//! B-DIAG-01, task 3.3): recursion, schema, cycle, and the eliminator
//! joins. Field mapping: `word_or_join` = the `UnsupportedKind` identity
//! (recursion/schema) or `Diagnostic::def` (cycle, joins);
//! `required_stack`/`actual_stack` = `expected`/`actual` (both branch
//! programs for the eliminator joins); `constraint` = `constraint`;
//! `source_span` = `node`; `value_origin_or_unavailable` =
//! `provenance_available == false`. Recursion and schema reject as
//! `unsupported` naming their identity, with no stack shape to report
//! and no candidate-body host request (B-RESULT-02).

use noble_kernel::contracts::{Definition, SchemaDecl, SchemaId};
use noble_kernel::types::{EffSet, Ty};
use noble_kernel::untrusted::{Constraint, Lit, Node, NodeId, Outcome, UnsupportedKind};
use noble_kernel::words::{Binding, Scheme, Variable, VariableKind};

use super::{invocation, lit_node, reject, request, stack, value};

const DUP: Definition = Definition(0);
const SWAP: Definition = Definition(2);
const CASE: Definition = Definition(17);
const LIST_CASE: Definition = Definition(21);

fn reference(index: u32) -> Binding {
    Binding::Ref(Variable(index))
}

/// A candidate over the given nodes and body; the environment preflight
/// runs before any body check, so an empty candidate still drives the
/// environment walks.
fn candidate(nodes: Vec<Node>, body: Vec<u32>) -> noble_kernel::untrusted::Candidate {
    noble_kernel::untrusted::Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision: 0,
        nodes,
        body: body.into_iter().map(NodeId).collect(),
    }
}

/// The empty candidate the environment-walk controls check against.
fn empty_candidate() -> noble_kernel::untrusted::Candidate {
    candidate(vec![], vec![])
}

/// One extra named definition: `S -- S Unit`.
fn unit_like() -> Scheme {
    Scheme {
        var_kinds: vec![VariableKind::Stack],
        stack_in: vec![noble_kernel::shapes::Pattern::StackVar(Variable(0))],
        stack_out: vec![
            noble_kernel::shapes::Pattern::StackVar(Variable(0)),
            noble_kernel::shapes::Pattern::Unit,
        ],
        effects: vec![],
    }
}

/// The bootstrap environment plus `extra` named definitions, each with
/// its declared dependency list (the extra definitions start after the
/// 23 table entries).
fn env_with_deps(deps: &[Vec<u32>]) -> noble_kernel::contracts::Env {
    let mut env = noble_kernel::contracts::environment().expect("table validates");
    let mut index = 0;
    while index < deps.len() {
        env.defs.push(unit_like());
        env.kinds.push(noble_kernel::contracts::Behavior::Named);
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

fn unsupported(outcome: Outcome) -> UnsupportedKind {
    match outcome {
        Outcome::Unsupported(kind) => kind,
        other => panic!("expected the unsupported outcome, got {other:?}"),
    }
}

/// The DX-01 fields every `Diagnostic`-carrying rejection must report:
/// the failing word, the failing node, the violated constraint, and
/// provenance explicitly unavailable.
fn assert_dx01_fields(
    diagnostic: &noble_kernel::untrusted::Diagnostic,
    def: Definition,
    node: u32,
    constraint: &Constraint,
) {
    assert_eq!(diagnostic.def, Some(def), "word_or_join names the word");
    assert_eq!(
        diagnostic.node,
        Some(NodeId(node)),
        "source_span names the node"
    );
    assert_eq!(&diagnostic.constraint, constraint, "constraint");
    assert!(
        !diagnostic.provenance_available,
        "value_origin reports unavailable"
    );
}

/// DX-01 recursion control: an environment definition whose dependency
/// list closes a cycle is rejected before any body check, and the
/// unsupported outcome names the definition the cycle closes on.
// r[verify DX-DIAG-01]
// r[verify B-DIAG-01]
#[test]
fn dx01_recursive_dependency_names_the_definition_identity() -> Result<(), String> {
    let env = env_with_deps(&[vec![23]]);
    let candidate = empty_candidate();
    let outcome = noble_kernel::acceptance::check(&env, &request(vec![], vec![]), &candidate);
    assert_eq!(
        unsupported(outcome),
        UnsupportedKind::RecursiveDependency(Definition(23)),
        "the rejection must name the definition identity"
    );
    Ok(())
}

/// DX-01 schema control: a user-declared recursive schema in environment
/// data is rejected before any body check, and the unsupported outcome
/// names the declaration.
// r[verify DX-DIAG-01]
// r[verify B-DIAG-01]
#[test]
fn dx01_recursive_schema_names_the_declaration() -> Result<(), String> {
    let mut env = noble_kernel::contracts::environment().expect("table validates");
    env.schemas.push(SchemaDecl {
        id: SchemaId(7),
        scheme: unit_like(),
        recursive: true,
    });
    let candidate = empty_candidate();
    let outcome = noble_kernel::acceptance::check(&env, &request(vec![], vec![]), &candidate);
    assert_eq!(
        unsupported(outcome),
        UnsupportedKind::RecursiveSchema(SchemaId(7)),
        "the rejection must name the schema declaration"
    );
    Ok(())
}

/// DX-01 cycle control: a witness bound to itself rejects as invalid
/// with the cyclic-witness constraint at the failing node, and the value
/// origin is reported as unavailable.
// r[verify DX-DIAG-01]
// r[verify B-DIAG-01]
#[test]
fn dx01_cyclic_witness_names_the_constraint_and_reports_unavailable_provenance(
) -> Result<(), String> {
    let candidate = candidate(
        vec![invocation(DUP, vec![stack(vec![]), reference(1)])],
        vec![0],
    );
    let diagnostic = reject(&candidate, &request(vec![], vec![]))?;
    assert_dx01_fields(&diagnostic, DUP, 0, &Constraint::CyclicWitness);
    Ok(())
}

/// DX-01 case-join control: the two `case` branch programs claim
/// different output stacks, and the diagnostic names the `case` word and
/// carries both branch stacks.
// r[verify DX-DIAG-01]
#[test]
fn dx01_case_join_names_the_word_and_both_branch_stacks() -> Result<(), String> {
    let claimed_left = Ty::program(vec![Ty::I64], vec![], EffSet::empty());
    let claimed_right = Ty::program(vec![Ty::Bool], vec![], EffSet::empty());
    let actual_left = Ty::program(vec![Ty::I64], vec![Ty::I64], EffSet::empty());
    let actual_right = Ty::program(vec![Ty::Bool], vec![Ty::Bool], EffSet::empty());
    let candidate = candidate(
        vec![invocation(
            CASE,
            vec![
                stack(vec![]),
                value(Ty::I64),
                value(Ty::Bool),
                stack(vec![]),
                Binding::Effect(EffSet::empty()),
                Binding::Effect(EffSet::empty()),
            ],
        )],
        vec![0],
    );
    let diagnostic = reject(
        &candidate,
        &request(
            vec![
                Ty::Sum(Box::new(Ty::I64), Box::new(Ty::Bool)),
                actual_left.clone(),
                actual_right.clone(),
            ],
            vec![],
        ),
    )?;
    assert_eq!(
        diagnostic.expected,
        vec![
            Ty::Sum(Box::new(Ty::I64), Box::new(Ty::Bool)),
            claimed_left,
            claimed_right
        ],
        "required_stack carries both branch programs with the claimed equal join"
    );
    assert_eq!(
        diagnostic.actual,
        vec![
            Ty::Sum(Box::new(Ty::I64), Box::new(Ty::Bool)),
            actual_left,
            actual_right
        ],
        "actual_stack carries both branch programs as they claim themselves"
    );
    assert_dx01_fields(&diagnostic, CASE, 0, &Constraint::StackJoin);
    Ok(())
}

/// DX-01 list.case-join control: the nil and cons branch programs claim
/// different output stacks, and the diagnostic names the `list.case`
/// word and carries both branch stacks.
// r[verify DX-DIAG-01]
#[test]
fn dx01_list_case_join_names_the_word_and_both_branch_stacks() -> Result<(), String> {
    let claimed_nil = Ty::program(vec![], vec![], EffSet::empty());
    let element = Ty::I64;
    let list = Ty::List(Box::new(element.clone()));
    let claimed_cons = Ty::program(vec![element.clone(), list.clone()], vec![], EffSet::empty());
    let actual_nil = Ty::program(vec![], vec![Ty::Unit], EffSet::empty());
    let actual_cons = Ty::program(vec![element, list], vec![Ty::Unit], EffSet::empty());
    let candidate = candidate(
        vec![invocation(
            LIST_CASE,
            vec![
                stack(vec![]),
                value(Ty::I64),
                stack(vec![]),
                Binding::Effect(EffSet::empty()),
                Binding::Effect(EffSet::empty()),
            ],
        )],
        vec![0],
    );
    let diagnostic = reject(
        &candidate,
        &request(
            vec![
                Ty::List(Box::new(Ty::I64)),
                actual_nil.clone(),
                actual_cons.clone(),
            ],
            vec![],
        ),
    )?;
    assert_eq!(
        diagnostic.expected,
        vec![Ty::List(Box::new(Ty::I64)), claimed_nil, claimed_cons],
        "required_stack carries both branch programs with the claimed equal join"
    );
    assert_eq!(
        diagnostic.actual,
        vec![Ty::List(Box::new(Ty::I64)), actual_nil, actual_cons],
        "actual_stack carries both branch programs as they claim themselves"
    );
    assert_dx01_fields(&diagnostic, LIST_CASE, 0, &Constraint::StackJoin);
    Ok(())
}

/// The eliminator-join diagnostics keep the DX-01 shape over a compound
/// program body too: an `if` whose branches disagree inside a quotation
/// reports the join at the `if` node (the quotation's body folds with
/// the same join discipline).
// r[verify DX-DIAG-01]
#[test]
fn dx01_swap_witness_cycle_inside_a_body_names_the_node() -> Result<(), String> {
    // `swap` with its two value variables bound to each other, inside a
    // two-node body: the diagnostic must name the swap node, not the body.
    let candidate = candidate(
        vec![
            lit_node(Lit::I64(1), vec![]),
            invocation(SWAP, vec![stack(vec![]), reference(2), reference(1)]),
        ],
        vec![0, 1],
    );
    let diagnostic = reject(&candidate, &request(vec![], vec![]))?;
    assert_dx01_fields(&diagnostic, SWAP, 1, &Constraint::CyclicWitness);
    Ok(())
}
