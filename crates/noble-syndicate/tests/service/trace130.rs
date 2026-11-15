//! Selected Profile trace against a hand-derived, independent host fixture.
//! Table's generated histories use the richer set-based reference in the
//! kernel target; these host observations exercise ownership of issued tokens.

fn record(
    profile: &noble_syndicate::Profile,
    operation: &str,
    actual: Result<bool, noble_syndicate::Error>,
    expected: Result<bool, noble_syndicate::Error>,
    counts: (u32, u32, u32),
    queue: &[noble_kernel::dataspace::Event],
) -> Result<(), String> {
    let actual_counts = profile
        .counts()
        .map_err(|error| format!("counts: {error:?}"))?;
    let actual_queue = profile
        .events()
        .map_err(|error| format!("events: {error:?}"))?;
    println!("M7HOST\t{operation}\t{expected:?} {counts:?} {queue:?}\t{actual:?} {actual_counts:?} {actual_queue:?}");
    if actual != expected || actual_counts != counts || actual_queue != queue {
        return Err(format!("{operation}: expected {expected:?} {counts:?} {queue:?}, actual {actual:?} {actual_counts:?} {actual_queue:?}"));
    }
    Ok(())
}

fn request(rights: noble_kernel::dataspace::Rights) -> noble_syndicate::AdmissionRequest {
    noble_syndicate::AdmissionRequest {
        shared_mutable_guest_memory: false,
        rights,
    }
}

fn event(
    observer: noble_kernel::dataspace::Facet,
    name: &str,
    ready: bool,
    change: noble_kernel::dataspace::Change,
) -> noble_kernel::dataspace::Event {
    noble_kernel::dataspace::Event {
        observer,
        service: noble_kernel::dataspace::Service {
            name: name.to_owned(),
            ready,
        },
        change,
    }
}

#[test]
fn issued_child_drop_foreign_profile_and_physical_exhaustion() -> Result<(), String> {
    let limits = noble_kernel::dataspace::Limits {
        facets: 4,
        assertions: 4,
        interests: 4,
        events: 8,
    };
    let profile =
        noble_syndicate::Profile::new(limits).map_err(|error| format!("profile: {error:?}"))?;
    let foreign = noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)
        .map_err(|error| format!("foreign profile: {error:?}"))?;
    let publish = noble_kernel::dataspace::Rights {
        publish: true,
        observe: false,
    };
    let observe = noble_kernel::dataspace::Rights {
        publish: false,
        observe: true,
    };
    // Every outcome/count/notification is authored from the frozen host
    // ownership relation, not from a prior call to Table or Profile.
    let root = profile
        .admit_participant(request(publish))
        .map_err(|error| format!("root: {error:?}"))?;
    record(&profile, "open root", Ok(true), Ok(true), (1, 0, 0), &[])?;
    let mut child = profile
        .child(&root, publish)
        .map_err(|error| format!("child: {error:?}"))?;
    record(&profile, "open child", Ok(true), Ok(true), (2, 0, 0), &[])?;
    let watcher = profile
        .admit_participant(request(observe))
        .map_err(|error| format!("watcher: {error:?}"))?;
    record(&profile, "open watcher", Ok(true), Ok(true), (3, 0, 0), &[])?;
    let mut unrelated = profile
        .admit_participant(request(publish))
        .map_err(|error| format!("unrelated: {error:?}"))?;
    record(
        &profile,
        "open unrelated",
        Ok(true),
        Ok(true),
        (4, 0, 0),
        &[],
    )?;
    record(
        &profile,
        "observe absent clock true",
        profile.observe(&watcher, "clock", true),
        Ok(false),
        (4, 0, 1),
        &[],
    )?;
    let add_clock = event(
        watcher.facet(),
        "clock",
        true,
        noble_kernel::dataspace::Change::Added,
    );
    record(
        &profile,
        "publish child clock true",
        profile.publish(&child, "clock", true),
        Ok(true),
        (4, 1, 1),
        std::slice::from_ref(&add_clock),
    )?;
    record(
        &profile,
        "publish unrelated alarm false",
        profile.publish(&unrelated, "alarm", false),
        Ok(true),
        (4, 2, 1),
        std::slice::from_ref(&add_clock),
    )?;

    // Cross-profile refusal is before any table mutation, on either profile.
    let foreign_refusal = foreign.observe(&watcher, "clock", true);
    record(
        &foreign,
        "foreign-profile refusal",
        foreign_refusal,
        Err(noble_syndicate::Error::WrongProfile),
        (0, 0, 0),
        &[],
    )?;
    record(
        &profile,
        "source profile remains unchanged",
        Ok(true),
        Ok(true),
        (4, 2, 1),
        std::slice::from_ref(&add_clock),
    )?;

    drop(root); // normal or trap cleanup commits child removal; unrelated root survives.
    let remove_clock = event(
        watcher.facet(),
        "clock",
        true,
        noble_kernel::dataspace::Change::Removed,
    );
    record(
        &profile,
        "drop parent and child",
        Ok(true),
        Ok(true),
        (2, 1, 1),
        &[add_clock.clone(), remove_clock.clone()],
    )?;
    record(
        &profile,
        "retired child rejects publish",
        profile.publish(&child, "clock", false),
        Err(noble_syndicate::Error::Kernel(
            noble_kernel::dataspace::Error::InvalidFacet,
        )),
        (2, 1, 1),
        &[add_clock.clone(), remove_clock.clone()],
    )?;
    let add_alarm = event(
        watcher.facet(),
        "alarm",
        false,
        noble_kernel::dataspace::Change::Added,
    );
    let expected_queue = vec![add_clock, remove_clock, add_alarm];
    record(
        &profile,
        "late false-pair observe",
        profile.observe(&watcher, "alarm", false),
        Ok(true),
        (2, 1, 2),
        &expected_queue,
    )?;
    let close_child = profile.close(&mut child).map(|()| true);
    record(
        &profile,
        "idempotent close retired child",
        close_child,
        Ok(true),
        (2, 1, 2),
        &expected_queue,
    )?;
    let drain = profile.clear_events().map(|()| true);
    record(&profile, "explicit drain", drain, Ok(true), (2, 1, 2), &[])?;
    drop(watcher);
    record(
        &profile,
        "drop observer",
        Ok(true),
        Ok(true),
        (1, 1, 0),
        &[],
    )?;
    let close_unrelated = profile.close(&mut unrelated).map(|()| true);
    record(
        &profile,
        "close unrelated",
        close_unrelated,
        Ok(true),
        (0, 0, 0),
        &[],
    )?;
    record(
        &profile,
        "physical slots not recycled",
        profile.admit_participant(request(publish)).map(|_| true),
        Err(noble_syndicate::Error::Kernel(
            noble_kernel::dataspace::Error::Capacity,
        )),
        (0, 0, 0),
        &[],
    )?;
    Ok(())
}
