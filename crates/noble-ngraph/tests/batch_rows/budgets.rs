#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::explicit_defaults,
    tigerstyle::non_trait_imports,
    tigerstyle::assertion_density,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::ambiguous_params,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this harness builds one checked graph, runs it as a batch and compares every row with independent single-row runs, so checked unwraps, default controls and dense assertions are the readable form."
)]
//! Fuel, cancellation and reuse of the graph-backed batch (`noble_ngraph::batch::Prepared`), each
//! compared with independent single-row runs (issue #41).

use std::cell::RefCell;

use noble_ngraph::batch::{Limits, Prepared, Stop};
use noble_ngraph::diag::BudgetKind;
use noble_ngraph::eval::Obs;
use noble_ngraph::sem::Value;

use super::graph::{compiled, flatten, independent, mixed_rows, OPEN};

#[test]
fn run_fuel_stops_inside_the_row_that_needed_more() {
    let (_, compiled) = compiled();
    let ordinary = [Value::I64(2), Value::I64(3)];
    let cost = independent(&compiled, &ordinary, None).evaluated_nodes;
    assert!(cost > 0);
    let rows = flatten(&[ordinary; 4]);
    let mut prepared = Prepared::new(&compiled).unwrap();

    for run_fuel in [2 * cost, 2 * cost + cost / 2] {
        let outcome = prepared.run(
            4,
            &rows,
            Limits {
                run_fuel: Some(run_fuel),
                ..OPEN
            },
            None,
        );
        assert_eq!(
            outcome.stop,
            Some(Stop::Exhausted { row: 2 }),
            "run fuel {run_fuel} covers exactly two rows"
        );
        assert_eq!(
            outcome.reports.len(),
            3,
            "rows 0..3 started; row 3 never ran"
        );
        assert_eq!(outcome.reports[0], independent(&compiled, &ordinary, None));
        assert_eq!(outcome.reports[1], independent(&compiled, &ordinary, None));
        assert_eq!(
            outcome.reports[2].obs,
            Obs::Exhausted(BudgetKind::Fuel),
            "the row that ran out reports it"
        );
    }

    // A row fuel below one row's cost is each row's own outcome; the run goes on.
    let short = Limits {
        row_fuel: Some(cost - 1),
        ..OPEN
    };
    let outcome = prepared.run(4, &rows, short, None);
    assert_eq!(outcome.stop, None);
    let exhausted = independent(&compiled, &ordinary, Some(cost - 1));
    assert_eq!(exhausted.obs, Obs::Exhausted(BudgetKind::Fuel));
    assert_eq!(outcome.reports, vec![exhausted; 4]);

    // A tie between row and run fuel goes to the row: row 0 completes, and the empty run budget
    // then stops the run in row 1.
    let tied = Limits {
        row_fuel: Some(cost),
        run_fuel: Some(cost),
        ..OPEN
    };
    let outcome = prepared.run(4, &rows, tied, None);
    assert_eq!(outcome.stop, Some(Stop::Exhausted { row: 1 }));
    assert_eq!(outcome.reports[0], independent(&compiled, &ordinary, None));
    assert_eq!(outcome.reports[1].obs, Obs::Exhausted(BudgetKind::Fuel));
}

#[test]
fn cancellation_names_the_first_row_that_never_ran() {
    let (_, compiled) = compiled();
    let rows = mixed_rows();
    let mut prepared = Prepared::new(&compiled).unwrap();
    let asked = RefCell::new(Vec::new());
    let checkpoint = |row: usize| {
        asked.borrow_mut().push(row);
        row == 3
    };
    let outcome = prepared.run(rows.len(), &flatten(&rows), OPEN, Some(&checkpoint));
    assert_eq!(outcome.stop, Some(Stop::Cancelled { next: 3 }));
    assert_eq!(*asked.borrow(), vec![0, 1, 2, 3]);
    for (index, row) in rows.iter().take(3).enumerate() {
        assert_eq!(outcome.reports[index], independent(&compiled, row, None));
    }
    assert_eq!(outcome.reports.len(), 3);
}

#[test]
fn a_preparation_is_reusable_after_failed_and_stopped_runs() {
    let (_, compiled) = compiled();
    let rows = mixed_rows();
    let values = flatten(&rows);
    let mut warm = Prepared::new(&compiled).unwrap();
    let ordinary = independent(&compiled, &[Value::I64(2), Value::I64(3)], None);
    let stopped = warm.run(
        rows.len(),
        &values,
        Limits {
            run_fuel: Some(5 * ordinary.evaluated_nodes),
            ..OPEN
        },
        None,
    );
    assert!(
        matches!(stopped.stop, Some(Stop::Exhausted { .. })),
        "the first run stops on its fuel: {:?}",
        stopped.stop
    );
    let mut fresh = Prepared::new(&compiled).unwrap();
    assert_eq!(
        warm.run(rows.len(), &values, OPEN, None),
        fresh.run(rows.len(), &values, OPEN, None),
        "a run after a stopped run equals the same run on a fresh preparation"
    );
}
