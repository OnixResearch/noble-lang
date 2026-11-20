//! OWN-CASE-04 non-borrow variants: a linear resource is never `Data`, so `dup`, `drop`, `quote`
//! and capture through `Pair`/`Sum`/`List` are statically rejected before any emission. The borrow
//! variants of the case are inapplicable (owner decision 2026-11-20: no borrow profile). This
//! harness uses the kernel acceptance boundary directly so the rejection is the exact
//! `Eligibility` constraint, not a downstream emission failure.

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
    assert_eq!(candidate.body.len(), 1, "fixture body must be one node");
    assert_eq!(
        request.expected.stack_in.len(),
        1,
        "fixture must take exactly one operand"
    );
    Ok((request, candidate))
}

#[test]
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
    assert_eq!(variants.len(), 6, "six non-borrow variants are declared");
    // Bounded growth: at most one violation per declared variant and per data control.
    let mut violations = Vec::with_capacity(variants.len().saturating_add(3));
    for (name, input, word) in variants {
        let (request, candidate) = invocation(&input, word)?;
        let is_eligible = matches!(
            noble_kernel::acceptance::check(&environment, &request, &candidate),
            noble_kernel::untrusted::Outcome::Invalid(diagnostic)
                if diagnostic.constraint == noble_kernel::untrusted::Constraint::Eligibility(input.clone())
        );
        if !is_eligible {
            violations.push(format!("{name} was not rejected by the eligibility rule"));
        }
        println!(
            "{{\"id\":\"OWN-CASE-04\",\"variant\":\"{name}\",\"outcome\":\"static-reject\",\"stage\":\"check\",\"emitted\":false,\"guest_requests\":0,\"protected_operations\":0}}"
        );
    }
    // The same single-word harness accepts an ordinary `Data` operand, so each rejection above is
    // the resource eligibility rule rather than a malformed fixture.
    for word in [0, 1, 8] {
        let (request, candidate) = invocation(&noble_kernel::types::Ty::I64, word)?;
        if !matches!(
            noble_kernel::acceptance::check(&environment, &request, &candidate),
            noble_kernel::untrusted::Outcome::Accepted(_)
        ) {
            violations.push(format!("data control {word} was not accepted"));
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("; "));
    Ok(())
}
