//! Fixed operation order and independent expected outcome for each replay row.

use super::rows::{Replacement, Reset, Step, Trace, Update};
use noble_ngraph::eval::Control;
use noble_ngraph::tick::Runner;

pub(super) fn replay(trace: &mut Trace, r: &mut Runner) {
    first_add(trace, r);
    second_add(trace, r);
    sub(trace, r);
    two(trace, r);
}

fn first_add(trace: &mut Trace, r: &mut Runner) {
    trace.advance(
        r,
        Step {
            key: "add",
            controls: &[2],
            expected: (0, 0, 0),
            input: &[3],
            control: Control::default(),
            extra: "",
            class: "ok",
        },
    );
    trace.advance(
        r,
        Step {
            key: "add",
            controls: &[2],
            expected: (0, 0, 0),
            input: &[3],
            control: Control::default(),
            extra: "",
            class: "stale",
        },
    );
    trace.update(
        r,
        Update {
            key: "add",
            controls: &[2],
            expected: (0, 0),
            next: &[],
            class: "malformed",
        },
    );
    trace.update(
        r,
        Update {
            key: "add",
            controls: &[4],
            expected: (0, 0),
            next: &[4],
            class: "ok",
        },
    );
    trace.advance(
        r,
        Step {
            key: "add",
            controls: &[4],
            expected: (0, 0, 1),
            input: &[5],
            control: Control::default(),
            extra: "",
            class: "stale",
        },
    );
}

fn second_add(trace: &mut Trace, r: &mut Runner) {
    let cancel = |_: u32| true;
    trace.advance(
        r,
        Step {
            key: "add",
            controls: &[4],
            expected: (0, 1, 1),
            input: &[5],
            control: Control {
                cancel: Some(&cancel),
                ..Default::default()
            },
            extra: ",\"cancel_before_commit\":true",
            class: "cancelled",
        },
    );
    trace.advance(
        r,
        Step {
            key: "add",
            controls: &[4],
            expected: (0, 1, 1),
            input: &[5],
            control: Control::default(),
            extra: "",
            class: "ok",
        },
    );
    trace.reset(
        r,
        Reset {
            active_key: "add",
            active_controls: &[4],
            expected: (0, 1),
            next: Replacement {
                key: "sub",
                graph: super::super::sub_graph(),
                partition: super::super::ONE,
                state: &[101],
                controls: &[7],
            },
            class: "ok",
        },
    );
}

fn sub(trace: &mut Trace, r: &mut Runner) {
    trace.advance(
        r,
        Step {
            key: "sub",
            controls: &[7],
            expected: (1, 2, 0),
            input: &[3],
            control: Control::default(),
            extra: "",
            class: "ok",
        },
    );
    trace.reset(
        r,
        Reset {
            active_key: "sub",
            active_controls: &[7],
            expected: (1, 2),
            next: Replacement {
                key: "two",
                graph: super::super::two_graph(),
                partition: super::super::TWO,
                state: &[17, -13],
                controls: &[2, 3],
            },
            class: "ok",
        },
    );
}

fn two(trace: &mut Trace, r: &mut Runner) {
    trace.advance(
        r,
        Step {
            key: "two",
            controls: &[2, 3],
            expected: (2, 3, 0),
            input: &[5, -4],
            control: Control::default(),
            extra: "",
            class: "ok",
        },
    );
    trace.update(
        r,
        Update {
            key: "two",
            controls: &[9, -7],
            expected: (2, 3),
            next: &[9, -7],
            class: "ok",
        },
    );
    trace.advance(
        r,
        Step {
            key: "two",
            controls: &[9, -7],
            expected: (2, 4, 1),
            input: &[-2, 1],
            control: Control {
                fuel: Some(0),
                ..Default::default()
            },
            extra: ",\"work_budget\":0",
            class: "budget",
        },
    );
    after_budget(trace, r);
}

fn after_budget(trace: &mut Trace, r: &mut Runner) {
    trace.advance(
        r,
        Step {
            key: "two",
            controls: &[9, -7],
            expected: (2, 4, 1),
            input: &[-2, 1],
            control: Control::default(),
            extra: "",
            class: "ok",
        },
    );
    trace.advance(
        r,
        Step {
            key: "two",
            controls: &[9, -7],
            expected: (2, 4, 2),
            input: &[1, 1],
            control: Control::default(),
            extra: "",
            class: "ok",
        },
    );
    trace.advance(
        r,
        Step {
            key: "two",
            controls: &[9, -7],
            expected: (2, 4, 3),
            input: &[1, 1],
            control: Control::default(),
            extra: "",
            class: "exhausted",
        },
    );
    trace.reset(
        r,
        Reset {
            active_key: "two",
            active_controls: &[9, -7],
            expected: (2, 4),
            next: Replacement {
                key: "add",
                graph: super::super::add_graph(),
                partition: super::super::ONE,
                state: &[],
                controls: &[3],
            },
            class: "malformed",
        },
    );
}
