#[test]
fn completion_requires_a_disjoint_complete_input_disposition() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let admitted = crate::support::accepted(table.admit(crate::support::request(), ()))?;
    let before = admitted.decision.record;
    let mut overlapping = crate::support::completion();
    overlapping.inputs.consumed = 3;
    assert_eq!(
        table.complete(
            admitted.callback,
            noble_kernel::async_tasks::Outcome::Success,
            overlapping
        ),
        Err(noble_kernel::async_tasks::Error::InvalidDisposition)
    );
    let mut missing = crate::support::completion();
    missing.inputs.retired = 0;
    assert_eq!(
        table.complete(
            admitted.callback,
            noble_kernel::async_tasks::Outcome::DomainError,
            missing
        ),
        Err(noble_kernel::async_tasks::Error::InvalidDisposition)
    );
    let mut unreserved = crate::support::completion();
    unreserved.produced = 4;
    assert_eq!(
        table.complete(
            admitted.callback,
            noble_kernel::async_tasks::Outcome::Success,
            unreserved
        ),
        Err(noble_kernel::async_tasks::Error::ResultCapacity)
    );
    assert_eq!(table.snapshot(before.handle.slot), Some(before));
    let mut forged = before;
    forged.state = noble_kernel::async_tasks::State::Ready;
    assert_eq!(
        noble_kernel::async_tasks::transition(
            forged,
            before.handle,
            noble_kernel::async_tasks::Event::Inspect
        ),
        Err(noble_kernel::async_tasks::Error::InvalidRecord)
    );
    Ok(())
}

#[test]
fn schema_bound_matrix_requires_every_pair_and_exact_disposition() -> Result<(), String> {
    let initial = noble_kernel::async_tasks::CoverageRow {
        state: noble_kernel::async_tasks::State::Pending,
        event: noble_kernel::async_tasks::EventKind::Inspect,
        rule: Ok(noble_kernel::async_tasks::Rule::Inspect),
    };
    let mut rows = [initial; 75];
    let mut index = 0;
    for state in noble_kernel::async_tasks::STATE_CONSTRUCTORS {
        for event in noble_kernel::async_tasks::EVENT_CONSTRUCTORS {
            rows[index] = noble_kernel::async_tasks::CoverageRow {
                state,
                event,
                rule: noble_kernel::async_tasks::classify(state, event.representative()),
            };
            index += 1;
        }
    }
    let states = noble_kernel::async_tasks::STATE_SCHEMA.as_bytes();
    let events = noble_kernel::async_tasks::EVENT_SCHEMA.as_bytes();
    crate::support::accepted(noble_kernel::async_tasks::validate_coverage(
        states, events, &rows,
    ))?;
    assert_eq!(
        noble_kernel::async_tasks::validate_coverage(states, events, &rows[..74]),
        Err(noble_kernel::async_tasks::CoverageError::MissingPair)
    );
    assert_eq!(
        noble_kernel::async_tasks::validate_coverage(b"old-state-schema", events, &rows),
        Err(noble_kernel::async_tasks::CoverageError::StateSchema)
    );
    assert_eq!(
        noble_kernel::async_tasks::validate_coverage(states, b"old-event-schema", &rows),
        Err(noble_kernel::async_tasks::CoverageError::EventSchema)
    );
    let last = rows[74];
    rows[74] = rows[0];
    assert_eq!(
        noble_kernel::async_tasks::validate_coverage(states, events, &rows),
        Err(noble_kernel::async_tasks::CoverageError::DuplicatePair)
    );
    rows[74] = last;
    rows[0].rule = Err(noble_kernel::async_tasks::Error::Pending);
    assert_eq!(
        noble_kernel::async_tasks::validate_coverage(states, events, &rows),
        Err(noble_kernel::async_tasks::CoverageError::WrongDisposition)
    );
    Ok(())
}
