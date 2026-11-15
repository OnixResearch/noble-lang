use super::reference::{self, Action, Event, Model, Refusal};
use noble_kernel::dataspace;
use noble_syndicate::{AdmissionRequest, Profile};

pub(super) fn request(publish: bool, observe: bool) -> AdmissionRequest {
    AdmissionRequest {
        shared_mutable_guest_memory: false,
        rights: dataspace::Rights { publish, observe },
    }
}

pub(super) fn convert(error: noble_syndicate::Error) -> Refusal {
    match error {
        noble_syndicate::Error::Kernel(dataspace::Error::InvalidFacet) => Refusal::InvalidFacet,
        noble_syndicate::Error::Kernel(dataspace::Error::Capacity) => Refusal::Capacity,
        noble_syndicate::Error::Kernel(dataspace::Error::Denied) => Refusal::Denied,
        other => panic!("unexpected host boundary refusal: {other:?}"),
    }
}

pub(super) fn check(
    profile: &Profile,
    model: &mut Model,
    action: Action,
    actual: reference::Result,
    ids: &[dataspace::Facet],
) -> Result<(), String> {
    let expected = model.step(action);
    let expected_state = model.observation();
    let counts = profile
        .counts()
        .map_err(|error| format!("counts: {error:?}"))?;
    let events = profile
        .events()
        .map_err(|error| format!("events: {error:?}"))?;
    let actual_queue = events
        .iter()
        .map(|event| {
            Ok(Event {
                observer: u8::try_from(
                    ids.iter()
                        .position(|id| *id == event.observer)
                        .ok_or_else(|| "event addressed to unissued facet".to_owned())?
                        + 1,
                )
                .map_err(|error| error.to_string())?,
                name: match event.service.name.as_str() {
                    "clock" => "clock",
                    "alarm" => "alarm",
                    other => return Err(format!("unexpected service {other}")),
                },
                ready: event.service.ready,
                change: match event.change {
                    dataspace::Change::Added => "Added",
                    dataspace::Change::Removed => "Removed",
                },
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let actual_counts = (counts.0 as usize, counts.1 as usize, counts.2 as usize);
    println!(
        "M7HOST\t{action:?}\t{expected:?} {:?}\t{actual:?} {:?} {:?}",
        expected_state, actual_counts, actual_queue
    );
    if expected != actual
        || expected_state.counts != actual_counts
        || expected_state.queue != actual_queue
    {
        return Err(format!("{action:?}: reference {expected:?} {expected_state:?}; host {actual:?} {actual_counts:?} {actual_queue:?}"));
    }
    Ok(())
}
