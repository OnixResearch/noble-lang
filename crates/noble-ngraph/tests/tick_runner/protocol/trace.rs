use super::super::{
    add_graph, expected_one, expected_two, fixture, sub_graph, two_graph, ONE, TWO,
};
use crate::common::IDS;
use noble_ngraph::eval::Control;
use noble_ngraph::tick::Runner;

fn first_ticks(r: &mut Runner) -> i64 {
    let mut state = i64::MAX - 1;
    let control = 3;
    for (tick, input) in [1, -2, i64::MIN, -1].into_iter().enumerate() {
        let next = expected_one(state, input, control, false);
        super::compare_engines(add_graph(), &[state, input, control], &[next, next]);
        let selected = r.selection();
        let result = r
            .tick(
                selected.graph_epoch,
                selected.control_epoch,
                tick as u64,
                &[input],
                &mut Control::default(),
            )
            .expect("tick");
        assert_eq!(result.selected, selected);
        assert_eq!(result.state, &[next]);
        assert_eq!(result.output, &[next]);
        state = next;
    }
    state
}

fn changed_controls_and_graph(r: &mut Runner, state: i64) {
    let old = r.selection();
    r.update_controls(0, 0, &[-4], &IDS)
        .expect("control update");
    assert_ne!(r.selection().controls, old.controls);
    assert_eq!(r.state(), &[state]);
    assert_eq!(r.output(), Some(&[state][..]));
    let control = -4;
    let next = expected_one(state, i64::MAX, control, false);
    super::compare_engines(add_graph(), &[state, i64::MAX, control], &[next, next]);
    let result = r
        .tick(0, 1, 4, &[i64::MAX], &mut Control::default())
        .expect("updated tick");
    assert_eq!(result.state, &[next]);
    assert_eq!(result.output, &[next]);

    // Same partition, different meaning: the old state cannot cross a reset.
    let selected = r
        .reset(
            &IDS,
            super::super::epochs(0, 1),
            super::super::start(sub_graph(), ONE, &[i64::MIN + 1], &[7]),
        )
        .expect("explicit reset");
    assert_eq!(
        (
            selected.graph_epoch,
            selected.control_epoch,
            selected.next_tick
        ),
        (1, 2, 0)
    );
    assert_eq!(r.output(), None);
    assert_eq!(r.state(), &[i64::MIN + 1]);
    let want = expected_one(i64::MIN + 1, 3, 7, true);
    super::compare_engines(sub_graph(), &[i64::MIN + 1, 3, 7], &[want, want]);
    let result = r
        .tick(1, 2, 0, &[3], &mut Control::default())
        .expect("post-reset tick");
    assert_eq!((result.state, result.output), (&[want][..], &[want][..]));
}

fn two_state_after_reset(r: &mut Runner) {
    // Multi-word asymmetric subtraction and non-state output order.
    r.reset(
        &IDS,
        super::super::epochs(1, 2),
        super::super::start(two_graph(), TWO, &[i64::MIN, 7], &[3, -5]),
    )
    .expect("second reset");
    let mut state = [i64::MIN, 7];
    let mut controls = [3, -5];
    for (tick, input) in [[-1, 4], [i64::MAX, -7], [2, i64::MIN]]
        .into_iter()
        .enumerate()
    {
        if tick == 1 {
            r.update_controls(2, 3, &[i64::MIN, 9], &IDS)
                .expect("two controls");
            controls = [i64::MIN, 9];
        }
        let (next, output) = expected_two(state, input, controls);
        super::compare_engines(
            two_graph(),
            &[
                state[0],
                state[1],
                input[0],
                input[1],
                controls[0],
                controls[1],
            ],
            &[next[0], next[1], output[0], output[1]],
        );
        let selected = r.selection();
        let result = r
            .tick(
                selected.graph_epoch,
                selected.control_epoch,
                tick as u64,
                &input,
                &mut Control::default(),
            )
            .expect("multi-state tick");
        assert_eq!(result.state, next);
        assert_eq!(result.output, output);
        state = next;
    }
}

#[test]
fn complete_state_output_matches_both_engines_and_host_oracle() {
    let mut r = fixture(add_graph(), ONE, &[i64::MAX - 1], &[3]);
    let state = first_ticks(&mut r);
    changed_controls_and_graph(&mut r, state);
    two_state_after_reset(&mut r);
}
