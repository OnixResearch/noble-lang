use super::domain::{Kind, Snapshot};
use crate::common::IDS;
use noble_ngraph::compiled::{Compiled, Workspace};
use noble_ngraph::eval::{Control, Obs};
use noble_ngraph::sem::Value;
use noble_ngraph::tick::{Limits, Runner};

pub(super) struct GraphMachine {
    compiled: Compiled,
    workspace: Workspace,
    inputs: Vec<Value>,
}

impl GraphMachine {
    pub(super) fn prepare(kind: Kind) -> Self {
        let checked =
            noble_ngraph::check::check(&IDS, kind.graph()).expect("checked scenario graph");
        let compiled = Compiled::new(&IDS, &checked);
        noble_ngraph::schedule::check(
            &checked,
            &compiled.packed_layout(),
            compiled.i64_instructions().expect("I64 instruction body"),
        )
        .expect("selected schedule validated");
        let workspace = compiled.workspace();
        let p = kind.partition();
        Self {
            compiled,
            workspace,
            inputs: vec![Value::I64(0); p.state_words + p.input_words + p.control_words],
        }
    }

    fn step(
        &mut self,
        state: &[i64],
        input: &[i64],
        controls: &[i64],
        cancel: bool,
    ) -> Option<(Vec<i64>, Vec<i64>)> {
        for (slot, value) in self
            .inputs
            .iter_mut()
            .zip(state.iter().chain(input).chain(controls))
        {
            *slot = Value::I64(*value);
        }
        let cancelled = |_: u32| true;
        let mut control = Control::default();
        if cancel {
            control.cancel = Some(&cancelled);
        }
        let report =
            self.compiled
                .evaluate_in(&mut self.workspace, &self.inputs, None, &mut control);
        let Obs::Normal(values) = report.obs else {
            return None;
        };
        let mut parts = values.into_iter().map(|value| {
            let Value::I64(word) = value else {
                panic!("admitted I64 graph returned another type")
            };
            word
        });
        let state: Vec<_> = parts.by_ref().take(state.len()).collect();
        let output: Vec<_> = parts.collect();
        Some((state, output))
    }

    fn footprint(&self) -> (u64, u64) {
        let scratch = self.compiled.scratch_bytes();
        (
            self.compiled.retained_bytes()
                + scratch
                + noble_ngraph::check::retained_bytes(self.compiled.checked().graph())
                + (self.inputs.capacity() * std::mem::size_of::<Value>()) as u64,
            scratch,
        )
    }
}

pub(super) struct HostMachine {
    pub(super) kind: Kind,
    pub(super) graph_epoch: u64,
    pub(super) control_epoch: u64,
    pub(super) next_tick: u64,
    pub(super) state: Vec<i64>,
    pub(super) controls: Vec<i64>,
    pub(super) output: Option<Vec<i64>>,
    pub(super) graph: Option<GraphMachine>,
}

impl HostMachine {
    fn new(kind: Kind, graph: bool) -> Self {
        let (state, controls) = kind.initial();
        Self {
            kind,
            graph_epoch: 0,
            control_epoch: 0,
            next_tick: 0,
            state,
            controls,
            output: None,
            graph: graph.then(|| GraphMachine::prepare(kind)),
        }
    }

    fn snapshot(&self) -> Snapshot {
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

    pub(super) fn tick(&mut self, input: &[i64], stale: bool, cancel: bool) -> bool {
        if stale {
            return false;
        }
        let result = if let Some(graph) = &mut self.graph {
            graph.step(&self.state, input, &self.controls, cancel)
        } else if cancel {
            None
        } else {
            Some(self.kind.oracle(&self.state, input, &self.controls))
        };
        let Some((state, output)) = result else {
            return false;
        };
        self.state = state;
        self.output = Some(output);
        self.next_tick += 1;
        true
    }

    fn footprint(&self) -> (u64, Option<u64>) {
        let vectors = (self.state.capacity()
            + self.controls.capacity()
            + self.output.as_ref().map_or(0, Vec::capacity)) as u64
            * 8;
        match &self.graph {
            Some(graph) => {
                let (retained, scratch) = graph.footprint();
                (vectors + retained, Some(scratch))
            }
            None => (vectors, None),
        }
    }
}

pub(super) enum Backend {
    Runner(Box<Runner>),
    Graph(Box<HostMachine>),
    Native(Box<HostMachine>),
}

// A fourth arm must implement all three observations, not silently use a baseline.
#[allow(tigerstyle::fragile_exhaustive_enum_match)]
impl Backend {
    pub(super) fn new(arm: &'static str, kind: Kind) -> Self {
        match arm {
            "runner" => {
                let (state, controls) = kind.initial();
                Self::Runner(Box::new(
                    Runner::prepare(
                        &IDS,
                        super::super::start(kind.graph(), kind.partition(), &state, &controls),
                        Limits::default(),
                    )
                    .expect("admitted runner"),
                ))
            }
            "graph" => Self::Graph(Box::new(HostMachine::new(kind, true))),
            "native" => Self::Native(Box::new(HostMachine::new(kind, false))),
            _ => panic!("unsupported benchmark arm"),
        }
    }

    pub(super) fn snapshot(&self, kind: Kind, controls: &[i64]) -> Snapshot {
        match self {
            Self::Runner(r) => {
                let selected = r.selection();
                Snapshot {
                    kind: kind.name(),
                    graph_epoch: selected.graph_epoch,
                    control_epoch: selected.control_epoch,
                    next_tick: selected.next_tick,
                    state: r.state().to_vec(),
                    controls: controls.to_vec(),
                    output: r.output().map(<[i64]>::to_vec),
                }
            }
            Self::Graph(h) | Self::Native(h) => h.snapshot(),
        }
    }

    pub(super) fn footprint(&self) -> (u64, Option<u64>) {
        match self {
            Self::Runner(r) => (r.retained_bytes(), Some(r.scratch_bytes())),
            Self::Graph(h) | Self::Native(h) => h.footprint(),
        }
    }

    pub(super) fn next_tick(&self) -> u64 {
        match self {
            Self::Runner(r) => r.selection().next_tick,
            Self::Graph(h) | Self::Native(h) => h.next_tick,
        }
    }
}
