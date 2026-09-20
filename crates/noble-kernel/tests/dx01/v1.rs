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

const DUP: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(0);
const SWAP: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(2);
const CASE: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(17);
const LIST_CASE: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(21);

/// The DX-01 fields every `Diagnostic`-carrying rejection must report:
/// the failing word, the failing node, the violated constraint, and
/// provenance explicitly unavailable.
fn assert_dx01_fields(
    diagnostic: &noble_kernel::untrusted::Diagnostic,
    def: noble_kernel::contracts::Definition,
    node: u32,
    constraint: &noble_kernel::untrusted::Constraint,
) {
    assert_eq!(diagnostic.def, Some(def), "word_or_join names the word");
    assert_eq!(
        diagnostic.node,
        Some(noble_kernel::untrusted::NodeId(node)),
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
    let env = crate::builders::env_with_deps(&[vec![23]])?;
    let candidate = crate::builders::candidate(vec![], vec![]);
    let outcome = noble_kernel::acceptance::check(
        &env,
        &crate::builders::request(vec![], vec![]),
        &candidate,
    );
    assert_eq!(
        crate::builders::unsupported(outcome)?,
        noble_kernel::untrusted::UnsupportedKind::RecursiveDependency(
            noble_kernel::contracts::Definition(23)
        ),
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
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; unsupported requires the rejection category and the assertion checks its schema identity; the declaration fixture does not need redundant assertions."
)]
fn dx01_recursive_schema_names_the_declaration() -> Result<(), String> {
    let mut env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    env.schemas.push(noble_kernel::contracts::SchemaDecl {
        id: noble_kernel::contracts::SchemaId(7),
        scheme: crate::builders::unit_like(),
        recursive: true,
    });
    let candidate = crate::builders::candidate(vec![], vec![]);
    let outcome = noble_kernel::acceptance::check(
        &env,
        &crate::builders::request(vec![], vec![]),
        &candidate,
    );
    assert_eq!(
        crate::builders::unsupported(outcome)?,
        noble_kernel::untrusted::UnsupportedKind::RecursiveSchema(
            noble_kernel::contracts::SchemaId(7)
        ),
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
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; reject requires an invalid outcome and assert_dx01_fields checks the word, node, cyclic constraint and unavailable provenance; this caller must not duplicate that helper's assertions."
)]
fn dx01_cyclic_witness_names_the_constraint_and_reports_unavailable_provenance(
) -> Result<(), String> {
    let candidate = crate::builders::candidate(
        vec![crate::builders::invocation(
            DUP,
            vec![
                crate::builders::stack(vec![]),
                crate::builders::reference(1),
            ],
        )],
        vec![0],
    );
    let diagnostic =
        crate::builders::reject(&candidate, &crate::builders::request(vec![], vec![]))?;
    assert_dx01_fields(
        &diagnostic,
        DUP,
        0,
        &noble_kernel::untrusted::Constraint::CyclicWitness,
    );
    Ok(())
}

/// DX-01 case-join control: the two `case` branch programs claim
/// different output stacks, and the diagnostic names the `case` word and
/// carries both branch stacks.
// r[verify DX-DIAG-01]
#[test]
fn dx01_case_join_names_the_word_and_both_branch_stacks() -> Result<(), String> {
    let claimed_left = crate::builders::program(vec![noble_kernel::types::Ty::I64], vec![]);
    let claimed_right = crate::builders::program(vec![noble_kernel::types::Ty::Bool], vec![]);
    let actual_left = crate::builders::program(
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::I64],
    );
    let actual_right = crate::builders::program(
        vec![noble_kernel::types::Ty::Bool],
        vec![noble_kernel::types::Ty::Bool],
    );
    let sum = noble_kernel::types::Ty::Sum(
        Box::new(noble_kernel::types::Ty::I64),
        Box::new(noble_kernel::types::Ty::Bool),
    );
    let candidate = crate::builders::candidate(
        vec![crate::builders::invocation(
            CASE,
            vec![
                crate::builders::stack(vec![]),
                crate::builders::value(noble_kernel::types::Ty::I64),
                crate::builders::value(noble_kernel::types::Ty::Bool),
                crate::builders::stack(vec![]),
                noble_kernel::words::Binding::Effect(noble_kernel::types::EffSet::empty()),
                noble_kernel::words::Binding::Effect(noble_kernel::types::EffSet::empty()),
            ],
        )],
        vec![0],
    );
    let diagnostic = crate::builders::reject(
        &candidate,
        &crate::builders::request(
            vec![sum.clone(), actual_left.clone(), actual_right.clone()],
            vec![],
        ),
    )?;
    assert_eq!(
        diagnostic.expected,
        vec![sum.clone(), claimed_left, claimed_right],
        "required_stack carries both branch programs with the claimed equal join"
    );
    assert_eq!(
        diagnostic.actual,
        vec![sum, actual_left, actual_right],
        "actual_stack carries both branch programs as they claim themselves"
    );
    assert_dx01_fields(
        &diagnostic,
        CASE,
        0,
        &noble_kernel::untrusted::Constraint::StackJoin,
    );
    Ok(())
}

/// DX-01 list.case-join control: the nil and cons branch programs claim
/// different output stacks, and the diagnostic names the `list.case`
/// word and carries both branch stacks.
// r[verify DX-DIAG-01]
#[test]
fn dx01_list_case_join_names_the_word_and_both_branch_stacks() -> Result<(), String> {
    let claimed_nil = crate::builders::program(vec![], vec![]);
    let element = noble_kernel::types::Ty::I64;
    let list = noble_kernel::types::Ty::List(Box::new(element.clone()));
    let claimed_cons = crate::builders::program(vec![element.clone(), list.clone()], vec![]);
    let actual_nil = crate::builders::program(vec![], vec![noble_kernel::types::Ty::Unit]);
    let actual_cons =
        crate::builders::program(vec![element, list], vec![noble_kernel::types::Ty::Unit]);
    let candidate = crate::builders::candidate(
        vec![crate::builders::invocation(
            LIST_CASE,
            vec![
                crate::builders::stack(vec![]),
                crate::builders::value(noble_kernel::types::Ty::I64),
                crate::builders::stack(vec![]),
                noble_kernel::words::Binding::Effect(noble_kernel::types::EffSet::empty()),
                noble_kernel::words::Binding::Effect(noble_kernel::types::EffSet::empty()),
            ],
        )],
        vec![0],
    );
    let diagnostic = crate::builders::reject(
        &candidate,
        &crate::builders::request(
            vec![
                noble_kernel::types::Ty::List(Box::new(noble_kernel::types::Ty::I64)),
                actual_nil.clone(),
                actual_cons.clone(),
            ],
            vec![],
        ),
    )?;
    assert_eq!(
        diagnostic.expected,
        vec![
            noble_kernel::types::Ty::List(Box::new(noble_kernel::types::Ty::I64)),
            claimed_nil,
            claimed_cons,
        ],
        "required_stack carries both branch programs with the claimed equal join"
    );
    assert_eq!(
        diagnostic.actual,
        vec![
            noble_kernel::types::Ty::List(Box::new(noble_kernel::types::Ty::I64)),
            actual_nil,
            actual_cons,
        ],
        "actual_stack carries both branch programs as they claim themselves"
    );
    assert_dx01_fields(
        &diagnostic,
        LIST_CASE,
        0,
        &noble_kernel::untrusted::Constraint::StackJoin,
    );
    Ok(())
}

/// A cyclic witness inside a compound body keeps the DX-01 shape: the
/// diagnostic reports the failing swap node, not the surrounding body.
// r[verify DX-DIAG-01]
#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; reject requires invalidity and assert_dx01_fields checks swap node 1, its cyclic constraint and unavailable provenance; fixture length does not warrant duplicate assertions."
)]
fn dx01_swap_witness_cycle_inside_a_body_names_the_node() -> Result<(), String> {
    // `swap` with its two value variables bound to each other, inside a
    // two-node body: the diagnostic must name the swap node, not the body.
    let candidate = crate::builders::candidate(
        vec![
            crate::builders::lit_node(noble_kernel::untrusted::Lit::I64(1), vec![]),
            crate::builders::invocation(
                SWAP,
                vec![
                    crate::builders::stack(vec![]),
                    crate::builders::reference(2),
                    crate::builders::reference(1),
                ],
            ),
        ],
        vec![0, 1],
    );
    let diagnostic =
        crate::builders::reject(&candidate, &crate::builders::request(vec![], vec![]))?;
    assert_dx01_fields(
        &diagnostic,
        SWAP,
        1,
        &noble_kernel::untrusted::Constraint::CyclicWitness,
    );
    Ok(())
}
