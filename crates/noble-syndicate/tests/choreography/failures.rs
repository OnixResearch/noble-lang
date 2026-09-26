use super::*;

fn pre_export_refusals(
    session: &noble_syndicate::choreography::Session,
    publisher: &noble_syndicate::choreography::ParticipantToken,
    subscriber: &noble_syndicate::choreography::ParticipantToken,
) -> Result<(), noble_syndicate::choreography::Error> {
    let before = session.snapshot()?;
    for (token, export, name, ready) in [
        (
            publisher,
            noble_syndicate::choreography::Export::Publisher,
            "clock",
            Some(false),
        ),
        (
            subscriber,
            noble_syndicate::choreography::Export::Observer,
            "clock",
            Some(true),
        ),
        (
            subscriber,
            noble_syndicate::choreography::Export::Observer,
            "alarm",
            Some(true),
        ),
    ] {
        assert_eq!(
            session.begin(token, export, name, ready),
            Err(noble_syndicate::choreography::Error::Denied)
        );
        assert_eq!(session.snapshot()?, before);
    }
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
    Ok(())
}

fn in_flight_refusal(
    session: &noble_syndicate::choreography::Session,
    publisher: &noble_syndicate::choreography::ParticipantToken,
) -> Result<(), noble_syndicate::choreography::Error> {
    session.begin(
        publisher,
        noble_syndicate::choreography::Export::Publisher,
        "clock",
        Some(false),
    )?;
    assert_eq!(
        session.import(
            publisher,
            noble_syndicate::choreography::Import::Publish {
                name: "clock",
                ready: false
            }
        ),
        Ok(noble_syndicate::choreography::ImportResult::Boolean(true))
    );
    let prior = session.snapshot()?;
    assert_eq!(
        session.import(
            publisher,
            noble_syndicate::choreography::Import::Retract { name: "clock" }
        ),
        Err(noble_syndicate::choreography::Error::ImportRefused)
    );
    let rejected = session.snapshot()?;
    assert_eq!(rejected.events_len, prior.events_len);
    assert_eq!(rejected.cursor, prior.cursor);
    assert_eq!(rejected.protected_operations, prior.protected_operations);
    assert_eq!(
        session.finish(
            publisher,
            noble_syndicate::choreography::InvocationOutcome::Trapped
        ),
        Err(noble_syndicate::choreography::Error::Invocation)
    );
    let cleaned = session.snapshot()?;
    assert_eq!(cleaned.cursor, prior.cursor);
    assert_eq!(cleaned.counts, (1, 0, 1));
    assert_eq!(
        session
            .event(
                u32::try_from(cleaned.events_len.saturating_sub(1))
                    .map_err(|_| noble_syndicate::choreography::Error::Capacity)?
            )?
            .ok_or(noble_syndicate::choreography::Error::Descriptor)?
            .change,
        noble_kernel::dataspace::Change::Removed
    );
    assert!(!session.complete()?);
    Ok(())
}

#[test]
fn pre_export_refusal_is_side_effect_free_and_inflight_refusal_retires(
) -> Result<(), noble_syndicate::choreography::Error> {
    let profile = noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)?;
    let session = noble_syndicate::choreography::Session::admit(
        profile,
        TWO,
        noble_syndicate::choreography::WitBinding::SERVICE,
    )?;
    let publisher = session.token(noble_syndicate::choreography::Role::Publisher)?;
    let subscriber = session.token(noble_syndicate::choreography::Role::Subscriber)?;
    pre_export_refusals(&session, &publisher, &subscriber)?;
    in_flight_refusal(&session, &publisher)?;
    Ok(())
}

#[test]
fn missing_fail_after_publication_is_not_a_successful_trap_step(
) -> Result<(), noble_syndicate::choreography::Error> {
    let bytes = br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"alarm","ready":true,"exit":"trap"}]}"#;
    let session = noble_syndicate::choreography::Session::admit(
        noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)?,
        bytes,
        noble_syndicate::choreography::WitBinding::SERVICE,
    )?;
    let subscriber = session.token(noble_syndicate::choreography::Role::Subscriber)?;
    let publisher = session.token(noble_syndicate::choreography::Role::Publisher)?;
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
    session.begin(
        &publisher,
        noble_syndicate::choreography::Export::PublishAndTrap,
        "alarm",
        Some(true),
    )?;
    assert_eq!(
        session.import(
            &publisher,
            noble_syndicate::choreography::Import::Publish {
                name: "alarm",
                ready: true,
            }
        ),
        Ok(noble_syndicate::choreography::ImportResult::Boolean(true))
    );
    assert_eq!(
        session.finish(
            &publisher,
            noble_syndicate::choreography::InvocationOutcome::Trapped
        ),
        Err(noble_syndicate::choreography::Error::Invocation)
    );
    let snapshot = session.snapshot()?;
    assert_eq!((snapshot.cursor, snapshot.counts), (1, (1, 0, 1)));
    assert_eq!(snapshot.events_len, 2);
    assert_eq!(
        session
            .event(0)?
            .ok_or(noble_syndicate::choreography::Error::Descriptor)?
            .change,
        noble_kernel::dataspace::Change::Added
    );
    assert_eq!(
        session
            .event(1)?
            .ok_or(noble_syndicate::choreography::Error::Descriptor)?
            .change,
        noble_kernel::dataspace::Change::Removed
    );
    assert!(!session.complete()?);
    Ok(())
}
