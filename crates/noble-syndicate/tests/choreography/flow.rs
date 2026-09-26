use super::*;

fn projected_actions() -> Result<(), noble_syndicate::choreography::Error> {
    let plan = noble_syndicate::choreography::project(TWO)?;
    assert_eq!(plan.global().len(), 8);
    assert_eq!(plan.publisher_steps(), &[1, 3, 6]);
    assert_eq!(plan.subscriber_steps(), &[0, 2, 4, 5, 7]);
    assert_eq!(
        plan.round(0)
            .ok_or(noble_syndicate::choreography::Error::Descriptor)?
            .exit,
        noble_syndicate::choreography::Exit::Withdraw
    );
    assert_eq!(
        plan.round(1)
            .ok_or(noble_syndicate::choreography::Error::Descriptor)?
            .exit,
        noble_syndicate::choreography::Exit::Trap
    );
    assert_eq!(
        plan.global()
            .iter()
            .map(|action| plan.name(action.step()))
            .collect::<Vec<_>>(),
        [
            Some("clock"),
            Some("clock"),
            Some("clock"),
            Some("clock"),
            Some("clock"),
            Some("alarm"),
            Some("alarm"),
            Some("alarm")
        ]
    );

    Ok(())
}

fn clock_publication(
    session: &noble_syndicate::choreography::Session,
    publisher: &noble_syndicate::choreography::ParticipantToken,
    subscriber: &noble_syndicate::choreography::ParticipantToken,
) -> Result<(), noble_syndicate::choreography::Error> {
    begin_import_finish(
        session,
        subscriber,
        (
            noble_syndicate::choreography::Export::Observer,
            "clock",
            Some(false),
        ),
        noble_syndicate::choreography::Import::Observe {
            name: "clock",
            ready: false,
        },
        false,
    )?;
    begin_import_finish(
        session,
        publisher,
        (
            noble_syndicate::choreography::Export::Publisher,
            "clock",
            Some(false),
        ),
        noble_syndicate::choreography::Import::Publish {
            name: "clock",
            ready: false,
        },
        true,
    )?;
    begin_import_finish(
        session,
        subscriber,
        (
            noble_syndicate::choreography::Export::Observer,
            "clock",
            Some(false),
        ),
        noble_syndicate::choreography::Import::Observe {
            name: "clock",
            ready: false,
        },
        true,
    )?;
    let snapshot = session.snapshot()?;
    assert_eq!((snapshot.cursor, snapshot.counts), (3, (2, 1, 1)));
    assert_eq!(snapshot.protected_operations, 3);
    Ok(())
}

fn clock_withdrawal(
    session: &noble_syndicate::choreography::Session,
    publisher: &noble_syndicate::choreography::ParticipantToken,
    subscriber: &noble_syndicate::choreography::ParticipantToken,
) -> Result<(), noble_syndicate::choreography::Error> {
    begin_import_finish(
        session,
        publisher,
        (
            noble_syndicate::choreography::Export::Withdraw,
            "clock",
            None,
        ),
        noble_syndicate::choreography::Import::Retract { name: "clock" },
        true,
    )?;
    begin_import_finish(
        session,
        subscriber,
        (
            noble_syndicate::choreography::Export::Observer,
            "clock",
            Some(false),
        ),
        noble_syndicate::choreography::Import::Observe {
            name: "clock",
            ready: false,
        },
        false,
    )?;
    let snapshot = session.snapshot()?;
    assert_eq!((snapshot.cursor, snapshot.counts), (5, (2, 0, 1)));
    assert_eq!(snapshot.protected_operations, 5);
    Ok(())
}

fn alarm_observation(
    session: &noble_syndicate::choreography::Session,
    subscriber: &noble_syndicate::choreography::ParticipantToken,
) -> Result<(), noble_syndicate::choreography::Error> {
    begin_import_finish(
        session,
        subscriber,
        (
            noble_syndicate::choreography::Export::Observer,
            "alarm",
            Some(true),
        ),
        noble_syndicate::choreography::Import::Observe {
            name: "alarm",
            ready: true,
        },
        false,
    )?;
    let snapshot = session.snapshot()?;
    assert_eq!(snapshot.cursor, 6);
    assert_eq!(snapshot.protected_operations, 6);
    Ok(())
}

fn trapped_publication(
    session: &noble_syndicate::choreography::Session,
    publisher: &noble_syndicate::choreography::ParticipantToken,
) -> Result<(), noble_syndicate::choreography::Error> {
    session.begin(
        publisher,
        noble_syndicate::choreography::Export::PublishAndTrap,
        "alarm",
        Some(true),
    )?;
    assert_eq!(
        session.import(
            publisher,
            noble_syndicate::choreography::Import::Publish {
                name: "alarm",
                ready: true
            }
        ),
        Ok(noble_syndicate::choreography::ImportResult::Boolean(true))
    );
    assert_eq!(
        session.import(
            publisher,
            noble_syndicate::choreography::Import::Fail { value: true }
        ),
        Err(noble_syndicate::choreography::Error::GuestFail)
    );
    session.finish(
        publisher,
        noble_syndicate::choreography::InvocationOutcome::Trapped,
    )?;
    Ok(())
}

fn checked_events(
    session: &noble_syndicate::choreography::Session,
) -> Result<(), noble_syndicate::choreography::Error> {
    let snapshot = session.snapshot()?;
    assert_eq!(snapshot.counts, (1, 0, 2));
    for (index, expected) in [
        (noble_kernel::dataspace::Change::Added, "clock", false),
        (noble_kernel::dataspace::Change::Removed, "clock", false),
        (noble_kernel::dataspace::Change::Added, "alarm", true),
        (noble_kernel::dataspace::Change::Removed, "alarm", true),
    ]
    .into_iter()
    .enumerate()
    {
        let at =
            u32::try_from(index).map_err(|_| noble_syndicate::choreography::Error::Capacity)?;
        let event = session
            .event(at)?
            .ok_or(noble_syndicate::choreography::Error::Descriptor)?;
        assert_eq!(
            (
                event.change,
                event
                    .name
                    .as_str()
                    .ok_or(noble_syndicate::choreography::Error::Descriptor)?,
                event.ready
            ),
            expected
        );
    }
    assert!(session
        .event(
            u32::try_from(snapshot.events_len)
                .map_err(|_| noble_syndicate::choreography::Error::Capacity)?
        )?
        .is_none());
    Ok(())
}

#[test]
fn projected_false_ready_and_terminal_compiled_failure(
) -> Result<(), noble_syndicate::choreography::Error> {
    projected_actions()?;
    let profile = noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)?;
    let external = profile.clone();
    let session = noble_syndicate::choreography::Session::admit(
        profile,
        TWO,
        noble_syndicate::choreography::WitBinding::SERVICE,
    )?;
    let publisher = session.token(noble_syndicate::choreography::Role::Publisher)?;
    let subscriber = session.token(noble_syndicate::choreography::Role::Subscriber)?;
    assert_eq!(
        external
            .admit_participant(noble_syndicate::AdmissionRequest {
                shared_mutable_guest_memory: false,
                rights: noble_kernel::dataspace::Rights {
                    publish: true,
                    observe: true
                },
            })
            .err(),
        Some(noble_syndicate::Error::ChoreographyActive)
    );
    clock_publication(&session, &publisher, &subscriber)?;
    clock_withdrawal(&session, &publisher, &subscriber)?;
    alarm_observation(&session, &subscriber)?;
    trapped_publication(&session, &publisher)?;
    checked_events(&session)?;
    begin_import_finish(
        &session,
        &subscriber,
        (
            noble_syndicate::choreography::Export::Observer,
            "alarm",
            Some(true),
        ),
        noble_syndicate::choreography::Import::Observe {
            name: "alarm",
            ready: true,
        },
        false,
    )?;
    assert!(session.complete()?);
    drop(session);
    assert_eq!(external.counts()?, (0, 0, 0));
    Ok(())
}
