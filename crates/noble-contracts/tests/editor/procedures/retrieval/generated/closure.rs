//! A dependency-root query meters every record and every examined edge of
//! the closure it walks.

use super::*;

#[test]
fn dependency_closure_meters_every_record_and_examined_edge() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let fixture = fixtures::records()?;
    let closure = [
        fixture[0].procedure_id,
        fixture[3].procedure_id,
        fixture[4].procedure_id,
    ];
    let owner = fixtures::owner(&fixture);
    let mut library = ProcedureLibrary::with_verifier(revision(7), owner.replica().boxed());
    library
        .admit_batch(&namespace, None, fixture)
        .map_err(|error| format!("initial admission: {error:?}"))?;
    let mut budget = QueryBudget::new();
    let result = library
        .retrieve(
            &Query {
                exact_interface: None,
                composition_context: None,
                dependency_root: Some(closure[0]),
                status: None,
            },
            &mut budget,
        )
        .map_err(|error| format!("dependency root: {error:?}"))?;
    let expected: Vec<ProcedureId> = library
        .records()
        .iter()
        .map(|record| record.procedure_id)
        .filter(|id| closure.contains(id))
        .collect();
    assert_eq!(result.candidates, expected);
    assert_eq!(result.examined_entries, 5);
    // The root, its child and its grandchild each take one closure pass that
    // meters all five records plus the single edge of child and grandchild
    // (3 * 7), then retrieval meters each of the five examined records.
    assert_eq!(budget.work(), 26);
    assert_eq!(budget.queries(), 1);
    Ok(())
}
