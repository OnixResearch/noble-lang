struct Completed {
    derived: noble_contracts::companion::Derived,
    observed: noble_contracts::companion::Subject,
    instance: crate::core::companions::ProgramSlot,
    argument_output: Option<(i64, crate::workflow::encoding::Json)>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; instantiation validates the live builder and capture, restores the original frame on every result and publishes only a core-accepted derived subject; input failures remain refusals."
)]
pub(in crate::core::companions) fn op_instantiate(
    driver: &mut crate::core::companions::Driver,
    args: &[&str],
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    let index = attempt!(crate::core::companions::reporting::index_of(
        args[0],
        driver.stack.len()
    ));
    let capture = attempt!(crate::core::companions::reporting::integer_of(args[1]));
    let argument = attempt!(args
        .get(2)
        .map(|token| crate::core::companions::reporting::integer_of(token))
        .transpose());
    let companion = attempt!(driver.certified_at(index));
    let builder = attempt!(driver.bound_companion(&companion));
    let parked = attempt!(driver.park());
    let outcome = crate::core::companions::operations::instantiation::instantiate_in_frame(
        driver,
        &builder,
        companion.evidence,
        capture,
        argument,
    );
    let completed = attempt!(driver.restore_frame(parked, outcome));
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
            completed.instance.handle
        )
    );
    driver.records.push(record);
    Ok(report_instance(
        completed,
        [
            published.contract_handle,
            published.evidence_handle,
            published.certified_handle,
        ],
        capture,
        companion.subject,
    ))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; instantiation runs and observes a compiled builder in the live engine, then invokes non-const core derivation."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; builder execution, returned-Program observation and family derivation use typed checks. A wrong capture or malformed runtime result must not panic."
)]
fn instantiate_in_frame(
    driver: &mut crate::core::companions::Driver,
    builder: &crate::core::companions::ProgramSlot,
    evidence: noble_contracts::companion::EvidenceId,
    capture: i64,
    argument: Option<i64>,
) -> crate::core::companions::Attempt<Completed> {
    let report = attempt!(driver.run_subject(builder, capture));
    let instance =
        attempt!(crate::core::companions::operations::derivations::top_program(driver, &report));
    let observed = attempt!(driver.observe_subject(&instance));
    let derived = attempt!(driver
        .core
        .instantiate(evidence, capture, &observed)
        .map_err(crate::core::companions::Refused::core));
    let argument_output = match argument {
        Some(argument) => {
            let report = attempt!(driver.run_subject(&instance, argument));
            Some((
                argument,
                crate::core::companions::reporting::values::top_value(&report),
            ))
        }
        None => None,
    };
    Ok(Completed {
        derived,
        observed,
        instance,
        argument_output,
    })
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; this serializer renders an already checked instance and its actual invocation result without changing certification or validating new input."
)]
fn report_instance(
    completed: Completed,
    handles: [u64; 3],
    capture: i64,
    builder: u64,
) -> crate::workflow::encoding::Json {
    let mut extras = std::vec::Vec::from([
        (
            "program_identity",
            crate::workflow::encoding::string(std::format!(
                "{:016x}",
                completed.observed.identity.0
            )),
        ),
        (
            "statement",
            crate::workflow::encoding::string(crate::workflow::admission::digest_text(
                completed.derived.statement_digest,
            )),
        ),
        (
            "capture",
            crate::workflow::encoding::string(std::format!("{capture}")),
        ),
        (
            "builder_handle",
            crate::workflow::encoding::Json::Number(builder),
        ),
        (
            "instance_handle",
            crate::workflow::encoding::Json::Number(completed.instance.handle),
        ),
        (
            "contract_handle",
            crate::workflow::encoding::Json::Number(handles[0]),
        ),
        (
            "evidence_handle",
            crate::workflow::encoding::Json::Number(handles[1]),
        ),
        (
            "certified_handle",
            crate::workflow::encoding::Json::Number(handles[2]),
        ),
        (
            "same_compiled_builder",
            crate::workflow::encoding::Json::Bool(true),
        ),
        (
            "literal_specialization_sufficient",
            crate::workflow::encoding::Json::Bool(false),
        ),
        ("prover_calls", crate::workflow::encoding::Json::Number(0)),
        (
            "candidate_prepare_requests",
            crate::workflow::encoding::Json::Number(0),
        ),
    ]);
    if let Some((argument, output)) = completed.argument_output {
        extras.push((
            "argument",
            crate::workflow::encoding::string(std::format!("{argument}")),
        ));
        extras.push(("output", output));
    }
    crate::core::companions::reporting::line(
        crate::core::companions::reporting::Header {
            operation: "instantiate",
            outcome: "each-instance-bound-and-certified",
        },
        extras,
    )
}
