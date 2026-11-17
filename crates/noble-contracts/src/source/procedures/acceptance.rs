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
use alloc::boxed::Box;
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

/// The owner's verifier as the library holds it: the only place a
/// `dyn EvidenceVerifier` is stored or called.
///
/// Reviewed extraction exception (#97): Aeneas cannot represent trait
/// objects, so this host plug-in handle is an opaque type, and its `verify`
/// and `Debug::fmt` are opaque; translated code only passes the handle on.
#[charon::opaque]
pub(super) struct OwnerVerifier(pub(super) Box<dyn EvidenceVerifier>);

impl OwnerVerifier {
    /// Ask the owner's verifier about `claim` against `record`'s immutable subject.
    ///
    /// Reviewed extraction exception (#97): opaque because it calls through
    /// the trait object.
    #[charon::opaque]
    pub(super) fn verify(
        &self,
        record: &ProcedureRecord,
        claim: &EvidenceRecord,
    ) -> Option<Attestation> {
        self.0.verify(VerificationSubject { record }, claim)
    }
}

impl core::fmt::Debug for OwnerVerifier {
    /// Format exactly as the boxed verifier does.
    ///
    /// Reviewed extraction exception (#97): opaque because it calls through
    /// the trait object.
    #[charon::opaque]
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.0, formatter)
    }
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
            && crate::scan::find_first(&self.toolchain, |toolchain| toolchain.is_empty()).is_none();
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
    crate::scan::find_first(&record.verified, |receipt| {
        receipt.class == EvidenceClass::Test && receipt.outcome == EvidenceOutcome::Passed
    })
    .is_some()
}

/// Whether the owner's verifier observed `obligation` proved under the
/// promotion's policy revision, in a class the selected consumer allows.
pub(super) fn has_verified_proof(
    record: &ProcedureRecord,
    promotion: &Promotion,
    obligation: u32,
    required: Option<&ConsumerObligation>,
) -> bool {
    crate::scan::find_first(&record.verified, |receipt| {
        receipt.obligation == obligation
            && receipt.policy_revision == promotion.policy_revision
            && receipt.outcome == EvidenceOutcome::Proved
            && receipt.class.proves()
            && required.is_none_or(|policy| policy.allowed_classes.contains(&receipt.class))
    })
    .is_some()
}

/// The retained claim a receipt answers, found by its exact claim digest.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; a receipt citing no retained claim is stored record content refused as a typed `invalid` refusal, and a digest failure propagates; neither is an internal invariant to assert"
)]
fn cited_claim<'a>(
    record: &'a ProcedureRecord,
    receipt: &VerifiedEvidence,
    index: u32,
) -> Result<&'a EvidenceRecord, Refusal> {
    let mut cited = None;
    let mut failure = None;
    let mut at = 0;
    while cited.is_none() && failure.is_none() && at < record.evidence.len() {
        match persistence::claim_digest(&record.evidence[at]) {
            Ok(digest) => {
                if digest == receipt.claim_digest {
                    cited = Some(at);
                }
            }
            Err(refusal) => failure = Some(refusal),
        }
        at += 1;
    }
    if let Some(refusal) = failure {
        return Err(refusal);
    }
    if let Some(at) = cited {
        return Ok(&record.evidence[at]);
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
    let mut outcome = Ok(());
    let mut index = 0;
    while outcome.is_ok() && index < record.verified.len() {
        outcome = validate_receipt(record, index);
        index += 1;
    }
    outcome
}

/// One retained receipt of [`validate`].
fn validate_receipt(record: &ProcedureRecord, index: usize) -> Result<(), Refusal> {
    let receipt = &record.verified[index];
    let position = index as u32;
    if receipt.subject_binding != record.procedure_id {
        return Err(Refusal::at(
            RefusalKind::StaleContext,
            Diagnostic {
                field: "verified_evidence.subject_binding",
                message: "a verification receipt is not bound to this immutable procedure subject",
            },
            position,
        ));
    }
    let claim = cited_claim(record, receipt, position)?;
    let is_answer = answers(receipt, claim);
    let is_repeated = crate::scan::find_first(&record.verified[..index], |earlier| {
        earlier.claim_digest == receipt.claim_digest
    })
    .is_some();
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
    Ok(())
}

/// Whether `receipt` answers `claim`: the same obligation, class and policy
/// revision.
fn answers(receipt: &VerifiedEvidence, claim: &EvidenceRecord) -> bool {
    receipt.obligation == claim.obligation
        && receipt.class == claim.class
        && receipt.policy_revision == claim.policy_revision
}

/// Replace every receipt `record` carries with the library's own decision.
/// A claim already retained for this subject keeps the decision made when it
/// first entered the library; every other claim goes to the owner's
/// verifier, once. Without a verifier every new claim stays unverified.
pub(super) fn accept(
    verifier: Option<&OwnerVerifier>,
    record: &mut ProcedureRecord,
    retained: Option<&ProcedureRecord>,
) -> Result<(), Refusal> {
    debug_assert!(retained.is_none_or(|existing| existing.procedure_id == record.procedure_id));
    record.verified = Vec::new();
    let mut receipts = Vec::with_capacity(record.evidence.len());
    let mut outcome = Ok(());
    let mut index = 0;
    while outcome.is_ok() && index < record.evidence.len() {
        outcome = accept_claim(verifier, record, retained, index, &mut receipts);
        index += 1;
    }
    outcome?;
    debug_assert!(receipts.len() <= record.evidence.len());
    record.verified = receipts;
    Ok(())
}

/// One claim of [`accept`]: keep the retained decision or ask the owner once.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; retained and newly minted receipts are pushed onto the caller's receipt list, which needs the growable Vec, not a fixed slice"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; repeated claims, retained decisions and the owner's answer are ordinary data, digest and minting failures propagate as typed refusals, and `accept` asserts the receipt bound once for all claims"
)]
fn accept_claim(
    verifier: Option<&OwnerVerifier>,
    record: &ProcedureRecord,
    retained: Option<&ProcedureRecord>,
    index: usize,
    receipts: &mut Vec<VerifiedEvidence>,
) -> Result<(), Refusal> {
    let claim = &record.evidence[index];
    if record.evidence[..index].contains(claim) {
        return Ok(());
    }
    if let Some(existing) = retained.filter(|existing| existing.evidence.contains(claim)) {
        let digest = persistence::claim_digest(claim)?;
        for receipt in existing.verified.iter() {
            if receipt.claim_digest == digest {
                receipts.push(receipt.clone());
            }
        }
        return Ok(());
    }
    let attestation = match verifier {
        Some(owner) => owner.verify(record, claim),
        None => None,
    };
    if let Some(attestation) = attestation {
        receipts.push(VerifiedEvidence::mint(record, claim, attestation)?);
    }
    Ok(())
}

/// Re-establish every persisted receipt of `record` through the loading
/// owner's verifier. A persisted receipt is never trusted from bytes: absent a
/// verifier, or when the verifier's answer differs, load refuses `invalid`.
/// Receipts already re-established for the same subject and claim (an earlier
/// generation retains the same receipt) are reused from `confirmed`, which is
/// returned extended with this record's receipts.
pub(super) fn confirm(
    verifier: Option<&OwnerVerifier>,
    record: &ProcedureRecord,
    mut confirmed: Vec<VerifiedEvidence>,
) -> Result<Vec<VerifiedEvidence>, Refusal> {
    let mut outcome = Ok(());
    let mut index = 0;
    while outcome.is_ok() && index < record.verified.len() {
        outcome = confirm_receipt(verifier, record, index, &mut confirmed);
        index += 1;
    }
    outcome?;
    Ok(confirmed)
}

/// One receipt of [`confirm`]: reuse an earlier confirmation or ask the owner.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; each re-established receipt is pushed onto the caller's confirmed list, which needs the growable Vec, not a fixed slice"
)]
fn confirm_receipt(
    verifier: Option<&OwnerVerifier>,
    record: &ProcedureRecord,
    index: usize,
    confirmed: &mut Vec<VerifiedEvidence>,
) -> Result<(), Refusal> {
    let receipt = &record.verified[index];
    if confirmed.contains(receipt) {
        return Ok(());
    }
    let position = index as u32;
    let claim = cited_claim(record, receipt, position)?;
    let attestation = match verifier {
        Some(owner) => owner.verify(record, claim),
        None => None,
    };
    let reestablished = match attestation {
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
    Ok(())
}
