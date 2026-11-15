use super::{add_graph, expected_one, sub_graph, two_graph, ONE, TWO};
use crate::common::IDS;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{Control, Obs};
use noble_ngraph::schedule::{self, I64Instruction};
use noble_ngraph::sem::Value;
use noble_ngraph::tick::{Limits, Runner};

fn actual(graph: noble_ngraph::ir::Graph, inputs: &[i64]) -> Vec<i64> {
    let checked = noble_ngraph::check::check(&IDS, graph).unwrap();
    let packed = Compiled::new(&IDS, &checked);
    let inputs: Vec<_> = inputs.iter().copied().map(Value::I64).collect();
    let Obs::Normal(values) = packed.evaluate(&inputs, None, &mut Control::default()).obs else {
        panic!("total graph did not evaluate");
    };
    values
        .into_iter()
        .map(|v| match v {
            Value::I64(n) => n,
            Value::Bool(_) | Value::F32(_) => panic!("not I64"),
        })
        .collect()
}

#[test]
fn seeded_faulty_selectors_are_each_detected_by_independent_oracle() {
    // Fixed nondegenerate seed; every fault gets a distinct named, executable
    // counterexample rather than merely citing a possible risk.
    let state = [17, -13];
    let input = [5, -4];
    let old = [2, 3];
    let new = [9, -7];
    let (want_state, want_output) = super::expected_two(state, input, new);
    let want = [want_state[0], want_state[1], want_output[0], want_output[1]];
    let proper = [state[0], state[1], input[0], input[1], new[0], new[1]];
    assert_eq!(actual(two_graph(), &proper), want);

    // Injected faulty selector: take first control from old epoch and second
    // from new epoch within one tick; the real sequential API cannot do this.
    assert_ne!(
        actual(two_graph(), &[17, -13, 5, -4, old[0], new[1]]),
        want,
        "mixed-control-epoch selector must be killed"
    );
    // Injected faulty input-slot/state selector, with all I64 types matching.
    assert_ne!(
        actual(two_graph(), &[-13, 17, 5, -4, 9, -7]),
        want,
        "wrong partition selector must be killed by expected semantics"
    );
    let mut swapped = want;
    swapped[..2].copy_from_slice(&want_output);
    swapped[2..].copy_from_slice(&want_state);
    assert_ne!(
        swapped, want,
        "swapped state/output selector must be killed"
    );
    // Updated tuple must be active on exactly the next tick, never the tick
    // before its update or the tick after it.
    assert_ne!(
        actual(two_graph(), &[17, -13, 5, -4, new[0], new[1]]),
        actual(two_graph(), &[17, -13, 5, -4, old[0], old[1]]),
        "one-tick-early activation selector must be killed"
    );
    assert_ne!(
        actual(two_graph(), &[17, -13, 5, -4, old[0], old[1]]),
        want,
        "one-tick-late activation selector must be killed"
    );

    let old_state = expected_one(11, 3, 2, false);
    let reset_state = 101;
    let want_reset = expected_one(reset_state, 3, 2, true);
    assert_ne!(
        actual(sub_graph(), &[old_state, 3, 2]),
        [want_reset, want_reset],
        "old-state-carryover selector must be killed"
    );
    assert_ne!(
        actual(add_graph(), &[reset_state, 3, 2]),
        [want_reset, want_reset],
        "stale-graph selector must be killed"
    );
    let first_commit = expected_one(11, 3, 2, false);
    assert_ne!(
        expected_one(first_commit, 3, 2, false),
        first_commit,
        "duplicate-commit selector must be killed"
    );
}

#[test]
fn stale_schedule_and_duplicate_publication_controls_use_real_admission() {
    let old = noble_ngraph::check::check(&IDS, add_graph()).unwrap();
    let changed = noble_ngraph::check::check(&IDS, sub_graph()).unwrap();
    let old_compiled = Compiled::new(&IDS, &old);
    let changed_compiled = Compiled::new(&IDS, &changed);
    assert_ne!(old_compiled.layout_key(), changed_compiled.layout_key());
    assert!(
        schedule::check(
            &changed,
            &old_compiled.packed_layout(),
            old_compiled.i64_instructions().unwrap()
        )
        .is_err(),
        "stale graph/schedule declaration must be refused"
    );
    let mut malformed = changed_compiled.i64_instructions().unwrap().to_vec();
    let pos = malformed
        .iter()
        .position(|op| matches!(op, I64Instruction::Sub(_, _, _)))
        .unwrap();
    if let I64Instruction::Sub(dst, lhs, rhs) = malformed[pos] {
        malformed[pos] = I64Instruction::Add(dst, lhs, rhs);
    }
    assert_eq!(
        schedule::check(&changed, &changed_compiled.packed_layout(), &malformed),
        Err(schedule::StreamRefusal::BodyMismatch)
    );
    let mut r = Runner::prepare(
        &IDS,
        super::start(add_graph(), ONE, &[11], &[2]),
        Limits::default(),
    )
    .unwrap();
    r.tick(0, 0, 0, &[3], &mut Control::default()).unwrap();
    assert_eq!(
        r.tick(0, 0, 0, &[3], &mut Control::default()),
        Err(noble_ngraph::tick::Refusal::TickOrder),
        "a duplicate publish through the real API must be refused"
    );
    let last = r.state().to_vec();
    assert_eq!(last, [17]);
    r.reset(
        &IDS,
        super::epochs(0, 0),
        super::start(two_graph(), TWO, &[17, -13], &[2, 3]),
    )
    .unwrap();
    assert!(r.output().is_none());
}
