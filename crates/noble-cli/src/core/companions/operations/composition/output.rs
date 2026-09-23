pub(super) fn refused(
    reason: noble_contracts::companion::Refusal,
    handle: u64,
) -> crate::workflow::encoding::Json {
    crate::core::companions::reporting::line(
        crate::core::companions::reporting::Header {
            operation: "compose",
            outcome: crate::core::companions::refusal_outcome(&reason),
        },
        std::vec::Vec::from([
            (
                "ordinary_composition_valid",
                crate::workflow::encoding::Json::Bool(true),
            ),
            (
                "new_certified_status",
                crate::workflow::encoding::Json::Bool(false),
            ),
            (
                "diagnostic",
                crate::workflow::encoding::object([
                    ("code", crate::workflow::encoding::string(reason.code())),
                    (
                        "message",
                        crate::workflow::encoding::string("the core withheld certification"),
                    ),
                ]),
            ),
            (
                "composed_handle",
                crate::workflow::encoding::Json::Number(handle),
            ),
            ("prover_calls", crate::workflow::encoding::Json::Number(0)),
            ("guest_requests", crate::workflow::encoding::Json::Number(0)),
            (
                "protected_operations",
                crate::workflow::encoding::Json::Number(0),
            ),
        ]),
    )
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; reporting preserves the actual checked composition result and propagates malformed worker observations instead of asserting over external reports."
)]
pub(super) fn accepted(
    completed: &super::Completed,
    handles: [u64; 3],
    increment: i64,
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    let mut fields = provenance(completed, handles);
    fields.extend([
        (
            "output",
            crate::core::companions::reporting::values::top_value(&completed.report),
        ),
        (
            "claim_kind",
            crate::workflow::encoding::string("partial-correctness"),
        ),
        (
            "increment_by",
            crate::workflow::encoding::string(std::format!("{increment}")),
        ),
        (
            "guest_requests",
            attempt!(crate::core::companions::reporting::runtime_count(
                &completed.report,
                "guest_requests"
            )),
        ),
        (
            "protected_operations",
            attempt!(crate::core::companions::reporting::runtime_count(
                &completed.report,
                "protected_operations",
            )),
        ),
        (
            "underlying_recipe_matches_plain_compose",
            crate::workflow::encoding::Json::Bool(true),
        ),
        (
            "assumptions_dropped",
            crate::workflow::encoding::Json::Bool(false),
        ),
        ("prover_calls", crate::workflow::encoding::Json::Number(0)),
        (
            "candidate_prepare_requests",
            crate::workflow::encoding::Json::Number(0),
        ),
    ]);
    Ok(crate::core::companions::reporting::line(
        crate::core::companions::reporting::Header {
            operation: "compose",
            outcome: "certified-composition",
        },
        fields,
    ))
}

fn provenance(
    completed: &super::Completed,
    handles: [u64; 3],
) -> std::vec::Vec<(&'static str, crate::workflow::encoding::Json)> {
    std::vec::Vec::from([
        (
            "statement",
            crate::workflow::encoding::string(crate::workflow::admission::digest_text(
                completed.derived.statement_digest,
            )),
        ),
        (
            "subject_identity",
            crate::workflow::encoding::string(std::format!(
                "{:016x}",
                completed.observed.identity.0
            )),
        ),
        (
            "composed_handle",
            crate::workflow::encoding::Json::Number(completed.subject.handle),
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
            "evidence_reference",
            crate::workflow::encoding::Json::Number(u64::from(completed.derived.evidence.0)),
        ),
        (
            "contract_reference",
            crate::workflow::encoding::Json::Number(u64::from(completed.derived.contract.0)),
        ),
    ])
}
