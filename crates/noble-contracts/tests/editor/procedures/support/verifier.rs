//! A library owner's fixture verifier.
//!
//! It stands for an owner who independently checked specific claims for exact
//! subjects outside the library (for example with a pinned checker) and kept a
//! ledger of what it observed. It attests only those exact claims and counts
//! every call, so tests can show when the library consults it.

use noble_contracts::source::procedures::{
    Attestation, EvidenceOutcome, EvidenceRecord, EvidenceVerifier, VerificationSubject,
};
use std::cell::Cell;
use std::rc::Rc;
use std::string::String;
use std::vec;
use std::vec::Vec;

pub const OWNER_VERIFIER: &str = "owner-fixture-checker/1";
pub const OWNER_TOOLCHAIN: &str = "owner-pinned-checker 1.0";

#[derive(Debug)]
pub struct OwnerLedger {
    observed: Vec<(EvidenceRecord, Attestation)>,
    calls: Rc<Cell<u32>>,
}

impl OwnerLedger {
    /// An owner that has not checked anything yet.
    pub fn empty() -> Self {
        Self {
            observed: Vec::new(),
            calls: Rc::new(Cell::new(0)),
        }
    }

    /// The owner independently checked exactly `claim` and observed `outcome`.
    pub fn observe(&mut self, claim: &EvidenceRecord, outcome: EvidenceOutcome) {
        self.attest(claim, attestation(claim, outcome));
    }

    /// The owner reports exactly `report` for `claim`, however malformed.
    pub fn attest(&mut self, claim: &EvidenceRecord, report: Attestation) {
        self.observed.push((claim.clone(), report));
    }

    /// The owner's own check reproduced exactly what `claim` asserts.
    pub fn confirm(&mut self, claim: &EvidenceRecord) {
        self.observe(claim, claim.outcome);
    }

    /// The call counter shared with every library this ledger is given to.
    pub fn calls(&self) -> Rc<Cell<u32>> {
        Rc::clone(&self.calls)
    }

    /// The same observations behind a fresh counter, for another library or
    /// a reload in a later "process".
    pub fn replica(&self) -> Self {
        Self {
            observed: self.observed.clone(),
            calls: Rc::new(Cell::new(0)),
        }
    }

    pub fn boxed(self) -> Box<dyn EvidenceVerifier> {
        Box::new(self)
    }
}

/// What the fixture owner reports for one checked claim.
pub fn attestation(claim: &EvidenceRecord, outcome: EvidenceOutcome) -> Attestation {
    let mut command_digest = [0xc5; 32];
    command_digest[..4].copy_from_slice(&claim.obligation.to_be_bytes());
    Attestation {
        verifier: String::from(OWNER_VERIFIER),
        toolchain: vec![String::from(OWNER_TOOLCHAIN)],
        command_digest,
        input_digest: claim.subject_binding,
        outcome,
    }
}

impl EvidenceVerifier for OwnerLedger {
    fn verify(
        &self,
        subject: VerificationSubject<'_>,
        claim: &EvidenceRecord,
    ) -> Option<Attestation> {
        self.calls.set(self.calls.get().saturating_add(1));
        if claim.subject_binding != *subject.procedure_id() {
            return None;
        }
        self.observed
            .iter()
            .find(|(observed, _)| observed == claim)
            .map(|(_, attestation)| attestation.clone())
    }
}
