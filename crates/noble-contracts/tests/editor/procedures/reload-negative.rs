#![allow(
    tigerstyle::assertion_density,
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; malformed persistence controls keep mutation, reload refusal, and unchanged-generation assertions together as one parser acceptance trace"
)]

use super::super::procedures::support::{envelope, literal_record, provenance, revision};
use noble_contracts::source::chunks::{self, DefinitionRef, Node, Op};
use noble_contracts::source::procedures::{
    EvidenceClass, EvidenceOutcome, EvidenceRecord, LibraryDependency, ProcedureId,
    ProcedureLibrary, ProcedureRecord, Producer, RefusalKind, MAX_GENERATIONS, MAX_RECORDS,
};
use noble_contracts::source::Session;
use noble_kernel::types::Ty;
use std::vec;

const U32_FIELD_BYTES: usize = 4;
const I64_LAST_BYTE_OFFSET: usize = 7;
const ENCODER_REVISION_OFFSET_BYTES: usize = 3;
const ENCODER_STATE_BYTES: usize = 5;
const REVISION_PAIR_BYTES: usize = 48;
const LIBRARY_HEADER_BYTES: usize = 48;
const GENERATION_STATE_BYTES: usize = 65;
const PROCEDURE_STATE_BYTES: usize = 17;
const DEPENDENCY_EDGE_BYTES: usize = 80;
/// Obligation, class, outcome, subject, policy revision and claim digest.
const RECEIPT_HEAD_BYTES: usize = 74;
/// Command and input digests.
const RECEIPT_BINDING_BYTES: usize = 64;

#[derive(Clone, Copy, Debug)]
struct RecordOffsets {
    procedure_id_offset_bytes: usize,
    body_i64_byte_offset: Option<usize>,
    dependencies: (usize, usize, usize),
    evidence: (usize, usize, usize),
    state: (usize, usize),
    supersedes: (usize, usize),
}

#[derive(Clone, Copy, Debug)]
struct Cursor<'a> {
    bytes: &'a [u8],
    byte_offset_bytes: usize,
}

impl Cursor<'_> {
    fn skip(&mut self, length_bytes: usize) -> Result<(), String> {
        self.byte_offset_bytes = self
            .byte_offset_bytes
            .checked_add(length_bytes)
            .filter(|end_byte_offset| *end_byte_offset <= self.bytes.len())
            .ok_or_else(|| {
                format!(
                    "truncated test encoding at {} by {length_bytes} of {}",
                    self.byte_offset_bytes,
                    self.bytes.len()
                )
            })?;
        Ok(())
    }

    fn byte(&mut self) -> Result<u8, String> {
        let value = *self
            .bytes
            .get(self.byte_offset_bytes)
            .ok_or("truncated test byte")?;
        self.skip(1)?;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, String> {
        let end_byte_offset_bytes = self
            .byte_offset_bytes
            .checked_add(U32_FIELD_BYTES)
            .ok_or("test offset overflow")?;
        let bytes: [u8; 4] = self
            .bytes
            .get(self.byte_offset_bytes..end_byte_offset_bytes)
            .ok_or("truncated test u32")?
            .try_into()
            .map_err(|_| "invalid u32")?;
        self.byte_offset_bytes = end_byte_offset_bytes;
        Ok(u32::from_be_bytes(bytes))
    }

    fn count(&mut self) -> Result<usize, String> {
        let value = self.u32()?;
        usize::try_from(value).map_err(|_| String::from("test count exceeds host index"))
    }

    fn sized(&mut self) -> Result<(), String> {
        let length_bytes = self.count()?;
        self.skip(length_bytes)
    }

    fn types(&mut self) -> Result<(), String> {
        let type_count = self.count()?;
        self.skip(type_count)
    }

    fn record(&mut self) -> Result<RecordOffsets, String> {
        self.skip(8)?;
        self.sized()?;
        self.skip(ENCODER_STATE_BYTES)?;
        let procedure_id_offset_bytes = self.byte_offset_bytes;
        self.skip(32)?;
        self.skip(8)?;
        self.sized()?;
        self.skip(REVISION_PAIR_BYTES)?;
        self.types()?;
        self.types()?;
        self.skip(4)?;
        for _ in 0..self.count()? {
            self.skip(2)?;
            if self.byte()? == 0 {
                self.skip(8)?;
            } else {
                self.skip(1)?;
            }
        }
        for _ in 0..self.count()? {
            self.skip(48)?;
        }
        for _ in 0..self.count()? {
            for _ in 0..self.count()? {
                self.skip(4)?;
            }
        }
        let mut body_i64_byte_offset = None;
        for _ in 0..self.count()? {
            match self.byte()? {
                0 => {
                    body_i64_byte_offset = Some(self.byte_offset_bytes);
                    self.skip(8)?;
                }
                1 => self.skip(1)?,
                2 => self.skip(2)?,
                3 => self.skip(3)?,
                4 => self.skip(4)?,
                5 => {
                    let item_count = self.count()?;
                    self.skip(item_count.saturating_mul(U32_FIELD_BYTES).saturating_add(2))?;
                }
                _ => return Err(String::from("unknown chunk node in test parser")),
            }
        }
        let entry_count = self.count()?;
        self.skip(entry_count.saturating_mul(4))?;
        self.skip(5)?;
        if self.byte()? != 0 {
            self.skip(32)?;
        }
        if self.byte()? != 0 {
            self.skip(4)?;
        }
        let state_start_bytes = self.byte_offset_bytes;
        let dependency_count_offset_bytes = self.byte_offset_bytes;
        let dependency_count = self.count()?;
        let dependency_start_bytes = self.byte_offset_bytes;
        self.skip(dependency_count.saturating_mul(DEPENDENCY_EDGE_BYTES))?;
        let dependency_end_bytes = self.byte_offset_bytes;
        let evidence_count_offset_bytes = self.byte_offset_bytes;
        let evidence_count = self.count()?;
        let evidence_start_bytes = self.byte_offset_bytes;
        for _ in 0..evidence_count {
            self.skip(6)?;
            for _ in 0..self.count()? {
                self.sized()?;
            }
            self.sized()?;
            self.skip(37)?;
        }
        let evidence_end_bytes = self.byte_offset_bytes;
        for _ in 0..self.count()? {
            self.skip(RECEIPT_HEAD_BYTES)?;
            self.sized()?;
            for _ in 0..self.count()? {
                self.sized()?;
            }
            self.skip(RECEIPT_BINDING_BYTES)?;
        }
        self.skip(1)?;
        if self.byte()? != 0 {
            self.skip(4)?;
            for _ in 0..self.count()? {
                self.skip(4)?;
            }
        }
        self.skip(12)?;
        self.types()?;
        self.types()?;
        self.skip(PROCEDURE_STATE_BYTES)?;
        let supersedes_start_bytes = self.byte_offset_bytes;
        if self.byte()? != 0 {
            self.skip(32)?;
        }
        let supersedes_end_bytes = self.byte_offset_bytes;
        if self.byte()? != 0 {
            self.skip(64)?;
        }
        Ok(RecordOffsets {
            procedure_id_offset_bytes,
            body_i64_byte_offset,
            dependencies: (
                dependency_count_offset_bytes,
                dependency_start_bytes,
                dependency_end_bytes,
            ),
            evidence: (
                evidence_count_offset_bytes,
                evidence_start_bytes,
                evidence_end_bytes,
            ),
            state: (state_start_bytes, self.byte_offset_bytes),
            supersedes: (supersedes_start_bytes, supersedes_end_bytes),
        })
    }
}

fn record_offsets(bytes: &[u8]) -> Result<Vec<RecordOffsets>, String> {
    let mut cursor = Cursor {
        bytes,
        byte_offset_bytes: 0,
    };
    cursor.skip(LIBRARY_HEADER_BYTES)?;
    let generation_count = cursor.count()?;
    let maximum_retained_records = MAX_RECORDS
        .checked_mul(MAX_GENERATIONS)
        .ok_or("retained-record test capacity overflow")?;
    let mut output = Vec::with_capacity(maximum_retained_records);
    for _ in 0..generation_count {
        cursor.skip(4)?;
        if cursor.byte()? != 0 {
            cursor.skip(4)?;
        }
        if cursor.byte()? != 0 {
            cursor.skip(4)?;
        }
        cursor.skip(GENERATION_STATE_BYTES)?;
        let closure_count = cursor.count()?;
        cursor.skip(closure_count.saturating_mul(64))?;
        let record_count = cursor.count()?;
        for _ in 0..record_count {
            output.push(cursor.record()?);
        }
    }
    Ok(output)
}

// Test-only wire producer: rebind the version-1 integrity indexes after a
// hostile state edit, so reload must check lineage rather than stale hashes.
fn wire_digest(domain: u64, bytes: &[u8]) -> ProcedureId {
    let mut output = [0; 32];
    for (lane, mask) in output.chunks_exact_mut(8).zip([
        0,
        0x9e37_79b9_7f4a_7c15,
        0x3c6e_f372_fe94_f82a,
        0xdaa6_6d2c_7ddf_743f,
    ]) {
        let prefix = (domain ^ mask)
            .to_le_bytes()
            .into_iter()
            .chain(1u64.to_le_bytes())
            .chain((bytes.len() as u64).to_le_bytes());
        let mut hash = prefix
            .chain(bytes.iter().copied())
            .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
            });
        hash = (hash ^ (hash >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
        hash = (hash ^ (hash >> 33)).wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        lane.copy_from_slice(&(hash ^ (hash >> 33)).to_be_bytes());
    }
    output
}

fn change_predecessor(
    bytes: &mut Vec<u8>,
    id: ProcedureId,
    predecessor: Option<ProcedureId>,
) -> Result<(), String> {
    let offsets = record_offsets(bytes)?
        .into_iter()
        .rev()
        .find(|offsets| {
            bytes.get(offsets.procedure_id_offset_bytes..offsets.procedure_id_offset_bytes + 32)
                == Some(id.as_slice())
        })
        .ok_or("lineage record missing from wire")?;
    let mut edge = Vec::with_capacity(33);
    edge.push(u8::from(predecessor.is_some()));
    if let Some(predecessor) = predecessor {
        edge.extend_from_slice(&predecessor);
    }
    bytes.splice(offsets.supersedes.0..offsets.supersedes.1, edge);
    Ok(())
}

fn rebind_head(bytes: &mut [u8], library: &ProcedureLibrary) -> Result<(), String> {
    let offsets = record_offsets(bytes)?;
    let records = library.records();
    let generations = library.generation_records();
    let head = generations.last().ok_or("head generation missing")?;
    let mut closure = Vec::with_capacity(records.len().saturating_mul(64));
    for ((record, offsets), entry) in records
        .iter()
        .zip(offsets.iter().rev().take(records.len()).rev())
        .zip(&head.closure)
    {
        assert_eq!(
            bytes.get(offsets.procedure_id_offset_bytes..offsets.procedure_id_offset_bytes + 32),
            Some(record.procedure_id.as_slice())
        );
        let state = bytes
            .get(offsets.state.0..offsets.state.1)
            .ok_or("state range missing")?;
        let digest = wire_digest(0x4e_50_52_4f_43_00_00_02, state);
        let old_binding = [entry.procedure_id.as_slice(), entry.state_digest.as_slice()].concat();
        let binding_offset_bytes = bytes
            .windows(old_binding.len())
            .rposition(|window| window == old_binding)
            .ok_or("closure entry missing")?;
        bytes
            .get_mut(binding_offset_bytes + 32..binding_offset_bytes + 64)
            .ok_or("closure digest missing")?
            .copy_from_slice(&digest);
        closure.extend_from_slice(&record.procedure_id);
        closure.extend_from_slice(&digest);
    }
    let revision_offset_bytes = bytes
        .windows(32)
        .rposition(|window| window == library.procedure_library_revision())
        .ok_or("head revision missing")?;
    bytes
        .get_mut(revision_offset_bytes..revision_offset_bytes + 32)
        .ok_or("revision range missing")?
        .copy_from_slice(&wire_digest(0x4e_50_52_4f_43_00_00_03, &closure));
    Ok(())
}

#[test]
fn refuses_rebound_forks_cycles_and_dangling_replacement_links() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let first = library
        .admit(
            &namespace,
            None,
            literal_record(1, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("{error:?}"))?;
    let second = library
        .replace(
            &namespace,
            None,
            first,
            literal_record(2, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("{error:?}"))?;
    let third = library
        .replace(
            &namespace,
            None,
            second,
            literal_record(3, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("{error:?}"))?;
    let independent = library
        .admit(
            &namespace,
            None,
            literal_record(4, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("{error:?}"))?;
    let encoded = library.encode().map_err(|error| format!("{error:?}"))?;
    let mut unchanged = encoded.clone();
    rebind_head(&mut unchanged, &library)?;
    assert_eq!(
        unchanged, encoded,
        "wire producer must reproduce all bindings"
    );
    ProcedureLibrary::load(&unchanged, &namespace)
        .map_err(|error| format!("positive reload: {error:?}"))?;
    for (case, changes) in [
        ("fork", vec![(independent, Some(first))]),
        ("cycle", vec![(first, Some(second)), (third, None)]),
        ("dangling", vec![(third, Some([0xff; 32]))]),
        ("live target", vec![(third, Some(independent))]),
        ("orphan", vec![(third, None)]),
        ("self cycle", vec![(first, Some(first)), (second, None)]),
    ] {
        let mut malformed = encoded.clone();
        for (id, predecessor) in changes {
            change_predecessor(&mut malformed, id, predecessor)?;
        }
        rebind_head(&mut malformed, &library)?;
        let refusal = ProcedureLibrary::load(&malformed, &namespace)
            .expect_err("rebound invalid lineage must refuse");
        assert_eq!(refusal.kind, RefusalKind::Invalid, "{case}: {refusal:?}");
        assert_eq!(refusal.location.field, "supersedes", "{case}: {refusal:?}");
        assert_eq!(library.current_generation(), 4);
        assert_eq!(
            library.encode().map_err(|error| format!("{error:?}"))?,
            encoded
        );
    }
    Ok(())
}

fn evidence(record: &ProcedureRecord) -> EvidenceRecord {
    EvidenceRecord {
        obligation: 1,
        class: EvidenceClass::Test,
        outcome: EvidenceOutcome::Passed,
        toolchain: vec![String::from("test-v1")],
        claim: String::from("retained candidate test"),
        subject_binding: record.procedure_id,
        policy_revision: 0,
        capability: None,
    }
}

fn definition(record: &ProcedureRecord) -> DefinitionRef {
    let mut prefix = [0; 8];
    prefix.copy_from_slice(&record.chunk.chunk_id[..8]);
    DefinitionRef {
        identity: u64::from_be_bytes(prefix),
        format: record.chunk.format,
        semantic_revision: record.chunk.semantic_revision,
        source_revision: record.chunk.source_revision,
    }
}

fn library_dependency(record: &ProcedureRecord) -> LibraryDependency {
    LibraryDependency {
        procedure_id: record.procedure_id,
        chunk_id: record.chunk.chunk_id,
        source_revision: record.chunk.source_revision,
    }
}

#[path = "reload-negative/body-mutation.rs"]
mod body_mutation;
#[path = "reload-negative/dependency-order.rs"]
mod dependency_order;
