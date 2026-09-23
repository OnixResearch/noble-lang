#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; requested indexes, live subject observations and core binding are fallible checks. An invalid script or mismatched subject must return a refusal rather than panic."
)]
pub(in crate::core::companions) fn op_certify(
    driver: &mut crate::core::companions::Driver,
    args: &[&str],
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    let index = attempt!(crate::core::companions::reporting::index_of(
        args[0],
        driver.stack.len()
    ));
    let record_index = attempt!(
        crate::core::companions::operations::binding::contract_index(driver, args.get(1).copied())
    );
    let (ty, handle) = attempt!(driver.program_slot(index));
    let subject =
        attempt!(driver.observe_subject(&crate::core::companions::ProgramSlot { handle, ty }));
    let record = *attempt!(driver.record_for(record_index));
    let evidence = attempt!(record.evidence.ok_or_else(|| {
        crate::core::companions::Refused::new(
            "refused",
            "unknown-evidence",
            "the admitted contract has no accepted evidence".into(),
        )
    }));
    let published = attempt!(
        crate::core::companions::operations::binding::publish_companion(
            driver, &record, &subject, handle
        )
    );
    Ok(render(handle, record_index, evidence, &published))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; explicit index parsing and missing-record diagnostics use non-const parse and allocation APIs."
)]
fn contract_index(
    driver: &crate::core::companions::Driver,
    token: Option<&str>,
) -> crate::core::companions::Attempt<u32> {
    match token {
        Some(token) => token.parse::<u32>().map_err(|_| {
            crate::core::companions::Refused::script(std::format!(
                "'{token}' is not a contract index"
            ))
        }),
        None => driver
            .records
            .last()
            .map(|record| record.contract.0)
            .ok_or_else(|| {
                crate::core::companions::Refused::new(
                    "refused",
                    "unknown-contract",
                    "no contract has been admitted".into(),
                )
            }),
    }
}

/// Recheck the current subject before publishing any companion cells.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; publication first requires actual core evidence binding and then validates every injected live cell handle. Missing evidence and worker failures propagate as typed refusals."
)]
pub(in crate::core::companions) fn publish_companion(
    driver: &mut crate::core::companions::Driver,
    contract: &crate::core::companions::Record,
    subject: &noble_contracts::companion::Subject,
    subject_handle: u64,
) -> crate::core::companions::Attempt<crate::core::companions::Published> {
    let evidence = attempt!(contract.evidence.ok_or_else(|| {
        crate::core::companions::Refused::new(
            "refused",
            "unknown-evidence",
            "the contract has no accepted evidence".into(),
        )
    }));
    let bound = attempt!(driver
        .core
        .bind(evidence, subject)
        .map_err(crate::core::companions::Refused::core));
    let class = contract
        .class
        .unwrap_or(noble_contracts::companion::EvidenceClass::Replay);
    let contract_handle = attempt!(driver.push_cell(
        &crate::core::companions::reporting::inputs::contract_injection(
            crate::core::companions::reporting::inputs::ContractCell {
                index: contract.contract.0,
                statement: contract.statement,
                revision: driver.policy,
            },
        ),
        noble_kernel::types::Ty::Contract,
    ));
    let evidence_handle = attempt!(driver.push_cell(
        &crate::core::companions::reporting::inputs::evidence_injection(
            evidence.0,
            class,
            contract_handle,
        ),
        noble_kernel::types::Ty::Evidence,
    ));
    let certified_handle = attempt!(driver.push_cell(
        &crate::core::companions::reporting::inputs::certified_injection(
            crate::core::companions::reporting::inputs::CertifiedCell {
                subject: subject_handle,
                contract: contract_handle,
                evidence: evidence_handle,
                identity: bound.identity.0,
            },
        ),
        noble_kernel::types::Ty::Certified,
    ));
    Ok(crate::core::companions::Published {
        contract_handle,
        evidence_handle,
        certified_handle,
        subject_identity: bound.identity.0,
    })
}

fn render(
    subject: u64,
    contract: u32,
    evidence: noble_contracts::companion::EvidenceId,
    published: &crate::core::companions::Published,
) -> crate::workflow::encoding::Json {
    crate::core::companions::reporting::line(
        crate::core::companions::reporting::Header {
            operation: "certify",
            outcome: "certified",
        },
        std::vec::Vec::from([
            (
                "subject_handle",
                crate::workflow::encoding::Json::Number(subject),
            ),
            (
                "subject_identity",
                crate::workflow::encoding::string(std::format!(
                    "{:016x}",
                    published.subject_identity
                )),
            ),
            (
                "certified_handle",
                crate::workflow::encoding::Json::Number(published.certified_handle),
            ),
            (
                "contract_reference",
                crate::workflow::encoding::Json::Number(u64::from(contract)),
            ),
            (
                "evidence_reference",
                crate::workflow::encoding::Json::Number(u64::from(evidence.0)),
            ),
            (
                "behavioral_certification_claimed",
                crate::workflow::encoding::Json::Bool(true),
            ),
            ("prover_calls", crate::workflow::encoding::Json::Number(0)),
            (
                "candidate_prepare_requests",
                crate::workflow::encoding::Json::Number(0),
            ),
        ]),
    )
}
