#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; admission reads bounded files, invokes the independent checker and mutates retained session records through non-const APIs."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; bounded source/proof reads and independent admission preserve typed external-input failures without executing a candidate or asserting over submitted evidence."
)]
pub(in crate::core::companions) fn op_contract(
    driver: &mut crate::core::companions::Driver,
    args: &[&str],
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    let path = std::path::PathBuf::from(args[0]);
    let source = attempt!(crate::core::companions::entry::read_bounded(
        &path,
        crate::core::companions::SOURCE_LIMIT,
    )
    .map_err(crate::core::companions::Refused::script));
    let proof = match args.get(1) {
        Some(proof_path) => {
            let bytes = attempt!(crate::core::companions::entry::read_bounded(
                &std::path::PathBuf::from(proof_path),
                crate::core::companions::PROOF_LIMIT,
            )
            .map_err(crate::core::companions::Refused::script));
            Some(bytes)
        }
        None => None,
    };

    match crate::workflow::admission::admit(
        &mut driver.core,
        &source,
        crate::core::SOURCE_LIMITS,
        proof.as_ref().map(|bytes| (bytes.as_slice(), false)),
        driver.timeout_ms,
    ) {
        Ok(admission) => Ok(crate::core::companions::operations::admission::retain(
            driver, admission,
        )),
        Err(refusal) => Ok(refused(&refusal)),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; retention serializes the checked decision and replaces or inserts its exact retained contract record. Refused admissions remain ordinary report data, not assertions."
)]
fn retain(
    driver: &mut crate::core::companions::Driver,
    admission: crate::workflow::admission::Completion,
) -> crate::workflow::encoding::Json {
    let class = if admission.evidence.is_some() {
        noble_contracts::companion::EvidenceClass::LeanExact
    } else {
        noble_contracts::companion::EvidenceClass::Replay
    };
    let mut extras = fields(&admission, class);
    let admitted = admission.decision;
    if let Some(contract) = admitted.contract {
        // One record per retained contract: re-admission with fresh
        // evidence revises the retained record in place, so a later certify
        // cannot bind the stale unproved one.
        match driver
            .records
            .iter_mut()
            .find(|record| record.contract.0 == contract.0)
        {
            Some(record) => {
                record.statement = admitted.statement_digest;
                record.evidence = admitted.evidence;
                record.class = Some(class);
            }
            None => driver.records.push(crate::core::companions::Record {
                contract,
                statement: admitted.statement_digest,
                evidence: admitted.evidence,
                class: Some(class),
            }),
        }
    }
    let mut at = 0;
    while at < admitted.refusals.len() {
        let refusal = &admitted.refusals[at];
        extras.push(("refusal", crate::workflow::encoding::string(refusal.code())));
        at = at.saturating_add(1);
    }

    crate::core::companions::reporting::line(
        crate::core::companions::reporting::Header {
            operation: "contract",
            outcome: admitted.outcome.code(),
        },
        extras,
    )
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; these fields serialize an already checked admission result without making an admission decision or indexing unchecked external data."
)]
fn fields(
    admission: &crate::workflow::admission::Completion,
    class: noble_contracts::companion::EvidenceClass,
) -> std::vec::Vec<(&'static str, crate::workflow::encoding::Json)> {
    let admitted = &admission.decision;
    let toolchain = match &admission.evidence {
        Some(evidence) => crate::workflow::admission::tools_of(evidence),
        None => crate::workflow::admission::no_tools(),
    };
    std::vec::Vec::from([
        (
            "contract_reference",
            match admitted.contract {
                Some(contract) => crate::workflow::encoding::Json::Number(u64::from(contract.0)),
                None => crate::workflow::encoding::Json::Null,
            },
        ),
        (
            "statement",
            crate::workflow::encoding::string(crate::workflow::admission::digest_text(
                admitted.statement_digest,
            )),
        ),
        (
            "evidence_reference",
            match admitted.evidence {
                Some(evidence) => crate::workflow::encoding::Json::Number(u64::from(evidence.0)),
                None => crate::workflow::encoding::Json::Null,
            },
        ),
        (
            "evidence_class",
            crate::workflow::encoding::string(
                crate::core::companions::reporting::evidence_class_name(class),
            ),
        ),
        (
            "ordinary_typing",
            crate::workflow::encoding::Json::Bool(true),
        ),
        (
            "independent_recheck",
            crate::workflow::encoding::Json::Bool(admission.evidence.is_some()),
        ),
        (
            "claim_kind",
            crate::workflow::encoding::string("partial-correctness"),
        ),
        (
            "prover_calls",
            crate::workflow::encoding::Json::Number(u64::from(admission.evidence.is_some())),
        ),
        ("toolchain", toolchain),
        ("guest_requests", crate::workflow::encoding::Json::Number(0)),
        (
            "protected_operations",
            crate::workflow::encoding::Json::Number(0),
        ),
        (
            "candidate_body_started",
            crate::workflow::encoding::Json::Bool(false),
        ),
    ])
}

fn refused(refusal: &crate::workflow::admission::Refusal) -> crate::workflow::encoding::Json {
    crate::core::companions::reporting::line(
        crate::core::companions::reporting::Header {
            operation: "contract",
            outcome: refusal.claim_outcome(),
        },
        std::vec::Vec::from([
            (
                "diagnostic",
                crate::workflow::admission::refusal_json(refusal),
            ),
            (
                "ordinary_typing",
                crate::workflow::encoding::Json::Bool(refusal.ordinary_typing),
            ),
            (
                "prover_calls",
                crate::workflow::encoding::Json::Number(u64::from(refusal.verification_ran)),
            ),
            ("toolchain", crate::workflow::admission::no_tools()),
            ("guest_requests", crate::workflow::encoding::Json::Number(0)),
            (
                "protected_operations",
                crate::workflow::encoding::Json::Number(0),
            ),
            (
                "candidate_body_started",
                crate::workflow::encoding::Json::Bool(false),
            ),
        ]),
    )
}
