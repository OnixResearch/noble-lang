use super::super::{add_graph, expected_one, expected_two, sub_graph, two_graph, ONE, TWO};
use noble_ngraph::ir::Graph;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Add,
    Two,
    Sub,
}

// This frozen three-scenario fixture must fail compilation when a fourth Kind is added
// until its graph and receipt identity have both been explicitly selected.
#[allow(tigerstyle::fragile_exhaustive_enum_match)]
impl Kind {
    pub(super) fn graph(self) -> Graph {
        match self {
            Self::Add => add_graph(),
            Self::Two => two_graph(),
            Self::Sub => sub_graph(),
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Two => "two",
            Self::Sub => "sub",
        }
    }

    pub(super) fn partition(self) -> noble_ngraph::tick::Partition {
        if self == Self::Two {
            TWO
        } else {
            ONE
        }
    }

    pub(super) fn initial(self) -> (Vec<i64>, Vec<i64>) {
        if self == Self::Two {
            (vec![i64::MIN, 7], vec![3, -5])
        } else if self == Self::Sub {
            (vec![i64::MIN + 1], vec![7])
        } else {
            (vec![i64::MAX - 1], vec![3])
        }
    }

    pub(super) fn oracle(self, s: &[i64], x: &[i64], c: &[i64]) -> (Vec<i64>, Vec<i64>) {
        if self == Self::Two {
            let (state, output) = expected_two([s[0], s[1]], [x[0], x[1]], [c[0], c[1]]);
            (state.to_vec(), output.to_vec())
        } else {
            let value = expected_one(s[0], x[0], c[0], self == Self::Sub);
            (vec![value], vec![value])
        }
    }
}

#[derive(Clone)]
pub(super) enum Event {
    Tick {
        input: Vec<i64>,
        stale: bool,
        cancel: bool,
    },
    Update(Vec<i64>),
    RejectUpdate {
        controls: Vec<i64>,
        stale: bool,
    },
    Reset(Kind),
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub(super) kind: &'static str,
    pub(super) graph_epoch: u64,
    pub(super) control_epoch: u64,
    pub(super) next_tick: u64,
    pub(super) state: Vec<i64>,
    pub(super) controls: Vec<i64>,
    pub(super) output: Option<Vec<i64>>,
}

pub(super) struct Model {
    kind: Kind,
    graph_epoch: u64,
    control_epoch: u64,
    next_tick: u64,
    state: Vec<i64>,
    controls: Vec<i64>,
    output: Option<Vec<i64>>,
}

impl Model {
    pub(super) fn new(kind: Kind) -> Self {
        let (state, controls) = kind.initial();
        Self {
            kind,
            graph_epoch: 0,
            control_epoch: 0,
            next_tick: 0,
            state,
            controls,
            output: None,
        }
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            kind: self.kind.name(),
            graph_epoch: self.graph_epoch,
            control_epoch: self.control_epoch,
            next_tick: self.next_tick,
            state: self.state.clone(),
            controls: self.controls.clone(),
            output: self.output.clone(),
        }
    }

    // The event trace is deliberately closed: adding a new event must update the oracle.
    #[allow(tigerstyle::fragile_exhaustive_enum_match)]
    pub(super) fn apply(&mut self, event: &Event) -> bool {
        match event {
            Event::Tick { stale, cancel, .. } if *stale || *cancel => false,
            Event::Tick { input, .. } => {
                let (state, output) = self.kind.oracle(&self.state, input, &self.controls);
                self.state = state;
                self.output = Some(output);
                self.next_tick += 1;
                true
            }
            Event::Update(controls) => {
                self.controls.clone_from(controls);
                self.control_epoch += 1;
                true
            }
            Event::RejectUpdate { .. } => false,
            Event::Reset(kind) => {
                let (state, controls) = kind.initial();
                self.kind = *kind;
                self.graph_epoch += 1;
                self.control_epoch += 1;
                self.next_tick = 0;
                self.state = state;
                self.controls = controls;
                self.output = None;
                true
            }
        }
    }
}

fn refusal_event(count: usize, current: Kind) -> Option<Event> {
    match count {
        4 => Some(Event::Tick {
            input: vec![1; current.partition().input_words],
            stale: true,
            cancel: false,
        }),
        5 => Some(Event::Tick {
            input: vec![1; current.partition().input_words],
            stale: false,
            cancel: true,
        }),
        6 => Some(Event::RejectUpdate {
            controls: current.initial().1,
            stale: true,
        }),
        9 => Some(Event::RejectUpdate {
            controls: vec![],
            stale: false,
        }),
        _ => None,
    }
}

fn update_events(count: usize, current: Kind) -> (Event, Event) {
    let controls = if current == Kind::Two {
        vec![-4 - count as i64, 5 + count as i64]
    } else {
        vec![-4 - count as i64]
    };
    (
        Event::Update(controls),
        Event::Tick {
            input: vec![1; current.partition().input_words],
            stale: true,
            cancel: false,
        },
    )
}

pub(super) fn events(kind: Kind, tick_count: usize) -> Vec<Event> {
    let refusals = [4, 5, 6, 9]
        .into_iter()
        .filter(|&index| index < tick_count)
        .count();
    let updates = usize::from(tick_count >= 8) + tick_count / 64;
    let reset_after = if tick_count == 32 { 16 } else { 256 };
    let resets = usize::from(kind == Kind::Sub && tick_count >= reset_after);
    let mut events = Vec::with_capacity(tick_count + refusals + 2 * updates + resets);
    let mut current = kind;
    for count in 0..tick_count {
        if let Some(event) = refusal_event(count, current) {
            events.push(event);
        }
        let input = if current == Kind::Two {
            vec![
                if count % 2 == 0 { -1 } else { i64::MAX },
                if count % 3 == 0 { i64::MIN } else { -7 },
            ]
        } else {
            vec![if count % 3 == 0 {
                i64::MIN
            } else if count % 2 == 0 {
                -2
            } else {
                1
            }]
        };
        events.push(Event::Tick {
            input,
            stale: false,
            cancel: false,
        });
        let completed = count + 1;
        if kind == Kind::Sub && completed == reset_after {
            current = Kind::Sub;
            events.push(Event::Reset(current));
        }
        if completed == 8 || completed % 64 == 0 {
            let (update, stale_tick) = update_events(count, current);
            events.push(update);
            events.push(stale_tick);
        }
    }
    events
}
