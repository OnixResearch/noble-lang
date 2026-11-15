use super::super::{add_graph, fixture, sub_graph, ONE};
use crate::common::IDS;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::ir::Op;
use noble_ngraph::tick::{Limits, Refusal, Runner};

fn check_profile_and_partition() -> Limits {
    let mut graph = add_graph();
    graph.nodes[4].op = Op::I64Sub(noble_ngraph::ir::NodeId(0), noble_ngraph::ir::NodeId(3));
    assert!(Runner::prepare(
        &IDS,
        super::super::start(graph, ONE, &[0], &[1]),
        Limits::default()
    )
    .is_ok());
    let mut wrong = ONE;
    wrong.state_words = 2;
    assert_eq!(
        Runner::prepare(
            &IDS,
            super::super::start(add_graph(), wrong, &[0, 0], &[1]),
            Limits::default()
        )
        .unwrap_err(),
        Refusal::Partition
    );
    let limits = Limits {
        state_bytes: 7,
        ..Limits::default()
    };
    assert_eq!(
        Runner::prepare(
            &IDS,
            super::super::start(add_graph(), ONE, &[0], &[1]),
            limits.clone()
        )
        .unwrap_err(),
        Refusal::Budget("state_bytes")
    );
    limits
}

fn check_work_cap(limits: &mut Limits) {
    limits.state_bytes = 8;
    limits.tick_work = 4;
    assert_eq!(
        Runner::prepare(
            &IDS,
            super::super::start(add_graph(), ONE, &[0], &[1]),
            limits.clone()
        )
        .unwrap_err(),
        Refusal::Budget("tick_work")
    );
    limits.tick_work = 5;
    assert!(Runner::prepare(
        &IDS,
        super::super::start(add_graph(), ONE, &[0], &[1]),
        limits.clone()
    )
    .is_ok());
}

fn check_schedule_and_memory(limits: &mut Limits) {
    let checked = noble_ngraph::check::check(&IDS, add_graph()).unwrap();
    let packed = Compiled::new(&IDS, &checked);
    let work = noble_ngraph::schedule::declared_work(
        &checked,
        &packed.packed_layout(),
        packed.i64_instructions().unwrap(),
    );
    limits.schedule_work = work - 1;
    assert!(matches!(
        Runner::prepare(
            &IDS,
            super::super::start(add_graph(), ONE, &[0], &[1]),
            limits.clone()
        ),
        Err(Refusal::Schedule(
            noble_ngraph::schedule::StreamRefusal::Exhausted { .. }
        ))
    ));
    limits.schedule_work = work;
    assert!(Runner::prepare(
        &IDS,
        super::super::start(add_graph(), ONE, &[0], &[1]),
        limits.clone()
    )
    .is_ok());
    let active = fixture(add_graph(), ONE, &[0], &[1]);
    limits.retained_bytes = active.retained_bytes() - 1;
    assert_eq!(
        Runner::prepare(
            &IDS,
            super::super::start(add_graph(), ONE, &[0], &[1]),
            limits.clone()
        )
        .unwrap_err(),
        Refusal::Budget("retained_bytes")
    );
    limits.retained_bytes = active.retained_bytes();
    let mut r = Runner::prepare(
        &IDS,
        super::super::start(add_graph(), ONE, &[0], &[1]),
        limits.clone(),
    )
    .unwrap();
    let selected = r.selection();
    assert_eq!(
        r.reset(
            &IDS,
            super::super::epochs(0, 0),
            super::super::start(sub_graph(), ONE, &[999], &[2])
        ),
        Err(Refusal::Budget("replacement_peak_bytes"))
    );
    assert_eq!(r.selection(), selected);
    assert_eq!(r.state(), &[0]);
}

#[test]
fn profile_and_byte_work_boundaries_refuse_before_activation() {
    let mut limits = check_profile_and_partition();
    check_work_cap(&mut limits);
    check_schedule_and_memory(&mut limits);
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
        Runner::prepare(
            &IDS,
            super::super::start(b.finish(), ONE, &[0], &[1]),
            Limits::default()
        )
        .unwrap_err(),
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
