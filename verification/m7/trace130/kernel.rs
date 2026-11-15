//! Actual Table versus the independently derived bounded reference.
mod faults;
mod generated;
mod reference;

use noble_kernel::dataspace::{self, Table};
use reference::{Action, Decision, Event, Fault, Limits, Model, Observation, Refusal, Rights};

#[derive(Clone, Debug)]
struct Row {
    action: Action,
    expected: (reference::Result, Observation),
    actual: (reference::Result, Observation),
    fired: bool,
}

impl Row {
    fn agrees(&self) -> bool {
        self.expected == self.actual
    }
}

fn actual_rights(rights: Rights) -> dataspace::Rights {
    dataspace::Rights {
        publish: matches!(rights, Rights::Publish),
        observe: matches!(rights, Rights::Observe),
    }
}

fn actual_limits(limits: Limits) -> dataspace::Limits {
    dataspace::Limits {
        facets: limits.facets,
        assertions: limits.assertions,
        interests: limits.interests,
        events: limits.events,
    }
}

fn refusal(error: dataspace::Error) -> Refusal {
    match error {
        dataspace::Error::InvalidFacet => Refusal::InvalidFacet,
        dataspace::Error::InvalidName => Refusal::InvalidName,
        dataspace::Error::Denied => Refusal::Denied,
        dataspace::Error::Capacity => Refusal::Capacity,
        other => panic!("unexpected wire refusal in typed trace: {other:?}"),
    }
}

fn facet(facets: &[dataspace::Facet], who: u8) -> dataspace::Facet {
    facets[usize::from(who - 1)]
}

fn actual_step(
    table: &mut Table,
    facets: &mut Vec<dataspace::Facet>,
    action: Action,
) -> reference::Result {
    use Action::{Drain, Observe, Open, Publish, Retire, Retract};
    let result = match action {
        Open { parent, rights } => {
            let parent = parent.map(|id| facet(facets, id));
            return match table.open(parent, actual_rights(rights)) {
                Ok(f) => {
                    facets.push(f);
                    Ok(Decision::Facet(
                        u8::try_from(facets.len()).expect("bounded facets"),
                    ))
                }
                Err(error) => Err(refusal(error)),
            };
        }
        Publish { who, name, ready } => table
            .publish(facet(facets, who), name, ready)
            .map(Decision::Bool),
        Observe { who, name, ready } => table
            .observe(facet(facets, who), name, ready)
            .map(Decision::Bool),
        Retract { who, name } => table.retract(facet(facets, who), name).map(Decision::Bool),
        Retire { who } => table.retire(facet(facets, who)).map(|()| Decision::Done),
        Drain => {
            table.clear_events();
            Ok(Decision::Done)
        }
    };
    result.map_err(refusal)
}

fn static_name(name: &str) -> &'static str {
    match name {
        "alarm" => "alarm",
        "clock" => "clock",
        other => panic!("name outside frozen alphabet: {other}"),
    }
}

fn actual_observation(table: &Table, facets: &[dataspace::Facet]) -> Observation {
    let counts = table.counts();
    let mut pairs = Vec::new();
    for name in ["alarm", "clock"] {
        for ready in [false, true] {
            if table.contains_pair(name, ready) {
                pairs.push((name, ready));
            }
        }
    }
    Observation {
        counts: (counts.0 as usize, counts.1 as usize, counts.2 as usize),
        slots: table.remaining_capacity().facets,
        pairs,
        queue: table
            .events()
            .iter()
            .map(|event| Event {
                observer: u8::try_from(
                    facets
                        .iter()
                        .position(|f| *f == event.observer)
                        .expect("host-issued observer")
                        + 1,
                )
                .expect("bounded facets"),
                name: static_name(&event.service.name),
                ready: event.service.ready,
                change: match event.change {
                    dataspace::Change::Added => "Added",
                    dataspace::Change::Removed => "Removed",
                },
            })
            .collect(),
    }
}

fn trace(limits: Limits, actions: &[Action], fault: Fault) -> Vec<Row> {
    let mut model = Model::new(limits, fault);
    let mut actual = Table::new(actual_limits(limits)).expect("valid frozen limits");
    let mut facets = Vec::new();
    let mut rows = Vec::with_capacity(actions.len());
    for action in actions {
        let before = model.observation();
        let expected_result = model.step(*action);
        let expected_observation = model.observation();
        if expected_result.is_err() {
            assert_eq!(
                expected_observation, before,
                "reference refused call changed state"
            );
        }
        let actual_before = actual_observation(&actual, &facets);
        let actual_result = actual_step(&mut actual, &mut facets, *action);
        let actual_after = actual_observation(&actual, &facets);
        if actual_result.is_err() {
            assert_eq!(
                actual_after, actual_before,
                "production refused call changed state"
            );
        }
        rows.push(Row {
            action: *action,
            expected: (expected_result, expected_observation),
            actual: (actual_result, actual_after),
            fired: model.fault_fired(),
        });
    }
    rows
}

fn emit(id: &str, limits: Limits, rows: &[Row]) {
    let history = rows
        .iter()
        .map(|row| format!("{:?}", row.action))
        .collect::<Vec<_>>()
        .join(";");
    let expected = rows
        .iter()
        .map(|row| format!("{:?}", row.expected))
        .collect::<Vec<_>>()
        .join(";");
    let actual = rows
        .iter()
        .map(|row| format!("{:?}", row.actual))
        .collect::<Vec<_>>()
        .join(";");
    println!("M7TRACE\t{id}\t{limits:?}\t{history}\t{expected}\t{actual}");
}

fn compare(id: &str, limits: Limits, actions: &[Action]) {
    let rows = trace(limits, actions, Fault::None);
    emit(id, limits, &rows);
    if let Some((index, row)) = rows.iter().enumerate().find(|(_, row)| !row.agrees()) {
        panic!("{id} step {index}: {row:?}");
    }
}

fn initial() -> [Action; 3] {
    [
        Action::Open {
            parent: None,
            rights: Rights::Publish,
        },
        Action::Open {
            parent: None,
            rights: Rights::Publish,
        },
        Action::Open {
            parent: None,
            rights: Rights::Observe,
        },
    ]
}
