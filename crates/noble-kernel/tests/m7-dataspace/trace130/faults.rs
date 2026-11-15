//! Faulted reference runs, trigger-constrained reductions and replay.
use super::{compare, initial, trace, Action, Fault, Limits, Rights, Row};

struct Witness {
    name: &'static str,
    trigger: &'static str,
    fault: Fault,
    limits: Limits,
    actions: Vec<Action>,
    fixed: usize,
}

fn pub_at(who: u8, name: &'static str, ready: bool) -> Action {
    Action::Publish { who, name, ready }
}
fn obs_at(who: u8, name: &'static str, ready: bool) -> Action {
    Action::Observe { who, name, ready }
}

fn witnesses() -> Vec<Witness> {
    let standard = Limits {
        facets: 3,
        assertions: 2,
        interests: 2,
        events: 8,
    };
    let fixture = initial();
    let common = |suffix: &[Action]| {
        let mut actions = fixture.to_vec();
        actions.extend_from_slice(suffix);
        actions
    };
    vec![
        Witness {
            name: "premature-final-owner-removal",
            trigger:
                "two owners of clock=true; one withdraws while the other and an observer remain",
            fault: Fault::PrematureRemoval,
            limits: standard,
            fixed: 3,
            actions: common(&[
                Action::Drain,
                obs_at(3, "clock", true),
                pub_at(1, "clock", true),
                pub_at(2, "clock", true),
                Action::Retract {
                    who: 1,
                    name: "clock",
                },
                obs_at(3, "clock", true),
            ]),
        },
        Witness {
            name: "duplicate-addition",
            trigger: "second owner publishes visible clock=true to an existing observer",
            fault: Fault::DuplicateAddition,
            limits: standard,
            fixed: 3,
            actions: common(&[
                Action::Drain,
                obs_at(3, "clock", true),
                pub_at(1, "clock", true),
                pub_at(2, "clock", true),
                Action::Drain,
            ]),
        },
        Witness {
            name: "false-versus-absence",
            trigger: "observer queries an asserted clock=false pair",
            fault: Fault::FalseAsAbsence,
            limits: standard,
            fixed: 3,
            actions: common(&[
                Action::Drain,
                pub_at(1, "clock", false),
                obs_at(3, "clock", false),
                obs_at(3, "clock", false),
            ]),
        },
        Witness {
            name: "descendant-not-retired",
            trigger: "retire parent with child-owned clock=true and surviving matching observer",
            fault: Fault::MissingDescendants,
            limits: standard,
            fixed: 3,
            actions: vec![
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
                Action::Drain,
                obs_at(3, "clock", true),
                pub_at(2, "clock", true),
                Action::Retire { who: 1 },
                pub_at(2, "clock", false),
            ],
        },
        Witness {
            name: "missing-removal-reservation",
            trigger: "admit interest whose queued addition fits but future removal does not",
            fault: Fault::UnderReservedRemoval,
            limits: Limits {
                events: 2,
                ..standard
            },
            fixed: 3,
            actions: common(&[
                Action::Drain,
                obs_at(3, "clock", true),
                pub_at(1, "clock", true),
                pub_at(2, "alarm", false),
                obs_at(3, "alarm", false),
                Action::Retire { who: 1 },
            ]),
        },
    ]
}

fn differs_with_trigger(witness: &Witness, actions: &[Action]) -> bool {
    let rows = trace(witness.limits, actions, witness.fault);
    rows.last().is_some_and(|last| last.fired) && rows.iter().any(|row| !row.agrees())
}

fn minimize(witness: &Witness) -> Vec<Action> {
    let mut actions = witness.actions.clone();
    loop {
        let mut smaller = None;
        for index in witness.fixed..actions.len() {
            let mut candidate = actions.clone();
            candidate.remove(index);
            // The fixed host-issued fixture stays intact, and deleting a later
            // Open is safe only if all remaining IDs have actually been issued.
            let issued = candidate
                .iter()
                .filter(|a| matches!(a, Action::Open { .. }))
                .count();
            if candidate.iter().any(|a| match a {
                Action::Open {
                    parent: Some(parent),
                    ..
                } => usize::from(*parent) > issued,
                Action::Publish { who, .. }
                | Action::Observe { who, .. }
                | Action::Retract { who, .. }
                | Action::Retire { who } => usize::from(*who) > issued,
                Action::Open { parent: None, .. } | Action::Drain => false,
            }) {
                continue;
            }
            if differs_with_trigger(witness, &candidate) {
                smaller = Some(candidate);
                break;
            }
        }
        if let Some(reduced) = smaller {
            actions = reduced;
        } else {
            break;
        }
    }
    actions
}

fn debug_trace(rows: &[Row], expected: bool) -> String {
    rows.iter()
        .map(|row| {
            if expected {
                format!("{:?}", row.expected)
            } else {
                format!("{:?}", row.actual)
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}

#[test]
fn seeded_faults_produce_minimized_replayable_mismatches() {
    for witness in witnesses() {
        compare(
            &format!("fault-control/{}", witness.name),
            witness.limits,
            &witness.actions,
        );
        let full = trace(witness.limits, &witness.actions, witness.fault);
        assert!(
            differs_with_trigger(&witness, &witness.actions),
            "{} did not fire and disagree",
            witness.name
        );
        let reduced = minimize(&witness);
        let reduced_rows = trace(witness.limits, &reduced, witness.fault);
        assert!(reduced.len() <= witness.actions.len());
        assert!(reduced_rows.last().is_some_and(|last| last.fired));
        let format_actions = |actions: &[Action]| {
            actions
                .iter()
                .map(|action| format!("{action:?}"))
                .collect::<Vec<_>>()
                .join(";")
        };
        println!(
            "M7FAULT\t{}\t{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}",
            witness.name,
            witness.trigger,
            witness.limits,
            format_actions(&witness.actions),
            format_actions(&reduced),
            debug_trace(&full, true),
            debug_trace(&full, false),
            debug_trace(&reduced_rows, true),
            debug_trace(&reduced_rows, false)
        );
    }
}

#[test]
fn independent_minimized_replays_preserve_triggers_and_are_one_minimal() {
    for witness in witnesses() {
        let reduced = minimize(&witness);
        assert!(
            differs_with_trigger(&witness, &reduced),
            "{} reduction lost trigger",
            witness.name
        );
        for index in witness.fixed..reduced.len() {
            let mut shorter = reduced.clone();
            shorter.remove(index);
            if shorter.iter().any(|a| match a {
                Action::Publish { who, .. }
                | Action::Observe { who, .. }
                | Action::Retract { who, .. }
                | Action::Retire { who } => {
                    usize::from(*who)
                        > shorter
                            .iter()
                            .filter(|a| matches!(a, Action::Open { .. }))
                            .count()
                }
                _ => false,
            }) {
                continue;
            }
            assert!(
                !differs_with_trigger(&witness, &shorter),
                "{}: deletion at {index} preserves failure",
                witness.name
            );
        }
    }
}
