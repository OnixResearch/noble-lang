//! Host-selected proof-required admission of one live definition (the H-LIVE-02
//! proof clause, LIVE-08).
//!
//! `noble live repl --require-proof NAME` selects one definition name whose
//! every admission needs fresh evidence. `:evidence ABS_CONTRACT ABS_PROOF`
//! stages one offer for the next admission of NAME. That attempt consumes the
//! offer whether it publishes or is refused, so evidence accepted for one
//! generation never authorizes a later one. The consumer route of
//! `noble build --require-proof` prepares the contract and independently
//! rechecks the Lean evidence; the core then binds the accepted evidence to
//! the candidate's own body, checked again as a closed single-quotation
//! subject in a session with no other definitions. A claim about any other
//! subject, including the subject the previous generation was admitted with,
//! is refused by the core. The bytes staged for the candidate must also be
//! exactly the bytes the host emitted for it: replayed bytes, even bytes valid
//! for the prior source, are refused before staging. Definitions other than
//! NAME stay proof-optional.
//!
//! Bounded subset: the subject is a dependency-free definition written as
//! `def NAME [ BODY ]`. Declared modules, self-edit grants, watch and host test
//! effects are refused when the policy is selected.

/// The selected subject and at most one staged, unconsumed evidence offer.
pub(super) struct Policy {
    subject: std::string::String,
    staged: Option<Offer>,
}

struct Offer {
    contract: std::vec::Vec<u8>,
    proof: std::vec::Vec<u8>,
    contract_sha256: std::string::String,
    proof_sha256: std::string::String,
}

/// The proof-policy decision for one candidate definition.
pub(super) enum Decision {
    /// Not the selected subject: ordinary proof-optional admission.
    Optional,
    /// Fresh evidence binds the exact candidate; the record joins its publication report.
    Admitted(serde_json::Value),
    /// Refused before staging: the diagnostic and the admission record.
    Refused(std::string::String, serde_json::Value),
}

/// The candidate bytes the proof policy compares: what the host emitted for
/// the checked candidate, and what would be staged in their place.
pub(super) struct Artifact<'a> {
    pub(super) emitted: &'a [u8],
    pub(super) served: &'a [u8],
}

impl Policy {
    pub(super) const fn new(subject: std::string::String) -> Self {
        Self {
            subject,
            staged: None,
        }
    }

    /// Stage one evidence offer read from two absolute, bounded regular files.
    /// A newer offer replaces an unconsumed one; nothing is checked until an
    /// admission of the selected subject consumes it.
    pub(super) fn stage(&mut self, terms: &str) -> Result<serde_json::Value, std::string::String> {
        let mut words = terms.split_ascii_whitespace();
        let (Some(contract_path), Some(proof_path), None) =
            (words.next(), words.next(), words.next())
        else {
            return Err("expected :evidence ABS_CONTRACT ABS_PROOF".into());
        };
        let contract = evidence_file(contract_path)?;
        let proof = evidence_file(proof_path)?;
        let replaced = self.staged.is_some();
        let offer = Offer {
            contract_sha256: crate::workflow::intrinsic::sha256(&contract),
            proof_sha256: crate::workflow::intrinsic::sha256(&proof),
            contract,
            proof,
        };
        let report = serde_json::json!({
            "schema": "noble-live-report/v1",
            "stage": "evidence",
            "outcome": "evidence-staged",
            "subject": self.subject,
            "contract_sha256": offer.contract_sha256,
            "proof_sha256": offer.proof_sha256,
            "replaced_unconsumed_offer": replaced,
            "guest_requests": 0,
            "protected_operations": 0,
        });
        self.staged = Some(offer);
        Ok(report)
    }

    /// Decide one candidate definition before anything is staged.
    pub(super) fn admit(&mut self, name: &str, source: &[u8], artifact: &Artifact<'_>) -> Decision {
        if name != self.subject {
            return Decision::Optional;
        }
        let Some(offer) = self.staged.take() else {
            return refused(
                serde_json::json!({ "policy": "proof-required", "subject": name }),
                "evidence-missing",
                std::format!("proof-required admission of `{name}` has no staged evidence"),
            );
        };
        let mut record = serde_json::json!({
            "policy": "proof-required",
            "subject": name,
            "contract_sha256": offer.contract_sha256,
            "proof_sha256": offer.proof_sha256,
            "emitted_sha256": crate::workflow::intrinsic::sha256(artifact.emitted),
            "served_sha256": crate::workflow::intrinsic::sha256(artifact.served),
        });
        let Some(body) = subject_body(source, name) else {
            return refused(
                record,
                "subject-shape",
                "the proof-required subject must be one dependency-free `def NAME [ BODY ]`".into(),
            );
        };
        let frontend = noble_contracts::source::Session::new();
        let prepared = match frontend.prepare(body, &[], crate::core::SOURCE_LIMITS) {
            Ok(prepared) => prepared,
            Err(error) => {
                return refused(
                    record,
                    "subject-unchecked",
                    std::format!(
                        "the candidate body is not a closed checked subject: {}",
                        error.diagnostic().message
                    ),
                );
            }
        };
        let Some(submission) = prepared.submission() else {
            return refused(
                record,
                "subject-unchecked",
                "the candidate body is not a closed checked subject expression".into(),
            );
        };
        let mut core = noble_contracts::companion::Core::new(crate::core::EVIDENCE_LIMITS);
        core.set_policy(crate::build::POLICY_REVISION);
        let completion = match crate::workflow::admission::admit(
            &mut core,
            &offer.contract,
            crate::core::SOURCE_LIMITS,
            Some((offer.proof.as_slice(), false)),
            crate::workflow::DEFAULT_TIMEOUT,
        ) {
            Ok(completion) => completion,
            Err(refusal) => {
                record["claim_outcome"] = serde_json::json!(refusal.outcome.name());
                return refused(record, refusal.code, refusal.message);
            }
        };
        let admission = completion.decision;
        record["claim_outcome"] = serde_json::json!(admission.outcome.code());
        let evidence = match (admission.outcome, admission.evidence) {
            (noble_contracts::companion::Outcome::Proved, Some(evidence)) => evidence,
            _ => {
                return refused(
                    record,
                    "not-proved",
                    std::format!(
                        "the consumer reported `{}`, not an accepted proved claim",
                        admission.outcome.code()
                    ),
                );
            }
        };
        let release = core
            .bind_artifact(evidence, submission)
            .and_then(|subject| core.release(evidence, &subject));
        let release = match release {
            Ok(release) => release,
            Err(refusal) => {
                return refused(
                    record,
                    refusal.code(),
                    "the accepted evidence does not bind the candidate subject".into(),
                );
            }
        };
        if artifact.served != artifact.emitted {
            return refused(
                record,
                "artifact-replay",
                "the bytes staged for the candidate are not the bytes emitted for it".into(),
            );
        }
        record["decision"] = serde_json::json!("admitted");
        record["core_subject"] = serde_json::json!(release.subject.0);
        record["core_statement"] = serde_json::json!(release.statement);
        record["core_contract"] = serde_json::json!(release.contract.0);
        record["core_evidence"] = serde_json::json!(release.evidence.0);
        record["policy_revision"] = serde_json::json!(release.policy);
        Decision::Admitted(record)
    }
}

fn refused(mut record: serde_json::Value, code: &str, diagnostic: std::string::String) -> Decision {
    record["decision"] = serde_json::json!("refused");
    record["refusal"] = serde_json::json!(code);
    Decision::Refused(diagnostic, record)
}

/// The body of `def NAME [ BODY ]` as its own closed quotation source.
fn subject_body<'a>(source: &'a [u8], name: &str) -> Option<&'a [u8]> {
    let text = std::str::from_utf8(source).ok()?.trim_end();
    let body = text
        .strip_prefix("def ")?
        .strip_prefix(name)?
        .strip_prefix(' ')?
        .trim_start();
    (body.starts_with('[') && body.ends_with(']')).then_some(body.as_bytes())
}

fn evidence_file(path: &str) -> Result<std::vec::Vec<u8>, std::string::String> {
    let selected = super::selected_path(std::path::Path::new(path))
        .map_err(|problem| std::format!("evidence file: {problem}"))?;
    super::snapshot(&selected)
        .map(|snapshot| snapshot.bytes)
        .map_err(|problem| std::format!("evidence file: {problem}"))
}

#[cfg(test)]
mod tests {
    use super::subject_body;

    #[test]
    fn subject_body_is_the_definition_quotation_only() {
        assert_eq!(
            subject_body(b"def addone [ 2 + ]\n", "addone"),
            Some(b"[ 2 + ]".as_slice())
        );
        assert_eq!(subject_body(b"def addone [ 2 + ]", "addtwo"), None);
        assert_eq!(subject_body(b"def addonex [ 2 + ]", "addone"), None);
        assert_eq!(subject_body(b"def addone 2 +", "addone"), None);
        assert_eq!(subject_body(b"20 addone", "addone"), None);
    }
}
