use super::*;

#[test]
fn retrieved_chunks_reenter_the_typed_checker_and_fixtures() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(super::revision(7));
    let records = vec![
        super::add_seven_record(Producer::Synthesis)
            .map_err(|error| format!("add seven: {error:?}"))?,
        super::times_five_record(Producer::Synthesis)
            .map_err(|error| format!("times five: {error:?}"))?,
        super::add_record(Producer::Synthesis).map_err(|error| format!("add: {error:?}"))?,
    ];
    let ids = library
        .admit_batch(&namespace, None, records)
        .map_err(|error| format!("batch admit: {error:?}"))?;
    assert_eq!(ids.len(), 3);
    assert_eq!(library.current_generation(), 1);

    let mut budget = QueryBudget::new();
    let transforms = library
        .retrieve(
            &Query {
                exact_interface: Some((vec![Ty::I64], vec![Ty::I64])),
                composition_context: None,
                dependency_root: None,
                status: None,
            },
            &mut budget,
        )
        .map_err(|error| format!("retrieve transformations: {error:?}"))?;
    assert_eq!(transforms.candidates.len(), 2);
    let add_query = library
        .retrieve(
            &Query {
                exact_interface: Some((vec![Ty::I64, Ty::I64], vec![Ty::I64])),
                composition_context: None,
                dependency_root: None,
                status: None,
            },
            &mut budget,
        )
        .map_err(|error| format!("retrieve add: {error:?}"))?;
    assert_eq!(add_query.candidates, [ids[2]]);
    assert_eq!(transforms.examined_entries, 3);
    assert_eq!(add_query.examined_entries, 3);
    assert_eq!(budget.queries(), 2);
    assert_eq!(budget.work(), 6);

    let candidate = library
        .compose(
            &namespace,
            &ids,
            &super::context(
                vec![Ty::I64, Ty::I64],
                vec![Ty::I64],
                EffSet::from_ids(&[EffId(17)]),
            ),
        )
        .map_err(|error| format!("compose: {error:?}"))?;
    assert_eq!(candidate.source, b"7 + 5 * + ");
    assert_eq!(candidate.interface.stack_in, [Ty::I64, Ty::I64]);
    assert_eq!(candidate.interface.stack_out, [Ty::I64]);
    assert_eq!(candidate.interface.effects, EffSet::empty());
    assert_eq!(
        library.current_generation(),
        1,
        "composition does not publish or mutate a record"
    );

    let request = Request {
        format: 1,
        semantic_revision: chunks::SEMANTIC_REVISION,
        profile: String::from("pure-i64-bool/1"),
        task: Task {
            stack_in: vec![Ty::I64, Ty::I64],
            stack_out: vec![Ty::I64],
            allowed_effects: EffSet::empty(),
        },
        editor: Candidate {
            format: 1,
            nodes: vec![EditorNode::Hole],
        },
        hole_path: vec![0],
        library_revision: super::revision(7),
        fixtures: Some(FixtureSet {
            id: [0x11; chunks::IDENTITY_BYTES],
            cases: vec![
                Fixture {
                    inputs: vec![Scalar::I64(5), Scalar::I64(-5)],
                    outputs: vec![Scalar::I64(15)],
                },
                Fixture {
                    inputs: vec![Scalar::I64(3_037_000_499), Scalar::I64(3_037_000_499)],
                    outputs: vec![Scalar::I64(18_222_003_029)],
                },
            ],
        }),
        budget: Budget::PROFILE,
    };
    let accepted = chunks::complete(&namespace, library.chunk_library(), &request, || 0);
    assert!(
        matches!(&accepted, Response::Accepted { .. }),
        "typed-chunk checker rejected transfer candidate: {accepted:?}"
    );
    Ok(())
}

#[test]
fn wrong_contract_and_unresolved_mc2_implication_refuse() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(super::revision(7));
    let records = vec![
        super::add_seven_record(Producer::Chunk)
            .map_err(|error| format!("add seven: {error:?}"))?,
        super::times_five_record(Producer::Chunk)
            .map_err(|error| format!("times five: {error:?}"))?,
        super::add_record(Producer::Chunk).map_err(|error| format!("add: {error:?}"))?,
    ];
    let ids = library
        .admit_batch(&namespace, None, records)
        .map_err(|error| format!("admit: {error:?}"))?;
    let wrong_output = library
        .compose(
            &namespace,
            &ids,
            &super::context(vec![Ty::I64, Ty::I64], vec![Ty::Bool], EffSet::empty()),
        )
        .expect_err("wrong consumer contract must refuse");
    assert_eq!(wrong_output.kind, RefusalKind::Invalid);
    assert_eq!(wrong_output.location.field, "composition.interface");

    let left = noble_contracts::companion::Subject {
        identity: noble_contracts::companion::SubjectDigest(1),
        input_signature: 11,
        output_signature: 22,
        captures: Vec::new(),
        events: Vec::new(),
    };
    let right = noble_contracts::companion::Subject {
        identity: noble_contracts::companion::SubjectDigest(2),
        input_signature: 33,
        output_signature: 44,
        captures: Vec::new(),
        events: Vec::new(),
    };
    assert_eq!(
        noble_contracts::companion::Core::compose_subject(&left, &right),
        Err(noble_contracts::companion::Refusal::UnresolvedImplication),
    );
    Ok(())
}

#[test]
fn stored_digest_cannot_self_authorize_companion_evidence() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(super::revision(7));
    let id = library
        .admit(
            &namespace,
            None,
            super::add_seven_record(Producer::Chunk)
                .map_err(|error| format!("record: {error:?}"))?,
        )
        .map_err(|error| format!("admit: {error:?}"))?;
    let core = noble_contracts::companion::Core::new(super::limits());
    let refusal = match library.bind_companion_evidence(
        &namespace,
        &core,
        id,
        noble_contracts::companion::EvidenceId(0),
        super::limits(),
    ) {
        Err(refusal) => refusal,
        Ok(_) => {
            return Err(String::from(
                "unknown core evidence was trusted by a record digest",
            ))
        }
    };
    assert!(refusal.companion.is_some());
    assert!(library
        .records()
        .iter()
        .any(|record| record.procedure_id == id));
    assert_eq!(library.current_generation(), 1);
    Ok(())
}
