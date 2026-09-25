const LIMITS: noble_kernel::dataspace::Limits = noble_kernel::dataspace::Limits {
    facets: 8,
    assertions: 8,
    interests: 8,
    events: 32,
};
const PRODUCER: noble_kernel::dataspace::Rights = noble_kernel::dataspace::Rights {
    publish: true,
    observe: false,
};
const OBSERVER: noble_kernel::dataspace::Rights = noble_kernel::dataspace::Rights {
    publish: false,
    observe: true,
};

#[test]
fn published_service_replaces_ready_and_scope_retirement_retracts_descendants(
) -> Result<(), noble_kernel::dataspace::Error> {
    let mut table = noble_kernel::dataspace::Table::new(LIMITS)?;
    let producer = table.open(None, PRODUCER)?;
    let child = table.open(Some(producer), PRODUCER)?;
    let observer = table.open(None, OBSERVER)?;
    assert!(!table.observe(observer, "clock", true)?);
    assert!(!table.observe(observer, "clock", false)?);
    assert!(table.publish(producer, "clock", true)?);
    assert_eq!(
        table.events()[0].change,
        noble_kernel::dataspace::Change::Added
    );
    assert!(table.publish(producer, "clock", false)?);
    assert_eq!(
        table.events()[1].change,
        noble_kernel::dataspace::Change::Removed
    );
    assert!(table.events()[1].service.ready);
    assert_eq!(
        table.events()[2].change,
        noble_kernel::dataspace::Change::Added
    );
    assert!(!table.events()[2].service.ready);
    table.publish(child, "alarm", true)?;
    table.retire(producer)?;
    assert_eq!(table.counts(), (1, 0, 2));
    assert_eq!(
        table.events()[3].change,
        noble_kernel::dataspace::Change::Removed
    );
    assert_eq!(
        table.publish(child, "alarm", true),
        Err(noble_kernel::dataspace::Error::InvalidFacet)
    );
    assert!(!table.observe(observer, "clock", false)?);
    assert_eq!(
        table.retire(producer),
        Err(noble_kernel::dataspace::Error::InvalidFacet)
    );
    Ok(())
}

#[test]
fn full_event_queue_cannot_block_mandatory_cleanup() -> Result<(), noble_kernel::dataspace::Error> {
    let mut table = noble_kernel::dataspace::Table::new(noble_kernel::dataspace::Limits {
        events: 3,
        ..LIMITS
    })?;
    let publisher = table.open(None, PRODUCER)?;
    let subscriber = table.open(None, OBSERVER)?;
    table.observe(subscriber, "clock", true)?;
    table.publish(publisher, "clock", true)?;
    // One queued Added + one reserved future Removed: a second Added would
    // consume the removal reservation, and must leave state unchanged.
    let next = table.open(None, PRODUCER)?;
    assert_eq!(table.publish(next, "clock", false), Ok(true));
    assert_eq!(table.publish(publisher, "clock", false), Ok(true));
    assert_eq!(table.events().len(), 2);
    assert_eq!(
        table.publish(publisher, "clock", true),
        Err(noble_kernel::dataspace::Error::Capacity)
    );
    table.retire(next)?;
    table.retire(publisher)?;
    assert_eq!(table.counts(), (1, 0, 1));
    assert_eq!(table.events().len(), 2);
    Ok(())
}

#[test]
fn duplicate_assertion_owners_emit_retraction_only_after_last_owner(
) -> Result<(), noble_kernel::dataspace::Error> {
    let mut table = noble_kernel::dataspace::Table::new(LIMITS)?;
    let a = table.open(None, PRODUCER)?;
    let b = table.open(None, PRODUCER)?;
    let watcher = table.open(None, OBSERVER)?;
    table.observe(watcher, "clock", true)?;
    table.publish(a, "clock", true)?;
    table.publish(b, "clock", true)?;
    assert_eq!(table.events().len(), 1);
    table.retire(a)?;
    assert!(table.observe(watcher, "clock", true)?);
    assert_eq!(table.events().len(), 1);
    table.retire(b)?;
    assert_eq!(table.events().len(), 2);
    assert_eq!(
        table.events()[1].change,
        noble_kernel::dataspace::Change::Removed
    );
    Ok(())
}

#[test]
fn duplicate_owners_use_exact_event_capacity_and_preserve_final_removal(
) -> Result<(), noble_kernel::dataspace::Error> {
    let mut table = noble_kernel::dataspace::Table::new(noble_kernel::dataspace::Limits {
        facets: 3,
        assertions: 2,
        interests: 1,
        events: 2,
    })?;
    let first = table.open(None, PRODUCER)?;
    let second = table.open(None, PRODUCER)?;
    let watcher = table.open(None, OBSERVER)?;
    assert!(!table.observe(watcher, "clock", true)?);
    assert!(table.publish(first, "clock", true)?);
    // Both available event slots are accounted for: Added is queued and the
    // eventual last-owner Removed is reserved. A duplicate costs no new slot.
    assert!(table.publish(second, "clock", true)?);
    assert_eq!(table.events().len(), 1);
    table.retire(first)?;
    assert!(table.observe(watcher, "clock", true)?);
    assert_eq!(table.events().len(), 1);
    table.retire(second)?;
    assert_eq!(table.events().len(), 2);
    assert_eq!(
        table.events()[0].change,
        noble_kernel::dataspace::Change::Added
    );
    assert_eq!(
        table.events()[1].change,
        noble_kernel::dataspace::Change::Removed
    );
    Ok(())
}

#[test]
fn hostile_preserves_is_bounded_and_schema_checked_before_typed_value(
) -> Result<(), noble_kernel::dataspace::Error> {
    for (input, error) in [
        (
            &b"<service \"clock\" #x>"[..],
            noble_kernel::dataspace::Error::Schema,
        ),
        (
            &b"<service \"clock\" \"not-a-bool\">"[..],
            noble_kernel::dataspace::Error::Schema,
        ),
        (
            &b"<service \"clock\" #t> "[..],
            noble_kernel::dataspace::Error::Schema,
        ),
        (
            &b"<service \"cl\xffck\" #t>"[..],
            noble_kernel::dataspace::Error::Schema,
        ),
        (
            &b"<service \"clock\" #:7>"[..],
            noble_kernel::dataspace::Error::Schema,
        ),
        (
            &b"<<<<<service \"x\" #t>>>>>"[..],
            noble_kernel::dataspace::Error::DepthLimit,
        ),
    ] {
        assert_eq!(
            noble_kernel::dataspace::decode_service(input, 4),
            Err(error)
        );
    }
    assert_eq!(
        noble_kernel::dataspace::decode_service(&[b'x'; 65], 4),
        Err(noble_kernel::dataspace::Error::ByteLimit)
    );
    for ready in [true, false] {
        let wire = noble_kernel::dataspace::encode_service("clock", ready)?;
        assert_eq!(
            noble_kernel::dataspace::decode_service(wire.as_bytes(), 4)?.ready,
            ready
        );
        assert_eq!(wire.as_bytes().len(), 20);
    }
    assert_eq!(
        noble_kernel::dataspace::decide_publication(Some(true), false),
        noble_kernel::dataspace::Publication::Replaced
    );
    assert!(!noble_kernel::dataspace::permits(
        OBSERVER,
        noble_kernel::dataspace::Operation::Publish
    ));
    Ok(())
}
