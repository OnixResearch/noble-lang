//! DX-01 negative diagnostic controls over the fragment diagnostics
//! (DX-DIAG-01, B-DIAG-01).
//!
//! The three variants come from `specs/conformance/developer-experience-cases.json`
//! case `DX-01` verbatim. Each control asserts the kernel's diagnostic
//! fields against the case's `expected.diagnostic_fields`:
//!
//! | DX-01 field | Kernel field |
//! |---|---|
//! | `word_or_join` | `Diagnostic::def` (the failing word) |
//! | `required_stack` | `Diagnostic::expected` (bottom-first) |
//! | `actual_stack` | `Diagnostic::actual` (bottom-first) |
//! | `constraint` | `Diagnostic::constraint` |
//! | `source_span` | `Diagnostic::node` (v0: node index; no character span yet) |
//! | `value_origin_or_unavailable` | `Diagnostic::provenance_available` (`false` reports "unavailable") |
//!
//! `guest_requests: 0` and `protected_operations: 0` are structural for this
//! fragment: checking is static and rejection issues no candidate-body host
//! request (B-RESULT-01); the checker makes no host calls of any kind.

use noble_kernel::contracts::Definition;
use noble_kernel::types::Ty;
use noble_kernel::untrusted::{Constraint, Limits, Lit, Node, NodeId, Request};
use noble_kernel::words::{Binding, Inst};

const DUP: Definition = Definition(0);
const ADD: Definition = Definition(4);
const IF: Definition = Definition(18);

fn stack(segment: Vec<Ty>) -> Binding {
    Binding::Stack(segment)
}

fn value(ty: Ty) -> Binding {
    Binding::Value(ty)
}

fn lit_node(lit: Lit, under: Vec<Ty>) -> Node {
    Node::Literal {
        lit,
        inst: Inst {
            bindings: vec![stack(under)],
        },
    }
}

fn quote_node(body: Vec<u32>, around: Vec<Ty>, start: Vec<Ty>, end: Vec<Ty>) -> Node {
    Node::Quotation {
        body: body.into_iter().map(NodeId).collect(),
        inst: Inst {
            bindings: vec![
                stack(around),
                stack(start),
                stack(end),
                Binding::Effect(noble_kernel::types::EffSet::empty()),
            ],
        },
    }
}

fn invocation(def: Definition, bindings: Vec<Binding>) -> Node {
    Node::Invocation {
        def,
        inst: Inst { bindings },
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

fn reject(
    candidate: &noble_kernel::untrusted::Candidate,
    request: &Request,
) -> Result<noble_kernel::untrusted::Diagnostic, String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    match noble_kernel::acceptance::check(&env, request, candidate) {
        noble_kernel::untrusted::Outcome::Invalid(diagnostic) => Ok(diagnostic),
        other => Err(format!("expected a rejection, got {other:?}")),
    }
}

/// DX-01 variant 1: `1 true +` fails on the `+` word's input types — the
/// stack holds `I64 Bool` where the word requires `I64 I64`.
// r[verify DX-DIAG-01]
// r[verify VT-M2-01]
#[test]
fn dx01_word_input_type_names_word_and_both_stacks() -> Result<(), String> {
    let candidate = noble_kernel::untrusted::Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision: 0,
        nodes: vec![
            lit_node(Lit::I64(1), vec![]),
            lit_node(Lit::Bool(true), vec![Ty::I64]),
            invocation(ADD, vec![stack(vec![])]),
        ],
        body: vec![NodeId(0), NodeId(1), NodeId(2)],
    };
    let diagnostic = reject(&candidate, &request(vec![], vec![Ty::I64]))?;
    assert_eq!(
        diagnostic.def,
        Some(ADD),
        "word_or_join must name the + word"
    );
    assert_eq!(
        diagnostic.node,
        Some(NodeId(2)),
        "source_span must name the failing node"
    );
    assert_eq!(
        diagnostic.expected,
        vec![Ty::I64, Ty::I64],
        "required_stack"
    );
    assert_eq!(diagnostic.actual, vec![Ty::I64, Ty::Bool], "actual_stack");
    assert_eq!(diagnostic.constraint, Constraint::StackJoin, "constraint");
    assert!(
        !diagnostic.provenance_available,
        "value_origin reports unavailable"
    );
    Ok(())
}

/// DX-01 variant 2: `true [ 1 ] [ false ] if` fails the branch join — the
/// two branch programs claim different output stacks, so the `if` word's
/// required stack does not match what the stack actually holds.
// r[verify DX-DIAG-01]
// r[verify VT-M2-01]
#[test]
fn dx01_branch_output_type_names_the_unequal_join() -> Result<(), String> {
    let left = Ty::program(vec![], vec![Ty::I64], noble_kernel::types::EffSet::empty());
    let right = Ty::program(vec![], vec![Ty::Bool], noble_kernel::types::EffSet::empty());
    let empty = Ty::program(vec![], vec![], noble_kernel::types::EffSet::empty());
    let after_left = vec![Ty::Bool, left.clone()];
    let candidate = noble_kernel::untrusted::Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision: 0,
        nodes: vec![
            lit_node(Lit::Bool(true), vec![]),
            quote_node(vec![2], vec![Ty::Bool], vec![], vec![Ty::I64]),
            lit_node(Lit::I64(1), vec![]),
            quote_node(vec![4], after_left, vec![], vec![Ty::Bool]),
            lit_node(Lit::Bool(false), vec![]),
            invocation(
                IF,
                vec![
                    stack(vec![]),
                    stack(vec![]),
                    Binding::Effect(noble_kernel::types::EffSet::empty()),
                    Binding::Effect(noble_kernel::types::EffSet::empty()),
                ],
            ),
        ],
        body: vec![NodeId(0), NodeId(1), NodeId(3), NodeId(5)],
    };
    let diagnostic = reject(&candidate, &request(vec![], vec![]))?;
    assert_eq!(
        diagnostic.def,
        Some(IF),
        "word_or_join must name the if word"
    );
    assert_eq!(
        diagnostic.node,
        Some(NodeId(5)),
        "source_span must name the failing node"
    );
    assert_eq!(
        diagnostic.expected,
        vec![Ty::Bool, empty.clone(), empty],
        "required_stack carries both branch programs with the claimed equal interface"
    );
    assert_eq!(
        diagnostic.actual,
        vec![Ty::Bool, left, right],
        "actual_stack carries the two unequal branch programs"
    );
    assert_eq!(diagnostic.constraint, Constraint::StackJoin, "constraint");
    assert!(
        !diagnostic.provenance_available,
        "value_origin reports unavailable"
    );
    Ok(())
}

/// DX-01 variant 3: `dup` over the input stack `[Resource<test.counter>]`
/// fails the resource-duplication eligibility constraint, and the value
/// origin is reported as unavailable, never invented.
// r[verify DX-DIAG-01]
// r[verify B-DIAG-01]
// r[verify VT-M2-01]
#[test]
fn dx01_resource_duplication_reports_unavailable_provenance() -> Result<(), String> {
    let resource = Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let candidate = noble_kernel::untrusted::Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision: 0,
        nodes: vec![invocation(
            DUP,
            vec![stack(vec![]), value(resource.clone())],
        )],
        body: vec![NodeId(0)],
    };
    let expected_out = vec![resource.clone(), resource.clone()];
    let diagnostic = reject(&candidate, &request(vec![resource.clone()], expected_out))?;
    assert_eq!(
        diagnostic.def,
        Some(DUP),
        "word_or_join must name the dup word"
    );
    assert_eq!(
        diagnostic.node,
        Some(NodeId(0)),
        "source_span must name the failing node"
    );
    assert_eq!(
        diagnostic.constraint,
        Constraint::Eligibility(resource),
        "constraint is the eligibility violation"
    );
    assert!(
        !diagnostic.provenance_available,
        "value_origin is explicitly unavailable"
    );
    Ok(())
}
