//! A resolved witness chain spends the request's work.
//!
//! `words::resolve` charges one unit per hop and returns the spend, but the
//! acceptance path discarded it, so a witness could name chains for free. These
//! controls measure the charge exactly: the same interface, stated with direct
//! bindings and with one or three reference hops, must move its accepting work
//! limit by exactly those hops.

use noble_kernel::contracts::{Behavior, Definition, Env};
use noble_kernel::shapes::Pattern;
use noble_kernel::types::Ty;
use noble_kernel::untrusted::{Candidate, LimitKind, Request};

use super::builders::{self, Seen};

/// Largest work limit this control searches; the environment charges well under it.
const SEARCH_LIMIT: u32 = 200;

/// `S -- S S S` over four stack variables, so one witness can chain through
/// two references and still terminate on a stack binding.
fn chained_environment() -> Result<(Env, Definition), String> {
    let mut env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    let row = env.defs.len();
    let definition = match u32::try_from(row) {
        Ok(index) => Definition(index),
        Err(_) => return Err("definition index must fit u32".to_string()),
    };
    let scheme = noble_kernel::words::Scheme {
        var_kinds: vec![
            noble_kernel::words::VariableKind::Stack,
            noble_kernel::words::VariableKind::Stack,
            noble_kernel::words::VariableKind::Stack,
            noble_kernel::words::VariableKind::Stack,
        ],
        stack_in: vec![Pattern::StackVar(noble_kernel::words::Variable(0))],
        stack_out: vec![
            Pattern::StackVar(noble_kernel::words::Variable(1)),
            Pattern::StackVar(noble_kernel::words::Variable(2)),
            Pattern::StackVar(noble_kernel::words::Variable(3)),
        ],
        effects: vec![],
    };
    env.defs.push(scheme);
    env.kinds.push(Behavior::Named);
    env.deps.push(Vec::new());
    env.definition_owners.push(None);
    Ok((env, definition))
}

fn candidate_with(
    definition: Definition,
    bindings: Vec<noble_kernel::words::Binding>,
) -> Candidate {
    builders::candidate(vec![builders::invocation(definition, bindings)], vec![0])
}

/// The interface stated directly: three `Bool` results, no reference binding.
fn direct(definition: Definition) -> Candidate {
    candidate_with(
        definition,
        vec![
            builders::segment(Vec::new()),
            builders::segment(vec![Ty::Bool]),
            builders::segment(vec![Ty::Bool]),
            builders::segment(vec![Ty::Bool]),
        ],
    )
}

/// Variable 1 names variable 3: one hop.
fn one_hop(definition: Definition) -> Candidate {
    candidate_with(
        definition,
        vec![
            builders::segment(Vec::new()),
            builders::reference(3),
            builders::segment(vec![Ty::Bool]),
            builders::segment(vec![Ty::Bool]),
        ],
    )
}

/// Variable 1 names 2, which names 3 (two hops), and variable 2 is itself a
/// reference (one hop): three hops in one witness.
fn three_hops(definition: Definition) -> Candidate {
    candidate_with(
        definition,
        vec![
            builders::segment(Vec::new()),
            builders::reference(2),
            builders::reference(3),
            builders::segment(vec![Ty::Bool]),
        ],
    )
}

fn request_at(work: u32) -> Request {
    builders::request(
        Vec::new(),
        vec![Ty::Bool, Ty::Bool, Ty::Bool],
        builders::limits(work, 16, 64),
    )
}

/// The smallest work limit that accepts this candidate.
fn first_accepting(env: &Env, candidate: &Candidate) -> Result<u32, String> {
    let mut work = 0;
    while work <= SEARCH_LIMIT {
        match builders::seen(env, &request_at(work), candidate)? {
            Seen::Accepted => return Ok(work),
            Seen::Exhausted(LimitKind::Work) => {}
            other => {
                return Err(format!(
                    "work limit {work} must accept or exhaust for work, got {other:?}"
                ))
            }
        }
        work += 1;
    }
    Err(format!(
        "no limit up to {SEARCH_LIMIT} accepted the candidate"
    ))
}

#[test]
fn one_reference_hop_costs_exactly_one_work_unit() -> Result<(), String> {
    let (env, definition) = chained_environment()?;
    let direct_min = first_accepting(&env, &direct(definition))?;
    let hop_min = first_accepting(&env, &one_hop(definition))?;
    assert_eq!(
        hop_min,
        direct_min.saturating_add(1),
        "one hop must cost one unit: direct accepted at {direct_min}, one-hop at {hop_min}"
    );
    Ok(())
}

#[test]
fn every_hop_of_a_longer_chain_is_charged() -> Result<(), String> {
    let (env, definition) = chained_environment()?;
    let direct_min = first_accepting(&env, &direct(definition))?;
    let chain_min = first_accepting(&env, &three_hops(definition))?;
    assert_eq!(
        chain_min,
        direct_min.saturating_add(3),
        "three hops must cost three units: direct accepted at {direct_min}, chained at {chain_min}"
    );
    Ok(())
}

#[test]
fn a_chain_longer_than_the_remaining_work_exhausts_for_work() -> Result<(), String> {
    let (env, definition) = chained_environment()?;
    let chain_min = first_accepting(&env, &three_hops(definition))?;
    // Every limit below the boundary reports work exhaustion, and the boundary
    // limit accepts the same body: the refusal is the chain's own spend.
    let mut work = 0;
    while work < chain_min {
        let observed = builders::seen(&env, &request_at(work), &three_hops(definition))?;
        assert!(
            matches!(observed, Seen::Exhausted(LimitKind::Work)),
            "work limit {work} must exhaust for work, got {observed:?}"
        );
        work += 1;
    }
    let observed = builders::seen(&env, &request_at(chain_min), &three_hops(definition))?;
    assert!(
        matches!(observed, Seen::Accepted),
        "the boundary limit must accept the chain, got {observed:?}"
    );
    Ok(())
}
