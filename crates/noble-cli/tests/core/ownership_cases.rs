//! OWN-CASE-04 non-borrow variants: a linear resource is never `Data`, so `dup`, `drop`, `quote`
//! and capture through `Pair`/`Sum`/`List` are statically rejected before any emission. The borrow
//! variants of the case are inapplicable (owner decision 2026-11-20: no borrow profile), and the
//! resource-bearing source forms that would build these stack values are the live-slot profile's
//! `Account@1` inputs; this harness uses the kernel acceptance boundary directly so the rejection
//! is the exact `Eligibility` constraint, not a downstream emission failure.

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

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; candidate construction clones recursive contract types and allocates Vec-backed bindings, which are not const on the pinned compiler."
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-maintainers; the typed fixture is assembled from the exact kernel vocabulary (input type, word, bindings, expected interface) so a rejection names the same values the checker sees."
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

#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the matrix requires exactly one Invalid/Eligibility outcome per declared resource variant plus accepted data controls, and every other outcome must name itself in the failure."
)]
fn own_case_04_non_borrow_variants_reject_before_emission() -> Result<(), String> {
    let environment =
        noble_kernel::contracts::environment().map_err(|error| format!("{error:?}"))?;
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let variants: [(&str, noble_kernel::types::Ty, u32); 6] = [
        ("dup", resource.clone(), 0),
        ("drop", resource.clone(), 1),
        ("quote", resource.clone(), 8),
        (
            "pair-then-quote",
            noble_kernel::types::Ty::Pair(
                Box::new(noble_kernel::types::Ty::I64),
                Box::new(resource.clone()),
            ),
            8,
        ),
        (
            "sum-then-drop",
            noble_kernel::types::Ty::Sum(
                Box::new(resource.clone()),
                Box::new(noble_kernel::types::Ty::I64),
            ),
            1,
        ),
        (
            "list-then-drop",
            noble_kernel::types::Ty::List(Box::new(resource.clone())),
            1,
        ),
    ];
    for (name, input, word) in variants {
        let (request, candidate) = invocation(&input, word)?;
        match noble_kernel::acceptance::check(&environment, &request, &candidate) {
            noble_kernel::untrusted::Outcome::Invalid(diagnostic) => assert_eq!(
                diagnostic.constraint,
                noble_kernel::untrusted::Constraint::Eligibility(input.clone()),
                "{name}"
            ),
            other => {
                return Err(format!(
                    "{name} unexpectedly returned {other:?} instead of an eligibility rejection"
                ))
            }
        }
        println!(
            "{{\"id\":\"OWN-CASE-04\",\"variant\":\"{name}\",\"outcome\":\"static-reject\",\"stage\":\"check\",\"emitted\":false,\"guest_requests\":0,\"protected_operations\":0}}"
        );
    }
    // The same single-word harness accepts an ordinary `Data` operand, so each rejection above is
    // the resource eligibility rule rather than a malformed fixture.
    for word in [0, 1, 8] {
        let (request, candidate) = invocation(&noble_kernel::types::Ty::I64, word)?;
        match noble_kernel::acceptance::check(&environment, &request, &candidate) {
            noble_kernel::untrusted::Outcome::Accepted(_) => {}
            other => return Err(format!("data control {word} was not accepted: {other:?}")),
        }
    }
    Ok(())
}
