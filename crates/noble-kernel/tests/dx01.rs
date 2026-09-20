#![feature(register_tool)]
#![register_tool(tigerstyle)]
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

#[path = "dx01/builders.rs"]
mod builders;
// The fragment-v1 rejection families (recursion, schema, cycle,
// eliminator joins) extend the DX-01 controls in their own module.
#[path = "dx01/v1.rs"]
mod v1;

const DUP: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(0);
const ADD: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(4);
const IF: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(18);

/// DX-01 variant 1: `1 true +` fails on the `+` word's input types — the
/// stack holds `I64 Bool` where the word requires `I64 I64`.
// r[verify DX-DIAG-01]
// r[verify VT-M2-01]
#[test]
fn dx01_word_input_type_names_word_and_both_stacks() -> Result<(), String> {
    let candidate = crate::builders::candidate(
        vec![
            crate::builders::lit_node(noble_kernel::untrusted::Lit::I64(1), vec![]),
            crate::builders::lit_node(
                noble_kernel::untrusted::Lit::Bool(true),
                vec![noble_kernel::types::Ty::I64],
            ),
            crate::builders::invocation(ADD, vec![crate::builders::stack(vec![])]),
        ],
        vec![0, 1, 2],
    );
    let diagnostic = crate::builders::reject(
        &candidate,
        &crate::builders::request(vec![], vec![noble_kernel::types::Ty::I64]),
    )?;
    assert_eq!(
        diagnostic.def,
        Some(ADD),
        "word_or_join must name the + word"
    );
    assert_eq!(
        diagnostic.node,
        Some(noble_kernel::untrusted::NodeId(2)),
        "source_span must name the failing node"
    );
    assert_eq!(
        diagnostic.expected,
        vec![noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64],
        "required_stack"
    );
    assert_eq!(
        diagnostic.actual,
        vec![noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool],
        "actual_stack"
    );
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::StackJoin,
        "constraint"
    );
    assert!(
        !diagnostic.provenance_available,
        "value_origin reports unavailable"
    );
    Ok(())
}

fn unequal_if_candidate(left: noble_kernel::types::Ty) -> noble_kernel::untrusted::Candidate {
    crate::builders::candidate(
        vec![
            crate::builders::lit_node(noble_kernel::untrusted::Lit::Bool(true), vec![]),
            crate::builders::quote_node(
                vec![2],
                vec![noble_kernel::types::Ty::Bool],
                vec![],
                vec![noble_kernel::types::Ty::I64],
            ),
            crate::builders::lit_node(noble_kernel::untrusted::Lit::I64(1), vec![]),
            crate::builders::quote_node(
                vec![4],
                vec![noble_kernel::types::Ty::Bool, left],
                vec![],
                vec![noble_kernel::types::Ty::Bool],
            ),
            crate::builders::lit_node(noble_kernel::untrusted::Lit::Bool(false), vec![]),
            crate::builders::invocation(
                IF,
                vec![
                    crate::builders::stack(vec![]),
                    crate::builders::stack(vec![]),
                    noble_kernel::words::Binding::Effect(noble_kernel::types::EffSet::empty()),
                    noble_kernel::words::Binding::Effect(noble_kernel::types::EffSet::empty()),
                ],
            ),
        ],
        vec![0, 1, 3, 5],
    )
}

/// DX-01 variant 2: `true [ 1 ] [ false ] if` fails the branch join — the
/// two branch programs claim different output stacks, so the `if` word's
/// required stack does not match what the stack actually holds.
// r[verify DX-DIAG-01]
// r[verify VT-M2-01]
#[test]
fn dx01_branch_output_type_names_the_unequal_join() -> Result<(), String> {
    let left = crate::builders::program(vec![], vec![noble_kernel::types::Ty::I64]);
    let right = crate::builders::program(vec![], vec![noble_kernel::types::Ty::Bool]);
    let empty = crate::builders::program(vec![], vec![]);
    let candidate = unequal_if_candidate(left.clone());
    let diagnostic =
        crate::builders::reject(&candidate, &crate::builders::request(vec![], vec![]))?;
    assert_eq!(
        diagnostic.def,
        Some(IF),
        "word_or_join must name the if word"
    );
    assert_eq!(
        diagnostic.node,
        Some(noble_kernel::untrusted::NodeId(5)),
        "source_span must name the failing node"
    );
    assert_eq!(
        diagnostic.expected,
        vec![noble_kernel::types::Ty::Bool, empty.clone(), empty],
        "required_stack carries both branch programs with the claimed equal interface"
    );
    assert_eq!(
        diagnostic.actual,
        vec![noble_kernel::types::Ty::Bool, left, right],
        "actual_stack carries the two unequal branch programs"
    );
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::StackJoin,
        "constraint"
    );
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
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let candidate = crate::builders::candidate(
        vec![crate::builders::invocation(
            DUP,
            vec![
                crate::builders::stack(vec![]),
                crate::builders::value(resource.clone()),
            ],
        )],
        vec![0],
    );
    let expected_out = vec![resource.clone(), resource.clone()];
    let diagnostic = crate::builders::reject(
        &candidate,
        &crate::builders::request(vec![resource.clone()], expected_out),
    )?;
    assert_eq!(
        diagnostic.def,
        Some(DUP),
        "word_or_join must name the dup word"
    );
    assert_eq!(
        diagnostic.node,
        Some(noble_kernel::untrusted::NodeId(0)),
        "source_span must name the failing node"
    );
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::Eligibility(resource),
        "constraint is the eligibility violation"
    );
    assert!(
        !diagnostic.provenance_available,
        "value_origin is explicitly unavailable"
    );
    Ok(())
}
