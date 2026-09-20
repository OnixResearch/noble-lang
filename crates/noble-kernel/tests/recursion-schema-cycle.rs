#![feature(register_tool)]
#![register_tool(tigerstyle)]
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

#[path = "recursion_schema_cycle/builders.rs"]
mod builders;
#[path = "recursion_schema_cycle/witnesses.rs"]
mod witnesses;

#[test]
fn recursive_dependency_self_names_identity() -> Result<(), String> {
    let env = crate::builders::env_with(&[vec![23]])?;
    let outcome = noble_kernel::acceptance::check(
        &env,
        &crate::builders::request(vec![], vec![]),
        &crate::builders::empty_program(),
    );
    assert_eq!(
        crate::builders::unsupported_of(outcome)?,
        noble_kernel::untrusted::UnsupportedKind::RecursiveDependency(
            noble_kernel::contracts::Definition(23)
        ),
        "the rejection names the definition the cycle closes on"
    );
    Ok(())
}

#[test]
fn recursive_dependency_mutual_names_identity() -> Result<(), String> {
    // Definitions 23 and 23+1 depend on each other.
    let env = crate::builders::env_with(&[vec![24], vec![23]])?;
    let outcome = noble_kernel::acceptance::check(
        &env,
        &crate::builders::request(vec![], vec![]),
        &crate::builders::empty_program(),
    );
    assert_eq!(
        crate::builders::unsupported_of(outcome)?,
        noble_kernel::untrusted::UnsupportedKind::RecursiveDependency(
            noble_kernel::contracts::Definition(23)
        ),
        "the mutual cycle closes on the definition the walk reaches on-path"
    );
    Ok(())
}

#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; two let-else checks require exact-boundary acceptance and below-boundary exhaustion, then the assertion identifies Work; repeating those checks would add no coverage."
)]
fn acyclic_dependencies_accept_at_the_walk_boundary() -> Result<(), String> {
    // One edge, and exactly one unit of declared work: the walk charges the
    // edge and succeeds; the empty body charges nothing.
    let env = crate::builders::env_with(&[vec![24], vec![]])?;
    let mut request = crate::builders::request(vec![], vec![]);
    request.limits.work = 1;
    let observed = crate::builders::seen(&env, &request, &crate::builders::empty_program())?;
    let crate::builders::Seen::Accepted = observed else {
        return Err(format!(
            "an acyclic dependency chain must check at the work boundary, got {observed:?}"
        ));
    };
    // The empty body records no derivations.
    // One below the boundary: the walk's single charge fails closed.
    request.limits.work = 0;
    let observed = crate::builders::seen(&env, &request, &crate::builders::empty_program())?;
    let crate::builders::Seen::Exhausted(limit) = observed else {
        return Err(format!(
            "a work limit below the walk's charge must fail closed, got {observed:?}"
        ));
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::Work);
    Ok(())
}

#[test]
fn external_dependency_data_is_validated_before_any_body_check() -> Result<(), String> {
    // The environment carries a recursive dependency; the candidate also
    // carries a dangling node reference that would reject as invalid. The
    // outcome is the unsupported environment rejection, proving external
    // environment data never enters checking unvalidated.
    let env = crate::builders::env_with(&[vec![23]])?;
    let dangling = crate::builders::candidate(vec![], vec![9]);
    let outcome =
        noble_kernel::acceptance::check(&env, &crate::builders::request(vec![], vec![]), &dangling);
    assert_eq!(
        crate::builders::unsupported_of(outcome)?,
        noble_kernel::untrusted::UnsupportedKind::RecursiveDependency(
            noble_kernel::contracts::Definition(23)
        )
    );
    Ok(())
}

#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; unsupported_of requires the rejection category and the equality assertion requires the exact recursive schema identity; the fixture needs no assertion padding."
)]
fn recursive_schema_rejects_unsupported_naming_the_declaration() -> Result<(), String> {
    let mut env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    env.schemas.push(noble_kernel::contracts::SchemaDecl {
        id: noble_kernel::contracts::SchemaId(7),
        scheme: crate::builders::unit_like(),
        recursive: true,
    });
    let outcome = noble_kernel::acceptance::check(
        &env,
        &crate::builders::request(vec![], vec![]),
        &crate::builders::empty_program(),
    );
    assert_eq!(
        crate::builders::unsupported_of(outcome)?,
        noble_kernel::untrusted::UnsupportedKind::RecursiveSchema(
            noble_kernel::contracts::SchemaId(7)
        ),
        "a user-declared recursive schema is unsupported and named"
    );
    Ok(())
}

#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; let-else checks require acceptance at two schema-work units and exhaustion at one, and the assertion identifies Work; declaration setup is not an assertion obligation."
)]
fn nonrecursive_schemas_scan_at_the_boundary_and_exhaust_below_it() -> Result<(), String> {
    let mut env = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    env.schemas.push(noble_kernel::contracts::SchemaDecl {
        id: noble_kernel::contracts::SchemaId(1),
        scheme: crate::builders::unit_like(),
        recursive: false,
    });
    env.schemas.push(noble_kernel::contracts::SchemaDecl {
        id: noble_kernel::contracts::SchemaId(2),
        scheme: crate::builders::unit_like(),
        recursive: false,
    });
    let mut request = crate::builders::request(vec![], vec![]);
    request.limits.work = 2;
    let observed = crate::builders::seen(&env, &request, &crate::builders::empty_program())?;
    let crate::builders::Seen::Accepted = observed else {
        return Err(format!(
            "two non-recursive schema entries scan within two work units, got {observed:?}"
        ));
    };
    request.limits.work = 1;
    let observed = crate::builders::seen(&env, &request, &crate::builders::empty_program())?;
    let crate::builders::Seen::Exhausted(limit) = observed else {
        return Err(format!(
            "the schema scan must fail closed below its charge, got {observed:?}"
        ));
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::Work);
    Ok(())
}
