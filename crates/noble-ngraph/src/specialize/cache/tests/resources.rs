use super::*;

fn custom_request(
    session: &mut Session,
    original: &crate::check::CheckedGraph,
    map: &[Option<i64>],
    budgets: &crate::check::Budgets,
    limits: noble_contracts::Limits,
) -> core::result::Result<Result, crate::diag::Diagnostic> {
    let recipe = crate::format::graph::print(original.graph());
    session.specialize(Invocation::new(
        &TestIds,
        original,
        recipe.as_bytes(),
        crate::specialize::Request {
            static_inputs: map,
            budgets,
        },
        limits,
    ))
}

#[test]
fn lowered_limits_do_not_reuse_or_publish_refusals() {
    let original = graph(&TestIds, false, 7, &[11, 12], false, false, false);
    let mut session = Session::default();
    assert_eq!(
        run(&mut session, &TestIds, &original, &[Some(5), None])
            .unwrap()
            .status,
        Status::MissStored
    );
    let budgets = crate::check::Budgets {
        residual_nodes: 0,
        ..crate::check::Budgets::default()
    };
    assert!(custom_request(
        &mut session,
        &original,
        &[Some(5), None],
        &budgets,
        noble_contracts::Limits::default(),
    )
    .is_err());
    assert_eq!(session.len(), 1, "a low-budget refusal is never retained");
    let limits = noble_contracts::Limits {
        bytes: 0,
        ..noble_contracts::Limits::default()
    };
    assert!(custom_request(
        &mut session,
        &original,
        &[Some(5), None],
        &crate::check::Budgets::default(),
        limits,
    )
    .is_err());
    assert_eq!(
        session.len(),
        1,
        "a source-admission refusal is never retained"
    );
    assert_eq!(
        run(&mut session, &TestIds, &original, &[Some(5), None])
            .unwrap()
            .status,
        Status::Hit,
        "the old positive entry remains usable under its own limits"
    );
}

#[test]
fn entry_variant_and_retained_byte_limits_bound_many_requests() {
    let original = graph(&TestIds, false, 7, &[11, 12], false, false, false);
    let mut single = Session::default();
    let first = run(&mut single, &TestIds, &original, &[Some(5), None]).unwrap();
    let one_entry_bytes = single.retained_bytes();
    assert!(one_entry_bytes > 0);
    let old_owner = first.residual;
    let mut session = Session::new(Limits {
        entries: 2,
        variants_per_subject: 1,
        retained_bytes: one_entry_bytes * 4,
        lookup_work: 2,
        eviction_work: 8,
        ..Limits::default()
    })
    .unwrap();
    for value in 0..32_i64 {
        let result = run(&mut session, &TestIds, &original, &[Some(value), None]).unwrap();
        assert_eq!(result.status, Status::MissStored);
        assert_eq!(session.len(), 1, "variants per subject are capped");
        assert!(session.retained_bytes() <= one_entry_bytes * 4);
        assert_eq!(result.retained_bytes, session.retained_bytes());
    }
    assert_eq!(
        run(&mut session, &TestIds, &original, &[Some(31), None])
            .unwrap()
            .status,
        Status::Hit
    );
    assert_eq!(
        evaluate(&old_owner.checked, &[3]),
        evaluate(&original, &[5, 3]),
        "eviction cannot invalidate a previously returned residual"
    );

    let other = graph(&TestIds, true, 7, &[11, 12], false, false, false);
    assert_eq!(
        run(&mut session, &TestIds, &other, &[Some(31), None])
            .unwrap()
            .status,
        Status::MissStored
    );
    assert_eq!(session.len(), 2, "distinct subjects use the entry cap");
    assert_eq!(
        run(&mut session, &TestIds, &original, &[Some(31), None])
            .unwrap()
            .status,
        Status::Hit
    );
    let third = graph(&TestIds, false, 8, &[11, 12], false, false, false);
    assert_eq!(
        run(&mut session, &TestIds, &third, &[Some(31), None])
            .unwrap()
            .status,
        Status::MissStored
    );
    assert_eq!(session.len(), 2, "the entry cap evicts the oldest");
    assert_eq!(
        run(&mut session, &TestIds, &original, &[Some(31), None])
            .unwrap()
            .status,
        Status::MissStored,
        "an evicted subject must miss"
    );

    let mut byte_limited = Session::new(Limits {
        entries: 2,
        variants_per_subject: 2,
        retained_bytes: one_entry_bytes,
        lookup_work: 2,
        ..Limits::default()
    })
    .unwrap();
    assert_eq!(
        run(&mut byte_limited, &TestIds, &original, &[Some(5), None])
            .unwrap()
            .status,
        Status::MissStored
    );
    let changed = run(&mut byte_limited, &TestIds, &original, &[Some(6), None]).unwrap();
    assert_ne!(changed.status, Status::Hit);
    assert!(byte_limited.retained_bytes() <= one_entry_bytes);
    assert!(byte_limited.len() <= 1);
}

#[test]
fn zero_caps_and_eviction_work_decline_publication_without_changing_result() {
    let original = graph(&TestIds, false, 7, &[11, 12], false, false, false);
    let mut disabled = Session::new(Limits {
        entries: 0,
        variants_per_subject: 0,
        lookup_work: 0,
        eviction_work: 0,
        ..Limits::default()
    })
    .unwrap();
    let disabled_result = run(&mut disabled, &TestIds, &original, &[Some(5), None]).unwrap();
    assert_eq!(disabled_result.status, Status::MissUncached);
    assert!(disabled.is_empty());
    assert_eq!(
        evaluate(&disabled_result.readmitted, &[3]),
        evaluate(&original, &[5, 3])
    );
    let mut bounded = Session::new(Limits {
        entries: 1,
        lookup_work: 1,
        eviction_work: 1,
        ..Limits::default()
    })
    .unwrap();
    let first = run(&mut bounded, &TestIds, &original, &[Some(5), None]).unwrap();
    let second = run(&mut bounded, &TestIds, &original, &[Some(6), None]).unwrap();
    assert_eq!(first.status, Status::MissStored);
    assert_eq!(second.status, Status::MissUncached);
    assert_eq!(bounded.len(), 1);
    assert_eq!(
        evaluate(&second.readmitted, &[3]),
        evaluate(&original, &[6, 3])
    );
    assert_eq!(
        run(&mut bounded, &TestIds, &original, &[Some(5), None])
            .unwrap()
            .status,
        Status::Hit
    );

    let mut no_manifest = Session::new(Limits {
        manifest_bytes: 0,
        ..Limits::default()
    })
    .unwrap();
    let bypass = run(&mut no_manifest, &TestIds, &original, &[Some(5), None]).unwrap();
    assert_eq!(bypass.status, Status::BudgetBypass);
    assert!(bypass.specialization_key.is_none());
    assert!(no_manifest.is_empty());
}

#[test]
fn interrupted_publication_cannot_leave_partial_entry() {
    let original = graph(&TestIds, false, 7, &[11, 12], false, false, false);
    let mut session = Session::default();
    let recipe = crate::format::graph::print(original.graph());
    let map = [Some(5), None];
    let budgets = crate::check::Budgets::default();
    let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        session
            .specialize_with_observer(
                Invocation::new(
                    &TestIds,
                    &original,
                    recipe.as_bytes(),
                    crate::specialize::Request {
                        static_inputs: &map,
                        budgets: &budgets,
                    },
                    noble_contracts::Limits::default(),
                ),
                |event| {
                    if event == StageEvent::Begin(Stage::Publication) {
                        panic!("stop before publishing");
                    }
                },
            )
            .unwrap();
    }));
    assert!(interrupted.is_err());
    assert!(session.is_empty());
    assert_eq!(session.retained_bytes(), 0);
    assert_eq!(
        run(&mut session, &TestIds, &original, &map).unwrap().status,
        Status::MissStored
    );
}

/// Static map of the later same-subject request; its larger constant makes
/// the residual strictly larger than the one for `[Some(5), None]`.
const LARGER_MAP: [Option<i64>; 2] = [Some(6_000_000_000), None];

/// Two-subject session holding A (subject S1, `[Some(5), None]`) then B
/// (subject S2) under a byte bound that fits A and B but not B and A',
/// where A' is S1 with [`LARGER_MAP`]. Returns the session, A's source and
/// A''s measured retained bytes.
fn two_subject_session(
    original: &crate::check::CheckedGraph,
    other: &crate::check::CheckedGraph,
    eviction_work: usize,
) -> (Session, alloc::string::String, usize) {
    let measure = |checked: &crate::check::CheckedGraph, map: &[Option<i64>]| {
        let mut alone = Session::default();
        run(&mut alone, &TestIds, checked, map).unwrap();
        alone.retained_bytes()
    };
    let first_bytes = measure(original, &[Some(5), None]);
    let other_bytes = measure(other, &[Some(31), None]);
    let larger_bytes = measure(original, &LARGER_MAP);
    assert!(larger_bytes > first_bytes, "A' must outgrow A");
    let bound = first_bytes + other_bytes;
    assert!(larger_bytes <= bound, "A' fits alone");
    let mut session = Session::new(Limits {
        entries: 2,
        variants_per_subject: 1,
        retained_bytes: bound,
        lookup_work: 2,
        eviction_work,
        ..Limits::default()
    })
    .unwrap();
    let first = run(&mut session, &TestIds, original, &[Some(5), None]).unwrap();
    assert_eq!(first.status, Status::MissStored);
    let second = run(&mut session, &TestIds, other, &[Some(31), None]).unwrap();
    assert_eq!(second.status, Status::MissStored);
    assert_eq!(session.len(), 2);
    assert_eq!(session.retained_bytes(), bound);
    (session, first.source, larger_bytes)
}

#[test]
fn declined_publication_keeps_entries_and_accounting() {
    let original = graph(&TestIds, false, 7, &[11, 12], false, false, false);
    let other = graph(&TestIds, true, 7, &[11, 12], false, false, false);
    // Evicting A costs 2 + 2 and then evicting B costs 1 + 1: six units
    // exceed the five-unit bound, so the publication must decline whole.
    let (mut session, first_source, _) = two_subject_session(&original, &other, 5);
    let entries_before = session.len();
    let bytes_before = session.retained_bytes();
    let declined = run(&mut session, &TestIds, &original, &LARGER_MAP).unwrap();
    assert_eq!(declined.status, Status::MissUncached);
    assert_eq!(
        session.len(),
        entries_before,
        "a declined insertion evicts nothing"
    );
    assert_eq!(session.retained_bytes(), bytes_before);
    assert_eq!(declined.retained_bytes, bytes_before);
    assert_eq!(
        evaluate(&declined.readmitted, &[3]),
        evaluate(&original, &[6_000_000_000, 3])
    );
    let repeated = run(&mut session, &TestIds, &original, &[Some(5), None]).unwrap();
    assert_eq!(repeated.status, Status::Hit, "A stays reusable");
    assert_eq!(repeated.source, first_source);
    assert_eq!(
        run(&mut session, &TestIds, &other, &[Some(31), None])
            .unwrap()
            .status,
        Status::Hit,
        "B stays reusable"
    );
}

#[test]
fn fitting_eviction_plan_evicts_subject_then_oldest() {
    let original = graph(&TestIds, false, 7, &[11, 12], false, false, false);
    let other = graph(&TestIds, true, 7, &[11, 12], false, false, false);
    // The same six-unit plan fits a six-unit bound: A goes for the variant
    // cap, then B for the byte bound, and A' is retained alone.
    let (mut session, _, larger_bytes) = two_subject_session(&original, &other, 6);
    let stored = run(&mut session, &TestIds, &original, &LARGER_MAP).unwrap();
    assert_eq!(stored.status, Status::MissStored);
    assert_eq!(session.len(), 1, "both A and B were evicted");
    assert_eq!(session.retained_bytes(), larger_bytes);
    assert_eq!(stored.retained_bytes, larger_bytes);
    assert_eq!(
        run(&mut session, &TestIds, &original, &LARGER_MAP)
            .unwrap()
            .status,
        Status::Hit
    );
    assert_ne!(
        run(&mut session, &TestIds, &other, &[Some(31), None])
            .unwrap()
            .status,
        Status::Hit,
        "an evicted subject must miss"
    );
}
