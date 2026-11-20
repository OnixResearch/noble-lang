//! Frozen owner-law record decoding for the independent release consumer.
//!
//! The record is verified by SHA-256 against an explicitly supplied expected
//! digest before any field is interpreted. The candidate under review does not
//! supply that digest, and a changed law byte is a different law identity. The
//! closed record schema means a new field is a new consumer version, never a
//! silently ignored addition.

use serde::Deserialize;

/// The largest owner-law record this consumer reads.
pub(crate) const LAW_LIMIT: usize = 65_536;

/// The one law-record schema this consumer understands.
const SCHEMA: &str = "noble-owner-law/v1";

/// The one conformance case and requirement the frozen records name.
const CASE: &str = "CONTRACT-16";
const REQUIREMENT: &str = "VC-OWNER-01";

/// The proof-required release policy and fragment this consumer implements.
pub(crate) const FRAGMENT: &str = "MC2-v1-resource-free-Core-Bootstrap";
pub(crate) const POLICY: &str = "proof-required-release";

/// One decoded frozen owner law.
#[derive(Debug)]
pub(crate) struct Law {
    pub(crate) revision: std::string::String,
    pub(crate) supersedes: Option<std::string::String>,
    pub(crate) variant: Option<std::string::String>,
    pub(crate) frozen: std::string::String,
    pub(crate) fragment: std::string::String,
    pub(crate) subject_interface: std::string::String,
    pub(crate) declaration: std::string::String,
    pub(crate) expected_statement: std::string::String,
    pub(crate) obligation: Obligation,
    pub(crate) observations: Observations,
    pub(crate) scope: std::string::String,
    pub(crate) preconditions: std::vec::Vec<std::string::String>,
    pub(crate) assumptions: std::vec::Vec<std::string::String>,
    pub(crate) proof_library: ProofLibrary,
    pub(crate) consumer_policy: std::string::String,
    pub(crate) non_claims: std::vec::Vec<std::string::String>,
    pub(crate) candidate_unknown_at_authoring: std::vec::Vec<std::string::String>,
    /// The digest of the exact bytes this process read.
    pub(crate) computed_sha256: std::string::String,
    /// The digest the release owner supplied independently of the candidate.
    pub(crate) expected_sha256: std::string::String,
}

/// The law's formal MC1 contract obligation.
#[derive(Debug)]
pub(crate) struct Obligation {
    pub(crate) language: std::string::String,
    pub(crate) input: std::vec::Vec<(std::string::String, std::string::String)>,
    pub(crate) output: std::vec::Vec<(std::string::String, std::string::String)>,
    pub(crate) requires: std::string::String,
    pub(crate) ensures: std::string::String,
    pub(crate) arithmetic: std::string::String,
}

/// The law's recorded initial and final observations.
#[derive(Debug)]
pub(crate) struct Observations {
    pub(crate) initial: std::string::String,
    pub(crate) final_observation: std::string::String,
}

/// The law's accepted proof-library requirement.
#[derive(Debug)]
pub(crate) struct ProofLibrary {
    pub(crate) requirement: std::string::String,
    pub(crate) accepted: std::string::String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: std::string::String,
    revision: std::string::String,
    #[serde(default)]
    supersedes: Option<std::string::String>,
    #[serde(default)]
    variant: Option<std::string::String>,
    case: std::string::String,
    requirement: std::string::String,
    frozen: std::string::String,
    frozen_by: std::string::String,
    release_policy: std::string::String,
    fragment: std::string::String,
    subject_interface: std::string::String,
    declaration: std::string::String,
    expected_statement: std::string::String,
    contract_obligation: RecordObligation,
    observations: RecordObservations,
    scope: std::string::String,
    preconditions: std::vec::Vec<std::string::String>,
    assumptions: std::vec::Vec<std::string::String>,
    proof_library: RecordProofLibrary,
    consumer_policy: std::string::String,
    candidate_author_can_edit_owner_law: bool,
    candidate_unknown_at_authoring: std::vec::Vec<std::string::String>,
    non_claims: std::vec::Vec<std::string::String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordObligation {
    language: std::string::String,
    input: std::vec::Vec<(std::string::String, std::string::String)>,
    output: std::vec::Vec<(std::string::String, std::string::String)>,
    requires: std::string::String,
    ensures: std::string::String,
    arithmetic: std::string::String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordObservations {
    initial: std::string::String,
    #[serde(rename = "final")]
    r#final: std::string::String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordProofLibrary {
    requirement: std::string::String,
    accepted: std::string::String,
}

/// Decode one digest-verified frozen law record.
///
/// # Errors
/// `owner-law-record-invalid` when the verified bytes are not the closed
/// record this consumer implements, plus the closed policy, fragment and
/// identity refusals the record's own fields select.
pub(crate) fn decode(
    bytes: &[u8],
    computed_sha256: std::string::String,
    expected_sha256: std::string::String,
) -> Result<Law, super::Refusal> {
    let record: Record = match serde_json::from_slice(bytes) {
        Ok(record) => record,
        Err(error) => {
            return Err(super::Refusal::rejected(
                "owner-law-record-invalid",
                "law-record",
                std::format!("owner law is not the closed noble-owner-law/v1 record: {error}"),
            ))
        }
    };
    let law = build(record, computed_sha256, expected_sha256)?;
    Ok(law)
}

fn build(
    record: Record,
    computed_sha256: std::string::String,
    expected_sha256: std::string::String,
) -> Result<Law, super::Refusal> {
    if record.schema != SCHEMA {
        return Err(super::Refusal::rejected(
            "owner-law-record-invalid",
            "law-record",
            std::format!("owner law schema is `{}`, not `{SCHEMA}`", record.schema),
        ));
    }
    if record.case != CASE || record.requirement != REQUIREMENT {
        return Err(super::Refusal::rejected(
            "owner-law-record-invalid",
            "law-record",
            std::format!(
                "owner law names case `{}` and requirement `{}`, not `{CASE}` and `{REQUIREMENT}`",
                record.case,
                record.requirement
            ),
        ));
    }
    if record.candidate_author_can_edit_owner_law {
        return Err(super::Refusal::rejected(
            "owner-law-record-invalid",
            "law-record",
            "the record admits candidate edits, so it is not an independently frozen law".into(),
        ));
    }
    if record.release_policy != POLICY || record.consumer_policy != POLICY {
        return Err(super::Refusal::rejected(
            "owner-law-policy-mismatch",
            "law-policy",
            std::format!(
                "owner law selects release policy `{}` and consumer policy `{}`, not `{POLICY}`",
                record.release_policy,
                record.consumer_policy
            ),
        ));
    }
    if record.fragment != FRAGMENT {
        return Err(super::Refusal::rejected(
            "owner-law-fragment-unsupported",
            "law-fragment",
            std::format!(
                "owner law selects fragment `{}`, not the one this consumer implements (`{FRAGMENT}`)",
                record.fragment
            ),
        ));
    }
    if record.revision.is_empty()
        || record.frozen.is_empty()
        || record.frozen_by.is_empty()
        || record.subject_interface.is_empty()
        || record.declaration.is_empty()
        || record.expected_statement.is_empty()
        || record.scope.is_empty()
    {
        return Err(super::Refusal::rejected(
            "owner-law-record-invalid",
            "law-record",
            "owner law omits a required identity field".into(),
        ));
    }
    if record
        .supersedes
        .as_deref()
        .is_some_and(|predecessor| predecessor == record.revision)
    {
        return Err(super::Refusal::rejected(
            "owner-law-record-invalid",
            "law-record",
            "a law revision cannot supersede itself".into(),
        ));
    }
    if record.contract_obligation.input.is_empty() || record.contract_obligation.output.is_empty() {
        return Err(super::Refusal::rejected(
            "owner-law-record-invalid",
            "law-record",
            "owner law states an empty input or output binding list".into(),
        ));
    }
    Ok(Law {
        revision: record.revision,
        supersedes: record.supersedes,
        variant: record.variant,
        frozen: record.frozen,
        fragment: record.fragment,
        subject_interface: record.subject_interface,
        declaration: record.declaration,
        expected_statement: record.expected_statement,
        obligation: Obligation {
            language: record.contract_obligation.language,
            input: record.contract_obligation.input,
            output: record.contract_obligation.output,
            requires: record.contract_obligation.requires,
            ensures: record.contract_obligation.ensures,
            arithmetic: record.contract_obligation.arithmetic,
        },
        observations: Observations {
            initial: record.observations.initial,
            final_observation: record.observations.r#final,
        },
        scope: record.scope,
        preconditions: record.preconditions,
        assumptions: record.assumptions,
        proof_library: ProofLibrary {
            requirement: record.proof_library.requirement,
            accepted: record.proof_library.accepted,
        },
        consumer_policy: record.consumer_policy,
        non_claims: record.non_claims,
        candidate_unknown_at_authoring: record.candidate_unknown_at_authoring,
        computed_sha256,
        expected_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::decode;

    const V1: &[u8] =
        include_bytes!("../../../../../verification/owner-law/laws/owner-law-v1.json");
    const V2: &[u8] =
        include_bytes!("../../../../../verification/owner-law/laws/owner-law-v2.json");
    const V1_SHA256: &str = "9a8eda12c74356ca0d312be840411d4445c6f377c16ee7b96a680581b2e91eac";
    const V2_SHA256: &str = "51f2a3158155077dcb29c8f354d356d31df3282b3c76f60c2bc8534c7932ee80";

    #[test]
    fn both_frozen_records_decode_under_their_recorded_digests() {
        assert_eq!(crate::workflow::intrinsic::sha256(V1), V1_SHA256);
        assert_eq!(crate::workflow::intrinsic::sha256(V2), V2_SHA256);
        let v1 =
            decode(V1, V1_SHA256.into(), std::string::String::from(V1_SHA256)).expect("v1 decodes");
        assert_eq!(v1.declaration, "OwnerLaw.increment");
        assert_eq!(v1.obligation.ensures, "(eq (out y) (add (in x) 1))");
        let v2 =
            decode(V2, V2_SHA256.into(), std::string::String::from(V2_SHA256)).expect("v2 decodes");
        assert_eq!(v2.declaration, "OwnerLaw.addTwo");
        assert!(v2
            .supersedes
            .as_deref()
            .is_some_and(|predecessor| predecessor == v1.revision));
    }

    #[test]
    fn records_outside_the_closed_schema_are_refused() {
        let mut value: serde_json::Value = serde_json::from_slice(V1).expect("v1 is JSON");
        value["extra"] = serde_json::Value::Number(1.into());
        let bytes = serde_json::to_vec(&value).expect("re-encodes");
        let refusal = decode(&bytes, "a".into(), "a".into()).expect_err("an extra field refuses");
        assert_eq!(refusal.code, "owner-law-record-invalid");
    }

    #[test]
    fn a_candidate_editable_record_is_refused() {
        let mut value: serde_json::Value = serde_json::from_slice(V1).expect("v1 is JSON");
        value["candidate_author_can_edit_owner_law"] = serde_json::Value::Bool(true);
        let bytes = serde_json::to_vec(&value).expect("re-encodes");
        let refusal = decode(&bytes, "a".into(), "a".into()).expect_err("refused");
        assert_eq!(refusal.code, "owner-law-record-invalid");
    }
}
