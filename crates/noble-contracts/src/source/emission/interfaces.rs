pub(super) fn collect(
    state: &super::super::inference::State,
    environment: &mut noble_kernel::contracts::Env,
    meter: &mut crate::Meter,
) -> Result<alloc::vec::Vec<noble_kernel::untrusted::Expected>, crate::Diagnostic> {
    let mut expected = alloc::vec::Vec::with_capacity(state.bodies.len());
    let mut at = 0usize;
    let mut failure = None;
    while at < state.bodies.len() {
        match install_interface(&state.bodies[at], &state.arena, environment, meter) {
            Ok(mut interface) => {
                if at == 0 {
                    if let Some(logical) = &state.logical_inputs {
                        interface.stack_in = logical.clone();
                    }
                }
                expected.push(interface);
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
        at += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(expected),
    }
}

fn install_interface(
    body: &super::super::inference::Body,
    arena: &crate::inference::Arena,
    environment: &mut noble_kernel::contracts::Env,
    meter: &mut crate::Meter,
) -> Result<noble_kernel::untrusted::Expected, crate::Diagnostic> {
    let interface = attempt!(super::materialization::interface(arena, body, meter));
    if body.identity.is_some() {
        let scheme = attempt!(super::contracts::scheme(
            &interface,
            environment,
            body.span,
            meter
        ));
        let deps = attempt!(super::materialization::dependencies(body, meter));
        environment.defs.reserve(1);
        environment.kinds.reserve(1);
        environment.deps.reserve(1);
        environment.defs.push(scheme);
        environment
            .kinds
            .push(noble_kernel::contracts::Behavior::Named);
        environment.deps.push(deps);
        environment.definition_owners.push(body.owner);
    }
    Ok(interface)
}
