use super::super::support::{limits, revision};
use noble_contracts::source::procedures::{
    CapabilityMarker, ComposeContext, EvidenceClass, EvidenceOutcome, EvidenceRecord, ProcedureId,
    ProcedureLibrary, ProcedureRecord, Query, QueryBudget, RefusalKind, Status,
};
use noble_contracts::source::Session;
use noble_kernel::types::{EffSet, Ty};
use std::string::String;
use std::vec::Vec;

#[path = "generated/fixtures.rs"]
mod fixtures;
#[path = "generated/query.rs"]
mod query;

const SEEDS: [u64; 4] = [1, 7, 23, 101];
const QUERY_MASKS: u8 = 16;

struct QueryFixture<'a> {
    namespace: &'a Session,
    library: &'a ProcedureLibrary,
    records: &'a [ProcedureRecord],
}

// Walk the record's declared parents, not the retrieval index's reverse-closure algorithm.
fn has_ancestor(records: &[ProcedureRecord], id: ProcedureId, root: ProcedureId) -> bool {
    assert_eq!(records.len(), 5);
    let mut visited = [id; 5];
    let mut filled = 1usize;
    let mut cursor = 0usize;
    while cursor < filled {
        let current = visited[cursor];
        if current == root {
            return true;
        }
        if let Some(record) = records.iter().find(|record| record.procedure_id == current) {
            for edge in &record.library_dependencies {
                if !visited[..filled].contains(&edge.procedure_id) {
                    if let Some(slot) = visited.get_mut(filled) {
                        *slot = edge.procedure_id;
                        filled = filled.saturating_add(1);
                    }
                }
            }
        }
        assert!(filled <= records.len());
        cursor = cursor.saturating_add(1);
    }
    false
}

// Enumerate the only concrete context tails emitted by this generator, including the empty tail.
fn fits_context(record: &ProcedureRecord, before: &[Ty], after: &[Ty]) -> bool {
    [&[][..], &[Ty::Bool][..], &[Ty::I64][..]]
        .into_iter()
        .any(|tail| {
            before.starts_with(tail)
                && after.starts_with(tail)
                && before.get(tail.len()..) == Some(record.chunk.interface.stack_in.as_slice())
                && after.get(tail.len()..) == Some(record.chunk.interface.stack_out.as_slice())
        })
}

fn expected_ids(records: &[ProcedureRecord], query: &Query) -> Vec<ProcedureId> {
    records
        .iter()
        .filter(|record| {
            query.status.is_none_or(|status| record.status == status)
                && query
                    .exact_interface
                    .as_ref()
                    .is_none_or(|(input, output)| {
                        record.chunk.interface.stack_in == *input
                            && record.chunk.interface.stack_out == *output
                    })
                && query
                    .composition_context
                    .as_ref()
                    .is_none_or(|(before, after)| fits_context(record, before, after))
                && query
                    .dependency_root
                    .is_none_or(|root| has_ancestor(records, record.procedure_id, root))
        })
        .map(|record| record.procedure_id)
        .collect()
}

fn readmit_and_exercise(
    namespace: &Session,
    original: &[ProcedureRecord],
    selected: &[ProcedureId],
) -> Result<(), String> {
    if selected.is_empty() {
        return Ok(());
    }
    let mut fresh = ProcedureLibrary::new(revision(7));
    fresh
        .admit_batch(namespace, None, original.to_vec())
        .map_err(|error| format!("fresh ordinary chunk/source/kernel admission: {error:?}"))?;
    assert_eq!(fresh.records().len(), original.len());
    assert_eq!(fresh.current_generation(), 1);
    for id in selected {
        exercise_selected(namespace, &fresh, *id)?;
    }
    Ok(())
}

fn exercise_selected(
    namespace: &Session,
    library: &ProcedureLibrary,
    id: ProcedureId,
) -> Result<(), String> {
    let record = library
        .records()
        .iter()
        .find(|record| record.procedure_id == id)
        .ok_or_else(|| String::from("retrieved identity absent after fresh admission"))?;
    let composed = library
        .compose(
            namespace,
            &[id],
            &ComposeContext {
                prefix: Vec::new(),
                suffix: Vec::new(),
                stack_in: record.chunk.interface.stack_in.clone(),
                stack_out: record.chunk.interface.stack_out.clone(),
                allowed_effects: EffSet::empty(),
                limits: limits(),
            },
        )
        .map_err(|error| format!("ordinary composed-source/kernel re-admission: {error:?}"))?;
    assert_eq!(composed.selected, [id]);
    assert_eq!(
        composed.interface.stack_out,
        record.chunk.interface.stack_out
    );
    assert_eq!(composed.interface.effects, EffSet::empty());
    Ok(())
}

fn check_generated_query(
    fixture: &QueryFixture<'_>,
    budget: &mut QueryBudget,
    state: &mut u64,
    seed: u64,
    mask: u8,
) -> Result<(bool, bool, bool, Option<ProcedureId>), String> {
    let query = query::sample(mask, state, fixture.records);
    let result = fixture
        .library
        .retrieve(&query, budget)
        .map_err(|error| format!("seed {seed}, mask {mask}: {error:?}"))?;
    let expected = expected_ids(fixture.records, &query);
    assert_eq!(result.candidates, expected, "seed {seed}, mask {mask}");
    assert_eq!(result.examined_entries, 5);
    assert_eq!(result.index_bytes, 160);
    let is_nonempty = !expected.is_empty();
    let is_tested = expected.iter().any(|id| {
        fixture
            .records
            .iter()
            .any(|record| record.procedure_id == *id && record.status == Status::Tested)
    });
    readmit_and_exercise(fixture.namespace, fixture.records, &result.candidates)?;
    Ok((
        is_nonempty,
        expected.is_empty(),
        is_tested,
        (mask == 0)
            .then(|| result.candidates.first().copied())
            .flatten(),
    ))
}

#[test]
fn generated_queries_return_only_ids_and_require_readmission() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let fixture = fixtures::records()?;
    library
        .admit_batch(&namespace, None, fixture)
        .map_err(|error| format!("initial admission: {error:?}"))?;
    let original = library.records().to_vec();
    assert_eq!(original.len(), 5);
    let fixture = QueryFixture {
        namespace: &namespace,
        library: &library,
        records: &original,
    };
    let mut is_nonempty = false;
    let mut is_empty = false;
    let mut is_tested = false;
    let mut retrieved_identity = None;
    let all_query = Query {
        exact_interface: None,
        composition_context: None,
        dependency_root: None,
        status: None,
    };
    for seed in SEEDS {
        let mut state = seed;
        let mut budget = QueryBudget::new();
        for mask in 0..QUERY_MASKS {
            let (has_matches, has_empty, has_tested, selected) =
                check_generated_query(&fixture, &mut budget, &mut state, seed, mask)?;
            is_nonempty |= has_matches;
            is_empty |= has_empty;
            is_tested |= has_tested;
            if selected.is_some() {
                retrieved_identity = selected;
            }
        }
        for _ in 0..QUERY_MASKS {
            let all = library
                .retrieve(&all_query, &mut budget)
                .map_err(|error| format!("seed {seed}, budget fill: {error:?}"))?;
            assert_eq!(all.candidates.len(), original.len());
        }
        assert_eq!(budget.queries(), 32);
        assert!(budget.work() <= 4_096);
        let work = budget.work();
        let refused = library
            .retrieve(&all_query, &mut budget)
            .expect_err("33rd query on a nonempty library must refuse");
        assert_eq!(refused.kind, RefusalKind::Exhausted);
        assert_eq!(budget.queries(), 32);
        assert_eq!(budget.work(), work);
    }
    assert!(is_nonempty);
    assert!(is_empty);
    assert!(is_tested);
    assert_eq!(library.current_generation(), 1);
    assert_eq!(
        library.records(),
        original,
        "retrieval grants no status or authority"
    );

    // The returned ID is not proof evidence: even a correctly bound reference
    // cannot smuggle a live capability through ordinary admission.
    let retrieved_identity =
        retrieved_identity.ok_or_else(|| String::from("unfiltered query returned no identity"))?;
    let mut forged = original
        .iter()
        .find(|record| record.procedure_id == retrieved_identity)
        .ok_or_else(|| String::from("retrieved identity absent from records"))?
        .clone();
    forged.evidence.push(EvidenceRecord {
        obligation: 5,
        class: EvidenceClass::Lean,
        outcome: EvidenceOutcome::Proved,
        toolchain: vec![String::from("untrusted-proof-reference")],
        claim: String::from("not attested by an identity-only retrieval"),
        subject_binding: forged.procedure_id,
        policy_revision: 1,
        capability: Some(CapabilityMarker::LiveCapability),
    });
    let refusal = library
        .admit(&namespace, None, forged)
        .expect_err("retrieval identity must not authorize a forged capability");
    assert_eq!(refusal.kind, RefusalKind::LiveCapabilityInEvidence);
    assert_eq!(library.records(), original);
    Ok(())
}
