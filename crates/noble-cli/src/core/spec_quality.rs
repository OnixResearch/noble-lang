//! Specification-quality report producer for the bounded pure-`I64`/`Bool` profile (issue #65).
//!
//! The producer *reports* submitted evidence against the declared profile; it never executes a
//! guest, mutates a source, or strengthens a specification. Every binding the evidence relies on is
//! declared here once and compared with the submission, so a submission carrying a different
//! obligation kind, predicate revision, model revision, observation relation or parameter binding is
//! refused with its own diagnostic instead of being read as agreement.
//!
//! The negotiation of a specification-quality obligation is *derived*, not attested: the submission
//! states the retained relation once and `relation` recomputes inhabitation, realizability and
//! determinacy itself, so the submitted `checked`/`vacuous` flags are compared with what was derived
//! rather than trusted. A manifest whose flags disagree with its own retained relation is refused.
//!
//! Four obligations are reported and never merged: specification quality, implementation
//! satisfaction (the unchanged `exportedClaim`), termination, and correspondence.

use super::output::{ErrorContext, Failure, Report};
use super::report::Value;
use crate::workflow::encoding::{object, string, Json};

mod relation;

const USAGE: &str = "usage: noble spec-quality MANIFEST_JSON";
const MANIFEST_LIMIT: u32 = 4096;
const SCHEMA: &str = "noble-mc2-spec-quality/v1";
/// Predicate semantics this profile reports against: MC1 `Expression.Holds` as pinned by the
/// contract gate. A different revision is refused rather than assumed compatible.
const PREDICATE_REVISION: u32 = 1;
/// The obligation-shape model revision landed for #65 (`Spec`, `Inhabited`, `Realizable`,
/// `Deterministic`, `Witness`, `Recheck`).
const MODEL_REVISION: u32 = 1;
/// Exact scalar equality only; a coarser observational quotient has to be selected, versioned and
/// checked separately, so a producer cannot choose one to make determinacy pass.
const OBSERVATION_RELATION: &str = "exact-scalar-equality";
/// Ordered exact `I64`/`Bool` stacks, both stacks declared in order.
const PARAMETER_BINDING: &str = "ordered-exact-i64-bool";
const SPEC_CLAIMS: [&str; 3] = ["inhabited", "realizable", "deterministic"];
const IMPLEMENTATION_CLAIM: &str = "satisfied";

/// One obligation the submission asks to have reported.
struct Obligation {
    claim: &'static str,
    checked: bool,
    vacuous: bool,
}

/// A binding or shape problem at the report boundary. The outcome names the class.
fn refusal(outcome: &'static str, message: impl AsRef<str>) -> Report {
    Report {
        outcome: outcome.into(),
        json: object([
            ("schema", string(SCHEMA)),
            ("stage", string("report")),
            ("outcome", string(outcome)),
            ("diagnostic", string(message)),
            ("guest_requests", Json::Number(0)),
            ("protected_operations", Json::Number(0)),
        ])
        .encode(),
    }
}

fn usage() -> Failure {
    Failure::new(
        ErrorContext {
            stage: "report",
            outcome: "invalid-input",
        },
        USAGE,
    )
}

const REFUSAL_BINDING_FIELDS: [(&str, &str); 4] = [
    ("predicate_revision", "wrong-predicate-revision"),
    ("model_revision", "wrong-model-revision"),
    ("observation_relation", "wrong-observation-relation"),
    ("parameter_binding", "wrong-parameter-binding"),
];

/// Compare one declared binding with the submission. `None` means agreement.
fn binding_refusal(field: &str, submitted: &Value) -> Option<(&'static str, std::string::String)> {
    let class = REFUSAL_BINDING_FIELDS
        .iter()
        .copied()
        .find(|(name, _)| *name == field)
        .map(|(_, class)| class)?;
    let agrees = match field {
        "predicate_revision" => submitted.index() == Some(u64::from(PREDICATE_REVISION)),
        "model_revision" => submitted.index() == Some(u64::from(MODEL_REVISION)),
        "observation_relation" => submitted.text() == Some(OBSERVATION_RELATION),
        _ => submitted.text() == Some(PARAMETER_BINDING),
    };
    if agrees {
        return None;
    }
    Some((
        class,
        std::format!("submitted {field} does not match the declared profile"),
    ))
}

/// Pairing of claim and obligation kind. Specification-quality claims are never discharged by the
/// implementation claim and the implementation claim is never discharged by a specification one.
fn obligation_refusal(claim: &str, obligation: &str) -> Option<&'static str> {
    if SPEC_CLAIMS.contains(&claim) {
        if obligation == "specification-quality" {
            return None;
        }
        return Some("wrong-obligation");
    }
    if claim == IMPLEMENTATION_CLAIM {
        if obligation == "implementation-satisfaction" {
            return None;
        }
        return Some("wrong-obligation");
    }
    Some("wrong-obligation")
}

fn obligations(
    value: &Value,
) -> Result<std::vec::Vec<Obligation>, (&'static str, std::string::String)> {
    let items = value.items().ok_or((
        "invalid-manifest",
        std::string::String::from("obligations must be an array"),
    ))?;
    let mut collected = std::vec::Vec::new();
    for item in items {
        let Value::Object(fields) = item else {
            return Err((
                "invalid-manifest",
                std::string::String::from("obligation must be an object"),
            ));
        };
        if fields
            .keys()
            .any(|key| !matches!(key.as_str(), "claim" | "obligation" | "checked" | "vacuous"))
        {
            return Err((
                "invalid-manifest",
                std::string::String::from("unknown obligation field"),
            ));
        }
        let claim = fields.get("claim").and_then(Value::text).ok_or((
            "invalid-manifest",
            std::string::String::from("obligation claim must be text"),
        ))?;
        let obligation = fields.get("obligation").and_then(Value::text).ok_or((
            "invalid-manifest",
            std::string::String::from("obligation kind must be text"),
        ))?;
        let claim_name = SPEC_CLAIMS
            .iter()
            .copied()
            .find(|name| *name == claim)
            .or_else(|| Some(IMPLEMENTATION_CLAIM).filter(|name| *name == claim))
            .ok_or((
                "invalid-manifest",
                std::string::String::from("unknown claim"),
            ))?;
        if let Some(class) = obligation_refusal(claim_name, obligation) {
            let message = std::format!("claim {claim_name} cannot be discharged as {obligation}");
            return Err((class, message));
        }
        let checked = match fields.get("checked") {
            Some(Value::Bool(value)) => *value,
            _ => {
                return Err((
                    "invalid-manifest",
                    std::string::String::from("obligation checked flag must be a boolean"),
                ));
            }
        };
        let vacuous = match fields.get("vacuous") {
            Some(Value::Bool(value)) => *value,
            None => false,
            _ => {
                return Err((
                    "invalid-manifest",
                    std::string::String::from("obligation vacuous flag must be a boolean"),
                ));
            }
        };
        collected.push(Obligation {
            claim: claim_name,
            checked,
            vacuous,
        });
    }
    Ok(collected)
}

/// The reported obligations, separated by what each one actually establishes.
fn spec_block(list: &[Obligation], derived: &relation::Derived) -> Json {
    let mut established = 0_u64;
    let mut refuted_or_unchecked = 0_u64;
    let mut vacuous = 0_u64;
    let mut rows = std::vec::Vec::new();
    for entry in list {
        // Implementation satisfaction belongs only to the separate partial-correctness block.
        let derived_value = match entry.claim {
            "inhabited" => derived.inhabited,
            "realizable" => derived.realizable,
            "deterministic" => derived.deterministic,
            _ => continue,
        };
        if !entry.checked {
            refuted_or_unchecked += 1;
        } else if entry.vacuous {
            vacuous += 1;
        } else {
            established += 1;
        }
        rows.push(object([
            ("claim", string(entry.claim)),
            ("kind", string("specification-quality")),
            ("checked", Json::Bool(entry.checked)),
            ("vacuous", Json::Bool(entry.vacuous)),
            ("derived", Json::Bool(derived_value)),
        ]));
    }
    object([
        ("schema", string("noble-mc2-spec-quality-derivation/v1")),
        ("derivation", derived.json()),
        ("obligations", Json::Array(rows)),
        ("established", Json::Number(established)),
        ("not_established", Json::Number(refuted_or_unchecked)),
        ("vacuous", Json::Number(vacuous)),
        (
            "scope",
            string(
                "Relation claims derived from the retained relation over the bounded pure-I64/Bool \
                 profile. A missing value, an undefined predicate, an unsupported construct or an \
                 exhausted budget is never a successful check.",
            ),
        ),
    ])
}

fn separated(established: &[Obligation]) -> [(&'static str, Json); 3] {
    let implementation = established
        .iter()
        .any(|entry| entry.claim == IMPLEMENTATION_CLAIM && entry.checked);
    [
        (
            "partial_correctness",
            object([
                ("claim", string("exportedClaim")),
                (
                    "substituted_by_specification_quality",
                    Json::Bool(false),
                ),
                (
                    "scope",
                    string(
                        "Normal-return partial correctness only, and only when the implementation claim \
                         is itself reported as checked. Never termination, traps, quotas or intent.",
                    ),
                ),
                ("reported", Json::Bool(implementation)),
            ]),
        ),
        (
            "termination",
            object([
                ("claimed", Json::Bool(false)),
                (
                    "scope",
                    string(
                        "No termination is claimed: relational realizability says that some typed output \
                         satisfies the postcondition, and output determinacy says nothing about intent.",
                    ),
                ),
            ]),
        ),
        (
            "correspondence",
            object([
                ("claimed", Json::Bool(false)),
                (
                    "scope",
                    string(
                        "No correspondence route relates these obligations to lowering, runtime steps or \
                         the retained contract renderer, so none is claimed here.",
                    ),
                ),
            ]),
        ),
    ]
}

fn decide(arguments: &[std::ffi::OsString]) -> Result<Report, Failure> {
    if arguments.len() != 2 {
        return Err(usage());
    }
    let path = std::path::PathBuf::from(&arguments[1]);
    let bytes = attempt!(super::framing::read_file(&path, MANIFEST_LIMIT));
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Ok(refusal("invalid-manifest", "manifest is not UTF-8"));
    };
    let Ok(Value::Object(fields)) = super::report::parse(text) else {
        return Ok(refusal("invalid-manifest", "manifest must be an object"));
    };
    for key in fields.keys() {
        if !matches!(
            key.as_str(),
            "schema"
                | "predicate_revision"
                | "model_revision"
                | "observation_relation"
                | "parameter_binding"
                | "relation"
                | "obligations"
        ) {
            return Ok(refusal("invalid-manifest", "unknown manifest field"));
        }
    }
    if fields.get("schema").and_then(Value::text) != Some(SCHEMA) {
        return Ok(refusal("invalid-manifest", "manifest schema mismatch"));
    }
    for (field, class) in REFUSAL_BINDING_FIELDS {
        let Some(submitted) = fields.get(field) else {
            return Ok(refusal(
                "invalid-manifest",
                "manifest binding field is missing",
            ));
        };
        if let Some((_, message)) = binding_refusal(field, submitted) {
            return Ok(refusal(class, message));
        }
    }
    let Some(listing) = fields.get("obligations") else {
        return Ok(refusal(
            "invalid-manifest",
            "manifest obligations are missing",
        ));
    };
    let list = match obligations(listing) {
        Ok(list) => list,
        Err((class, message)) => return Ok(refusal(class, message)),
    };
    // The spec claims are derived from the retained relation, and the submitted flags are only
    // compared with the derivation. An implementation claim needs no relation.
    let retained = fields.get("relation");
    let derived = match retained {
        Some(Value::Null) | None => relation::Derived::unavailable(),
        Some(value) => match relation::derive(value) {
            Ok(derived) => derived,
            Err((class, message)) => return Ok(refusal(class, message)),
        },
    };
    for entry in &list {
        let attested = match entry.claim {
            "inhabited" => derived.inhabited,
            "realizable" => derived.realizable,
            "deterministic" => derived.deterministic,
            _ => continue,
        };
        if entry.checked != attested {
            let message = std::format!(
                "claim {} is submitted as {} but its retained relation derives {}",
                entry.claim,
                if entry.checked {
                    "checked"
                } else {
                    "unchecked"
                },
                attested
            );
            return Ok(refusal("self-attested-obligation", message));
        }
        if entry.vacuous != derived.vacuous {
            let message = std::format!(
                "claim {} declares vacuous={} but its retained relation derives vacuous={}",
                entry.claim,
                entry.vacuous,
                derived.vacuous
            );
            return Ok(refusal("self-attested-obligation", message));
        }
    }
    let [partial_correctness, termination, correspondence] = separated(&list);
    let json = object([
        ("schema", string(SCHEMA)),
        ("stage", string("report")),
        ("outcome", string("reported")),
        ("guest_requests", Json::Number(0)),
        (
            "predicate_revision",
            Json::Number(u64::from(PREDICATE_REVISION)),
        ),
        ("model_revision", Json::Number(u64::from(MODEL_REVISION))),
        ("observation_relation", string(OBSERVATION_RELATION)),
        ("parameter_binding", string(PARAMETER_BINDING)),
        ("specification_quality", spec_block(&list, &derived)),
        partial_correctness,
        termination,
        correspondence,
    ]);
    Ok(Report {
        outcome: std::string::String::from("reported"),
        json: json.encode(),
    })
}

pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let report = match decide(arguments) {
        Ok(report) => report,
        Err(error) => refusal(error.context.outcome, error.message),
    };
    if let Err(error) = super::output::print(&report) {
        eprintln!("noble: {}", error.message);
        return std::process::ExitCode::from(error.exit());
    }
    std::process::ExitCode::from(report.exit())
}
