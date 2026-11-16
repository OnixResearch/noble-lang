use super::super::super::support::verifier::OwnerLedger;
use super::super::super::support::{
    add_record, add_seven_record, envelope, provenance, times_five_record,
};
use noble_contracts::source::chunks::{self, DefinitionRef, Node};
use noble_contracts::source::procedures::{
    EvidenceClass, EvidenceOutcome, EvidenceRecord, LibraryDependency, ProcedureRecord, Producer,
    Status,
};
use noble_kernel::types::Ty;
use std::string::String;
use std::vec;
use std::vec::Vec;

fn dependent_on(parent: &ProcedureRecord) -> Result<ProcedureRecord, String> {
    let mut chunk = envelope(
        &[Ty::I64],
        &[Ty::I64],
        b"7 + ",
        vec![Node::Invoke {
            dependency: 0,
            instantiation: 0,
        }],
        vec![0],
    );
    let mut prefix = [0; 8];
    prefix.copy_from_slice(&parent.chunk.chunk_id[..8]);
    chunk.dependencies.push(DefinitionRef {
        identity: u64::from_be_bytes(prefix),
        format: parent.chunk.format,
        semantic_revision: parent.chunk.semantic_revision,
        source_revision: parent.chunk.source_revision,
    });
    chunk.chunk_id = chunks::chunk_identity_of(&chunk);
    assert_eq!(chunk.dependencies.len(), 1);
    assert_eq!(chunk.chunk_id, chunks::chunk_identity_of(&chunk));
    ProcedureRecord::candidate_with_dependencies(
        chunk,
        provenance(Producer::Composition),
        vec![LibraryDependency {
            procedure_id: parent.procedure_id,
            chunk_id: parent.chunk.chunk_id,
            source_revision: parent.chunk.source_revision,
        }],
    )
    .map_err(|error| format!("dependent record: {error:?}"))
}

pub(super) fn records() -> Result<Vec<ProcedureRecord>, String> {
    let first = add_seven_record(Producer::Chunk).map_err(|error| format!("first: {error:?}"))?;
    let mut tested =
        times_five_record(Producer::Chunk).map_err(|error| format!("tested: {error:?}"))?;
    tested.evidence.push(EvidenceRecord {
        obligation: 5,
        class: EvidenceClass::Test,
        outcome: EvidenceOutcome::Passed,
        toolchain: vec![String::from("generated-retrieval-fixture")],
        claim: String::from("a fixture test, not a proof"),
        subject_binding: tested.procedure_id,
        policy_revision: 1,
        capability: None,
    });
    tested.status = Status::Tested;
    tested.refresh_retrieval_signature();
    let binary = add_record(Producer::Chunk).map_err(|error| format!("binary: {error:?}"))?;
    let child = dependent_on(&first)?;
    let grandchild = dependent_on(&child)?;
    assert_ne!(first.procedure_id, tested.procedure_id);
    assert_eq!(
        grandchild.library_dependencies[0].procedure_id,
        child.procedure_id
    );
    Ok(vec![first, tested, binary, child, grandchild])
}

/// The library owner executed the fixture test behind every `tested` record.
pub(super) fn owner(records: &[ProcedureRecord]) -> OwnerLedger {
    let mut ledger = OwnerLedger::empty();
    for record in records
        .iter()
        .filter(|record| record.status == Status::Tested)
    {
        for claim in &record.evidence {
            ledger.confirm(claim);
        }
    }
    ledger
}
