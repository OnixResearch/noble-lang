use super::{add_graph, expected_one, expected_two, runner, sub_graph, two_graph, ONE, TWO};
use crate::common::IDS;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{Graph, Op};
use noble_ngraph::sem::Value;
use noble_ngraph::tick::{Limits, Refusal, Runner};

fn compare_engines(graph: Graph, inputs: &[i64], expected: &[i64]) {
    let checked = noble_ngraph::check::check(&IDS, graph).expect("checked graph");
    let compiled = Compiled::new(&IDS, &checked);
    let layout = compiled.packed_layout();
    noble_ngraph::schedule::check(
        &checked,
        &layout,
        compiled.i64_instructions().expect("I64 instructions"),
    )
    .expect("actual packed body");
    let values: Vec<_> = inputs.iter().copied().map(Value::I64).collect();
    let want = Obs::Normal(expected.iter().copied().map(Value::I64).collect());
    let reference = eval::reference(&checked, &values, None, &mut Control::default());
    let packed = compiled.evaluate_in(
        &mut compiled.workspace(),
        &values,
        None,
        &mut Control::default(),
    );
    assert_eq!(
        reference.obs, want,
        "independent recurrence versus reference"
    );
    assert_eq!(packed.obs, want, "independent recurrence versus packed");
    assert!(reference.obs.obs_eq(&packed.obs));
}

#[test]
fn complete_state_output_trace_matches_both_engines_and_host_oracle() {
    let mut r = runner(add_graph(), ONE, &[i64::MAX - 1], &[3]);
    let mut state = i64::MAX - 1;
    let mut control = 3;
    for (tick, input) in [1, -2, i64::MIN, -1].into_iter().enumerate() {
        let next = expected_one(state, input, control, false);
        compare_engines(add_graph(), &[state, input, control], &[next, next]);
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
    let old = r.selection();
    r.update_controls(0, 0, &[-4], &IDS)
        .expect("control update");
    assert_ne!(r.selection().controls, old.controls);
    assert_eq!(r.state(), &[state]);
    assert_eq!(r.output(), Some(&[state][..]));
    control = -4;
    let next = expected_one(state, i64::MAX, control, false);
    compare_engines(add_graph(), &[state, i64::MAX, control], &[next, next]);
    let result = r
        .tick(0, 1, 4, &[i64::MAX], &mut Control::default())
        .expect("updated tick");
    assert_eq!(result.state, &[next]);
    assert_eq!(result.output, &[next]);

    // Same partition, different meaning: the old state cannot cross a reset.
    let selected = r
        .reset(&IDS, 0, 1, sub_graph(), ONE, &[i64::MIN + 1], &[7])
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
    compare_engines(sub_graph(), &[i64::MIN + 1, 3, 7], &[want, want]);
    let result = r
        .tick(1, 2, 0, &[3], &mut Control::default())
        .expect("post-reset tick");
    assert_eq!((result.state, result.output), (&[want][..], &[want][..]));

    // Multi-word asymmetric subtraction and non-state output order.
    r.reset(&IDS, 1, 2, two_graph(), TWO, &[i64::MIN, 7], &[3, -5])
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
        compare_engines(
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
fn malformed_stale_cancel_exhaust_reuse_and_terminal_preserve_all() {
    let mut limits = Limits {
        ticks: 2,
        ..Limits::default()
    };
    let mut r = Runner::prepare(&IDS, add_graph(), ONE, &[13], &[2], limits.clone()).unwrap();
    let initial = r.selection();
    assert_eq!(r.output(), None);
    assert_eq!(
        r.tick(0, 1, 0, &[3], &mut Control::default()),
        Err(Refusal::Stale)
    );
    assert_eq!(
        r.tick(0, 0, 1, &[3], &mut Control::default()),
        Err(Refusal::TickOrder)
    );
    assert_eq!(
        r.tick(0, 0, 0, &[], &mut Control::default()),
        Err(Refusal::Partition)
    );
    assert_eq!(r.update_controls(0, 0, &[], &IDS), Err(Refusal::Partition));
    assert_eq!(r.update_controls(1, 0, &[4], &IDS), Err(Refusal::Stale));
    assert_eq!(
        r.reset(&IDS, 0, 1, sub_graph(), ONE, &[9], &[4]),
        Err(Refusal::Stale)
    );
    let stop = |_: u32| true;
    let mut cancelled = Control {
        cancel: Some(&stop),
        ..Control::default()
    };
    assert_eq!(
        r.tick(0, 0, 0, &[3], &mut cancelled),
        Err(Refusal::Evaluation(Obs::Cancelled))
    );
    let mut exhausted = Control {
        fuel: Some(0),
        ..Control::default()
    };
    assert_eq!(
        r.tick(0, 0, 0, &[3], &mut exhausted),
        Err(Refusal::Evaluation(Obs::Exhausted(
            noble_ngraph::diag::BudgetKind::Fuel
        )))
    );
    assert_eq!(r.state(), &[13]);
    assert_eq!(r.output(), None);
    assert_eq!(r.selection(), initial);
    let admitted = r.prepare_control_update(0, 0, &[4], &IDS).unwrap();
    r.tick(0, 0, 0, &[3], &mut Control::default())
        .expect("reuse after refusal");
    assert_eq!(r.state(), &[19]);
    assert_eq!(r.activate_control_update(admitted), Err(Refusal::Stale));
    assert_eq!(r.selection().control_epoch, 0);
    let committed = r.selection();
    assert_eq!(
        r.tick(0, 0, 1, &[7], &mut cancelled),
        Err(Refusal::Evaluation(Obs::Cancelled))
    );
    assert_eq!(
        r.reset(&IDS, 0, 0, sub_graph(), ONE, &[], &[7]),
        Err(Refusal::Partition)
    );
    assert_eq!(r.selection(), committed);
    assert_eq!(r.state(), &[19]);
    assert_eq!(r.output(), Some(&[19][..]));
    assert_eq!(
        r.tick(0, 0, 0, &[3], &mut Control::default()),
        Err(Refusal::TickOrder)
    );
    r.tick(0, 0, 1, &[2], &mut Control::default())
        .expect("second tick");
    let selected = r.selection();
    let state = r.state().to_vec();
    let output = r.output().unwrap().to_vec();
    assert_eq!(
        r.tick(0, 0, 2, &[1], &mut Control::default()),
        Err(Refusal::Terminal)
    );
    assert_eq!(r.selection(), selected);
    assert_eq!(r.state(), state);
    assert_eq!(r.output(), Some(output.as_slice()));
    limits.update_bytes = 7;
    let mut update_limited = Runner::prepare(&IDS, add_graph(), ONE, &[1], &[1], limits).unwrap();
    assert_eq!(
        update_limited.update_controls(0, 0, &[9], &IDS),
        Err(Refusal::Budget("update_bytes"))
    );
    assert_eq!(update_limited.state(), &[1]);
    assert_eq!(update_limited.selection().control_epoch, 0);
}

#[test]
fn profile_and_byte_work_boundaries_refuse_before_activation() {
    let mut graph = add_graph();
    graph.nodes[4].op = Op::I64Sub(noble_ngraph::ir::NodeId(0), noble_ngraph::ir::NodeId(3));
    assert!(Runner::prepare(&IDS, graph, ONE, &[0], &[1], Limits::default()).is_ok());
    let mut wrong = ONE;
    wrong.state_words = 2;
    assert_eq!(
        Runner::prepare(&IDS, add_graph(), wrong, &[0, 0], &[1], Limits::default()).unwrap_err(),
        Refusal::Partition
    );
    let mut limits = Limits::default();
    limits.state_bytes = 7;
    assert_eq!(
        Runner::prepare(&IDS, add_graph(), ONE, &[0], &[1], limits.clone()).unwrap_err(),
        Refusal::Budget("state_bytes")
    );
    limits.state_bytes = 8;
    limits.tick_work = 4;
    assert_eq!(
        Runner::prepare(&IDS, add_graph(), ONE, &[0], &[1], limits.clone()).unwrap_err(),
        Refusal::Budget("tick_work")
    );
    limits.tick_work = 5;
    assert!(Runner::prepare(&IDS, add_graph(), ONE, &[0], &[1], limits.clone()).is_ok());
    let checked = noble_ngraph::check::check(&IDS, add_graph()).unwrap();
    let packed = Compiled::new(&IDS, &checked);
    let work = noble_ngraph::schedule::declared_work(
        &checked,
        &packed.packed_layout(),
        packed.i64_instructions().unwrap(),
    );
    limits.schedule_work = work - 1;
    assert!(matches!(
        Runner::prepare(&IDS, add_graph(), ONE, &[0], &[1], limits.clone()),
        Err(Refusal::Schedule(
            noble_ngraph::schedule::StreamRefusal::Exhausted { .. }
        ))
    ));
    limits.schedule_work = work;
    assert!(Runner::prepare(&IDS, add_graph(), ONE, &[0], &[1], limits.clone()).is_ok());
    let active = runner(add_graph(), ONE, &[0], &[1]);
    limits.retained_bytes = active.retained_bytes() - 1;
    assert_eq!(
        Runner::prepare(&IDS, add_graph(), ONE, &[0], &[1], limits.clone()).unwrap_err(),
        Refusal::Budget("retained_bytes")
    );
    limits.retained_bytes = active.retained_bytes();
    let mut r = Runner::prepare(&IDS, add_graph(), ONE, &[0], &[1], limits).unwrap();
    let selected = r.selection();
    assert_eq!(
        r.reset(&IDS, 0, 0, sub_graph(), ONE, &[999], &[2]),
        Err(Refusal::Budget("replacement_peak_bytes"))
    );
    assert_eq!(r.selection(), selected);
    assert_eq!(r.state(), &[0]);
}

#[test]
fn non_prefix_input_nodes_are_outside_exportable_profile() {
    use noble_ngraph::ir::{GraphBuilder, ScalarType, SubProfile, Subject};
    let mut b = GraphBuilder::new(SubProfile::I64, Subject::authored(&IDS, "tick/nonprefix"));
    let s = b.input(ScalarType::I64);
    let constant = b.i64_const(3);
    let x = b.input(ScalarType::I64);
    let c = b.input(ScalarType::I64);
    let product = b.op(Op::I64Mul(x, c));
    let sum = b.op(Op::I64Add(s, constant));
    let next = b.op(Op::I64Add(sum, product));
    b.output(next);
    b.output(next);
    assert_eq!(
        Runner::prepare(&IDS, b.finish(), ONE, &[0], &[1], Limits::default()).unwrap_err(),
        Refusal::Profile
    );
}

#[test]
fn real_schedule_validator_rejects_stale_graph_instruction_body() {
    let first = noble_ngraph::check::check(&IDS, add_graph()).unwrap();
    let second = noble_ngraph::check::check(&IDS, sub_graph()).unwrap();
    let from_first = Compiled::new(&IDS, &first);
    let from_second = Compiled::new(&IDS, &second);
    assert_ne!(from_first.layout_key(), from_second.layout_key());
    assert!(noble_ngraph::schedule::check(
        &second,
        &from_first.packed_layout(),
        from_first.i64_instructions().unwrap()
    )
    .is_err());
}
