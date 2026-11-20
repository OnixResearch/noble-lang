struct Completed {
    report: crate::core::report::Value,
    /// The selected template's stable code, from either set version.
    template: &'static str,
    wrapper: std::string::String,
    /// The session prelude that removes the companion operand cell so the
    /// wrapper's `run` sees exactly the contract's declared input. The v1 path
    /// parks the frame instead and has none.
    prelude: Option<&'static str>,
    /// The template-set version. `None` keeps the frozen v1 report fields.
    set_version: Option<u32>,
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
    // The v2 set is a separate versioned dispatch. A stale or unsupported v2
    // set is not an error here: the frozen v1 path below owns that refusal with
    // its own message, so v1 behaviour stays identical.
    if let Ok(templates) = driver.core.evidence_guard_templates_v2(companion.evidence) {
        if templates.contains(&noble_contracts::companion::GuardTemplateV2::NonEmptyList) {
            return invoke_list(driver, args, companion, program);
        }
    }
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
        template: template.code(),
        wrapper,
        prelude: None,
        set_version: None,
        companion,
        has_changed_identity,
    })
}

/// The v2 `NonEmptyList` invocation path.
///
/// The argument is a typed `List A` taken from a live session slot, never a CLI
/// literal: it cannot be reconstructed as an `I64` injection. The subject's
/// declared input is exactly that one value, and `run` types only when the
/// whole stack matches it, so the session stack must be the argument with the
/// companion operand cell directly above it (`[list, Certified]`). The prelude
/// `swap drop` removes that companion cell from the operand stack; the wrapper
/// text itself is then exactly the fixed live text and consumes the argument.
fn invoke_list(
    driver: &mut crate::core::companions::Driver,
    args: &[&str],
    companion: crate::core::companions::CertifiedSlot,
    program: crate::core::companions::ProgramSlot,
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    let list_slot = attempt!(crate::core::companions::reporting::index_of(
        args[1],
        driver.stack.len()
    ));
    if driver.stack.len() != 2 || list_slot != 0 {
        return Err(crate::core::companions::Refused::new(
            "refused",
            "stack-type",
            std::format!(
                "the list argument must be the bottom of a two-slot session stack, found slot {list_slot} of {}",
                driver.stack.len()
            ),
        ));
    }
    let list_ty = attempt!(driver.stack.first().cloned().ok_or_else(|| {
        crate::core::companions::Refused::new(
            "malformed",
            "stack-type",
            std::string::String::from("the session stack has no list slot"),
        )
    }));
    let expected = attempt!(driver
        .core
        .list_guard_interface(companion.contract)
        .map_err(crate::core::companions::Refused::core));
    if list_ty != expected {
        return Err(crate::core::companions::Refused::new(
            "refused",
            "stack-type",
            std::string::String::from(
                "the list slot type does not match the retained list interface",
            ),
        ));
    }
    let wrapper = driver
        .core
        .invocation_wrapper_v2(noble_contracts::companion::GuardTemplateV2::NonEmptyList);
    const PRELUDE: &str = "swap drop";
    let source = std::format!("{PRELUDE} {wrapper} run");
    let report = attempt!(driver.execute(
        source.as_bytes(),
        &[list_ty, noble_kernel::types::Ty::Certified, program.ty,],
        &[crate::core::companions::reporting::inputs::program_injection(companion.subject)],
    ));
    let outcome = report
        .member("outcome")
        .and_then(crate::core::report::Value::text)
        .unwrap_or_default();
    if outcome != "normal" {
        return Err(crate::core::companions::Refused::new(
            "refused",
            "guard-runtime",
            std::format!("the v2 guard invocation did not return normally: {outcome}"),
        ));
    }
    // The frame injected the observed subject handle itself, so the wrapper
    // never reconstructs or replaces code.
    let has_changed_identity = program.handle != companion.subject;
    render(&Completed {
        report,
        template: noble_contracts::companion::GuardTemplateV2::NonEmptyList.code(),
        wrapper,
        prelude: Some(PRELUDE),
        set_version: Some(noble_contracts::companion::GUARD_SET_VERSION_V2),
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
    if let Some(prelude) = invocation.prelude {
        fields.push((
            "invocation_prelude",
            crate::workflow::encoding::string(prelude),
        ));
    }
    if let Some(version) = invocation.set_version {
        fields.push((
            "guard_template_set_version",
            crate::workflow::encoding::Json::Number(u64::from(version)),
        ));
    }
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
            crate::workflow::encoding::string(invocation.template),
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
