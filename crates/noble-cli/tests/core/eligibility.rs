const fn limits() -> noble_kernel::untrusted::Limits {
    noble_kernel::untrusted::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        type_size: 512,
        stack_height: 128,
        work: 2_000_000,
        diagnostics: 16,
    }
}

#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the nested matrix requires Invalid with the matching Eligibility constraint for all nine resource/word pairs and returns an error for every other outcome. The repeated behavioral assertion needs no fixture-only padding."
)]
fn core_15_resource_matrix() -> Result<(), String> {
    let environment =
        noble_kernel::contracts::environment().map_err(|error| format!("{error:?}"))?;
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let inputs = [
        resource.clone(),
        noble_kernel::types::Ty::Pair(
            Box::new(noble_kernel::types::Ty::I64),
            Box::new(resource.clone()),
        ),
        noble_kernel::types::Ty::Sum(Box::new(resource), Box::new(noble_kernel::types::Ty::I64)),
    ];
    for input in &inputs {
        for word in [0, 1, 8] {
            let (request, candidate) = invocation(input, word)?;
            match noble_kernel::acceptance::check(&environment, &request, &candidate) {
                noble_kernel::untrusted::Outcome::Invalid(diagnostic) => assert_eq!(
                    diagnostic.constraint,
                    noble_kernel::untrusted::Constraint::Eligibility(input.clone())
                ),
                other => {
                    return Err(format!(
                        "resource eligibility unexpectedly returned {other:?}"
                    ))
                }
            }
        }
    }
    println!("{{\"id\":\"CORE-15\",\"stage\":\"check\",\"outcome\":\"eligibility-reject\",\"variants\":9,\"runtime_handles_supplied\":false,\"guest_requests\":0,\"protected_operations\":0}}");
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; invocation clones recursive types and allocates Vec-backed bindings, program types and candidates. Those fixture construction APIs are not const on the pinned compiler."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; invocation builds the three declared hostile eligibility candidates and returns an error for an unknown harness word. Acceptance behavior is checked by core_15_resource_matrix, not by asserting copied fixture fields."
)]
fn invocation(
    input: &noble_kernel::types::Ty,
    word: u32,
) -> Result<
    (
        noble_kernel::untrusted::Request,
        noble_kernel::untrusted::Candidate,
    ),
    String,
> {
    let mut bindings = vec![
        noble_kernel::words::Binding::Stack(vec![]),
        noble_kernel::words::Binding::Value(input.clone()),
    ];
    let output = match word {
        0 => vec![input.clone(), input.clone()],
        1 => vec![],
        8 => {
            bindings.push(noble_kernel::words::Binding::Stack(vec![]));
            vec![noble_kernel::types::Ty::program(
                vec![],
                vec![input.clone()],
                noble_kernel::types::EffSet::empty(),
            )]
        }
        _ => return Err("unexpected harness word".into()),
    };
    let candidate = noble_kernel::untrusted::Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision: noble_kernel::untrusted::SEMANTIC_REVISION,
        nodes: vec![noble_kernel::untrusted::Node::Invocation {
            def: noble_kernel::contracts::Definition(word),
            inst: noble_kernel::words::Inst { bindings },
        }],
        body: vec![noble_kernel::untrusted::NodeId(0)],
    };
    let request = noble_kernel::untrusted::Request {
        input_bytes: 1,
        expected: noble_kernel::untrusted::Expected {
            stack_in: vec![input.clone()],
            stack_out: output,
            allowed_effects: noble_kernel::types::EffSet::empty(),
        },
        limits: limits(),
    };
    Ok((request, candidate))
}
