//! Calculator rows are granted visibility by index, so validation must pin
//! their schemes.
//!
//! `visibility` grants row 28 to `CalculatorCompare` by ordinal alone. Until
//! the profile prefix was validated, a host that replaced that row with
//! `S Resource(FIXTURE) -- S` kept the visible name and gained an interface
//! that consumes a linear resource — the kernel accepted such a call.

use noble_kernel::contracts::{Behavior, Definition, Env, FIXTURE_RESOURCE};
use noble_kernel::shapes::Pattern;
use noble_kernel::types::{ResourceKind, Ty};
use noble_kernel::untrusted::Constraint;

use super::builders::{self, Seen};

/// The calculator row `CalculatorCompare` occupies.
const COMPARE_ROW: usize = 28;
/// The same row as the definition a candidate names.
const COMPARE_DEFINITION: Definition = Definition(28);

fn calculator() -> Result<Env, String> {
    noble_kernel::contracts::calculator_environment()
        .map_err(|defect| format!("calculator table must validate: {defect:?}"))
}

/// The replaced row: `S Resource(FIXTURE) -- S`, still named
/// `CalculatorCompare` but no longer the canonical comparison.
fn resource_consuming() -> noble_kernel::words::Scheme {
    let row = Pattern::StackVar(noble_kernel::words::Variable(0));
    noble_kernel::words::Scheme {
        var_kinds: vec![noble_kernel::words::VariableKind::Stack],
        stack_in: vec![row.clone(), Pattern::Resource(FIXTURE_RESOURCE)],
        stack_out: vec![row],
        effects: vec![],
    }
}

/// One call of the compare row with an empty underlying stack.
fn call() -> noble_kernel::untrusted::Candidate {
    builders::candidate(
        vec![builders::invocation(
            COMPARE_DEFINITION,
            vec![builders::segment(Vec::new())],
        )],
        vec![0],
    )
}

#[test]
fn canonical_calculator_row_accepts_its_interface() -> Result<(), String> {
    let env = calculator()?;
    let request = builders::request(
        vec![Ty::I64, Ty::I64],
        vec![Ty::I64],
        builders::limits(10_000, 16, 512),
    );
    let observed = builders::seen(&env, &request, &call())?;
    assert!(
        matches!(observed, Seen::Accepted),
        "the canonical compare row must accept two integers, got {observed:?}"
    );
    Ok(())
}

#[test]
fn replaced_calculator_row_is_refused_as_an_invalid_contract() -> Result<(), String> {
    let mut env = calculator()?;
    assert_eq!(
        env.kinds[COMPARE_ROW],
        Behavior::CalculatorCompare,
        "the control needs the visible compare row"
    );
    env.defs[COMPARE_ROW] = resource_consuming();
    let request = builders::request(
        vec![Ty::Resource(ResourceKind(0))],
        Vec::new(),
        builders::limits(10_000, 16, 512),
    );
    let observed = builders::seen(&env, &request, &call())?;
    let Seen::Bad(diagnostic) = observed else {
        return Err(format!(
            "a replaced calculator row must reject the environment, got {observed:?}"
        ));
    };
    assert_eq!(diagnostic.constraint, Constraint::InvalidContract);
    Ok(())
}

#[test]
fn replaced_calculator_row_is_refused_even_for_its_canonical_call() -> Result<(), String> {
    let mut env = calculator()?;
    env.defs[COMPARE_ROW] = resource_consuming();
    let request = builders::request(
        vec![Ty::I64, Ty::I64],
        vec![Ty::I64],
        builders::limits(10_000, 16, 512),
    );
    let observed = builders::seen(&env, &request, &call())?;
    let Seen::Bad(diagnostic) = observed else {
        return Err(format!(
            "a replaced calculator row must reject the environment, got {observed:?}"
        ));
    };
    assert_eq!(diagnostic.constraint, Constraint::InvalidContract);
    Ok(())
}
