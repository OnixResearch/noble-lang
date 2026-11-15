use super::domain::{Event, Kind};
use super::engines::{Backend, GraphMachine};
use super::execution::{Replay, Timings};
use crate::common::IDS;
use noble_ngraph::eval::Control;
use std::hint::black_box;
use std::time::Instant;

// The closed event vocabulary must force each new operation through every arm.
#[allow(tigerstyle::fragile_exhaustive_enum_match)]
impl Replay {
    pub(super) fn run_operation(&mut self, event: &Event, timing: &mut Timings) -> bool {
        match event {
            Event::Tick {
                input,
                stale,
                cancel,
            } => self.tick(input, *stale, *cancel),
            Event::Update(words) => self.update(words, timing),
            Event::RejectUpdate { controls, stale } => self.reject_update(controls, *stale),
            Event::Reset(kind) => self.reset(*kind),
        }
    }

    fn tick(&mut self, input: &[i64], stale: bool, cancel: bool) -> bool {
        match &mut self.backend {
            Backend::Runner(r) => {
                let s = r.selection();
                let cancelled = |_: u32| true;
                let mut control = Control::default();
                if cancel {
                    control.cancel = Some(&cancelled);
                }
                let answer = r.tick(
                    s.graph_epoch + u64::from(stale),
                    s.control_epoch,
                    s.next_tick,
                    black_box(input),
                    &mut control,
                );
                answer.is_ok()
            }
            Backend::Graph(h) | Backend::Native(h) => h.tick(black_box(input), stale, cancel),
        }
    }

    // Exact Vec identity preserves the original clone_from capacity and avoids a
    // second temporary allocation on each staged control update.
    #[allow(tigerstyle::borrowed_argument_types)]
    fn update(&mut self, words: &Vec<i64>, timing: &mut Timings) -> bool {
        let start = Instant::now();
        let admitted = match &mut self.backend {
            Backend::Runner(r) => {
                let s = r.selection();
                Some(
                    r.prepare_control_update(
                        s.graph_epoch,
                        s.control_epoch,
                        black_box(words),
                        &IDS,
                    )
                    .expect("update admitted"),
                )
            }
            Backend::Graph(_) | Backend::Native(_) => None,
        };
        let staged = if admitted.is_none() {
            Some(words.clone())
        } else {
            None
        };
        timing.record("control_admission", start.elapsed().as_nanos());
        let activation = Instant::now();
        match &mut self.backend {
            Backend::Runner(r) => {
                r.activate_control_update(admitted.expect("token"))
                    .expect("activate");
            }
            Backend::Graph(h) | Backend::Native(h) => {
                h.controls = staged.expect("host staged words");
                h.control_epoch += 1;
            }
        }
        timing.record("control_activation", activation.elapsed().as_nanos());
        self.controls.clone_from(words);
        true
    }

    fn reject_update(&mut self, words: &[i64], stale: bool) -> bool {
        match &mut self.backend {
            Backend::Runner(r) => {
                let selected = r.selection();
                assert!(
                    r.update_controls(
                        selected.graph_epoch + u64::from(stale),
                        selected.control_epoch,
                        words,
                        &IDS
                    )
                    .is_err(),
                    "rejected update admitted"
                );
            }
            Backend::Graph(h) | Backend::Native(h) => {
                let is_invalid = stale || words.len() != h.controls.len();
                assert!(is_invalid, "host rejected valid update");
            }
        }
        false
    }

    fn reset(&mut self, kind: Kind) -> bool {
        let (state, new_controls) = kind.initial();
        match &mut self.backend {
            Backend::Runner(r) => {
                let s = r.selection();
                r.reset(
                    &IDS,
                    s.epochs(),
                    super::super::start(kind.graph(), kind.partition(), &state, &new_controls),
                )
                .expect("explicit reset");
            }
            Backend::Graph(h) => {
                h.graph = Some(GraphMachine::prepare(kind));
                h.state = state;
                h.controls = new_controls.clone();
                h.output = None;
                h.next_tick = 0;
                h.kind = kind;
                h.graph_epoch += 1;
                h.control_epoch += 1;
            }
            Backend::Native(h) => {
                h.state = state;
                h.controls = new_controls.clone();
                h.output = None;
                h.next_tick = 0;
                h.kind = kind;
                h.graph_epoch += 1;
                h.control_epoch += 1;
            }
        }
        self.current = kind;
        self.controls.clone_from(&new_controls);
        true
    }
}
