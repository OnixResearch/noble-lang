//! Owner-verified evidence acceptance.
//!
//! An [`EvidenceRecord`] is the submitter's claim. The library retains it as
//! visible, unverified provenance, and a claim alone never establishes
//! `tested` or `promoted`. Only a [`VerifiedEvidence`] receipt counts toward
//! those states. The library mints a receipt at admission, and only from the
//! [`EvidenceVerifier`] the library owner supplied when constructing or
//! loading the library. A record, its submitter or persisted bytes cannot
//! mint, transplant or vouch for a receipt: receipts carried by an incoming
//! record are discarded and re-derived, and load re-establishes every
//! persisted receipt through the loading owner's verifier. Retrieval, export,
//! encoding and every other inspection never call the verifier.

use super::*;
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;

/// What an owner-supplied verifier observed after independently checking one
/// claim against one exact immutable subject. Every field is the verifier's
/// own observation. The library copies the obligation, class and policy
/// revision from the claim it asked about and the subject identity from the
/// recomputed subject, never from an attestation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attestation {
    /// Versioned identity of the verifier that ran.
    pub verifier: String,
    /// The exact toolchain identity the verifier ran.
    pub toolchain: Vec<String>,
    /// Digest of the exact command the verifier ran.
    pub command_digest: [u8; 32],
    /// Digest of the exact inputs it checked (subject, fixtures, proof bytes).
    pub input_digest: [u8; 32],
    /// The outcome the verifier observed; it may differ from the claim.
    pub outcome: EvidenceOutcome,
}

/// The library owner's trust root for evidence acceptance. The owner passes
/// it to `ProcedureLibrary::with_verifier` or
/// `ProcedureLibrary::load_with_verifier`; no record field or submitter can
/// select, replace or bypass it.
pub trait EvidenceVerifier: core::fmt::Debug {
    /// Independently check `claim` against `subject`. Return `None` to leave
    /// the claim as unverified provenance. The answer must depend only on the
    /// subject and the claim: load re-asks for every persisted receipt and
    /// refuses the library when the answer differs.
    fn verify(
        &self,
        subject: VerificationSubject<'_>,
        claim: &EvidenceRecord,
    ) -> Option<Attestation>;
}

/// The exact immutable subject a verifier judges. It exposes only the fields
/// the procedure identity is recomputed from, never the record's mutable
/// status, promotion, cost or other claims.
#[derive(Clone, Copy, Debug)]
pub struct VerificationSubject<'a> {
    record: &'a ProcedureRecord,
}

impl<'a> VerificationSubject<'a> {
    /// The procedure identity, already recomputed from the fields below.
    pub const fn procedure_id(&self) -> &'a ProcedureId {
        &self.record.procedure_id
    }

    pub const fn chunk(&self) -> &'a chunks::Envelope {
        &self.record.chunk
    }

    pub const fn provenance(&self) -> &'a Provenance {
        &self.record.provenance
    }

    pub fn library_dependencies(&self) -> &'a [LibraryDependency] {
        &self.record.library_dependencies
    }

    /// The experimental immutable-subject wire bytes behind `procedure_id`.
    pub fn identity_subject_bytes(&self) -> Result<Vec<u8>, Refusal> {
        persistence::identity_bytes(self.record)
    }
}

/// A verification receipt. Only the library constructs one: at admission,
/// from its owner's verifier, for one claim of one exact subject. Its fields
/// are read-only outside the library, so a caller cannot forge, edit or
/// re-target a receipt, and the library never trusts one it did not derive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedEvidence {
    pub(super) obligation: u32,
    pub(super) class: EvidenceClass,
    pub(super) outcome: EvidenceOutcome,
    pub(super) subject_binding: ProcedureId,
    pub(super) policy_revision: u32,
    pub(super) claim_digest: ProcedureId,
    pub(super) verifier: String,
    pub(super) toolchain: Vec<String>,
    pub(super) command_digest: [u8; 32],
    pub(super) input_digest: [u8; 32],
}

impl VerifiedEvidence {
    pub const fn obligation(&self) -> u32 {
        self.obligation
    }

    pub const fn class(&self) -> EvidenceClass {
        self.class
    }

    /// The outcome the owner's verifier observed, not the claimed outcome.
    pub const fn outcome(&self) -> EvidenceOutcome {
        self.outcome
    }

    pub const fn subject_binding(&self) -> &ProcedureId {
        &self.subject_binding
    }

    pub const fn policy_revision(&self) -> u32 {
        self.policy_revision
    }

    /// Digest of the exact retained claim this receipt answers.
    pub const fn claim_digest(&self) -> &ProcedureId {
        &self.claim_digest
    }

    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    pub fn toolchain(&self) -> &[String] {
        &self.toolchain
    }

    pub const fn command_digest(&self) -> &[u8; 32] {
        &self.command_digest
    }

    pub const fn input_digest(&self) -> &[u8; 32] {
        &self.input_digest
    }

    fn mint(
        subject: &ProcedureRecord,
        claim: &EvidenceRecord,
        attestation: Attestation,
    ) -> Result<Self, Refusal> {
        let receipt = Self {
            obligation: claim.obligation,
            class: claim.class,
            outcome: attestation.outcome,
            subject_binding: subject.procedure_id,
            policy_revision: claim.policy_revision,
            claim_digest: persistence::claim_digest(claim)?,
            verifier: attestation.verifier,
            toolchain: attestation.toolchain,
            command_digest: attestation.command_digest,
            input_digest: attestation.input_digest,
        };
        receipt.check_shape(0)?;
        Ok(receipt)
    }

    fn check_shape(&self, index: u32) -> Result<(), Refusal> {
        let is_identified = !self.verifier.is_empty()
            && !self.toolchain.is_empty()
            && self.toolchain.iter().all(|toolchain| !toolchain.is_empty());
        if !is_identified || self.toolchain.len() > MAX_EVIDENCE {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "verified_evidence.toolchain",
                    message: "a verification receipt names the verifier and the bounded toolchain that ran",
                },
                index,
            ));
        }
        if self.command_digest == [0; 32] || self.input_digest == [0; 32] {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "verified_evidence.binding",
                    message: "a verification receipt binds the exact command and inputs that ran",
                },
                index,
            ));
        }
        let is_proof_outcome = matches!(
            self.outcome,
            EvidenceOutcome::Proved | EvidenceOutcome::Disproved
        );
        if is_proof_outcome && !self.class.proves() {
            return Err(Refusal::at(
                RefusalKind::ForgedStatus,
                Diagnostic {
                    field: "verified_evidence.outcome",
                    message: "test, validation, review or assumption evidence cannot be verified as a proof outcome",
                },
                index,
            ));
        }
        Ok(())
    }
}

/// Canonical receipt order: obligation, class rank, then every other field.
pub(super) fn compare(left: &VerifiedEvidence, right: &VerifiedEvidence) -> Ordering {
    left.obligation
        .cmp(&right.obligation)
        .then_with(|| left.class.rank().cmp(&right.class.rank()))
        .then_with(|| left.outcome.code().cmp(right.outcome.code()))
        .then_with(|| left.policy_revision.cmp(&right.policy_revision))
        .then_with(|| left.claim_digest.cmp(&right.claim_digest))
        .then_with(|| left.verifier.cmp(&right.verifier))
        .then_with(|| left.toolchain.cmp(&right.toolchain))
        .then_with(|| left.command_digest.cmp(&right.command_digest))
        .then_with(|| left.input_digest.cmp(&right.input_digest))
        .then_with(|| left.subject_binding.cmp(&right.subject_binding))
}

/// `tested` needs a passing test outcome observed by the owner's verifier.
pub(super) fn has_verified_passing_test(record: &ProcedureRecord) -> bool {
    record.verified.iter().any(|receipt| {
        receipt.class == EvidenceClass::Test && receipt.outcome == EvidenceOutcome::Passed
    })
}

/// Whether the owner's verifier observed `obligation` proved under the
/// promotion's policy revision, in a class the selected consumer allows.
pub(super) fn has_verified_proof(
    record: &ProcedureRecord,
    promotion: &Promotion,
    obligation: u32,
    required: Option<&ConsumerObligation>,
) -> bool {
    record.verified.iter().any(|receipt| {
        receipt.obligation == obligation
            && receipt.policy_revision == promotion.policy_revision
            && receipt.outcome == EvidenceOutcome::Proved
            && receipt.class.proves()
            && required.is_none_or(|policy| policy.allowed_classes.contains(&receipt.class))
    })
}

/// The retained claim a receipt answers, found by its exact claim digest.
fn cited_claim<'a>(
    record: &'a ProcedureRecord,
    receipt: &VerifiedEvidence,
    index: u32,
) -> Result<&'a EvidenceRecord, Refusal> {
    for claim in &record.evidence {
        if persistence::claim_digest(claim)? == receipt.claim_digest {
            return Ok(claim);
        }
    }
    Err(Refusal::at(
        RefusalKind::Invalid,
        Diagnostic {
            field: "verified_evidence.claim_digest",
            message: "a verification receipt must answer a retained claim; deleting the claim invalidates it",
        },
        index,
    ))
}

/// Consistency of retained receipts: each binds this exact subject, answers
/// exactly one retained claim with that claim's obligation, class and policy
/// revision, and carries its verifier, toolchain, command and input binding.
pub(super) fn validate(record: &ProcedureRecord) -> Result<(), Refusal> {
    if record.verified.len() > MAX_EVIDENCE {
        return Err(Refusal::new(
            RefusalKind::Exhausted,
            Diagnostic {
                field: "verified_evidence",
                message: "verification receipt limit reached",
            },
        ));
    }
    for (index, receipt) in record.verified.iter().enumerate() {
        let position = index as u32;
        if receipt.subject_binding != record.procedure_id {
            return Err(Refusal::at(
                RefusalKind::StaleContext,
                Diagnostic {
                    field: "verified_evidence.subject_binding",
                    message:
                        "a verification receipt is not bound to this immutable procedure subject",
                },
                position,
            ));
        }
        let claim = cited_claim(record, receipt, position)?;
        let is_answer = receipt.obligation == claim.obligation
            && receipt.class == claim.class
            && receipt.policy_revision == claim.policy_revision;
        let is_repeated = record.verified[..index]
            .iter()
            .any(|earlier| earlier.claim_digest == receipt.claim_digest);
        if !is_answer || is_repeated {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "verified_evidence.claim_digest",
                    message: "each verification receipt answers exactly one retained claim with its obligation, class and policy revision",
                },
                position,
            ));
        }
        receipt.check_shape(position)?;
        debug_assert_eq!(receipt.class, claim.class);
        debug_assert_eq!(receipt.policy_revision, claim.policy_revision);
    }
    Ok(())
}

/// Replace every receipt `record` carries with the library's own decision.
/// A claim already retained for this subject keeps the decision made when it
/// first entered the library; every other claim goes to the owner's
/// verifier, once. Without a verifier every new claim stays unverified.
pub(super) fn accept(
    verifier: Option<&dyn EvidenceVerifier>,
    record: &mut ProcedureRecord,
    retained: Option<&ProcedureRecord>,
) -> Result<(), Refusal> {
    debug_assert!(retained.is_none_or(|existing| existing.procedure_id == record.procedure_id));
    record.verified = Vec::new();
    let mut receipts = Vec::with_capacity(record.evidence.len());
    for (index, claim) in record.evidence.iter().enumerate() {
        if record.evidence[..index].contains(claim) {
            continue;
        }
        if let Some(existing) = retained.filter(|existing| existing.evidence.contains(claim)) {
            let digest = persistence::claim_digest(claim)?;
            receipts.extend(
                existing
                    .verified
                    .iter()
                    .filter(|receipt| receipt.claim_digest == digest)
                    .cloned(),
            );
            continue;
        }
        let subject = VerificationSubject { record: &*record };
        if let Some(attestation) = verifier.and_then(|owner| owner.verify(subject, claim)) {
            receipts.push(VerifiedEvidence::mint(&*record, claim, attestation)?);
        }
    }
    debug_assert!(receipts.len() <= record.evidence.len());
    record.verified = receipts;
    Ok(())
}

/// Re-establish every persisted receipt of `record` through the loading
/// owner's verifier. A persisted receipt is never trusted from bytes: absent a
/// verifier, or when the verifier's answer differs, load refuses `invalid`.
/// Receipts already re-established for the same subject and claim (an earlier
/// generation retains the same receipt) are reused from `confirmed`, which is
/// returned extended with this record's receipts.
pub(super) fn confirm(
    verifier: Option<&dyn EvidenceVerifier>,
    record: &ProcedureRecord,
    mut confirmed: Vec<VerifiedEvidence>,
) -> Result<Vec<VerifiedEvidence>, Refusal> {
    for (index, receipt) in record.verified.iter().enumerate() {
        if confirmed.contains(receipt) {
            continue;
        }
        let position = index as u32;
        let claim = cited_claim(record, receipt, position)?;
        let subject = VerificationSubject { record };
        let reestablished = match verifier.and_then(|owner| owner.verify(subject, claim)) {
            Some(attestation) => Some(VerifiedEvidence::mint(record, claim, attestation)?),
            None => None,
        };
        if reestablished.as_ref() != Some(receipt) {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "verified_evidence",
                    message: "a persisted verification receipt is not re-established by the loading owner's verifier",
                },
                position,
            ));
        }
        debug_assert_eq!(receipt.subject_binding, record.procedure_id);
        debug_assert_eq!(receipt.obligation, claim.obligation);
        confirmed.push(receipt.clone());
    }
    Ok(confirmed)
}
