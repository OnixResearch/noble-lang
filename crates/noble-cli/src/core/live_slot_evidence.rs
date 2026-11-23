//! Selected-target evidence for a proof-required live slot (LSLOT-05, P-ID-06).
//!
//! A publication into a proof-required slot names one checked returned target
//! and carries one evidence record. The record must declare exactly the
//! target's ProgramValueId and DefinitionId, its captures, the host's semantic
//! context, the digest of the claim the slot's authority selected, the
//! consumer's assumptions and the target's composite code-donor manifest. The
//! proof is then rechecked by the consumer route of `noble build
//! --require-proof` against the selected claim, and the core binds the accepted
//! claim to the target's own closed subject: its verified template with every
//! quoted root input replaced by the captured value. Evidence about any other
//! subject, capture, context, claim, assumption set or artifact is refused
//! before publication, and an accepted proof never changes the target's
//! identity.

use serde_json::{json, Value};

/// The record fields an evidence object carries, besides its proof.
const FIELDS: [&str; 7] = [
    "subject",
    "captures",
    "context",
    "claim_sha256",
    "assumptions",
    "artifact_sha256",
    "proof",
];

/// The selected target the evidence must be about.
pub(super) struct Target<'a> {
    pub(super) identity: &'a Value,
    pub(super) subject: &'a str,
}

/// A refused evidence record: a stable code, a diagnostic and the record.
pub(super) struct Refusal {
    pub(super) code: String,
    pub(super) diagnostic: String,
    pub(super) record: Value,
}

/// The host's semantic context for selected-target claims.
pub(super) fn context() -> Value {
    json!({
        "semantic_revision": noble_kernel::untrusted::SEMANTIC_REVISION,
        "policy_revision": crate::build::POLICY_REVISION,
    })
}

/// Check one evidence record for the selected target against the slot's
/// claim. Every declared field is compared before the proof is rechecked.
pub(super) fn admit(
    claim: &str,
    target: &Target<'_>,
    evidence: &Value,
    timeout_ms: u64,
) -> Result<Value, Refusal> {
    let mut record = json!({
        "policy": "proof-required-for-this-slot",
        "claim_sha256": crate::workflow::intrinsic::sha256(claim.as_bytes()),
        "program_value_id": target.identity["program_value_id"],
        "definition_id": target.identity["definition_id"],
        "artifact_sha256": target.identity["artifact_sha256"],
        "subject": target.subject,
        "context": context(),
    });
    let Some(fields) = evidence.as_object() else {
        return Err(refused(
            record,
            "malformed-evidence",
            "evidence must be one JSON object",
        ));
    };
    if fields.len() != FIELDS.len() || FIELDS.iter().any(|field| !fields.contains_key(*field)) {
        return Err(refused(
            record,
            "malformed-evidence",
            "evidence carries exactly subject, captures, context, claim_sha256, assumptions, artifact_sha256 and proof",
        ));
    }
    let declared = json!({
        "program_value_id": target.identity["program_value_id"],
        "definition_id": target.identity["definition_id"],
    });
    let checks = [
        (
            evidence["subject"] == declared,
            "subject-mismatch",
            "evidence names another ProgramValueId or DefinitionId",
        ),
        (
            evidence["captures"] == target.identity["captures"],
            "capture-mismatch",
            "evidence names other captures",
        ),
        (
            evidence["context"] == context(),
            "context-mismatch",
            "evidence names another semantic context",
        ),
        (
            evidence["claim_sha256"] == record["claim_sha256"],
            "claim-mismatch",
            "evidence is about another claim than the slot selects",
        ),
        (
            evidence["assumptions"] == json!(crate::workflow::obligations::ALLOWED_AXIOMS),
            "assumption-mismatch",
            "evidence declares other assumptions than the consumer allows",
        ),
        (
            evidence["artifact_sha256"] == target.identity["artifact_sha256"],
            "artifact-mismatch",
            "evidence names another installed code-donor manifest",
        ),
    ];
    if let Some((_, code, diagnostic)) = checks.iter().find(|(matched, _, _)| !matched) {
        return Err(refused(record, code, diagnostic));
    }
    let Some(proof) = evidence["proof"]
        .as_str()
        .filter(|proof| proof.len() <= crate::core::EVIDENCE_LIMITS.bytes as usize)
    else {
        return Err(refused(
            record,
            "malformed-evidence",
            "evidence proof must be bounded text",
        ));
    };
    record["proof_sha256"] = json!(crate::workflow::intrinsic::sha256(proof.as_bytes()));
    let frontend = noble_contracts::source::Session::new();
    let prepared =
        match frontend.prepare(target.subject.as_bytes(), &[], crate::core::SOURCE_LIMITS) {
            Ok(prepared) => prepared,
            Err(error) => {
                let diagnostic = std::format!(
                    "the target subject is not a closed checked expression: {}",
                    error.diagnostic().message
                );
                return Err(refused(record, "subject-unchecked", &diagnostic));
            }
        };
    let Some(submission) = prepared.submission() else {
        return Err(refused(
            record,
            "subject-unchecked",
            "the target subject is not a closed checked expression",
        ));
    };
    let mut core = noble_contracts::companion::Core::new(crate::core::EVIDENCE_LIMITS);
    core.set_policy(crate::build::POLICY_REVISION);
    let completion = match crate::workflow::admission::admit(
        &mut core,
        claim.as_bytes(),
        crate::core::SOURCE_LIMITS,
        Some((proof.as_bytes(), false)),
        timeout_ms,
    ) {
        Ok(completion) => completion,
        Err(refusal) => {
            record["claim_outcome"] = json!(refusal.outcome.name());
            return Err(refused(record, refusal.code, &refusal.message));
        }
    };
    let admission = completion.decision;
    record["claim_outcome"] = json!(admission.outcome.code());
    let evidence_id = match (admission.outcome, admission.evidence) {
        (noble_contracts::companion::Outcome::Proved, Some(evidence_id)) => evidence_id,
        _ => {
            let diagnostic = std::format!(
                "the consumer reported `{}`, not an accepted proved claim",
                admission.outcome.code()
            );
            return Err(refused(record, "not-proved", &diagnostic));
        }
    };
    let release = core
        .bind_artifact(evidence_id, submission)
        .and_then(|subject| core.release(evidence_id, &subject));
    let release = match release {
        Ok(release) => release,
        Err(refusal) => {
            return Err(refused(
                record,
                refusal.code(),
                "the accepted claim does not bind the selected target's subject",
            ));
        }
    };
    record["decision"] = json!("admitted");
    record["core_subject"] = json!(release.subject.0);
    record["core_statement"] = json!(release.statement);
    record["core_contract"] = json!(release.contract.0);
    record["core_evidence"] = json!(release.evidence.0);
    Ok(record)
}

fn refused(mut record: Value, code: &str, diagnostic: &str) -> Refusal {
    record["decision"] = json!("refused");
    record["refusal"] = json!(code);
    Refusal {
        code: code.to_owned(),
        diagnostic: diagnostic.to_owned(),
        record,
    }
}
