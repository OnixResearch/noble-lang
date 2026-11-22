struct Completed {
    report: crate::core::report::Value,
    /// The selected template's stable code, from either set version.
    template: &'static str,
    wrapper: std::string::String,
    /// The v2 prelude that replays the copied list argument below the
    /// injected subject, so the wrapper's `run` sees exactly the contract's
    /// declared input. The v1 path injects its `I64` argument directly and has
    /// none.
    prelude: Option<&'static str>,
    /// The v2 session text that copied the list argument out of its slot.
    argument_copy: Option<std::string::String>,
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
            return invoke_list(driver, args, index, companion, program);
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
        argument_copy: None,
        set_version: None,
        companion,
        has_changed_identity,
    })
}

/// The prelude that runs the injected argument copy below the subject.
const LIST_PRELUDE: &str = "[ run ] dip";

/// The copy text nests one `[ … ] dip swap` per cell above the list slot;
/// half of the frontend's 64-level nesting bound leaves room for the
/// session's own frames, and a deeper slot is refused before any execution.
const MAX_CELLS_ABOVE_LIST: usize = 32;

/// The v2 `NonEmptyList` invocation path, at parity with the v1 path.
///
/// The argument is the typed `List A` in a live session slot, never a CLI
/// literal. The guest exposes no cell-copy injection, so the slot's value is
/// copied into a Program cell by `dup quote` (K-PROG-07: `quote` captures only
/// data, immutably), moved to the top through one `dip`/`swap` per cell above
/// the slot. Then, exactly as on the v1 path, the whole frame is parked, the
/// copy and the retained subject are injected into a fresh frame, the prelude
/// replays the copy below the subject and the fixed live wrapper text runs on
/// `[list, Program]`. Restoring the frame preserves every session cell,
/// including any tail below the argument and the companion itself, whose
/// identity is re-checked; the copy cell is dropped afterwards on every path.
fn invoke_list(
    driver: &mut crate::core::companions::Driver,
    args: &[&str],
    index: usize,
    companion: crate::core::companions::CertifiedSlot,
    program: crate::core::companions::ProgramSlot,
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    let list_slot = attempt!(crate::core::companions::reporting::index_of(
        args[1],
        driver.stack.len()
    ));
    let list_ty = attempt!(driver.stack.get(list_slot).cloned().ok_or_else(|| {
        crate::core::companions::Refused::new(
            "malformed",
            "stack-index",
            std::format!("stack has no slot {list_slot}"),
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
    let above = driver
        .stack
        .len()
        .saturating_sub(list_slot.saturating_add(1));
    if above > MAX_CELLS_ABOVE_LIST {
        return Err(crate::core::companions::Refused::new(
            "refused",
            "stack-depth",
            std::format!(
                "the list slot is {above} cells below the stack top; at most {MAX_CELLS_ABOVE_LIST} are supported"
            ),
        ));
    }
    let copy = copy_to_top(above);
    let input = driver.stack.clone();
    let copied = attempt!(driver.execute(copy.as_bytes(), &input, &[]));
    if copied
        .member("outcome")
        .and_then(crate::core::report::Value::text)
        != Some("normal")
    {
        return Err(crate::core::companions::Refused::new(
            "refused",
            "argument-copy",
            std::string::String::from("the list argument could not be copied out of its slot"),
        ));
    }
    let wrapper = driver
        .core
        .invocation_wrapper_v2(noble_contracts::companion::GuardTemplateV2::NonEmptyList);
    let guarded = run_list_guard(driver, &wrapper, &program);
    // The copy cell is the restored top slot; it is released on every path,
    // after the frame (or its refusal) is settled.
    let released = release_copy(driver);
    let report = attempt!(guarded);
    attempt!(released);
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
    let again = attempt!(driver.certified_at(index));
    let has_changed_identity = again.subject != companion.subject;
    render(&Completed {
        report,
        template: noble_contracts::companion::GuardTemplateV2::NonEmptyList.code(),
        wrapper,
        prelude: Some(LIST_PRELUDE),
        argument_copy: Some(copy),
        set_version: Some(noble_contracts::companion::GUARD_SET_VERSION_V2),
        companion,
        has_changed_identity,
    })
}

/// `dup quote` at the list slot, carried to the top past `above` cells.
fn copy_to_top(above: usize) -> std::string::String {
    let mut source = std::string::String::from("dup quote");
    let mut level = 0usize;
    while level < above {
        source = std::format!("[ {source} ] dip swap");
        level = level.saturating_add(1);
    }
    source
}

/// Park the session, run the guard on `[copy, subject]` and restore the frame.
/// The copy is the session's top Program slot; the subject is the companion's
/// retained handle, never a reconstruction.
fn run_list_guard(
    driver: &mut crate::core::companions::Driver,
    wrapper: &str,
    program: &crate::core::companions::ProgramSlot,
) -> crate::core::companions::Attempt<crate::core::report::Value> {
    let top = driver.stack.len().saturating_sub(1);
    let (copy_ty, copy) = attempt!(driver.program_slot(top));
    let source = std::format!("{LIST_PRELUDE} {wrapper} run");
    let parked = attempt!(driver.park());
    let outcome = driver.execute(
        source.as_bytes(),
        &[copy_ty, program.ty.clone()],
        &[
            crate::core::companions::reporting::inputs::program_injection(copy),
            crate::core::companions::reporting::inputs::program_injection(program.handle),
        ],
    );
    driver.restore_frame(parked, outcome)
}

/// Drop the argument copy from the top of the restored session stack.
fn release_copy(
    driver: &mut crate::core::companions::Driver,
) -> crate::core::companions::Attempt<()> {
    let input = driver.stack.clone();
    let report = attempt!(driver.execute(b"drop", &input, &[]));
    if report
        .member("outcome")
        .and_then(crate::core::report::Value::text)
        != Some("normal")
    {
        return Err(crate::core::companions::Refused::script(
            "could not release the list argument copy",
        ));
    }
    Ok(())
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
    if let Some(copy) = &invocation.argument_copy {
        fields.push((
            "invocation_argument_copy",
            crate::workflow::encoding::string(copy),
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
