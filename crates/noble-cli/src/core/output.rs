#[derive(Clone, Copy)]
pub(super) struct ErrorContext {
    pub stage: &'static str,
    pub outcome: &'static str,
}

pub(super) struct Failure {
    pub context: ErrorContext,
    pub message: std::string::String,
}

impl Failure {
    pub fn new(context: ErrorContext, message: impl Into<std::string::String>) -> Self {
        Self {
            context,
            message: message.into(),
        }
    }

    pub fn source(error: noble_contracts::source::Error) -> Self {
        Self::new(
            ErrorContext {
                stage: source_stage(error.stage()),
                outcome: source_outcome(&error),
            },
            &error.diagnostic().message,
        )
    }

    pub fn backend(error: noble_wasm::Diagnostic) -> Self {
        let outcome = match error {
            noble_wasm::Diagnostic::Invalid => "reject",
            noble_wasm::Diagnostic::Unsupported => "unsupported",
            noble_wasm::Diagnostic::Exhausted => "exhausted",
            noble_wasm::Diagnostic::Defective => "internal-failure",
        };
        Self::new(
            ErrorContext {
                stage: "acceptance",
                outcome,
            },
            "independent source compilation refused",
        )
    }

    pub const fn exit(&self) -> u8 {
        exit(self.context.outcome)
    }
}

pub(super) struct Report {
    pub outcome: std::string::String,
    pub json: std::string::String,
}

impl Report {
    pub fn failure(failure: &Failure) -> Self {
        Self::static_report(failure.context, &failure.message, 0, None)
    }

    pub fn source_error(error: &noble_contracts::source::Error, submission: u64) -> Self {
        let diagnostic = error.diagnostic();
        Self::static_report(
            ErrorContext {
                stage: source_stage(error.stage()),
                outcome: source_outcome(error),
            },
            &diagnostic.message,
            submission,
            Some(diagnostic.span),
        )
    }

    pub fn backend_error(error: noble_wasm::Diagnostic, submission: u64) -> Self {
        let failure = Failure::backend(error);
        Self::static_report(failure.context, &failure.message, submission, None)
    }

    pub fn defined(submission: u64) -> Self {
        Self::static_report(
            ErrorContext {
                stage: "check",
                outcome: "defined",
            },
            "definition installed; body not executed",
            submission,
            None,
        )
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; static_report allocates owned outcome text and JSON fields and encodes them into a String. These runtime allocation and encoding APIs are not const on the pinned compiler."
    )]
    fn static_report(
        context: ErrorContext,
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
                    crate::workflow::encoding::string("Core-Bootstrap"),
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
                ("source_span", source_span(span)),
                ("guest_requests", crate::workflow::encoding::Json::Number(0)),
                (
                    "protected_operations",
                    crate::workflow::encoding::Json::Number(0),
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
                    crate::workflow::encoding::string(if context.outcome == "defined" {
                        "extended"
                    } else {
                        "unchanged"
                    }),
                ),
                (
                    "prior_session",
                    crate::workflow::encoding::string(if context.outcome == "defined" {
                        "definition-installed"
                    } else {
                        "unchanged"
                    }),
                ),
            ])
            .encode(),
        }
    }

    pub fn exit(&self) -> u8 {
        exit(&self.outcome)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.outcome.as_str(),
            "trap" | "runtime-exhausted" | "internal-failure"
        )
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; source_span constructs an owned JSON object for a reported span. The object builder allocates its fields at runtime and is not const on the pinned compiler."
)]
fn source_span(span: Option<noble_contracts::Span>) -> crate::workflow::encoding::Json {
    match span {
        Some(span) => crate::workflow::encoding::object([
            (
                "start",
                crate::workflow::encoding::Json::Number(u64::from(span.start)),
            ),
            (
                "end",
                crate::workflow::encoding::Json::Number(u64::from(span.end)),
            ),
        ]),
        None => crate::workflow::encoding::Json::Null,
    }
}

pub(super) fn print(report: &Report) -> Result<(), Failure> {
    let mut output = std::io::stdout().lock();
    attempt!(
        std::io::Write::write_all(&mut output, report.json.as_bytes())
            .map_err(super::framing::io_error)
    );
    attempt!(std::io::Write::write_all(&mut output, b"\n").map_err(super::framing::io_error));
    std::io::Write::flush(&mut output).map_err(super::framing::io_error)
}

const fn diagnostic_kind(kind: noble_contracts::DiagnosticKind) -> &'static str {
    match kind {
        noble_contracts::DiagnosticKind::Invalid => "reject",
        noble_contracts::DiagnosticKind::Unsupported => "unsupported",
        noble_contracts::DiagnosticKind::Exhausted => "exhausted",
        noble_contracts::DiagnosticKind::Internal => "internal-failure",
    }
}

const fn source_stage(stage: noble_contracts::source::Stage) -> &'static str {
    match stage {
        noble_contracts::source::Stage::Parse => "parse",
        noble_contracts::source::Stage::Resolve => "resolve",
        noble_contracts::source::Stage::Check => "check",
        noble_contracts::source::Stage::Acceptance => "acceptance",
    }
}

const fn source_outcome(error: &noble_contracts::source::Error) -> &'static str {
    if matches!(
        error.diagnostic().kind,
        noble_contracts::DiagnosticKind::Invalid
    ) {
        return match error.stage() {
            noble_contracts::source::Stage::Resolve => "unbound-word",
            noble_contracts::source::Stage::Check => "type-reject",
            noble_contracts::source::Stage::Parse | noble_contracts::source::Stage::Acceptance => {
                "reject"
            }
        };
    }
    diagnostic_kind(error.diagnostic().kind)
}

const fn exit(outcome: &str) -> u8 {
    match outcome.as_bytes() {
        b"normal" | b"defined" | b"ready" => 0,
        b"unsupported" => 4,
        b"exhausted" | b"runtime-exhausted" => 5,
        b"trap" => 1,
        _ => 2,
    }
}
