//! Host-adapter boundary check using the same independently derived oracle.
// The reference contains additional fault/enumeration operations used only by
// the kernel target; this host target deliberately exercises its subset.
#[allow(
    dead_code,
    reason = "The shared reference's fault and enumeration operations are exercised by the kernel target, not repeated by the host target."
)]
#[path = "reference.rs"]
mod reference;

#[path = "host/compare.rs"]
mod compare;

use compare::{check, convert, request};
use noble_kernel::dataspace;
use noble_syndicate::Profile;
use reference::{Action, Decision, Fault, Limits, Model, Refusal, Rights};

#[test]
fn issued_child_drop_foreign_profile_and_physical_exhaustion() -> Result<(), String> {
    let limits = Limits {
        facets: 4,
        assertions: 4,
        interests: 4,
        events: 8,
    };
    let profile = Profile::new(dataspace::Limits {
        facets: limits.facets,
        assertions: limits.assertions,
        interests: limits.interests,
        events: limits.events,
    })
    .map_err(|error| format!("create host: {error:?}"))?;
    let foreign = Profile::new(noble_syndicate::SELECTED_LIMITS)
        .map_err(|error| format!("create foreign: {error:?}"))?;
    let mut model = Model::new(limits, Fault::None);
    let mut ids = Vec::new();
    let root = profile
        .admit_participant(request(true, false))
        .map_err(|error| format!("admit root: {error:?}"))?;
    ids.push(root.facet());
    check(
        &profile,
        &mut model,
        Action::Open {
            parent: None,
            rights: Rights::Publish,
        },
        Ok(Decision::Facet(1)),
        &ids,
    )?;
    let mut child = profile
        .child(
            &root,
            dataspace::Rights {
                publish: true,
                observe: false,
            },
        )
        .map_err(|error| format!("admit child: {error:?}"))?;
    ids.push(child.facet());
    check(
        &profile,
        &mut model,
        Action::Open {
            parent: Some(1),
            rights: Rights::Publish,
        },
        Ok(Decision::Facet(2)),
        &ids,
    )?;
    let watcher = profile
        .admit_participant(request(false, true))
        .map_err(|error| format!("admit watcher: {error:?}"))?;
    ids.push(watcher.facet());
    check(
        &profile,
        &mut model,
        Action::Open {
            parent: None,
            rights: Rights::Observe,
        },
        Ok(Decision::Facet(3)),
        &ids,
    )?;
    let mut unrelated = profile
        .admit_participant(request(true, false))
        .map_err(|error| format!("admit unrelated: {error:?}"))?;
    ids.push(unrelated.facet());
    check(
        &profile,
        &mut model,
        Action::Open {
            parent: None,
            rights: Rights::Publish,
        },
        Ok(Decision::Facet(4)),
        &ids,
    )?;
    let action = Action::Observe {
        who: 3,
        name: "clock",
        ready: true,
    };
    let result = profile
        .observe(&watcher, "clock", true)
        .map(Decision::Bool)
        .map_err(convert);
    check(&profile, &mut model, action, result, &ids)?;
    let action = Action::Publish {
        who: 2,
        name: "clock",
        ready: true,
    };
    let result = profile
        .publish(&child, "clock", true)
        .map(Decision::Bool)
        .map_err(convert);
    check(&profile, &mut model, action, result, &ids)?;
    let action = Action::Publish {
        who: 4,
        name: "alarm",
        ready: false,
    };
    let result = profile
        .publish(&unrelated, "alarm", false)
        .map(Decision::Bool)
        .map_err(convert);
    check(&profile, &mut model, action, result, &ids)?;

    let before = (
        profile
            .counts()
            .map_err(|error| format!("counts: {error:?}"))?,
        profile
            .events()
            .map_err(|error| format!("events: {error:?}"))?,
    );
    let refusal = foreign.observe(&watcher, "clock", true);
    assert_eq!(refusal, Err(noble_syndicate::Error::WrongProfile));
    assert_eq!(
        before.0,
        profile
            .counts()
            .map_err(|error| format!("counts: {error:?}"))?
    );
    assert_eq!(
        before.1,
        profile
            .events()
            .map_err(|error| format!("events: {error:?}"))?
    );
    println!(
        "M7HOST\tforeign-profile\tErr(WrongProfile) state unchanged\t{refusal:?} state unchanged"
    );

    drop(root);
    check(
        &profile,
        &mut model,
        Action::Retire { who: 1 },
        Ok(Decision::Done),
        &ids,
    )?;
    let result = profile
        .publish(&child, "clock", false)
        .map(Decision::Bool)
        .map_err(convert);
    check(
        &profile,
        &mut model,
        Action::Publish {
            who: 2,
            name: "clock",
            ready: false,
        },
        result,
        &ids,
    )?;
    let result = profile
        .observe(&watcher, "alarm", false)
        .map(Decision::Bool)
        .map_err(convert);
    check(
        &profile,
        &mut model,
        Action::Observe {
            who: 3,
            name: "alarm",
            ready: false,
        },
        result,
        &ids,
    )?;
    profile
        .close(&mut child)
        .map_err(|error| format!("close retired child: {error:?}"))?;
    profile
        .clear_events()
        .map_err(|error| format!("drain: {error:?}"))?;
    check(
        &profile,
        &mut model,
        Action::Drain,
        Ok(Decision::Done),
        &ids,
    )?;
    drop(watcher);
    check(
        &profile,
        &mut model,
        Action::Retire { who: 3 },
        Ok(Decision::Done),
        &ids,
    )?;
    profile
        .close(&mut unrelated)
        .map_err(|error| format!("close unrelated: {error:?}"))?;
    check(
        &profile,
        &mut model,
        Action::Retire { who: 4 },
        Ok(Decision::Done),
        &ids,
    )?;
    let exhausted = profile.admit_participant(request(true, false));
    assert!(matches!(
        exhausted,
        Err(noble_syndicate::Error::Kernel(dataspace::Error::Capacity))
    ));
    check(
        &profile,
        &mut model,
        Action::Open {
            parent: None,
            rights: Rights::Publish,
        },
        Err(Refusal::Capacity),
        &ids,
    )?;
    Ok(())
}
