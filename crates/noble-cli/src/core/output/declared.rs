impl super::Report {
    /// A module/import is a checked namespace transition, never guest work.
    pub fn declared_link(
        submission: u64,
        name: &str,
        module: Option<(u64, std::string::String, u32)>,
        binding: Option<(std::string::String, u32)>,
        contracts: crate::workflow::encoding::Json,
        intrinsic_proof: crate::workflow::encoding::Json,
    ) -> Self {
        let module = match module {
            Some((identity, name, version)) => crate::workflow::encoding::object([
                (
                    "identity",
                    crate::workflow::encoding::string(identity.to_string()),
                ),
                ("name", crate::workflow::encoding::string(name)),
                (
                    "version",
                    crate::workflow::encoding::Json::Number(u64::from(version)),
                ),
            ]),
            None => crate::workflow::encoding::Json::Null,
        };
        let binding = match binding {
            Some((identity, slot)) => crate::workflow::encoding::object([
                (
                    "adapter_identity",
                    crate::workflow::encoding::string(identity),
                ),
                (
                    "slot",
                    crate::workflow::encoding::Json::Number(u64::from(slot)),
                ),
            ]),
            None => crate::workflow::encoding::Json::Null,
        };
        let json = crate::workflow::encoding::object([
            (
                "schema",
                crate::workflow::encoding::string("noble-core-report/v1"),
            ),
            (
                "profile",
                crate::workflow::encoding::string("Declared-Modules-v1"),
            ),
            (
                "backend",
                crate::workflow::encoding::string("managed-linear-memory"),
            ),
            (
                "submission",
                crate::workflow::encoding::Json::Number(submission),
            ),
            ("stage", crate::workflow::encoding::string("link")),
            ("outcome", crate::workflow::encoding::string("linked")),
            ("diagnostic", crate::workflow::encoding::string(name)),
            ("resolved_module", module),
            ("binding", binding),
            ("contracts", contracts),
            ("intrinsic_proof", intrinsic_proof),
            ("guest_requests", crate::workflow::encoding::Json::Number(0)),
            ("host_requests", crate::workflow::encoding::Json::Number(0)),
            (
                "protected_operations",
                crate::workflow::encoding::Json::Number(0),
            ),
            (
                "ambient_fallback_calls",
                crate::workflow::encoding::Json::Number(0),
            ),
            (
                "acquired_authority",
                crate::workflow::encoding::Json::Bool(false),
            ),
            (
                "candidate_prepare_requests",
                crate::workflow::encoding::Json::Number(0),
            ),
            (
                "prior_stack",
                crate::workflow::encoding::string("unchanged"),
            ),
            (
                "prior_namespace",
                crate::workflow::encoding::string("extended"),
            ),
        ])
        .encode();
        Self {
            outcome: "linked".into(),
            json,
        }
    }

    pub fn declared_definition(submission: u64) -> Self {
        Self::declared_static(
            super::ErrorContext {
                stage: "check",
                outcome: "defined",
            },
            "definition installed; body not executed",
            submission,
            None,
        )
    }

    pub fn declared_source_error(error: &noble_contracts::source::Error, submission: u64) -> Self {
        Self::declared_static(
            super::ErrorContext {
                stage: super::source_stage(error.stage()),
                outcome: super::source_outcome(error),
            },
            &error.diagnostic().message,
            submission,
            Some(error.diagnostic().span),
        )
    }

    pub fn declared_backend_error(error: noble_wasm::Diagnostic, submission: u64) -> Self {
        let failure = super::Failure::backend(error);
        Self::declared_static(failure.context, &failure.message, submission, None)
    }

    pub(super) fn declared_static(
        context: super::ErrorContext,
        message: &str,
        submission: u64,
        span: Option<noble_contracts::Span>,
    ) -> Self {
        Self {
            outcome: context.outcome.into(),
            json: crate::workflow::encoding::object([
                (
                    "schema",
                    crate::workflow::encoding::string("noble-core-report/v1"),
                ),
                (
                    "profile",
                    crate::workflow::encoding::string("Declared-Modules-v1"),
                ),
                (
                    "backend",
                    crate::workflow::encoding::string("managed-linear-memory"),
                ),
                (
                    "submission",
                    crate::workflow::encoding::Json::Number(submission),
                ),
                ("stage", crate::workflow::encoding::string(context.stage)),
                (
                    "outcome",
                    crate::workflow::encoding::string(context.outcome),
                ),
                ("diagnostic", crate::workflow::encoding::string(message)),
                ("source_span", super::source_span(span)),
                ("guest_requests", crate::workflow::encoding::Json::Number(0)),
                ("host_requests", crate::workflow::encoding::Json::Number(0)),
                (
                    "protected_operations",
                    crate::workflow::encoding::Json::Number(0),
                ),
                (
                    "ambient_fallback_calls",
                    crate::workflow::encoding::Json::Number(0),
                ),
                (
                    "acquired_authority",
                    crate::workflow::encoding::Json::Bool(false),
                ),
                (
                    "candidate_prepare_requests",
                    crate::workflow::encoding::Json::Number(0),
                ),
                (
                    "prior_stack",
                    crate::workflow::encoding::string("unchanged"),
                ),
                (
                    "prior_namespace",
                    crate::workflow::encoding::string(
                        if context.outcome == "linked" || context.outcome == "defined" {
                            "extended"
                        } else {
                            "unchanged"
                        },
                    ),
                ),
            ])
            .encode(),
        }
    }
}
