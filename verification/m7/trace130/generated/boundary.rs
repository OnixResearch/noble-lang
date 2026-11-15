use super::super::{compare, initial, Action, Limits, Rights};
use super::{ENUM_LIMITS, EXPANDED};

fn pub_at(who: u8, name: &'static str, ready: bool) -> Action {
    Action::Publish { who, name, ready }
}
fn obs_at(who: u8, name: &'static str, ready: bool) -> Action {
    Action::Observe { who, name, ready }
}
fn retract(who: u8, name: &'static str) -> Action {
    Action::Retract { who, name }
}

#[test]
fn named_boundary_histories() {
    let shared = [
        obs_at(3, "clock", true),
        pub_at(1, "clock", true),
        pub_at(2, "clock", true),
        obs_at(3, "clock", true),
        retract(1, "clock"),
        obs_at(3, "clock", true),
        retract(2, "clock"),
        obs_at(3, "clock", true),
        Action::Drain,
    ];
    let false_pair = [
        obs_at(3, "clock", false),
        pub_at(1, "clock", false),
        obs_at(3, "clock", false),
        pub_at(2, "clock", false),
        pub_at(1, "clock", true),
        obs_at(3, "clock", false),
        retract(2, "clock"),
        obs_at(3, "clock", false),
        pub_at(1, "clock", false),
        obs_at(3, "clock", false),
    ];
    let small_capacity = [
        obs_at(3, "clock", true),
        pub_at(1, "clock", true),
        pub_at(2, "clock", true),
        retract(1, "clock"),
        retract(2, "clock"),
        pub_at(1, "clock", true),
        Action::Drain,
        pub_at(1, "clock", true),
        Action::Retire { who: 1 },
    ];
    let reservation = [
        obs_at(3, "clock", true),
        pub_at(1, "clock", true),
        pub_at(2, "alarm", false),
        obs_at(3, "alarm", false),
        Action::Drain,
        obs_at(3, "alarm", false),
        Action::Retire { who: 1 },
        Action::Retire { who: 2 },
    ];
    let replacement = [
        obs_at(3, "clock", true),
        obs_at(3, "clock", false),
        pub_at(1, "clock", true),
        pub_at(1, "clock", false),
        Action::Drain,
        pub_at(1, "clock", false),
        Action::Retire { who: 1 },
    ];
    for (name, limits, suffix) in [
        ("duplicate-withdrawal", ENUM_LIMITS, shared.as_slice()),
        (
            "false-presence-and-replacement",
            EXPANDED,
            false_pair.as_slice(),
        ),
        (
            "queue-full-drain-retire",
            Limits {
                events: 2,
                ..ENUM_LIMITS
            },
            small_capacity.as_slice(),
        ),
        (
            "future-removal-refusal",
            Limits {
                interests: 2,
                events: 2,
                ..ENUM_LIMITS
            },
            reservation.as_slice(),
        ),
        (
            "atomic-replacement",
            Limits {
                interests: 2,
                events: 3,
                ..ENUM_LIMITS
            },
            replacement.as_slice(),
        ),
    ] {
        let mut actions = initial().to_vec();
        actions.extend_from_slice(suffix);
        compare(&format!("boundary/{name}"), limits, &actions);
    }
    let mut nested = vec![
        Action::Open {
            parent: None,
            rights: Rights::Publish,
        },
        Action::Open {
            parent: Some(1),
            rights: Rights::Publish,
        },
        Action::Open {
            parent: None,
            rights: Rights::Observe,
        },
        Action::Open {
            parent: None,
            rights: Rights::Publish,
        },
        obs_at(3, "clock", true),
        pub_at(2, "clock", true),
        pub_at(4, "alarm", false),
        Action::Retire { who: 1 },
        pub_at(2, "clock", false),
        Action::Retire { who: 2 },
        obs_at(3, "clock", true),
        obs_at(3, "alarm", false),
        Action::Retire { who: 3 },
        retract(4, "alarm"),
    ];
    compare("boundary/nested-and-unrelated", EXPANDED, &nested);
    nested = vec![
        Action::Open {
            parent: None,
            rights: Rights::Publish,
        },
        Action::Open {
            parent: None,
            rights: Rights::Observe,
        },
        pub_at(1, "clock", true),
        obs_at(2, "clock", true),
        Action::Retire { who: 2 },
        retract(1, "clock"),
    ];
    compare("boundary/retired-observer-queue", EXPANDED, &nested);
    let mut slots = initial().to_vec();
    for _ in 3..8 {
        slots.push(Action::Open {
            parent: None,
            rights: Rights::Publish,
        });
    }
    slots.extend([
        Action::Retire { who: 2 },
        Action::Open {
            parent: None,
            rights: Rights::Publish,
        },
        pub_at(2, "clock", true),
        Action::Open {
            parent: Some(2),
            rights: Rights::Observe,
        },
    ]);
    compare("boundary/physical-slot-exhaustion", EXPANDED, &slots);
}
