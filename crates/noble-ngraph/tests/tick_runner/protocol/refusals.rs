use super::super::{add_graph, sub_graph, ONE};
use crate::common::IDS;
use noble_ngraph::eval::{Control, Obs};
use noble_ngraph::tick::{ControlUpdate, Limits, Refusal, Runner};

fn refuse_before_commit(r: &mut Runner) -> ControlUpdate {
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
        r.reset(
            &IDS,
            super::super::epochs(0, 1),
            super::super::start(sub_graph(), ONE, &[9], &[4])
        ),
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
    r.prepare_control_update(0, 0, &[4], &IDS).unwrap()
}

fn refuse_after_commit(r: &mut Runner, admitted: ControlUpdate) {
    r.tick(0, 0, 0, &[3], &mut Control::default())
        .expect("reuse after refusal");
    assert_eq!(r.state(), &[19]);
    assert_eq!(r.activate_control_update(admitted), Err(Refusal::Stale));
    assert_eq!(r.selection().control_epoch, 0);
    let committed = r.selection();
    let stop = |_: u32| true;
    let mut cancelled = Control {
        cancel: Some(&stop),
        ..Control::default()
    };
    assert_eq!(
        r.tick(0, 0, 1, &[7], &mut cancelled),
        Err(Refusal::Evaluation(Obs::Cancelled))
    );
    assert_eq!(
        r.reset(
            &IDS,
            super::super::epochs(0, 0),
            super::super::start(sub_graph(), ONE, &[], &[7])
        ),
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
}

fn refuse_update_above_byte_cap(mut limits: Limits) {
    limits.update_bytes = 7;
    let mut update_limited = Runner::prepare(
        &IDS,
        super::super::start(add_graph(), ONE, &[1], &[1]),
        limits,
    )
    .unwrap();
    assert_eq!(
        update_limited.update_controls(0, 0, &[9], &IDS),
        Err(Refusal::Budget("update_bytes"))
    );
    assert_eq!(update_limited.state(), &[1]);
    assert_eq!(update_limited.selection().control_epoch, 0);
}

#[test]
fn malformed_stale_cancel_exhaust_reuse_and_terminal_preserve_all() {
    let limits = Limits {
        ticks: 2,
        ..Limits::default()
    };
    let mut r = Runner::prepare(
        &IDS,
        super::super::start(add_graph(), ONE, &[13], &[2]),
        limits.clone(),
    )
    .unwrap();
    let admitted = refuse_before_commit(&mut r);
    refuse_after_commit(&mut r, admitted);
    refuse_update_above_byte_cap(limits);
}
