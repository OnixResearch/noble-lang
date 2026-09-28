#[test]
fn transitive_resources_reject_both_variant_arms_and_opaque_wrappers() -> Result<(), String> {
    let mut env = required!(super::env(), "bootstrap");
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let wrapper = resource_wrapper(resource.clone());
    let right_resource = right_resource_variant(resource.clone());
    let left_resource = left_resource_variant(resource);
    declare!(env, wrapper.clone(), "resource wrapper");
    declare!(env, right_resource.clone(), "resource alternative");
    declare!(env, left_resource.clone(), "left resource alternative");
    for ty in [
        super::declared_type(&wrapper),
        super::declared_type(&right_resource),
        super::declared_type(&left_resource),
    ] {
        assert!(!ty.is_data(), "{ty:?} must not be data");
        rejects_resource_data(&env, ty);
    }
    let unvalidated = super::opaque(
        noble_kernel::types::NominalTypeId {
            module: 71,
            ordinal: 3,
        },
        noble_kernel::types::Ty::Resource(noble_kernel::types::ResourceKind(999)),
        true,
    );
    assert_eq!(
        env.clone().declare_nominal(unvalidated).err(),
        Some(noble_kernel::contracts::NominalError::InvalidRepresentation)
    );
    Ok(())
}

fn resource_wrapper(resource: noble_kernel::types::Ty) -> noble_kernel::contracts::NominalDecl {
    super::opaque(
        noble_kernel::types::NominalTypeId {
            module: 71,
            ordinal: 0,
        },
        noble_kernel::types::Ty::List(Box::new(resource)),
        true,
    )
}

fn right_resource_variant(
    resource: noble_kernel::types::Ty,
) -> noble_kernel::contracts::NominalDecl {
    super::variant(
        noble_kernel::types::NominalTypeId {
            module: 71,
            ordinal: 1,
        },
        noble_kernel::types::Ty::I64,
        noble_kernel::types::Ty::Pair(Box::new(noble_kernel::types::Ty::Text), Box::new(resource)),
        [true, true],
    )
}

fn left_resource_variant(
    resource: noble_kernel::types::Ty,
) -> noble_kernel::contracts::NominalDecl {
    super::variant(
        noble_kernel::types::NominalTypeId {
            module: 71,
            ordinal: 2,
        },
        noble_kernel::types::Ty::List(Box::new(resource)),
        noble_kernel::types::Ty::I64,
        [true, true],
    )
}

fn rejects_resource_data(env: &noble_kernel::contracts::Env, ty: noble_kernel::types::Ty) {
    let mut bindings = vec![
        super::super::support::segment(vec![]),
        super::super::support::value(ty.clone()),
    ];
    for def in [
        noble_kernel::contracts::Definition(0),
        noble_kernel::contracts::Definition(1),
    ] {
        reject_resource_binding(env, &ty, def, bindings.clone());
    }
    bindings.push(super::super::support::segment(vec![]));
    reject_resource_binding(env, &ty, noble_kernel::contracts::Definition(8), bindings);
}

fn reject_resource_binding(
    env: &noble_kernel::contracts::Env,
    ty: &noble_kernel::types::Ty,
    def: noble_kernel::contracts::Definition,
    bindings: Vec<noble_kernel::words::Binding>,
) {
    let request = super::super::support::request(vec![ty.clone()], vec![], &[]);
    let outcome = super::check(env, request, super::single(def, bindings));
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem)
            if problem.constraint == noble_kernel::untrusted::Constraint::Eligibility(ty.clone())),
        "expected eligibility refusal for {ty:?} from {def:?}, got {outcome:?}"
    );
}
