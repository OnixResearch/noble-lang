mod output;

struct Completed {
    derived: noble_contracts::companion::Derived,
    observed: noble_contracts::companion::Subject,
    subject: crate::core::companions::ProgramSlot,
    report: crate::core::report::Value,
}

#[octet::sealed_enum]
enum Outcome {
    Accepted(Completed),
    Refused(noble_contracts::companion::Refusal, u64),
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; composition operates a live engine frame and fallible mutable session state through non-const APIs."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; composition validates both live companions, parks their exact frame, restores it on every result and reports core refusals rather than asserting over external inputs."
)]
pub(in crate::core::companions) fn op_compose(
    driver: &mut crate::core::companions::Driver,
    args: &[&str],
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    let left = attempt!(crate::core::companions::reporting::index_of(
        args[0],
        driver.stack.len()
    ));
    let right = attempt!(crate::core::companions::reporting::index_of(
        args[1],
        driver.stack.len()
    ));
    let argument = attempt!(crate::core::companions::reporting::integer_of(args[2]));
    let companions = [
        attempt!(driver.certified_at(left)),
        attempt!(driver.certified_at(right)),
    ];
    let programs = [
        attempt!(driver.bound_companion(&companions[0])),
        attempt!(driver.bound_companion(&companions[1])),
    ];
    let parked = attempt!(driver.park());
    let outcome = crate::core::companions::operations::composition::compose_in_frame(
        driver,
        &companions,
        &programs,
        argument,
    );
    let outcome = attempt!(driver.restore_frame(parked, outcome));
    match outcome {
        Outcome::Accepted(completed) => {
            crate::core::companions::operations::composition::publish(driver, completed)
        }
        Outcome::Refused(reason, handle) => Ok(output::refused(reason, handle)),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this operation compiles and executes the composition in the parked engine frame, then calls the non-const deterministic core."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; compiled composition is observed and checked against both carried evidence references. Missing implications and runtime failures are typed refusals, never panic assertions."
)]
fn compose_in_frame(
    driver: &mut crate::core::companions::Driver,
    companions: &[crate::core::companions::CertifiedSlot; 2],
    programs: &[crate::core::companions::ProgramSlot; 2],
    argument: i64,
) -> crate::core::companions::Attempt<Outcome> {
    let composed = attempt!(driver.execute(
        b"compose",
        &[programs[0].ty.clone(), programs[1].ty.clone()],
        &[
            crate::core::companions::reporting::inputs::program_injection(programs[0].handle),
            crate::core::companions::reporting::inputs::program_injection(programs[1].handle),
        ],
    ));
    let subject =
        attempt!(crate::core::companions::operations::derivations::top_program(driver, &composed));
    let observed = attempt!(driver.observe_subject(&subject));
    let derived =
        match driver
            .core
            .derive_compose(companions[0].evidence, companions[1].evidence, &observed)
        {
            Ok(derived) => derived,
            Err(reason) => return Ok(Outcome::Refused(reason, subject.handle)),
        };
    let report = attempt!(driver.run_subject(&subject, argument));
    Ok(Outcome::Accepted(Completed {
        derived,
        observed,
        subject,
        report,
    }))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; publishing injects actual live cells and appends retained records through non-const engine and Vec APIs."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; publication obtains the derived core descriptor, requires the observed subject to bind and reports fallible live-cell publication. It must preserve operational failure outcomes."
)]
fn publish(
    driver: &mut crate::core::companions::Driver,
    completed: Completed,
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    let record = crate::core::companions::Record {
        contract: completed.derived.contract,
        statement: completed.derived.statement_digest,
        evidence: Some(completed.derived.evidence),
        class: Some(noble_contracts::companion::EvidenceClass::Replay),
    };
    let published = attempt!(
        crate::core::companions::operations::binding::publish_companion(
            driver,
            &record,
            &completed.observed,
            completed.subject.handle
        )
    );
    driver.records.push(record);
    let increment = match attempt!(driver
        .core
        .claim(completed.derived.evidence)
        .map_err(crate::core::companions::Refused::core))
    {
        noble_contracts::companion::ClaimTemplate::IncrementBy(value) => value,
        _ => {
            return Err(crate::core::companions::Refused::new(
                "internal-failure",
                "derived-claim",
                "composition has no increment conclusion".into(),
            ))
        }
    };
    output::accepted(
        &completed,
        [
            published.contract_handle,
            published.evidence_handle,
            published.certified_handle,
        ],
        increment,
    )
}
