#![allow(
    tigerstyle::assertion_density,
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; this composition integration fixture retains the complete MC2 and fresh-re-admission acceptance trace as its test oracle"
)]

use super::super::procedures::support::{
    add_record, add_seven_record, envelope, increment_record, limits, provenance, revision,
    times_five_record,
};
use noble_contracts::source::chunks::{
    self, Budget, DefinitionRef, Fixture, FixtureSet, Node, Request, Response, Scalar, Task,
};
use noble_contracts::source::editor::{Candidate, Node as EditorNode};
use noble_contracts::source::procedures::{
    ComposeContext, LibraryDependency, ProcedureLibrary, ProcedureRecord, Producer, Query,
    QueryBudget, RefusalKind,
};
use noble_contracts::source::Session;
use noble_kernel::types::{EffId, EffSet, Ty};
use std::vec;

const INCREMENT_CONTRACT: &[u8] =
    include_bytes!("../../../../../verification/mc2/contracts/increment.contract");
const INCREMENT_PROOF: &[u8] =
    include_bytes!("../../../../../verification/mc2/contracts/increment.proof.lean");

fn context(stack_in: Vec<Ty>, stack_out: Vec<Ty>, effects: EffSet) -> ComposeContext {
    ComposeContext {
        prefix: Vec::new(),
        suffix: Vec::new(),
        stack_in,
        stack_out,
        allowed_effects: effects,
        limits: limits(),
    }
}

fn proved_increment_evidence(
    core: &mut noble_contracts::companion::Core,
) -> Result<noble_contracts::companion::EvidenceId, String> {
    let prepared = noble_contracts::prepare(INCREMENT_CONTRACT, limits())
        .map_err(|error| format!("prepare increment contract: {error:?}"))?;
    let offer = noble_contracts::companion::EvidenceOffer {
        class: noble_contracts::companion::EvidenceClass::LeanExact,
        refutation: false,
        payload: noble_contracts::companion::EvidencePayload::Declaration(INCREMENT_PROOF.to_vec()),
    };
    let request = core
        .begin_admission(&prepared, offer)
        .map_err(|admission| format!("begin increment evidence: {:?}", admission.refusals))?;
    let admission = core.complete_admission(
        request,
        noble_contracts::companion::CheckObservation::new(
            noble_contracts::export_lean(&prepared),
            INCREMENT_PROOF.to_vec(),
            Ok(noble_contracts::companion::EvidenceClass::LeanExact),
        ),
    );
    if admission.outcome != noble_contracts::companion::Outcome::Proved {
        return Err(format!(
            "increment evidence refused: {:?}",
            admission.refusals
        ));
    }
    admission
        .evidence
        .ok_or_else(|| String::from("proved increment has no evidence identity"))
}

fn definition(record: &ProcedureRecord) -> DefinitionRef {
    let mut identity = [0; 8];
    identity.copy_from_slice(&record.chunk.chunk_id[..8]);
    DefinitionRef {
        identity: u64::from_be_bytes(identity),
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

#[test]
fn certified_path_reenters_mc2_and_retains_ordered_dependencies() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let left = increment_record(Producer::Chunk).map_err(|error| format!("left: {error:?}"))?;
    let right =
        increment_record(Producer::Synthesis).map_err(|error| format!("right: {error:?}"))?;
    let left_id = left.procedure_id;
    let right_id = right.procedure_id;
    library
        .admit_batch(&namespace, None, vec![left.clone(), right.clone()])
        .map_err(|error| format!("admit operands: {error:?}"))?;

    let mut core = noble_contracts::companion::Core::new(limits());
    let evidence = proved_increment_evidence(&mut core)?;
    let mut wrong_library = ProcedureLibrary::new(revision(7));
    let wrong =
        times_five_record(Producer::Chunk).map_err(|error| format!("wrong body: {error:?}"))?;
    let wrong_id = wrong_library
        .admit(&namespace, None, wrong)
        .map_err(|error| format!("admit wrong body: {error:?}"))?;
    let mismatch = match wrong_library.bind_companion_evidence(
        &namespace,
        &core,
        wrong_id,
        evidence,
        limits(),
    ) {
        Err(refusal) => refusal,
        Ok(_) => {
            return Err(String::from(
                "same-interface different body borrowed increment evidence",
            ))
        }
    };
    assert_eq!(
        mismatch.companion,
        Some(noble_contracts::companion::Refusal::MismatchedSubject)
    );
    let left_bound = library
        .bind_companion_evidence(&namespace, &core, left_id, evidence, limits())
        .map_err(|error| format!("bind left evidence: {error:?}"))?;
    let right_bound = library
        .bind_companion_evidence(&namespace, &core, right_id, evidence, limits())
        .map_err(|error| format!("bind right evidence: {error:?}"))?;
    let certified = library
        .compose_certified(
            &namespace,
            &mut core,
            &left_bound,
            &right_bound,
            &context(vec![Ty::I64], vec![Ty::I64], EffSet::empty()),
        )
        .map_err(|error| format!("certified composition: {error:?}"))?;
    assert_eq!(certified.composition.source, b"1 + 1 + ");
    assert_eq!(certified.composition.selected, [left_id, right_id]);
    assert_eq!(
        core.evidence_rule(certified.derivation.evidence)
            .map_err(|error| format!("derivation rule: {error:?}"))?,
        Some(noble_contracts::companion::RuleId::ComposeV1)
    );
    assert_eq!(
        core.claim(certified.derivation.evidence)
            .map_err(|error| format!("derived claim: {error:?}"))?,
        noble_contracts::ClaimTemplate::IncrementBy(2)
    );

    let mut context_with_prefix = context(vec![Ty::I64], vec![Ty::I64], EffSet::empty());
    context_with_prefix.prefix = b"1 + ".to_vec();
    let refusal = library
        .compose_certified(
            &namespace,
            &mut core,
            &left_bound,
            &right_bound,
            &context_with_prefix,
        )
        .expect_err("extra context bytes cannot inherit operand-only MC2 evidence");
    assert_eq!(refusal.kind, RefusalKind::Unsupported);
    assert_eq!(refusal.location.field, "composition.context");

    let mut context_with_suffix = context(vec![Ty::I64], vec![Ty::I64], EffSet::empty());
    context_with_suffix.suffix = b"1 + ".to_vec();
    let refusal = library
        .compose_certified(
            &namespace,
            &mut core,
            &left_bound,
            &right_bound,
            &context_with_suffix,
        )
        .expect_err("suffix bytes cannot inherit operand-only MC2 evidence");
    assert_eq!(refusal.kind, RefusalKind::Unsupported);
    assert_eq!(refusal.location.field, "composition.context");

    let mut chunk = envelope(
        &[Ty::I64],
        &[Ty::I64],
        &certified.composition.source,
        vec![
            Node::Invoke {
                dependency: 0,
                instantiation: 0,
            },
            Node::Invoke {
                dependency: 1,
                instantiation: 0,
            },
        ],
        vec![0, 1],
    );
    chunk.dependencies = vec![definition(&left), definition(&right)];
    chunk.chunk_id = chunks::chunk_identity_of(&chunk);
    let composed = ProcedureRecord::candidate_with_dependencies(
        chunk,
        provenance(Producer::Composition),
        vec![library_dependency(&left), library_dependency(&right)],
    )
    .map_err(|error| format!("composed record: {error:?}"))?;
    let composed_id = library
        .admit(&namespace, None, composed)
        .map_err(|error| format!("admit composed record: {error:?}"))?;
    let retained = library
        .records()
        .iter()
        .find(|record| record.procedure_id == composed_id)
        .ok_or("composed record missing")?;
    assert_eq!(retained.provenance.producer, Producer::Composition);
    assert_eq!(
        retained.library_dependencies,
        [library_dependency(&left), library_dependency(&right)]
    );
    assert_eq!(library.current_generation(), 2);
    Ok(())
}

#[path = "composition/retrieval.rs"]
mod retrieval;
