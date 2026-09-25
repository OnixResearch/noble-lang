const PUBLISHER: noble_kernel::dataspace::Rights = noble_kernel::dataspace::Rights {
    publish: true,
    observe: false,
};
const OBSERVER: noble_kernel::dataspace::Rights = noble_kernel::dataspace::Rights {
    publish: false,
    observe: true,
};

fn request(rights: noble_kernel::dataspace::Rights) -> noble_syndicate::AdmissionRequest {
    noble_syndicate::AdmissionRequest {
        shared_mutable_guest_memory: false,
        rights,
    }
}

#[test]
fn admitted_facets_observe_both_ready_states_and_drop_retires_children(
) -> Result<(), noble_syndicate::Error> {
    let profile = noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)?;
    let publisher = profile.admit_participant(request(PUBLISHER))?;
    let subscriber = profile.admit_participant(request(OBSERVER))?;
    let child = profile.child(&publisher, PUBLISHER)?;
    assert!(!profile.observe(&subscriber, "clock", false)?);
    profile.publish(&publisher, "clock", false)?;
    assert!(profile.observe(&subscriber, "clock", false)?);
    profile.publish(&child, "alarm", true)?;
    let event = profile.events()?;
    assert_eq!(event.len(), 1);
    assert!(!event[0].service.ready);
    drop(publisher);
    assert_eq!(profile.counts()?, (1, 0, 1));
    assert_eq!(
        profile.events()?[1].change,
        noble_kernel::dataspace::Change::Removed
    );
    assert_eq!(
        profile.publish(&child, "alarm", true),
        Err(noble_syndicate::Error::Kernel(
            noble_kernel::dataspace::Error::InvalidFacet
        ))
    );
    drop(child);
    drop(subscriber);
    assert_eq!(profile.counts()?, (0, 0, 0));
    Ok(())
}

#[test]
fn hostile_wire_and_rights_do_not_mint_a_facet_or_publish() -> Result<(), noble_syndicate::Error> {
    let profile = noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)?;
    assert!(matches!(
        profile.admit_participant(noble_syndicate::AdmissionRequest {
            shared_mutable_guest_memory: true,
            rights: PUBLISHER,
        }),
        Err(noble_syndicate::Error::UnsupportedSharedGuestMemory)
    ));
    assert_eq!(profile.counts()?, (0, 0, 0));
    let subscriber = profile.admit_participant(request(OBSERVER))?;
    let forgery = b"<service \"Directory\" #t><resource 7>";
    assert_eq!(
        profile.publish_wire(&subscriber, forgery),
        Err(noble_syndicate::Error::Kernel(
            noble_kernel::dataspace::Error::Schema
        ))
    );
    assert_eq!(
        profile.publish_wire(&subscriber, b"<service \"Directory\" #t>"),
        Err(noble_syndicate::Error::Kernel(
            noble_kernel::dataspace::Error::Denied
        ))
    );
    assert_eq!(profile.counts()?, (1, 0, 0));
    drop(subscriber);
    assert_eq!(profile.counts()?, (0, 0, 0));
    Ok(())
}

#[test]
fn wrong_profile_cannot_reuse_host_issued_facet() -> Result<(), noble_syndicate::Error> {
    let one = noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)?;
    let two = noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)?;
    let subscriber = one.admit_participant(request(OBSERVER))?;
    assert_eq!(
        two.observe(&subscriber, "clock", true),
        Err(noble_syndicate::Error::WrongProfile)
    );
    assert_eq!(two.counts()?, (0, 0, 0));
    Ok(())
}
