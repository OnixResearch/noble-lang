//! Minimized schedules that diverged before their defects were fixed (criteria section 6).
//! Each token replays one schedule against its serial baseline; all must now agree. The owning
//! crates keep the exact regression assertions (`noble-kernel` `recursion_schema_cycle/orders.rs`
//! and the `noble-ngraph` cache unit tests).

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

#[test]
fn retained_minimized_schedules_agree() {
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
