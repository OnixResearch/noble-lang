pub(in crate::core::companions) fn i64_injection(value: i64) -> crate::workflow::encoding::Json {
    crate::workflow::encoding::object([
        ("type", crate::workflow::encoding::string("I64")),
        (
            "value",
            crate::workflow::encoding::string(std::format!("{value}")),
        ),
    ])
}

pub(in crate::core::companions) fn program_injection(
    handle: u64,
) -> crate::workflow::encoding::Json {
    crate::workflow::encoding::object([
        ("type", crate::workflow::encoding::string("Program")),
        ("handle", crate::workflow::encoding::Json::Number(handle)),
    ])
}

pub(in crate::core::companions) struct ContractCell {
    pub index: u32,
    pub statement: u64,
    pub revision: u32,
}

pub(in crate::core::companions) fn contract_injection(
    cell: crate::core::companions::reporting::inputs::ContractCell,
) -> crate::workflow::encoding::Json {
    crate::workflow::encoding::object([
        ("type", crate::workflow::encoding::string("Contract")),
        (
            "index",
            crate::workflow::encoding::Json::Number(u64::from(cell.index)),
        ),
        (
            "statement",
            crate::workflow::encoding::string(std::format!("{}", cell.statement)),
        ),
        (
            "claim_kind",
            crate::workflow::encoding::Json::Number(u64::from(
                crate::core::companions::CLAIM_PARTIAL_CORRECTNESS,
            )),
        ),
        (
            "revision",
            crate::workflow::encoding::Json::Number(u64::from(cell.revision)),
        ),
    ])
}

pub(in crate::core::companions) fn evidence_injection(
    index: u32,
    class: noble_contracts::companion::EvidenceClass,
    contract: u64,
) -> crate::workflow::encoding::Json {
    crate::workflow::encoding::object([
        ("type", crate::workflow::encoding::string("Evidence")),
        (
            "index",
            crate::workflow::encoding::Json::Number(u64::from(index)),
        ),
        (
            "class",
            crate::workflow::encoding::Json::Number(u64::from(class.code())),
        ),
        (
            "ruleset",
            crate::workflow::encoding::Json::Number(u64::from(crate::core::companions::RULESET)),
        ),
        (
            "contract",
            crate::workflow::encoding::Json::Number(contract),
        ),
    ])
}

pub(in crate::core::companions) struct CertifiedCell {
    pub subject: u64,
    pub contract: u64,
    pub evidence: u64,
    pub identity: u64,
}

pub(in crate::core::companions) fn certified_injection(
    cell: crate::core::companions::reporting::inputs::CertifiedCell,
) -> crate::workflow::encoding::Json {
    crate::workflow::encoding::object([
        ("type", crate::workflow::encoding::string("Certified")),
        (
            "subject",
            crate::workflow::encoding::Json::Number(cell.subject),
        ),
        (
            "contract",
            crate::workflow::encoding::Json::Number(cell.contract),
        ),
        (
            "evidence",
            crate::workflow::encoding::Json::Number(cell.evidence),
        ),
        (
            "identity",
            crate::workflow::encoding::string(std::format!("{}", cell.identity)),
        ),
    ])
}

pub(in crate::core::companions) fn injection_frame(
    values: &[crate::workflow::encoding::Json],
) -> std::string::String {
    crate::workflow::encoding::Json::Array(values.to_vec()).encode()
}
