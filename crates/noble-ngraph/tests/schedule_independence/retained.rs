//! Retained counterexamples of the harness (criteria section 6 and criteria-oracle sections 3-4).
//!
//! The minimized schedules that diverged before their defects were fixed replay against their
//! serial baselines and must now agree; the owning crates keep the exact regression assertions
//! (`noble-kernel` `recursion_schema_cycle/orders.rs` and the `noble-ngraph` cache unit tests).
//!
//! [`PositionLeak`] is the table oracle's planted control: an analysis that leaks its schedule, so
//! the oracle must catch it and retain exactly two rows for it.

use super::{digest, rows_of, Case, Normalized, Tabulate, Verdict};

/// `(replay token, what diverged before the fix)`.
pub(super) const RETAINED: &[(&str, &str)] = &[
    (
        "kernel-dependencies/self-loop-and-entry/forward/work-10000@0,1,2",
        "the dependency walk kept going after the self-loop on item 1 was found and named item 0, \
         which lies on no cycle (noble-kernel validate::dependencies, last error wins)",
    ),
    (
        "cache-sequences/tight-eviction@2,3,1",
        "a declined publication (miss-uncached) still evicted request 2's entry: the session went \
         from (2 entries, 5834 bytes) to (1, 2917) (specialize::cache::Session::retain, evictions \
         not rolled back)",
    ),
];

/// The item whose first-occurrence position is observed.
const MIDDLE: usize = 1;

/// An analysis whose result is the position of the queried item in the schedule.
pub(super) struct PositionLeak;

impl Case for PositionLeak {
    type Seen = usize;

    fn name(&self) -> String {
        String::from("oracle-control/position-leak")
    }

    fn items(&self) -> usize {
        3
    }

    fn is_query_case(&self) -> bool {
        true
    }

    fn run(&self, order: &[usize]) -> Vec<(usize, usize)> {
        let Some(position) = order.iter().position(|item| *item == MIDDLE) else {
            return Vec::new();
        };
        vec![(MIDDLE, position)]
    }

    fn compare(&self, _query: usize, base: &usize, other: &usize) -> Verdict {
        Verdict::equal("first-occurrence position", base, other)
    }

    fn orders(&self) -> Vec<(String, Vec<usize>)> {
        super::query_orders(self.items())
    }

    fn item_schedules(&self) -> Option<u64> {
        None
    }
}

impl Tabulate for PositionLeak {
    fn input(&self, query: usize) -> String {
        format!("oracle-control/position-leak/query-{query}")
    }

    fn normalized(&self, _query: usize, seen: &usize) -> Normalized {
        Normalized::Output(format!("first-occurrence-position-{seen}"))
    }

    fn presentation(&self, _query: usize, _seen: &usize) -> String {
        String::new()
    }

    fn schedule(&self, _query: usize, label: &str, position: usize) -> String {
        super::schedule_id(&self.name(), label, position)
    }
}

/// Reruns one named schedule of the control, if the token names it.
pub(super) fn replay(name: &str, order: &[usize]) -> Option<Result<(), String>> {
    let control = PositionLeak;
    if control.name() != name {
        return None;
    }
    Some(super::replay_case(&control, order))
}

#[test]
fn minimized_divergences_stay_fixed() {
    for (token, _) in RETAINED {
        let (name, order) = super::parse_token(token).expect("retained tokens parse");
        let outcome = super::kernel::replay(&name, &order)
            .or_else(|| super::contracts::replay(&name, &order))
            .or_else(|| super::graph::replay(&name, &order))
            .or_else(|| super::cache::replay(&name, &order))
            .unwrap_or_else(|| panic!("retained token names no case: {token}"));
        assert_eq!(outcome, Ok(()), "retained schedule {token} diverges again");
    }
    println!(
        "NOBLE_SCHEDULE_RETAINED {{\"fixtures\":{}}}",
        RETAINED.len()
    );
}

#[test]
fn planted_order_dependence_is_caught_and_retained() {
    let control = PositionLeak;
    let analysis = super::analyse(&rows_of(&control));
    assert_eq!(
        analysis.conflicts.len(),
        1,
        "the planted control is caught exactly once"
    );
    assert_eq!(analysis.exempt, [0; 5], "the control emits no exemption");
    assert_eq!(
        analysis.presentation_differences, 0,
        "the control presents nothing that may vary"
    );
    let pair = &analysis.conflicts[0];
    assert_eq!(
        pair.first.input, pair.second.input,
        "the retained rows share one input"
    );
    assert_ne!(
        pair.first.normalized, pair.second.normalized,
        "the retained rows differ in normalized output"
    );
    assert_ne!(
        pair.first.schedule, pair.second.schedule,
        "the retained rows come from two schedules"
    );
    let is_seeded = pair.first.schedule.contains("seed-") || pair.second.schedule.contains("seed-");
    assert!(
        is_seeded,
        "at least one retained row comes from a seeded order: {} / {}",
        pair.first.schedule, pair.second.schedule
    );

    // Both rows re-execute to their recorded normalized digest, and at least one differs from the
    // serial baseline, so the existing order mechanism replays the divergence.
    let mut replayed = 0u64;
    for row in [&pair.first, &pair.second] {
        let token = row
            .replay
            .as_deref()
            .expect("the control rows carry an order");
        let (name, order) = super::parse_token(token).expect("the token parses");
        assert_eq!(name, control.name());
        let observed = control.run(&order);
        let seen = observed
            .iter()
            .find(|(query, _)| *query == row.query)
            .map(|(_, seen)| *seen)
            .expect("the replayed order observes the row's query");
        let Normalized::Output(text) = control.normalized(row.query, &seen) else {
            panic!("the control has no exempt row");
        };
        assert_eq!(
            Some(digest(&text)),
            row.normalized,
            "row {} re-executes to its recorded digest",
            row.schedule
        );
        if super::replay_case(&control, &order).is_err() {
            replayed += 1;
        }
    }
    assert!(
        replayed >= 1,
        "at least one retained row replays the divergence through NOBLE_SCHEDULE_REPLAY"
    );

    println!(
        "NOBLE_SCHEDULE_ORACLE_CONTROL {{\"case\":\"{}\",\"rows\":[{},{}],\"reproduced\":2,\"replayed\":{replayed},\"allowed_presentation_differences\":{}}}",
        control.name(),
        pair.first.json(),
        pair.second.json(),
        analysis.presentation_differences
    );
}
