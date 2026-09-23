impl super::ReleaseDecision {
    pub(in crate::build) fn blocked(reason: &str) -> Self {
        Self {
            allowed: false,
            reason: reason.into(),
            refusals: std::vec::Vec::new(),
            core_subject: None,
            core_statement: None,
            core_contract: None,
            core_evidence: None,
        }
    }

    pub(in crate::build) fn json(&self) -> crate::workflow::encoding::Json {
        crate::workflow::encoding::object([
            (
                "allowed",
                crate::workflow::encoding::Json::Bool(self.allowed),
            ),
            ("reason", crate::workflow::encoding::string(&self.reason)),
            (
                "refusals",
                crate::workflow::encoding::Json::Array(
                    self.refusals
                        .iter()
                        .map(crate::workflow::encoding::string)
                        .collect(),
                ),
            ),
            ("core_subject_identity", super::hex_u64(self.core_subject)),
            ("core_statement_digest", super::hex_u64(self.core_statement)),
            (
                "core_contract_index",
                super::number_or_null(self.core_contract.map(u64::from)),
            ),
            (
                "core_evidence_index",
                super::number_or_null(self.core_evidence.map(u64::from)),
            ),
            (
                "policy_revision",
                crate::workflow::encoding::Json::Number(u64::from(super::POLICY_REVISION)),
            ),
            (
                "ruleset",
                crate::workflow::encoding::Json::Number(u64::from(
                    noble_contracts::companion::RULESET_V1,
                )),
            ),
        ])
    }
}
