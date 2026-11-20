//! Diagnostic categories and the selectable intrinsic module report revision.
//!
//! The protocol's claim outcomes are closed (`output::Outcome`). A
//! category and a remediation sit beside an outcome and never replace one.
//! Revision `v1` keeps the published field set and the published meaning of
//! every field byte for byte; revision `v2`, the current default, replaces the
//! non-standard `unproved` string with the VC-TOOL-02 outcome `unknown` and
//! appends `category` and `remediation` next to the outcome.

use super::{encoding, output};

/// The intrinsic module report revision a caller selected.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Schema {
    /// The historical report, including the `unproved` outcome and exit 0.
    V1,
    /// The current report: VC-TOOL-02 outcomes plus category/remediation.
    V2,
}

/// The revision `verify-module` emits when no `--report-schema` is given.
pub(crate) const DEFAULT_SCHEMA: Schema = Schema::V2;

/// Options that consume the following argument; the revision scan must step
/// over their values so an option-looking value is never read as a selector.
const VALUE_OPTIONS: [&str; 5] = [
    "--module",
    "--timeout-ms",
    "--normalization-work",
    "--substitution-work",
    "--report-schema",
];

impl Schema {
    /// The revision selected by a `--report-schema` value.
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "v1" => Some(Self::V1),
            "v2" => Some(Self::V2),
            _ => None,
        }
    }

    /// The report schema string a `verify-module` report carries. A named v2
    /// acceptance has always reported `v2`; v1 never reinterprets its fields.
    pub(crate) const fn module_schema(self, named: bool) -> &'static str {
        match (self, named) {
            (Self::V2, _) | (Self::V1, true) => "noble-intrinsic-module-report/v2",
            (Self::V1, false) => "noble-intrinsic-module-report/v1",
        }
    }
}

fn usage(message: &str) -> output::Failure {
    output::Failure::error("usage", std::string::String::from(message))
}

/// The revision a `verify-module` invocation selected, or the usage refusal for
/// a repeated or unknown selector. Option values are stepped over.
pub(crate) fn from_arguments(arguments: &[std::ffi::OsString]) -> Result<Schema, output::Failure> {
    let mut selected = DEFAULT_SCHEMA;
    let mut seen = false;
    let mut at = 2usize;
    while at < arguments.len() {
        let option = arguments[at]
            .to_str()
            .ok_or_else(|| usage("argument must be UTF-8"))?;
        at += 1;
        if !VALUE_OPTIONS.contains(&option) {
            continue;
        }
        let value = arguments
            .get(at)
            .ok_or_else(|| usage("missing option value"))?;
        if option == "--report-schema" {
            if seen {
                return Err(usage("repeated report schema"));
            }
            seen = true;
            selected = value
                .to_str()
                .and_then(Schema::parse)
                .ok_or_else(|| usage("invalid report schema"))?;
        }
        at += 1;
    }
    Ok(selected)
}

/// One classified diagnostic: the design's category plus its remediation.
#[derive(Clone, Copy)]
pub(crate) struct Category {
    pub(crate) category: &'static str,
    pub(crate) remediation: &'static str,
}

/// No proof declaration supplied any evidence; nothing was checked.
pub(crate) const NO_EVIDENCE: Category = Category {
    category: "no-evidence",
    remediation: "supply-a-proof-declaration",
};

/// An accepted claim for this exact statement digest.
pub(crate) const CHECKED: Category = Category {
    category: "checked",
    remediation: "none",
};

const fn category(category: &'static str, remediation: &'static str) -> Category {
    Category {
        category,
        remediation,
    }
}

/// Classify one intrinsic workflow diagnostic. The examples the design names
/// are fixed here: `intrinsic-stale-proof` is `stale-dependency` and every
/// `*-work-exhausted` code is `checker-exhausted`. Unknown codes keep their
/// outcome's own reading rather than borrowing a category.
pub(crate) fn classify(outcome: output::Outcome, code: &str) -> Category {
    match code {
        "intrinsic-stale-proof" => {
            category("stale-dependency", "recheck-against-current-revisions")
        }
        "normalization-work-exhausted"
        | "substitution-work-exhausted"
        | "intrinsic-source-exhausted"
        | "named-proof-limit" => category("checker-exhausted", "raise-the-work-budget"),
        "intrinsic-lowering-mismatch" | "intrinsic-kind-mismatch" => {
            category("claim-mismatch", "regenerate-the-claim")
        }
        "intrinsic-source-binding" => category("forged-identity", "resubmit-the-pinned-subject"),
        "intrinsic-parse-invalid"
        | "intrinsic-resolve-invalid"
        | "intrinsic-link-invalid"
        | "intrinsic-contract-invalid" => {
            category("rejected-source", "fix-the-reported-source-span")
        }
        "intrinsic-proof-refused"
        | "intrinsic-source-invalid"
        | "intrinsic-consumer-protocol"
        | "consumer-protocol"
        | "proof-wire-encoding"
        | "intrinsic-checker-process-failure" => {
            category("rejected-term", "fix-the-reported-proof-constructor")
        }
        "intrinsic-unpinned-import"
        | "intrinsic-model-mismatch"
        | "named-model-mismatch"
        | "named-unpinned-import"
        | "intrinsic-source-unsupported"
        | "library-import-cycle"
        | "toolchain-mismatch" => category("outside-profile", "select-a-supported-profile"),
        "artifact-io"
        | "temporary-root"
        | "library-read"
        | "proof-artifact"
        | "sandbox-unavailable" => category("unavailable", "fix-the-host-environment"),
        "usage" | "module-limit" => category("usage", "correct-the-arguments"),
        _ => match outcome {
            output::Outcome::Timeout => category("checker-timeout", "raise-the-deadline"),
            _ => category("internal", "report-the-tool-defect"),
        },
    }
}

/// The `category` and `remediation` fields of the current revision. Revision
/// `v1` reports keep their exact field set, so it contributes none.
pub(crate) fn classification_fields(
    schema: Schema,
    classified: Category,
) -> std::vec::Vec<(&'static str, encoding::Json)> {
    if schema != Schema::V2 {
        return std::vec::Vec::new();
    }
    std::vec::Vec::from([
        ("category", encoding::string(classified.category)),
        ("remediation", encoding::string(classified.remediation)),
    ])
}

/// The leading fields `verify-module` reports have always published.
fn head(
    schema: Schema,
    named: bool,
    profile: &str,
) -> std::vec::Vec<(&'static str, encoding::Json)> {
    std::vec::Vec::from([
        ("schema", encoding::string(schema.module_schema(named))),
        ("profile", encoding::string(profile)),
        ("command", encoding::string("verify-module")),
    ])
}

fn source_fields(
    source_path: &str,
    submitted: Option<&str>,
) -> [(&'static str, encoding::Json); 2] {
    [
        ("source_path", encoding::string(source_path)),
        ("source_sha256", encoding::optional_string(submitted)),
    ]
}

fn counters() -> [(&'static str, encoding::Json); 6] {
    [
        ("guest_requests", encoding::Json::Number(0)),
        ("host_requests", encoding::Json::Number(0)),
        ("protected_operations", encoding::Json::Number(0)),
        ("candidate_executions", encoding::Json::Number(0)),
        ("runtime_prover_calls", encoding::Json::Number(0)),
        ("proof_grants_host_authority", encoding::Json::Bool(false)),
    ]
}

/// The report of a module that carries no proof obligation: revision `v1`
/// keeps `unproved` and exit 0 exactly as published; revision `v2` reports the
/// VC-TOOL-02 outcome `unknown` with `category: no-evidence`.
pub(crate) fn no_evidence_report(
    schema: Schema,
    profile: &str,
    source_path: &str,
    submitted: Option<&str>,
) -> encoding::Json {
    let mut fields = head(schema, false, profile);
    fields.push((
        "outcome",
        encoding::string(match schema {
            Schema::V1 => "unproved",
            Schema::V2 => "unknown",
        }),
    ));
    fields.push(("stage", encoding::string("source-contract")));
    fields.extend(source_fields(source_path, submitted));
    fields.extend(counters());
    fields.push(("independent_recheck", encoding::Json::Bool(false)));
    fields.extend(classification_fields(schema, NO_EVIDENCE));
    encoding::Json::Object(fields)
}

fn stage_of(code: &str) -> &'static str {
    match code {
        "intrinsic-parse-invalid" => "parse",
        "intrinsic-resolve-invalid" => "resolve",
        "intrinsic-link-invalid" => "link",
        "intrinsic-contract-invalid" => "check",
        "intrinsic-source-exhausted" => "source-exhausted",
        "intrinsic-proof-refused" => "acceptance",
        "intrinsic-source-invalid"
        | "normalization-work-exhausted"
        | "substitution-work-exhausted" => "proof-check",
        _ => "independent-recheck",
    }
}

/// The report of a refused `verify-module` invocation.
pub(crate) fn failure_report(
    schema: Schema,
    profile: &str,
    source_path: &str,
    submitted: Option<&str>,
    error: &output::Failure,
) -> encoding::Json {
    let mut fields = head(schema, false, profile);
    fields.push(("outcome", encoding::string(error.outcome.name())));
    fields.push(("stage", encoding::string(stage_of(error.code))));
    fields.extend(source_fields(source_path, submitted));
    fields.extend(counters());
    fields.push(("independent_recheck", encoding::Json::Bool(false)));
    fields.push(("diagnostic", error.json()));
    fields.extend(classification_fields(
        schema,
        classify(error.outcome, error.code),
    ));
    encoding::Json::Object(fields)
}

#[cfg(test)]
mod tests {
    #[test]
    fn schema_parses_only_published_revisions() {
        assert!(matches!(
            super::Schema::parse("v1"),
            Some(super::Schema::V1)
        ));
        assert!(matches!(
            super::Schema::parse("v2"),
            Some(super::Schema::V2)
        ));
        assert!(super::Schema::parse("v3").is_none());
        assert!(super::Schema::parse("").is_none());
    }

    #[test]
    fn named_acceptance_keeps_its_published_v2_schema() {
        assert_eq!(
            super::Schema::V1.module_schema(true),
            "noble-intrinsic-module-report/v2"
        );
        assert_eq!(
            super::Schema::V1.module_schema(false),
            "noble-intrinsic-module-report/v1"
        );
        assert_eq!(
            super::Schema::V2.module_schema(false),
            "noble-intrinsic-module-report/v2"
        );
    }

    #[test]
    fn design_examples_classify_as_written() {
        let stale = super::classify(
            crate::workflow::output::Outcome::Unsupported,
            "intrinsic-stale-proof",
        );
        assert_eq!(stale.category, "stale-dependency");
        let normalization = super::classify(
            crate::workflow::output::Outcome::Error,
            "normalization-work-exhausted",
        );
        assert_eq!(normalization.category, "checker-exhausted");
        let substitution = super::classify(
            crate::workflow::output::Outcome::Error,
            "substitution-work-exhausted",
        );
        assert_eq!(substitution.category, "checker-exhausted");
        let deadline = super::classify(crate::workflow::output::Outcome::Timeout, "timeout");
        assert_eq!(deadline.category, "checker-timeout");
        let unknown = super::classify(
            crate::workflow::output::Outcome::Unknown,
            "evidence-missing",
        );
        assert_eq!(unknown.category, "internal");
    }

    #[test]
    fn every_classification_names_a_remediation() {
        let codes = [
            "intrinsic-stale-proof",
            "normalization-work-exhausted",
            "substitution-work-exhausted",
            "intrinsic-source-exhausted",
            "intrinsic-lowering-mismatch",
            "intrinsic-kind-mismatch",
            "intrinsic-source-binding",
            "intrinsic-parse-invalid",
            "intrinsic-resolve-invalid",
            "intrinsic-link-invalid",
            "intrinsic-contract-invalid",
            "intrinsic-proof-refused",
            "intrinsic-source-invalid",
            "intrinsic-unpinned-import",
            "intrinsic-model-mismatch",
            "named-model-mismatch",
            "named-unpinned-import",
            "artifact-io",
            "temporary-root",
            "library-read",
            "proof-artifact",
            "sandbox-unavailable",
            "usage",
            "module-limit",
            "unclassified-code",
        ];
        for code in codes {
            let classified = super::classify(crate::workflow::output::Outcome::Error, code);
            assert!(!classified.category.is_empty(), "{code}");
            assert!(!classified.remediation.is_empty(), "{code}");
        }
        assert_eq!(super::NO_EVIDENCE.category, "no-evidence");
        assert_eq!(super::CHECKED.category, "checked");
    }
}
