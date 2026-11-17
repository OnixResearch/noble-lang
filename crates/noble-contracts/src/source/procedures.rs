//! Content-addressed learned procedure records over the typed-chunk/1 library.
//!
//! Procedure identifiers are experimental, encoder-local indexes. They are
//! recomputed from immutable subject fields and never authorize a program;
//! every retained chunk is admitted by `chunks::Library`, and every composed
//! source is freshly checked by `Session::prepare` under the caller's limits.
//!
//! Evidence records are submitter claims. They stay visible as unverified
//! provenance; `tested` and `promoted` count only verification receipts that
//! the library minted at admission from its owner's `EvidenceVerifier`.

#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; this public host-side domain uses chunk and companion names directly in its bounded record API"
)]
#![expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the public API validates runtime vectors and performs bounded allocation"
)]
#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; library transitions mutate their explicitly owned generation state after all candidate checks succeed"
)]

mod acceptance;
mod alias;
mod composition;
mod persistence;
mod retrieval;

pub use acceptance::{Attestation, EvidenceVerifier, VerificationSubject, VerifiedEvidence};
pub use alias::{
    Certificate, ExactPairCertificate, Monomial, Polynomial, MAX_ALIAS_INPUTS, MAX_ALIAS_OUTPUTS,
    MAX_ALIAS_TERMS,
};
pub use composition::{BoundOperand, CertifiedComposedProgram, ComposeContext, ComposedProgram};
pub use persistence::{ArmMetrics, Metrics};
pub use retrieval::{Query, QueryBudget, QueryResult, RuntimeProducer};

use super::chunks;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use noble_kernel::types::{EffSet, Ty};

pub const RECORD_FORMAT: u32 = 1;
pub const SEMANTIC_REVISION: u32 = 0;
pub const ENCODER_ID: &str = "noble-procedure-record/1";
pub const ENCODING_REVISION: u32 = 1;
pub const MAX_RECORDS: usize = 64;
pub const MAX_LIBRARY_DEPENDENCIES: usize = 16;
pub const MAX_EVIDENCE: usize = 16;
pub const MAX_GENERATIONS: usize = 16;
pub const MAX_INDEX_BYTES: usize = 65_536;
pub const MAX_MANIFEST_BYTES: usize = 262_144;
pub const MAX_METRICS_BYTES: usize = 65_536;
pub const MAX_STORAGE_BYTES: usize = 262_144;

pub type ProcedureId = [u8; 32];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encoder {
    pub id: String,
    pub encoding_revision: u32,
    pub experimental: bool,
}

impl Encoder {
    pub fn current() -> Self {
        Self {
            id: String::from(ENCODER_ID),
            encoding_revision: ENCODING_REVISION,
            experimental: true,
        }
    }
}

#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Producer {
    Chunk,
    Synthesis,
    Composition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
    pub producer: Producer,
    pub producer_tool_revision: u32,
    pub task_id: Option<[u8; 32]>,
    pub grammar_revision: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LibraryDependency {
    pub procedure_id: ProcedureId,
    pub chunk_id: [u8; chunks::IDENTITY_BYTES],
    pub source_revision: [u8; chunks::REVISION_BYTES],
}

#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceClass {
    Lean,
    AeneasLean,
    Verus,
    Validation,
    Test,
    Review,
    Assumption,
}

impl EvidenceClass {
    const fn rank(self) -> u8 {
        match self {
            Self::Lean => 0,
            Self::AeneasLean => 1,
            Self::Verus => 2,
            Self::Validation => 3,
            Self::Test => 4,
            Self::Review => 5,
            Self::Assumption => 6,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Lean => "lean",
            Self::AeneasLean => "aeneas-lean",
            Self::Verus => "verus",
            Self::Validation => "validation",
            Self::Test => "test",
            Self::Review => "review",
            Self::Assumption => "assumption",
        }
    }

    const fn proves(self) -> bool {
        matches!(self, Self::Lean | Self::AeneasLean | Self::Verus)
    }
}

#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceOutcome {
    Proved,
    Disproved,
    Unknown,
    Timeout,
    Unsupported,
    Error,
    NotRun,
    Passed,
    Failed,
}

impl EvidenceOutcome {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Proved => "proved",
            Self::Disproved => "disproved",
            Self::Unknown => "unknown",
            Self::Timeout => "timeout",
            Self::Unsupported => "unsupported",
            Self::Error => "error",
            Self::NotRun => "not-run",
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

/// This marker is input-only. Any record carrying it is refused before
/// retention; the library never stores an authority-bearing value.
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityMarker {
    LiveCapability,
    ServiceHandle,
    Session,
}

/// A submitter's evidence claim. The library retains it as visible,
/// unverified provenance. By itself it never establishes `tested` or
/// `promoted`: only a `VerifiedEvidence` receipt, minted at admission from the
/// library owner's verifier for this exact claim, counts toward those states.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceRecord {
    pub obligation: u32,
    pub class: EvidenceClass,
    pub outcome: EvidenceOutcome,
    pub toolchain: Vec<String>,
    pub claim: String,
    pub subject_binding: ProcedureId,
    pub policy_revision: u32,
    pub capability: Option<CapabilityMarker>,
}

#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Candidate,
    Tested,
    Promoted,
    Superseded,
}

impl Status {
    const fn rank(self) -> u8 {
        match self {
            Self::Promoted => 0,
            Self::Tested => 1,
            Self::Candidate => 2,
            Self::Superseded => 3,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Tested => "tested",
            Self::Promoted => "promoted",
            Self::Superseded => "superseded",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Promotion {
    pub policy_revision: u32,
    pub obligations: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cost {
    pub expanded_nodes: u32,
    pub emitted_source_bytes: u32,
    pub evaluation_steps: u32,
}

impl Cost {
    const fn key(self) -> [u32; 3] {
        [
            self.expanded_nodes,
            self.emitted_source_bytes,
            self.evaluation_steps,
        ]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RetrievalSignature {
    pub stack_in: Vec<Ty>,
    pub stack_out: Vec<Ty>,
    pub effects: EffSet,
    pub status_rank: u8,
    pub cost_key: [u32; 3],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alias {
    pub procedure_id: ProcedureId,
    pub certificate: Certificate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcedureRecord {
    pub format: u32,
    pub semantic_revision: u32,
    pub encoder: Encoder,
    /// Recomputed on every admission, reload and retrieval.
    pub procedure_id: ProcedureId,
    pub chunk: chunks::Envelope,
    pub provenance: Provenance,
    pub library_dependencies: Vec<LibraryDependency>,
    /// Submitter claims, kept as unverified provenance.
    pub evidence: Vec<EvidenceRecord>,
    /// Receipts minted by the admitting library from its owner's verifier.
    /// Callers can read them but never construct, edit or transplant one: an
    /// incoming record's receipts are discarded and re-derived on admission.
    verified: Vec<VerifiedEvidence>,
    pub status: Status,
    pub promotion: Option<Promotion>,
    pub cost: Cost,
    pub retrieval: RetrievalSignature,
    pub supersedes: Option<ProcedureId>,
    pub alias_of: Option<Alias>,
}

impl ProcedureRecord {
    pub fn candidate(chunk: chunks::Envelope, provenance: Provenance) -> Result<Self, Refusal> {
        Self::candidate_with_dependencies(chunk, provenance, Vec::new())
    }

    pub fn candidate_with_dependencies(
        chunk: chunks::Envelope,
        provenance: Provenance,
        library_dependencies: Vec<LibraryDependency>,
    ) -> Result<Self, Refusal> {
        let encoder = Encoder::current();
        let procedure_id = persistence::identity(
            &encoder,
            SEMANTIC_REVISION,
            &chunk,
            &provenance,
            &library_dependencies,
        )?;
        let retrieval = RetrievalSignature {
            stack_in: chunk.interface.stack_in.clone(),
            stack_out: chunk.interface.stack_out.clone(),
            effects: chunk.interface.effects.clone(),
            status_rank: Status::Candidate.rank(),
            cost_key: Cost::default().key(),
        };
        Ok(Self {
            format: RECORD_FORMAT,
            semantic_revision: SEMANTIC_REVISION,
            encoder,
            procedure_id,
            chunk,
            provenance,
            library_dependencies,
            evidence: Vec::new(),
            verified: Vec::new(),
            status: Status::Candidate,
            promotion: None,
            cost: Cost::default(),
            retrieval,
            supersedes: None,
            alias_of: None,
        })
    }

    pub fn identity_digest(&self) -> Result<ProcedureId, Refusal> {
        persistence::identity(
            &self.encoder,
            self.semantic_revision,
            &self.chunk,
            &self.provenance,
            &self.library_dependencies,
        )
    }

    /// The experimental, versioned immutable-subject wire bytes used by the
    /// identity digest. This is not a language-wide canonical encoding.
    pub fn identity_subject_bytes(&self) -> Result<Vec<u8>, Refusal> {
        persistence::identity_bytes(self)
    }

    /// Recompute the public retrieval projection after a state-only edit. This
    /// does not alter content identity or make the edited record admissible.
    pub fn refresh_retrieval_signature(&mut self) {
        self.retrieval = retrieval::signature(self);
    }

    pub fn source_revision(&self) -> &[u8; chunks::REVISION_BYTES] {
        &self.chunk.source_revision
    }

    /// Verification receipts the library minted from its owner's verifier.
    /// Only these, never the claims in `evidence`, support `tested` and
    /// `promoted`. Reading them never runs a verifier.
    pub fn verified_evidence(&self) -> &[VerifiedEvidence] {
        &self.verified
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefusalKind {
    Invalid,
    Unsupported,
    Exhausted,
    Internal,
    DuplicateEntry,
    LiveCapabilityInEvidence,
    UnsupportedEvidenceClass,
    ForgedStatus,
    StaleContext,
    UnresolvedImplication,
    UnknownRecord,
    UnknownGeneration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub kind: RefusalKind,
    pub location: chunks::Location,
    pub message: String,
    pub companion: Option<crate::companion::Refusal>,
}

#[derive(Clone, Copy)]
// Extraction-only name: the crate-root `Diagnostic` is printed unqualified in
// Lean, where it would resolve to this type inside `source.procedures`.
#[charon::rename("ProcedureDiagnostic")]
struct Diagnostic<'a> {
    field: &'a str,
    message: &'a str,
}

impl Refusal {
    fn new(kind: RefusalKind, diagnostic: Diagnostic<'_>) -> Self {
        Self {
            kind,
            location: chunks::Location::field(diagnostic.field),
            message: String::from(diagnostic.message),
            companion: None,
        }
    }

    fn at(kind: RefusalKind, diagnostic: Diagnostic<'_>, index: u32) -> Self {
        Self {
            kind,
            location: chunks::Location::at(diagnostic.field, index),
            message: String::from(diagnostic.message),
            companion: None,
        }
    }

    fn from_chunk(refusal: chunks::Refusal) -> Self {
        let kind = match refusal.class {
            chunks::Class::Invalid => RefusalKind::Invalid,
            chunks::Class::Unsupported => RefusalKind::Unsupported,
            chunks::Class::Exhausted => RefusalKind::Exhausted,
            chunks::Class::Internal => RefusalKind::Internal,
        };
        Self {
            kind,
            location: refusal.location,
            message: refusal.message,
            companion: None,
        }
    }

    fn from_companion(refusal: crate::companion::Refusal) -> Self {
        let kind = match refusal {
            crate::companion::Refusal::UnresolvedImplication => RefusalKind::UnresolvedImplication,
            crate::companion::Refusal::StaleContext => RefusalKind::StaleContext,
            crate::companion::Refusal::UnsupportedEvidenceClass => {
                RefusalKind::UnsupportedEvidenceClass
            }
            crate::companion::Refusal::LiveCapabilityInEvidence => {
                RefusalKind::LiveCapabilityInEvidence
            }
            crate::companion::Refusal::ForgedStatus => RefusalKind::ForgedStatus,
            crate::companion::Refusal::DuplicateEntry => RefusalKind::DuplicateEntry,
            crate::companion::Refusal::ExhaustedRegistry
            | crate::companion::Refusal::ExhaustedReplay => RefusalKind::Exhausted,
            crate::companion::Refusal::UnknownRule
            | crate::companion::Refusal::UnsupportedRuleset
            | crate::companion::Refusal::UnsupportedSemanticRevision
            | crate::companion::Refusal::CyclicDerivation
            | crate::companion::Refusal::DuplicatePremise
            | crate::companion::Refusal::MissingPremise
            | crate::companion::Refusal::WrongPremiseClass
            | crate::companion::Refusal::MismatchedSubject
            | crate::companion::Refusal::MismatchedClaim
            | crate::companion::Refusal::MismatchedContext
            | crate::companion::Refusal::WrongInstantiation
            | crate::companion::Refusal::UnknownContract
            | crate::companion::Refusal::UnknownEvidence
            | crate::companion::Refusal::UnsupportedGuardTemplate
            | crate::companion::Refusal::IneligibleGhost
            | crate::companion::Refusal::InvalidEvidenceEncoding
            | crate::companion::Refusal::CorrespondenceMismatch
            | crate::companion::Refusal::Internal => RefusalKind::Invalid,
        };
        Self {
            kind,
            location: chunks::Location::field("companion"),
            message: String::from("MC2 companion admission refused the composition"),
            companion: Some(refusal),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsumerObligation {
    pub id: u32,
    pub allowed_classes: Vec<EvidenceClass>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsumerPolicy {
    pub revision: u32,
    pub obligations: Vec<ConsumerObligation>,
}

#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationKind {
    Publish,
    Rollback,
}

impl GenerationKind {
    const fn code(self) -> u8 {
        match self {
            Self::Publish => 0,
            Self::Rollback => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureEntry {
    pub procedure_id: ProcedureId,
    pub state_digest: ProcedureId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationRecord {
    pub generation: u32,
    pub parent_generation: Option<u32>,
    pub rollback_of: Option<u32>,
    pub kind: GenerationKind,
    pub chunk_library_revision: [u8; chunks::REVISION_BYTES],
    pub procedure_library_revision: ProcedureId,
    pub closure: Vec<ClosureEntry>,
}

#[derive(Clone, Copy)]
struct GenerationTransition {
    generation: u32,
    parent_generation: Option<u32>,
    rollback_of: Option<u32>,
    kind: GenerationKind,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Snapshot {
    pub(super) record: GenerationRecord,
    pub(super) records: Vec<ProcedureRecord>,
}

#[derive(Debug)]
pub struct ProcedureLibrary {
    pub(super) chunk_revision: [u8; chunks::REVISION_BYTES],
    pub(super) chunk_library: chunks::Library,
    pub(super) generations: Vec<Snapshot>,
    pub(super) current_generation: u32,
    /// The owner's trust root for evidence acceptance; `None` keeps every
    /// claim unverified, so `tested` and `promoted` cannot be reached.
    verifier: Option<acceptance::OwnerVerifier>,
}

impl ProcedureLibrary {
    /// A library without an owner verifier. Every evidence record stays a
    /// visible, unverified claim, so no record can become `tested` or
    /// `promoted`.
    pub fn new(revision: [u8; chunks::REVISION_BYTES]) -> Self {
        let chunk_library = chunks::Library::new(revision);
        let empty_records = Vec::new();
        let record = GenerationRecord {
            generation: 0,
            parent_generation: None,
            rollback_of: None,
            kind: GenerationKind::Publish,
            chunk_library_revision: *chunk_library.revision(),
            procedure_library_revision: persistence::empty_procedure_revision(),
            closure: Vec::new(),
        };
        Self {
            chunk_revision: revision,
            chunk_library,
            generations: alloc::vec![Snapshot {
                record,
                records: empty_records,
            }],
            current_generation: 0,
            verifier: None,
        }
    }

    /// A library whose owner supplies the verifier that alone decides which
    /// claims become verification receipts at admission. Records and their
    /// submitters cannot select or replace it.
    ///
    /// Reviewed extraction exclusion (#97): the signature names
    /// `dyn EvidenceVerifier`, which Aeneas cannot represent; the checked
    /// logic is `ProcedureLibrary::new`.
    #[charon::exclude]
    pub fn with_verifier(
        revision: [u8; chunks::REVISION_BYTES],
        verifier: Box<dyn EvidenceVerifier>,
    ) -> Self {
        let mut library = Self::new(revision);
        library.verifier = Some(acceptance::OwnerVerifier(verifier));
        library
    }

    pub const fn chunk_library_revision(&self) -> &[u8; chunks::REVISION_BYTES] {
        self.chunk_library.revision()
    }

    pub fn procedure_library_revision(&self) -> &ProcedureId {
        &self.current().record.procedure_library_revision
    }

    pub const fn current_generation(&self) -> u32 {
        self.current_generation
    }

    pub fn generation_records(&self) -> Vec<&GenerationRecord> {
        let mut records = Vec::with_capacity(self.generations.len());
        for snapshot in self.generations.iter() {
            records.push(&snapshot.record);
        }
        records
    }

    pub fn records(&self) -> &[ProcedureRecord] {
        &self.current().records
    }

    pub const fn chunk_library(&self) -> &chunks::Library {
        &self.chunk_library
    }

    /// Canonical order of the shared typed-chunk/1 view, projected to the
    /// procedure identities that own those exact admitted envelopes.
    pub fn canonical_chunk_order(&self) -> Result<Vec<ProcedureId>, Refusal> {
        let mut order = Vec::with_capacity(self.chunk_library.entries().len());
        let entry_order = chunks::canonical_entry_order(&self.chunk_library);
        let mut failure = None;
        let mut position = 0;
        while failure.is_none() && position < entry_order.len() {
            match self.chunk_order_owner(entry_order[position]) {
                Ok(procedure_id) => order.push(procedure_id),
                Err(refusal) => failure = Some(refusal),
            }
            position += 1;
        }
        if let Some(refusal) = failure {
            return Err(refusal);
        }
        if order.len() != self.records().len() {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "chunk_order",
                    message: "typed-chunk and procedure generation closures differ in size",
                },
            ));
        }
        Ok(order)
    }

    /// The procedure owning the typed-chunk entry at `entry_index`.
    fn chunk_order_owner(&self, entry_index: u16) -> Result<ProcedureId, Refusal> {
        let entry = self
            .chunk_library
            .entries()
            .get(usize::from(entry_index))
            .ok_or_else(|| {
                Refusal::new(
                    RefusalKind::Invalid,
                    Diagnostic {
                        field: "chunk_order",
                        message: "typed-chunk canonical order names an absent entry",
                    },
                )
            })?;
        let record = crate::scan::find_first(self.records(), |record| {
            record.chunk.chunk_id == entry.envelope().chunk_id
                && record.chunk.source_revision == entry.envelope().source_revision
        })
        .ok_or_else(|| {
            Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "chunk_order",
                    message: "typed-chunk entry has no owning procedure record",
                },
            )
        })?;
        Ok(record.procedure_id)
    }

    pub(super) fn current(&self) -> &Snapshot {
        crate::scan::find_first(&self.generations, |snapshot| {
            snapshot.record.generation == self.current_generation
        })
        .unwrap_or(&self.generations[0])
    }

    pub(super) fn generation(&self, id: u32) -> Option<&Snapshot> {
        crate::scan::find_first(&self.generations, |snapshot| {
            snapshot.record.generation == id
        })
    }

    pub fn admit(
        &mut self,
        namespace: &super::Session,
        policy: Option<&ConsumerPolicy>,
        record: ProcedureRecord,
    ) -> Result<ProcedureId, Refusal> {
        self.admit_state(namespace, policy, record, false)
    }

    /// Admit one producer batch as a single immutable generation. Dependencies
    /// may point forward within the batch; the final closure is topologically
    /// re-admitted before publication.
    pub fn admit_batch(
        &mut self,
        namespace: &super::Session,
        policy: Option<&ConsumerPolicy>,
        mut incoming_records: Vec<ProcedureRecord>,
    ) -> Result<Vec<ProcedureId>, Refusal> {
        if incoming_records.is_empty() {
            return Ok(Vec::new());
        }
        if incoming_records.len() > MAX_RECORDS {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "library.entries",
                    message: "procedure batch exceeds retained record bound",
                },
            ));
        }
        let mut candidate_records = self.current().records.clone();
        let mut identities = Vec::with_capacity(incoming_records.len());
        let mut failure = None;
        let mut index = 0;
        while failure.is_none() && index < incoming_records.len() {
            match self.admit_batch_record(
                policy,
                &mut candidate_records,
                &mut incoming_records[index],
            ) {
                Ok(procedure_id) => identities.push(procedure_id),
                Err(refusal) => failure = Some(refusal),
            }
            index += 1;
        }
        if let Some(refusal) = failure {
            return Err(refusal);
        }
        Self::canonicalize_evidence(&mut candidate_records)?;
        candidate_records.sort_by(|left: &ProcedureRecord, right: &ProcedureRecord| {
            retrieval::compare_records(left, right)
        });
        if candidate_records == self.current().records {
            return Ok(identities);
        }
        let next_generation = self.current_generation.checked_add(1).ok_or_else(|| {
            Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "generation",
                    message: "generation counter overflow",
                },
            )
        })?;
        if self.generations.len() >= MAX_GENERATIONS {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "generations",
                    message: "retained generation limit reached",
                },
            ));
        }
        let chunk_library =
            Self::rebuild_chunks(namespace, self.chunk_revision, &candidate_records)?;
        let snapshot = Self::snapshot(
            GenerationTransition {
                generation: next_generation,
                parent_generation: Some(self.current_generation),
                rollback_of: None,
                kind: GenerationKind::Publish,
            },
            *chunk_library.revision(),
            candidate_records,
        )?;
        let mut candidate_generations = self.generations.clone();
        candidate_generations.push(snapshot);
        let encoded = persistence::encode_parts(
            self.chunk_revision,
            &candidate_generations,
            next_generation,
        )?;
        if encoded.len() > MAX_STORAGE_BYTES {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "library.bytes",
                    message: "append-only procedure generations exceed the retained storage bound",
                },
            ));
        }
        self.chunk_library = chunk_library;
        self.generations = candidate_generations;
        self.current_generation = next_generation;
        Ok(identities)
    }

    /// One record of [`Self::admit_batch`], checked and merged into `candidate_records`.
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-contracts; a newly admitted record is pushed onto the caller's candidate closure, which needs the growable Vec, not a fixed slice"
    )]
    fn admit_batch_record(
        &self,
        policy: Option<&ConsumerPolicy>,
        candidate_records: &mut Vec<ProcedureRecord>,
        record: &mut ProcedureRecord,
    ) -> Result<ProcedureId, Refusal> {
        Self::validate_claims(record)?;
        if record.identity_digest()? != record.procedure_id {
            return Err(Refusal::new(RefusalKind::Invalid, Diagnostic { field: "procedure_id", message: "retained procedure identity differs from recomputed immutable subject fields" }));
        }
        if record.supersedes.is_some() {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "supersedes",
                    message:
                        "replacement must use the atomic replace operation, not batch admission",
                },
            ));
        }
        let existing_position = position_first(candidate_records, |existing| {
            existing.procedure_id == record.procedure_id
        });
        let retained = match existing_position {
            Some(index) => candidate_records.get(index),
            None => None,
        };
        acceptance::accept(self.verifier.as_ref(), record, retained)?;
        Self::validate_record(record, policy, true)?;
        if let Some(existing_index) = existing_position {
            let existing = candidate_records.get(existing_index).ok_or_else(|| {
                Refusal::new(
                    RefusalKind::Internal,
                    Diagnostic {
                        field: "procedure_id",
                        message: "record index disappeared",
                    },
                )
            })?;
            if persistence::identity_bytes(existing)? != persistence::identity_bytes(record)? {
                return Err(Refusal::new(
                    RefusalKind::Invalid,
                    Diagnostic {
                        field: "procedure_id",
                        message: "identity collision between unequal immutable subjects",
                    },
                ));
            }
            if existing.library_dependencies != record.library_dependencies {
                return Err(Refusal::new(
                    RefusalKind::Invalid,
                    Diagnostic {
                        field: "library_dependencies",
                        message: "an existing subject cannot change its dependency closure",
                    },
                ));
            }
            if existing.status == Status::Superseded {
                return Err(Refusal::new(
                    RefusalKind::StaleContext,
                    Diagnostic {
                        field: "status",
                        message: "a superseded record cannot be silently reactivated",
                    },
                ));
            }
            let updated = Self::merge_state(existing, record.clone())?;
            Self::validate_record(&updated, policy, true)?;
            if let Some(slot) = candidate_records.get_mut(existing_index) {
                *slot = updated;
            }
        } else {
            if candidate_records.len() >= MAX_RECORDS {
                return Err(Refusal::new(
                    RefusalKind::Exhausted,
                    Diagnostic {
                        field: "library.entries",
                        message: "procedure record limit reached",
                    },
                ));
            }
            if record.provenance.producer == Producer::Composition
                && crate::scan::find_first(candidate_records, |existing| {
                    existing.provenance.producer == Producer::Composition
                        && existing.library_dependencies == record.library_dependencies
                        && existing.chunk.chunk_id == record.chunk.chunk_id
                })
                .is_some()
            {
                return Err(Refusal::new(
                    RefusalKind::DuplicateEntry,
                    Diagnostic {
                        field: "library_dependencies",
                        message: "composition restates an existing derivation",
                    },
                ));
            }
            candidate_records.push(record.clone());
        }
        Ok(record.procedure_id)
    }

    pub fn replace(
        &mut self,
        namespace: &super::Session,
        policy: Option<&ConsumerPolicy>,
        old_id: ProcedureId,
        mut replacement: ProcedureRecord,
    ) -> Result<ProcedureId, Refusal> {
        if replacement.procedure_id == old_id {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "supersedes",
                    message: "a replacement must have a distinct immutable subject identity",
                },
            ));
        }
        replacement.supersedes = Some(old_id);
        self.admit_state(namespace, policy, replacement, true)
    }

    fn admit_state(
        &mut self,
        namespace: &super::Session,
        policy: Option<&ConsumerPolicy>,
        mut record: ProcedureRecord,
        replacement: bool,
    ) -> Result<ProcedureId, Refusal> {
        Self::validate_claims(&record)?;
        let recomputed = record.identity_digest()?;
        if recomputed != record.procedure_id {
            return Err(Refusal::new(RefusalKind::Invalid, Diagnostic { field: "procedure_id", message: "retained procedure identity differs from recomputed immutable subject fields" }));
        }
        let result_id = record.procedure_id;
        let mut candidate_records = self.current().records.clone();
        let existing_position = position_first(&candidate_records, |existing| {
            existing.procedure_id == record.procedure_id
        });
        let retained = match existing_position {
            Some(index) => candidate_records.get(index),
            None => None,
        };
        acceptance::accept(self.verifier.as_ref(), &mut record, retained)?;
        Self::validate_record(&record, policy, true)?;
        if replacement {
            Self::supersede_existing(&mut candidate_records, record.supersedes)?;
        }
        if let Some(existing_index) = existing_position {
            let existing = candidate_records.get(existing_index).ok_or_else(|| {
                Refusal::new(
                    RefusalKind::Internal,
                    Diagnostic {
                        field: "procedure_id",
                        message: "record index disappeared",
                    },
                )
            })?;
            if persistence::identity_bytes(existing)? != persistence::identity_bytes(&record)? {
                return Err(Refusal::new(
                    RefusalKind::Invalid,
                    Diagnostic {
                        field: "procedure_id",
                        message: "identity collision between unequal immutable subjects",
                    },
                ));
            }
            if existing.library_dependencies != record.library_dependencies {
                return Err(Refusal::new(
                    RefusalKind::Invalid,
                    Diagnostic {
                        field: "library_dependencies",
                        message: "an existing subject cannot change its dependency closure",
                    },
                ));
            }
            if record.supersedes.is_some()
                && record.supersedes != existing.supersedes
                && (!replacement || existing.supersedes.is_some())
            {
                return Err(Refusal::new(
                    RefusalKind::Invalid,
                    Diagnostic {
                        field: "supersedes",
                        message: "retained replacement predecessors cannot change outside atomic replacement",
                    },
                ));
            }
            if existing.status == Status::Superseded && record.status != Status::Superseded {
                return Err(Refusal::new(
                    RefusalKind::StaleContext,
                    Diagnostic {
                        field: "status",
                        message: "a superseded record cannot be silently reactivated",
                    },
                ));
            }
            let updated = Self::merge_state(existing, record)?;
            Self::validate_record(&updated, policy, true)?;
            if *existing == updated {
                return Ok(updated.procedure_id);
            }
            if let Some(slot) = candidate_records.get_mut(existing_index) {
                *slot = updated;
            }
        } else {
            if candidate_records.len() >= MAX_RECORDS {
                return Err(Refusal::new(
                    RefusalKind::Exhausted,
                    Diagnostic {
                        field: "library.entries",
                        message: "procedure record limit reached",
                    },
                ));
            }
            if record.provenance.producer == Producer::Composition
                && crate::scan::find_first(&candidate_records, |existing| {
                    existing.provenance.producer == Producer::Composition
                        && existing.library_dependencies == record.library_dependencies
                        && existing.chunk.chunk_id == record.chunk.chunk_id
                })
                .is_some()
            {
                return Err(Refusal::new(
                    RefusalKind::DuplicateEntry,
                    Diagnostic {
                        field: "library_dependencies",
                        message: "composition restates an existing derivation",
                    },
                ));
            }
            if !replacement && record.supersedes.is_some() {
                return Err(Refusal::new(
                    RefusalKind::Invalid,
                    Diagnostic {
                        field: "supersedes",
                        message: "replacement references require the atomic replace operation",
                    },
                ));
            }
            Self::check_dependencies(&candidate_records, &record)?;
            candidate_records.push(record.clone());
        }
        Self::canonicalize_evidence(&mut candidate_records)?;
        candidate_records.sort_by(|left: &ProcedureRecord, right: &ProcedureRecord| {
            retrieval::compare_records(left, right)
        });
        let next_generation = self.current_generation.checked_add(1).ok_or_else(|| {
            Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "generation",
                    message: "generation counter overflow",
                },
            )
        })?;
        if self.generations.len() >= MAX_GENERATIONS {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "generations",
                    message: "retained generation limit reached",
                },
            ));
        }
        let chunk_library =
            Self::rebuild_chunks(namespace, self.chunk_revision, &candidate_records)?;
        let parent_generation = Some(self.current_generation);
        let snapshot = Self::snapshot(
            GenerationTransition {
                generation: next_generation,
                parent_generation,
                rollback_of: None,
                kind: GenerationKind::Publish,
            },
            *chunk_library.revision(),
            candidate_records,
        )?;
        let mut candidate_generations = self.generations.clone();
        candidate_generations.push(snapshot);
        let encoded = persistence::encode_parts(
            self.chunk_revision,
            &candidate_generations,
            next_generation,
        )?;
        if encoded.len() > MAX_STORAGE_BYTES {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "library.bytes",
                    message: "append-only procedure generations exceed the retained storage bound",
                },
            ));
        }
        self.chunk_library = chunk_library;
        self.generations = candidate_generations;
        self.current_generation = next_generation;
        Ok(result_id)
    }

    fn merge_state(
        existing: &ProcedureRecord,
        mut incoming: ProcedureRecord,
    ) -> Result<ProcedureRecord, Refusal> {
        Self::merge_claims(existing, &mut incoming);
        incoming.status = Self::merged_status(existing.status, incoming.status);
        Self::inherit_promotion_and_supersession(existing, &mut incoming);
        Self::inherit_alias_and_cost(existing, &mut incoming);
        incoming.retrieval = retrieval::signature(&incoming);
        if incoming.evidence.len() > MAX_EVIDENCE {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "evidence",
                    message: "evidence record limit reached",
                },
            ));
        }
        Ok(incoming)
    }

    /// Add the retained claims, and the receipts minted for them, that
    /// `incoming` lacks.
    fn merge_claims(existing: &ProcedureRecord, incoming: &mut ProcedureRecord) {
        for evidence in existing.evidence.iter() {
            if !incoming.evidence.contains(evidence) {
                incoming.evidence.push(evidence.clone());
            }
        }
        // Retained claims keep the receipts the library minted for them.
        for receipt in existing.verified.iter() {
            if !incoming.verified.contains(receipt) {
                incoming.verified.push(receipt.clone());
            }
        }
    }

    /// The merged status: a better-ranked retained status wins unless the
    /// incoming record is superseded, and a retained status never falls back
    /// to candidate.
    fn merged_status(existing: Status, incoming: Status) -> Status {
        let mut status = incoming;
        if existing.rank() < status.rank() && status != Status::Superseded {
            status = existing;
        }
        if status == Status::Candidate && existing != Status::Candidate {
            status = existing;
        }
        status
    }

    /// Keep the retained promotion and supersession link that `incoming` omits.
    fn inherit_promotion_and_supersession(
        existing: &ProcedureRecord,
        incoming: &mut ProcedureRecord,
    ) {
        if incoming.promotion.is_none() {
            incoming.promotion = existing.promotion.clone();
        }
        if incoming.supersedes.is_none() {
            incoming.supersedes = existing.supersedes;
        }
    }

    /// Keep the retained alias claim and cost that `incoming` omits.
    fn inherit_alias_and_cost(existing: &ProcedureRecord, incoming: &mut ProcedureRecord) {
        if incoming.alias_of.is_none() {
            incoming.alias_of.clone_from(&existing.alias_of);
        }
        if incoming.cost == Cost::default() {
            incoming.cost = existing.cost;
        }
    }

    fn canonicalize_evidence(records: &mut [ProcedureRecord]) -> Result<(), Refusal> {
        let mut is_exhausted = false;
        let mut index = 0;
        while !is_exhausted && index < records.len() {
            let record = &mut records[index];
            record
                .evidence
                .sort_by(|left: &EvidenceRecord, right: &EvidenceRecord| {
                    persistence::compare_evidence(left, right)
                });
            record
                .verified
                .sort_by(|left: &VerifiedEvidence, right: &VerifiedEvidence| {
                    acceptance::compare(left, right)
                });
            is_exhausted = record.evidence.len() > MAX_EVIDENCE;
            index += 1;
        }
        if is_exhausted {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "evidence",
                    message: "evidence record limit reached",
                },
            ));
        }
        Ok(())
    }

    /// Shape, bounds, projection and per-claim checks: everything that holds
    /// before the library decides which claims its owner's verifier accepts.
    fn validate_claims(record: &ProcedureRecord) -> Result<(), Refusal> {
        if record.format != RECORD_FORMAT || record.semantic_revision != SEMANTIC_REVISION {
            return Err(Refusal::new(
                RefusalKind::Unsupported,
                Diagnostic {
                    field: "format",
                    message: "unsupported procedure record format or semantic revision",
                },
            ));
        }
        if record.encoder.id != ENCODER_ID
            || record.encoder.encoding_revision != ENCODING_REVISION
            || !record.encoder.experimental
        {
            return Err(Refusal::new(
                RefusalKind::Unsupported,
                Diagnostic {
                    field: "encoder",
                    message: "procedure identity encoder is not the selected experimental revision",
                },
            ));
        }
        if record.chunk.interface.effects != EffSet::empty() {
            return Err(Refusal::new(
                RefusalKind::Unsupported,
                Diagnostic {
                    field: "chunk.interface.effects",
                    message: "procedure effect and capability closure must be empty",
                },
            ));
        }
        if crate::scan::find_first(&record.chunk.interface.stack_in, |ty| {
            !chunks::profile_type(ty)
        })
        .is_some()
            || crate::scan::find_first(&record.chunk.interface.stack_out, |ty| {
                !chunks::profile_type(ty)
            })
            .is_some()
        {
            return Err(Refusal::new(
                RefusalKind::Unsupported,
                Diagnostic {
                    field: "chunk.interface",
                    message: "procedure interface exceeds pure-i64-bool/1",
                },
            ));
        }
        if record.library_dependencies.len() > MAX_LIBRARY_DEPENDENCIES {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "library_dependencies",
                    message: "procedure dependency limit reached",
                },
            ));
        }
        if record.evidence.len() > MAX_EVIDENCE {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "evidence",
                    message: "evidence record limit reached",
                },
            ));
        }
        if record.retrieval != retrieval::signature(record) {
            return Err(Refusal::new(RefusalKind::Invalid, Diagnostic { field: "retrieval", message: "retrieval signature differs from recomputed interface, status and integer cost" }));
        }
        Self::validate_each_claim(record)
    }

    /// Every evidence claim of `record`, in order, up to the first refusal.
    fn validate_each_claim(record: &ProcedureRecord) -> Result<(), Refusal> {
        let mut outcome = Ok(());
        let mut index = 0;
        while outcome.is_ok() && index < record.evidence.len() {
            outcome = Self::validate_claim(record, index);
            index += 1;
        }
        outcome
    }

    /// One evidence claim of [`Self::validate_claims`].
    fn validate_claim(record: &ProcedureRecord, index: usize) -> Result<(), Refusal> {
        let evidence = &record.evidence[index];
        if evidence.capability.is_some() {
            return Err(Refusal::at(RefusalKind::LiveCapabilityInEvidence, Diagnostic { field: "evidence.capability", message: "capability-bearing evidence cannot enter the resource-free procedure profile" }, index as u32));
        }
        if evidence.subject_binding != record.procedure_id {
            return Err(Refusal::at(
                RefusalKind::StaleContext,
                Diagnostic {
                    field: "evidence.subject_binding",
                    message: "evidence is not bound to this immutable procedure subject",
                },
                index as u32,
            ));
        }
        if matches!(
            evidence.outcome,
            EvidenceOutcome::Proved | EvidenceOutcome::Disproved
        ) && !evidence.class.proves()
        {
            return Err(Refusal::at(RefusalKind::ForgedStatus, Diagnostic { field: "evidence.outcome", message: "test, validation, review or assumption evidence cannot claim a proof outcome" }, index as u32));
        }
        if evidence.toolchain.is_empty() || evidence.claim.is_empty() {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "evidence",
                    message: "evidence requires an explicit claim and toolchain identity",
                },
                index as u32,
            ));
        }
        Ok(())
    }

    /// The full record check. Status is decided only by verification
    /// receipts, never by the claims they answer.
    fn validate_record(
        record: &ProcedureRecord,
        policy: Option<&ConsumerPolicy>,
        require_selected_policy: bool,
    ) -> Result<(), Refusal> {
        Self::validate_claims(record)?;
        acceptance::validate(record)?;
        match record.status {
            Status::Candidate => {
                if record.promotion.is_some() {
                    return Err(Refusal::new(
                        RefusalKind::ForgedStatus,
                        Diagnostic {
                            field: "promotion",
                            message: "candidate record cannot carry a promotion decision",
                        },
                    ));
                }
            }
            Status::Tested => {
                if record.promotion.is_some() || !acceptance::has_verified_passing_test(record) {
                    return Err(Refusal::new(RefusalKind::ForgedStatus, Diagnostic { field: "status", message: "tested status requires a passing test outcome verified by the library owner's verifier and no promotion decision" }));
                }
            }
            Status::Promoted => {
                let promotion = record.promotion.as_ref().ok_or_else(|| {
                    Refusal::new(
                        RefusalKind::ForgedStatus,
                        Diagnostic {
                            field: "promotion",
                            message: "promoted status requires a consumer policy decision",
                        },
                    )
                })?;
                if require_selected_policy && policy.is_none() {
                    return Err(Refusal::new(
                        RefusalKind::StaleContext,
                        Diagnostic {
                            field: "promotion.policy_revision",
                            message: "promoted state requires an explicit consumer policy",
                        },
                    ));
                }
                if promotion.obligations.is_empty() {
                    return Err(Refusal::new(
                        RefusalKind::ForgedStatus,
                        Diagnostic {
                            field: "promotion.obligations",
                            message: "promotion must name at least one consumer obligation",
                        },
                    ));
                }
                if let Some(policy) = policy {
                    if policy.revision != promotion.policy_revision {
                        return Err(Refusal::new(
                            RefusalKind::StaleContext,
                            Diagnostic {
                                field: "promotion.policy_revision",
                                message: "consumer policy revision differs from the promoted state",
                            },
                        ));
                    }
                }
                let mut outcome = Ok(());
                let mut index = 0;
                while outcome.is_ok() && index < promotion.obligations.len() {
                    outcome = Self::validate_promoted_obligation(record, promotion, policy, index);
                    index += 1;
                }
                outcome?;
                if let Some(policy) = policy {
                    if crate::scan::find_first(&policy.obligations, |required| {
                        !promotion.obligations.contains(&required.id)
                    })
                    .is_some()
                    {
                        return Err(Refusal::new(
                            RefusalKind::ForgedStatus,
                            Diagnostic {
                                field: "promotion.obligations",
                                message: "promotion omits an obligation demanded by the selected consumer policy",
                            },
                        ));
                    }
                }
            }
            Status::Superseded => {
                if record.promotion.is_some() {
                    return Err(Refusal::new(
                        RefusalKind::ForgedStatus,
                        Diagnostic {
                            field: "promotion",
                            message: "superseded record cannot retain a promotion decision",
                        },
                    ));
                }
            }
        }
        if record
            .supersedes
            .is_some_and(|id| id == record.procedure_id)
        {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "supersedes",
                    message: "a procedure cannot supersede itself",
                },
            ));
        }
        if let Some(alias) = &record.alias_of {
            if alias.procedure_id == record.procedure_id {
                return Err(Refusal::new(
                    RefusalKind::Invalid,
                    Diagnostic {
                        field: "alias_of",
                        message: "alias requires a distinct exact-pair identity",
                    },
                ));
            }
            if matches!(&alias.certificate, Certificate::LegacyDigest(_)) {
                return Err(Refusal::new(
                    RefusalKind::Unsupported,
                    Diagnostic {
                        field: "alias_of",
                        message: "a digest alone is not a verified exact-pair certificate",
                    },
                ));
            }
        }
        Ok(())
    }

    /// One promoted obligation of [`Self::validate_record`]: strictly ordered,
    /// named by the selected policy and proved under it.
    fn validate_promoted_obligation(
        record: &ProcedureRecord,
        promotion: &Promotion,
        policy: Option<&ConsumerPolicy>,
        index: usize,
    ) -> Result<(), Refusal> {
        let obligation = &promotion.obligations[index];
        if index > 0
            && promotion
                .obligations
                .get(index.saturating_sub(1))
                .is_some_and(|previous| previous >= obligation)
        {
            return Err(Refusal::at(
                RefusalKind::ForgedStatus,
                Diagnostic {
                    field: "promotion.obligations",
                    message: "consumer obligations must be strictly increasing",
                },
                index as u32,
            ));
        }
        let policy_obligation = if let Some(policy) = policy {
            Some(
                crate::scan::find_first(&policy.obligations, |candidate| {
                    candidate.id == *obligation
                })
                .ok_or_else(|| {
                    Refusal::at(
                        RefusalKind::StaleContext,
                        Diagnostic {
                            field: "promotion.obligations",
                            message:
                                "promotion obligation is absent from the selected consumer policy",
                        },
                        index as u32,
                    )
                })?,
            )
        } else {
            None
        };
        let is_satisfied =
            acceptance::has_verified_proof(record, promotion, *obligation, policy_obligation);
        if !is_satisfied {
            return Err(Refusal::at(RefusalKind::ForgedStatus, Diagnostic { field: "promotion.obligations", message: "each promoted obligation requires exact-subject proof evidence verified by the library owner's verifier and allowed by policy" }, index as u32));
        }
        Ok(())
    }

    fn check_dependencies(
        records: &[ProcedureRecord],
        record: &ProcedureRecord,
    ) -> Result<(), Refusal> {
        if record.library_dependencies.len() != record.chunk.dependencies.len() {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "library_dependencies",
                    message: "procedure and typed-chunk dependency closures differ in length",
                },
            ));
        }
        let unresolved = position_first(&record.library_dependencies, |dependency| {
            dependency.procedure_id == record.procedure_id
                || crate::scan::find_first(records, |candidate| {
                    candidate.procedure_id == dependency.procedure_id
                        && candidate.chunk.chunk_id == dependency.chunk_id
                        && candidate.chunk.source_revision == dependency.source_revision
                })
                .is_none()
        });
        if let Some(index) = unresolved {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "library_dependencies",
                    message: "library dependency does not resolve to an exact retained record",
                },
                index as u32,
            ));
        }
        let mut mismatched = None;
        let mut index = 0;
        while mismatched.is_none()
            && index < record.chunk.dependencies.len()
            && index < record.library_dependencies.len()
        {
            if !Self::is_edge_matched(
                records,
                &record.chunk.dependencies[index],
                &record.library_dependencies[index],
            ) {
                mismatched = Some(index);
            }
            index += 1;
        }
        if let Some(index) = mismatched {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "library_dependencies",
                    message:
                        "typed-chunk and procedure dependency edges must match in retained order",
                },
                index as u32,
            ));
        }
        Ok(())
    }

    /// Whether a typed-chunk dependency edge and its procedure edge name the
    /// same retained record.
    fn is_edge_matched(
        records: &[ProcedureRecord],
        dependency: &chunks::DefinitionRef,
        procedure: &LibraryDependency,
    ) -> bool {
        let mut identity = [0u8; 8];
        identity.copy_from_slice(&procedure.chunk_id[..8]);
        u64::from_be_bytes(identity) == dependency.identity
            && procedure.source_revision == dependency.source_revision
            && crate::scan::find_first(records, |candidate| {
                candidate.procedure_id == procedure.procedure_id
                    && candidate.chunk.chunk_id == procedure.chunk_id
                    && candidate.chunk.source_revision == procedure.source_revision
                    && candidate.chunk.format == dependency.format
                    && candidate.chunk.semantic_revision == dependency.semantic_revision
            })
            .is_some()
    }

    fn validate_supersession(records: &[ProcedureRecord]) -> Result<(), Refusal> {
        let mut successors = [None; MAX_RECORDS];
        let mut outcome = Ok(());
        let mut index = 0;
        while outcome.is_ok() && index < records.len() {
            outcome = Self::link_successor(records, index, &mut successors);
            index += 1;
        }
        outcome?;
        let mut rooted_records = 0;
        let mut outcome = Ok(());
        let mut index = 0;
        while outcome.is_ok() && index < records.len() {
            match Self::rooted_path(records, &successors, index) {
                Ok(count) => rooted_records += count,
                Err(refusal) => outcome = Err(refusal),
            }
            index += 1;
        }
        outcome?;
        if rooted_records != records.len() {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "supersedes",
                    message: "replacement relation must be acyclic",
                },
            ));
        }
        Ok(())
    }

    /// One predecessor edge of [`Self::validate_supersession`]. Refusals stay
    /// outside its loop so the complete lineage check remains extractable.
    fn link_successor(
        records: &[ProcedureRecord],
        index: usize,
        successors: &mut [Option<usize>],
    ) -> Result<(), Refusal> {
        if let Some(replaced_id) = records[index].supersedes {
            let old_index = position_first(records, |old| {
                old.procedure_id == replaced_id && old.status == Status::Superseded
            })
            .ok_or_else(|| {
                Refusal::at(
                    RefusalKind::Invalid,
                    Diagnostic {
                        field: "supersedes",
                        message: "replacement target is not retained as superseded",
                    },
                    index as u32,
                )
            })?;
            if successors[old_index].replace(index).is_some() {
                return Err(Refusal::at(
                    RefusalKind::Invalid,
                    Diagnostic {
                        field: "supersedes",
                        message: "a replaced subject must have exactly one retained successor",
                    },
                    index as u32,
                ));
            }
        }
        Ok(())
    }

    /// Single predecessor/successor edges make rooted paths disjoint; records
    /// outside those paths belong to a cycle. A non-root contributes zero.
    fn rooted_path(
        records: &[ProcedureRecord],
        successors: &[Option<usize>],
        index: usize,
    ) -> Result<usize, Refusal> {
        let record = &records[index];
        if record.status == Status::Superseded && successors[index].is_none() {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "supersedes",
                    message: "replacement target is not retained as superseded",
                },
                index as u32,
            ));
        }
        let mut count = 0;
        let mut current = record.supersedes.is_none().then_some(index);
        while let Some(link_index) = current {
            count += 1;
            current = successors[link_index];
        }
        Ok(count)
    }

    fn supersede_existing(
        records: &mut [ProcedureRecord],
        old_id: Option<ProcedureId>,
    ) -> Result<(), Refusal> {
        let old_id = old_id.ok_or_else(|| {
            Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "supersedes",
                    message: "replacement has no prior identity",
                },
            )
        })?;
        let old = match crate::scan::find_first_mut(records, |record| record.procedure_id == old_id)
        {
            Some(old) => old,
            None => {
                return Err(Refusal::new(
                    RefusalKind::Invalid,
                    Diagnostic {
                        field: "supersedes",
                        message: "replaced identity is unavailable",
                    },
                ));
            }
        };
        if old.status == Status::Superseded {
            return Err(Refusal::new(
                RefusalKind::StaleContext,
                Diagnostic {
                    field: "supersedes",
                    message: "a superseded record already has a retained successor",
                },
            ));
        }
        old.status = Status::Superseded;
        old.promotion = None;
        old.retrieval = retrieval::signature(old);
        Ok(())
    }

    fn rebuild_chunks(
        namespace: &super::Session,
        revision: [u8; chunks::REVISION_BYTES],
        records: &[ProcedureRecord],
    ) -> Result<chunks::Library, Refusal> {
        if records.len() > MAX_RECORDS {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "library.entries",
                    message: "procedure record limit reached",
                },
            ));
        }
        Self::validate_supersession(records)?;
        let mut outcome = Ok(());
        let mut index = 0;
        while outcome.is_ok() && index < records.len() {
            outcome = Self::check_retained(records, index);
            index += 1;
        }
        outcome?;
        let order = Self::admission_order(records)?;
        let mut library = chunks::Library::new(revision);
        let mut admitted: Vec<ProcedureRecord> = Vec::with_capacity(records.len());
        let mut outcome = Ok(());
        let mut position = 0;
        while outcome.is_ok() && position < order.len() {
            outcome = Self::admit_rebuilt(
                namespace,
                records,
                order[position],
                &mut library,
                &mut admitted,
            );
            position += 1;
        }
        outcome?;
        alias::verify(records)?;
        if library.revision() != &revision {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "chunk_library_revision",
                    message: "typed-chunk library pin changed while rebuilding a generation",
                },
            ));
        }
        Ok(library)
    }

    /// One retained record of [`Self::rebuild_chunks`]: valid and identity-stable.
    fn check_retained(records: &[ProcedureRecord], index: usize) -> Result<(), Refusal> {
        let record = &records[index];
        Self::validate_record(record, None, false).map_err(|mut refusal| {
            refusal.location = chunks::Location::at("records", index as u32);
            refusal
        })?;
        if record.identity_digest()? != record.procedure_id {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "procedure_id",
                    message: "retained procedure identity differs from recomputed immutable fields",
                },
                index as u32,
            ));
        }
        Ok(())
    }

    /// Re-admit the record at `index` of [`Self::rebuild_chunks`] after its dependencies.
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-contracts; each re-admitted record is pushed onto the caller's admitted closure, which needs the growable Vec, not a fixed slice"
    )]
    fn admit_rebuilt(
        namespace: &super::Session,
        records: &[ProcedureRecord],
        index: usize,
        library: &mut chunks::Library,
        admitted: &mut Vec<ProcedureRecord>,
    ) -> Result<(), Refusal> {
        let record = records.get(index).ok_or_else(|| {
            Refusal::new(
                RefusalKind::Internal,
                Diagnostic {
                    field: "library.entries",
                    message: "admission order invalid",
                },
            )
        })?;
        Self::check_dependencies(admitted, record)?;
        library
            .admit(namespace, record.chunk.clone())
            .map_err(|refusal: chunks::Refusal| Refusal::from_chunk(refusal))?;
        admitted.push(record.clone());
        Ok(())
    }

    fn admission_order(records: &[ProcedureRecord]) -> Result<Vec<usize>, Refusal> {
        let mut order = Vec::with_capacity(records.len());
        let mut is_stuck = false;
        while !is_stuck && order.len() < records.len() {
            match Self::next_ready(records, &order) {
                Some(index) => order.push(index),
                None => is_stuck = true,
            }
        }
        if is_stuck {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "library_dependencies",
                    message: "procedure dependency closure is cyclic or names a missing chunk",
                },
            ));
        }
        Ok(order)
    }

    /// The first record outside `order` whose dependencies are all in `order`.
    fn next_ready(records: &[ProcedureRecord], order: &[usize]) -> Option<usize> {
        let mut selected = None;
        let mut candidate_index = 0;
        while selected.is_none() && candidate_index < records.len() {
            if !order.contains(&candidate_index)
                && Self::is_dependencies_ready(records, order, &records[candidate_index])
            {
                selected = Some(candidate_index);
            }
            candidate_index += 1;
        }
        selected
    }

    /// Whether every procedure and typed-chunk dependency of `record` is in `order`.
    fn is_dependencies_ready(
        records: &[ProcedureRecord],
        order: &[usize],
        record: &ProcedureRecord,
    ) -> bool {
        let mut is_ready = true;
        let mut index = 0;
        while is_ready && index < record.library_dependencies.len() {
            let dependency = &record.library_dependencies[index];
            is_ready = Self::any_record(records, |dependency_index, candidate| {
                candidate.procedure_id == dependency.procedure_id
                    && order.contains(&dependency_index)
            });
            index += 1;
        }
        let mut index = 0;
        while is_ready && index < record.chunk.dependencies.len() {
            let dependency = &record.chunk.dependencies[index];
            is_ready = Self::any_record(records, |dependency_index, candidate| {
                order.contains(&dependency_index)
                    && candidate.chunk.chunk_id[..8] == dependency.identity.to_be_bytes()
                    && candidate.chunk.source_revision == dependency.source_revision
            });
            index += 1;
        }
        is_ready
    }

    /// Whether some record satisfies `predicate` with its index, as
    /// `records.iter().enumerate().any(..)`.
    fn any_record(
        records: &[ProcedureRecord],
        mut predicate: impl FnMut(usize, &ProcedureRecord) -> bool,
    ) -> bool {
        let mut is_found = false;
        let mut index = 0;
        while !is_found && index < records.len() {
            is_found = predicate(index, &records[index]);
            index += 1;
        }
        is_found
    }

    fn snapshot(
        transition: GenerationTransition,
        chunk_library_revision: [u8; chunks::REVISION_BYTES],
        records: Vec<ProcedureRecord>,
    ) -> Result<Snapshot, Refusal> {
        let mut closure = Vec::with_capacity(records.len());
        let mut failure = None;
        let mut index = 0;
        while failure.is_none() && index < records.len() {
            let record = &records[index];
            match persistence::state_digest(record) {
                Ok(state_digest) => closure.push(ClosureEntry {
                    procedure_id: record.procedure_id,
                    state_digest,
                }),
                Err(refusal) => failure = Some(refusal),
            }
            index += 1;
        }
        if let Some(refusal) = failure {
            return Err(refusal);
        }
        let procedure_library_revision = persistence::procedure_revision(&records)?;
        Ok(Snapshot {
            record: GenerationRecord {
                generation: transition.generation,
                parent_generation: transition.parent_generation,
                rollback_of: transition.rollback_of,
                kind: transition.kind,
                chunk_library_revision,
                procedure_library_revision,
                closure,
            },
            records,
        })
    }

    pub fn demote(
        &mut self,
        namespace: &super::Session,
        procedure_id: ProcedureId,
    ) -> Result<(), Refusal> {
        let mut record =
            crate::scan::find_first(self.records(), |record| record.procedure_id == procedure_id)
                .cloned()
                .ok_or_else(|| {
                    Refusal::new(
                        RefusalKind::UnknownRecord,
                        Diagnostic {
                            field: "procedure_id",
                            message: "record unavailable",
                        },
                    )
                })?;
        if record.status != Status::Promoted {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "status",
                    message: "only promoted records can be demoted",
                },
            ));
        }
        record.status = if acceptance::has_verified_passing_test(&record) {
            Status::Tested
        } else {
            Status::Candidate
        };
        record.promotion = None;
        record.retrieval = retrieval::signature(&record);
        let mut candidate_records = self.current().records.clone();
        match crate::scan::find_first_mut(&mut candidate_records, |candidate| {
            candidate.procedure_id == procedure_id
        }) {
            Some(slot) => *slot = record,
            None => {
                return Err(Refusal::new(
                    RefusalKind::Internal,
                    Diagnostic {
                        field: "procedure_id",
                        message: "record disappeared",
                    },
                ));
            }
        }
        candidate_records.sort_by(|left: &ProcedureRecord, right: &ProcedureRecord| {
            retrieval::compare_records(left, right)
        });
        let generation = self.current_generation.checked_add(1).ok_or_else(|| {
            Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "generation",
                    message: "generation counter overflow",
                },
            )
        })?;
        self.publish_snapshot(
            namespace,
            candidate_records,
            GenerationTransition {
                generation,
                parent_generation: Some(self.current_generation),
                rollback_of: None,
                kind: GenerationKind::Publish,
            },
        )
    }

    fn publish_snapshot(
        &mut self,
        namespace: &super::Session,
        records: Vec<ProcedureRecord>,
        transition: GenerationTransition,
    ) -> Result<(), Refusal> {
        if self.generations.len() >= MAX_GENERATIONS {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "generations",
                    message: "retained generation limit reached",
                },
            ));
        }
        let chunk_library = Self::rebuild_chunks(namespace, self.chunk_revision, &records)?;
        let snapshot = Self::snapshot(transition, *chunk_library.revision(), records)?;
        let mut candidate_generations = self.generations.clone();
        candidate_generations.push(snapshot);
        let encoded = persistence::encode_parts(
            self.chunk_revision,
            &candidate_generations,
            transition.generation,
        )?;
        if encoded.len() > MAX_STORAGE_BYTES {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "library.bytes",
                    message: "append-only procedure generations exceed the retained storage bound",
                },
            ));
        }
        self.chunk_library = chunk_library;
        self.generations = candidate_generations;
        self.current_generation = transition.generation;
        Ok(())
    }

    pub fn rollback(
        &mut self,
        namespace: &super::Session,
        generation_id: u32,
        available_toolchains: &[String],
    ) -> Result<(), Refusal> {
        let retained = self.generation(generation_id).ok_or_else(|| {
            Refusal::new(
                RefusalKind::UnknownGeneration,
                Diagnostic {
                    field: "generation",
                    message: "generation unavailable",
                },
            )
        })?;
        let records = retained.records.clone();
        Self::verify_snapshot(retained)?;
        let mut outcome = Ok(());
        let mut record_index = 0;
        while outcome.is_ok() && record_index < records.len() {
            outcome = Self::check_rollback_record(
                &records[record_index],
                record_index,
                available_toolchains,
            );
            record_index += 1;
        }
        outcome?;
        let generation = self.current_generation.checked_add(1).ok_or_else(|| {
            Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "generation",
                    message: "generation counter overflow",
                },
            )
        })?;
        self.publish_snapshot(
            namespace,
            records,
            GenerationTransition {
                generation,
                parent_generation: Some(generation_id),
                rollback_of: Some(self.current_generation),
                kind: GenerationKind::Rollback,
            },
        )
    }

    /// One record of [`Self::rollback`]: every toolchain it names is still
    /// available and the closure stays within the record bound.
    fn check_rollback_record(
        record: &ProcedureRecord,
        record_index: usize,
        available_toolchains: &[String],
    ) -> Result<(), Refusal> {
        let mut unavailable = None;
        let mut evidence_index = 0;
        while unavailable.is_none() && evidence_index < record.evidence.len() {
            if crate::scan::find_first(&record.evidence[evidence_index].toolchain, |toolchain| {
                !available_toolchains.contains(toolchain)
            })
            .is_some()
            {
                unavailable = Some(evidence_index);
            }
            evidence_index += 1;
        }
        if let Some(evidence_index) = unavailable {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "evidence.toolchain",
                    message: "rollback refused because an evidence toolchain is unavailable",
                },
                evidence_index as u32,
            ));
        }
        let mut unavailable = None;
        let mut receipt_index = 0;
        while unavailable.is_none() && receipt_index < record.verified.len() {
            if crate::scan::find_first(&record.verified[receipt_index].toolchain, |toolchain| {
                !available_toolchains.contains(toolchain)
            })
            .is_some()
            {
                unavailable = Some(receipt_index);
            }
            receipt_index += 1;
        }
        if let Some(receipt_index) = unavailable {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "verified_evidence.toolchain",
                    message:
                        "rollback refused because a verified evidence toolchain is unavailable",
                },
                receipt_index as u32,
            ));
        }
        if record_index >= MAX_RECORDS {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                Diagnostic {
                    field: "closure",
                    message: "rollback closure exceeds the record bound",
                },
            ));
        }
        Ok(())
    }

    fn verify_snapshot(snapshot: &Snapshot) -> Result<(), Refusal> {
        if snapshot.record.procedure_library_revision
            != persistence::procedure_revision(&snapshot.records)?
        {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "procedure_library_revision",
                    message:
                        "retained procedure revision differs from recomputed generation closure",
                },
            ));
        }
        if snapshot.record.closure.len() != snapshot.records.len() {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "closure",
                    message: "retained generation closure length differs from its records",
                },
            ));
        }
        let mut outcome = Ok(());
        let mut index = 0;
        while outcome.is_ok()
            && index < snapshot.record.closure.len()
            && index < snapshot.records.len()
        {
            outcome = Self::check_closure_entry(
                &snapshot.record.closure[index],
                &snapshot.records[index],
                index,
            );
            index += 1;
        }
        outcome
    }

    /// One generation closure entry of [`Self::verify_snapshot`] against its record.
    fn check_closure_entry(
        entry: &ClosureEntry,
        record: &ProcedureRecord,
        index: usize,
    ) -> Result<(), Refusal> {
        if entry.procedure_id != record.procedure_id {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "closure.procedure_id",
                    message: "retained identity differs from generation closure",
                },
                index as u32,
            ));
        }
        if entry.state_digest != persistence::state_digest(record)? {
            return Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "closure.state_digest",
                    message: "retained mutable state differs from generation closure",
                },
                index as u32,
            ));
        }
        Ok(())
    }
}

/// The index of the first element satisfying `predicate`, as
/// `items.iter().position(..)`: elements are tested in order and none after
/// the first match.
fn position_first<T>(items: &[T], mut predicate: impl FnMut(&T) -> bool) -> Option<usize> {
    let mut index = 0;
    let mut found = None;
    while found.is_none() && index < items.len() {
        if predicate(&items[index]) {
            found = Some(index);
        }
        index += 1;
    }
    found
}
