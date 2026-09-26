use super::*;

#[test]
fn preflight_rejects_insufficient_quota_or_nonempty_profile(
) -> Result<(), noble_syndicate::choreography::Error> {
    for limits in [
        noble_kernel::dataspace::Limits {
            facets: 1,
            ..noble_syndicate::SELECTED_LIMITS
        },
        noble_kernel::dataspace::Limits {
            assertions: 0,
            ..noble_syndicate::SELECTED_LIMITS
        },
        noble_kernel::dataspace::Limits {
            interests: 1,
            ..noble_syndicate::SELECTED_LIMITS
        },
        noble_kernel::dataspace::Limits {
            events: 3,
            ..noble_syndicate::SELECTED_LIMITS
        },
    ] {
        if let Ok(profile) = noble_syndicate::Profile::new(limits) {
            assert_eq!(
                noble_syndicate::choreography::Session::admit(
                    profile.clone(),
                    TWO,
                    noble_syndicate::choreography::WitBinding::SERVICE
                )
                .err(),
                Some(noble_syndicate::choreography::Error::Capacity)
            );
            assert_eq!(profile.counts()?, (0, 0, 0));
        }
    }
    let profile = noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)?;
    let _existing = profile.admit_participant(noble_syndicate::AdmissionRequest {
        shared_mutable_guest_memory: false,
        rights: noble_kernel::dataspace::Rights {
            publish: true,
            observe: false,
        },
    })?;
    assert_eq!(
        noble_syndicate::choreography::Session::admit(
            profile.clone(),
            b"not json",
            noble_syndicate::choreography::WitBinding::SERVICE
        )
        .err(),
        Some(noble_syndicate::choreography::Error::Nonempty)
    );
    assert_eq!(
        noble_syndicate::choreography::Session::admit(
            profile,
            TWO,
            noble_syndicate::choreography::WitBinding::SERVICE
        )
        .err(),
        Some(noble_syndicate::choreography::Error::Nonempty)
    );
    Ok(())
}

#[test]
fn one_round_still_reserves_the_full_session_quota(
) -> Result<(), noble_syndicate::choreography::Error> {
    let one = br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"a","ready":false,"exit":"withdraw"}]}"#;
    for limits in [
        noble_kernel::dataspace::Limits {
            facets: 2,
            assertions: 1,
            interests: 1,
            events: 4,
        },
        noble_kernel::dataspace::Limits {
            facets: 2,
            assertions: 1,
            interests: 2,
            events: 2,
        },
    ] {
        let profile = noble_syndicate::Profile::new(limits)?;
        assert_eq!(
            noble_syndicate::choreography::Session::admit(
                profile.clone(),
                one,
                noble_syndicate::choreography::WitBinding::SERVICE
            )
            .err(),
            Some(noble_syndicate::choreography::Error::Capacity)
        );
        assert_eq!(profile.counts()?, (0, 0, 0));
        assert!(profile.events()?.is_empty());
    }
    let profile = noble_syndicate::Profile::new(noble_kernel::dataspace::Limits {
        facets: 2,
        assertions: 1,
        interests: 2,
        events: 4,
    })?;
    let session = noble_syndicate::choreography::Session::admit(
        profile.clone(),
        one,
        noble_syndicate::choreography::WitBinding::SERVICE,
    )?;
    assert_eq!(session.snapshot()?.counts, (2, 0, 0));
    drop(session);
    assert_eq!(profile.counts()?, (0, 0, 0));
    Ok(())
}

#[test]
fn foreign_token_and_wrong_export_do_not_start_a_guest_call(
) -> Result<(), noble_syndicate::choreography::Error> {
    let one = noble_syndicate::choreography::Session::admit(
        noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)?,
        TWO,
        noble_syndicate::choreography::WitBinding::SERVICE,
    )?;
    let two = noble_syndicate::choreography::Session::admit(
        noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)?,
        TWO,
        noble_syndicate::choreography::WitBinding::SERVICE,
    )?;
    let foreign = two.token(noble_syndicate::choreography::Role::Subscriber)?;
    let subscriber = one.token(noble_syndicate::choreography::Role::Subscriber)?;
    let baseline = one.snapshot()?;
    assert_eq!(
        one.begin(
            &foreign,
            noble_syndicate::choreography::Export::Observer,
            "clock",
            Some(false)
        ),
        Err(noble_syndicate::choreography::Error::Denied)
    );
    assert_eq!(
        one.begin(
            &subscriber,
            noble_syndicate::choreography::Export::Publisher,
            "clock",
            Some(false)
        ),
        Err(noble_syndicate::choreography::Error::Denied)
    );
    assert_eq!(one.snapshot()?, baseline);
    let mut wrong = noble_syndicate::choreography::WitBinding::SERVICE;
    wrong.exports[3] = "publish-and-trap(string)->bool";
    let profile = noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)?;
    assert_eq!(
        noble_syndicate::choreography::Session::admit(profile.clone(), TWO, wrong).err(),
        Some(noble_syndicate::choreography::Error::Binding)
    );
    assert_eq!(profile.counts()?, (0, 0, 0));
    Ok(())
}

#[test]
fn retired_facets_still_consume_preflight_capacity(
) -> Result<(), noble_syndicate::choreography::Error> {
    let profile = noble_syndicate::Profile::new(noble_kernel::dataspace::Limits {
        facets: 2,
        ..noble_syndicate::SELECTED_LIMITS
    })?;
    let spent = profile.admit_participant(noble_syndicate::AdmissionRequest {
        shared_mutable_guest_memory: false,
        rights: noble_kernel::dataspace::Rights {
            publish: true,
            observe: false,
        },
    })?;
    drop(spent);
    assert_eq!(profile.counts()?, (0, 0, 0));
    assert_eq!(
        noble_syndicate::choreography::Session::admit(
            profile.clone(),
            TWO,
            noble_syndicate::choreography::WitBinding::SERVICE
        )
        .err(),
        Some(noble_syndicate::choreography::Error::Capacity)
    );
    assert_eq!(profile.counts()?, (0, 0, 0));
    Ok(())
}
