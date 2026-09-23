struct Completed {
    report: crate::core::report::Value,
    template: noble_contracts::companion::GuardTemplate,
    wrapper: std::string::String,
    companion: crate::core::companions::CertifiedSlot,
    has_changed_identity: bool,
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; invocation borrows live engine state, observes the subject and compiles a checked guard through non-const runtime APIs."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; current evidence applicability and the exact bound guard template are checked before live invocation, with frame restoration and typed refusal on every failure path."
)]
pub(in crate::core::companions) fn op_invoke(
    driver: &mut crate::core::companions::Driver,
    args: &[&str],
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    let index = attempt!(crate::core::companions::reporting::index_of(
        args[0],
        driver.stack.len()
    ));
    let argument = attempt!(crate::core::companions::reporting::integer_of(args[1]));
    let companion = attempt!(driver.certified_at(index));
    let program = attempt!(driver.bound_companion(&companion));
    let templates = attempt!(driver
        .core
        .evidence_guard_templates(companion.evidence)
        .map_err(crate::core::companions::Refused::core));
    let template = attempt!(templates.first().copied().ok_or_else(|| {
        crate::core::companions::Refused::new(
            "refused",
            "unsupported-guard-template",
            "the contract has no prechecked guard template".into(),
        )
    }));
    // The guard consumes the original live Program, never a reconstruction.
    let wrapper = driver.core.invocation_wrapper(template);
    let source = std::format!("{wrapper} run");
    let parked = attempt!(driver.park());
    let outcome = driver.execute(
        source.as_bytes(),
        &[noble_kernel::types::Ty::I64, program.ty],
        &[
            crate::core::companions::reporting::inputs::i64_injection(argument),
            crate::core::companions::reporting::inputs::program_injection(companion.subject),
        ],
    );
    let report = attempt!(driver.restore_frame(parked, outcome));
    let again = attempt!(driver.certified_at(index));
    let has_changed_identity = again.subject != companion.subject;
    render(&Completed {
        report,
        template,
        wrapper,
        companion,
        has_changed_identity,
    })
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; invocation reporting allocates JSON fields and reads parsed observations through non-const helpers."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the guard result's exact left/right variant is checked with a typed refusal; malformed external observations must not trigger assertion panics."
)]
fn render(
    invocation: &Completed,
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    let variant = invocation
        .report
        .member("stack")
        .and_then(crate::core::report::Value::items)
        .and_then(|items| items.last())
        .and_then(|entry| entry.member("variant"))
        .and_then(crate::core::report::Value::text)
        .unwrap_or_default();
    if !matches!(variant, "left" | "right") {
        return Err(crate::core::companions::Refused::script(
            "guard wrapper returned neither rejection nor invocation",
        ));
    }
    let is_accepted = variant == "right";
    let mut fields = provenance(invocation);
    fields.extend([
        (
            "output",
            crate::core::companions::reporting::values::top_payload(&invocation.report),
        ),
        ("runtime_return", crate::workflow::encoding::string(variant)),
        (
            "candidate_body_started",
            crate::workflow::encoding::Json::Bool(is_accepted),
        ),
        ("prover_calls", crate::workflow::encoding::Json::Number(0)),
        (
            "candidate_prepare_requests",
            crate::workflow::encoding::Json::Number(0),
        ),
        (
            "guest_requests",
            attempt!(crate::core::companions::reporting::runtime_count(
                &invocation.report,
                "guest_requests",
            )),
        ),
        (
            "protected_operations",
            attempt!(crate::core::companions::reporting::runtime_count(
                &invocation.report,
                "protected_operations",
            )),
        ),
        (
            "wrapper_executions",
            crate::workflow::encoding::Json::Number(1),
        ),
    ]);
    Ok(crate::core::companions::reporting::line(
        crate::core::companions::reporting::Header {
            operation: "invoke",
            outcome: if is_accepted {
                "certified-invocation"
            } else {
                "precondition-reject"
            },
        },
        fields,
    ))
}

fn provenance(
    invocation: &Completed,
) -> std::vec::Vec<(&'static str, crate::workflow::encoding::Json)> {
    std::vec::Vec::from([
        (
            "guard_template",
            crate::workflow::encoding::string(invocation.template.code()),
        ),
        ("guard_erased", crate::workflow::encoding::Json::Bool(false)),
        (
            "verified_backend_claimed",
            crate::workflow::encoding::Json::Bool(false),
        ),
        (
            "underlying_program_identity_changed",
            crate::workflow::encoding::Json::Bool(invocation.has_changed_identity),
        ),
        (
            "subject_handle",
            crate::workflow::encoding::Json::Number(invocation.companion.subject),
        ),
        (
            "subject_identity",
            crate::workflow::encoding::string(&invocation.companion.identity),
        ),
        (
            "artifact_correspondence",
            crate::workflow::encoding::string("recorded-trusted-build"),
        ),
        (
            "host_authority",
            crate::workflow::encoding::string("independently-checked-resource-free"),
        ),
        (
            "wrapper_source",
            crate::workflow::encoding::string(&invocation.wrapper),
        ),
        (
            "conditional_theorem_valid",
            crate::workflow::encoding::Json::Bool(true),
        ),
    ])
}
