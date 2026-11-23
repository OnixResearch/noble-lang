use noble_kernel::ledger::{
    ack_abort_root, ack_finish_root, classify_guest, enter_frame, frame_pin_counts, leave_frame,
    plan_abort_root, plan_finish_root, AbortAck, BackendAck, ClearStackAck, Error, ExitRelease,
    FinishAck, FramePinCounts, FrameToken, GuestObservation, GuestSettlement, MapEpoch,
    PreAbortObservation, RootStatus, RootToken, SessionHealth, SessionState, Snapshot,
    UnprovenReason, MAX_ACTIVE_FRAMES,
};

fn active_root(id: u64, epoch: u64) -> Snapshot {
    Snapshot::pinned(RootToken(id), MapEpoch(epoch)).expect("valid pinned root")
}

fn exact_abort_ack(root: RootToken) -> AbortAck {
    AbortAck {
        root,
        clear_stack: ClearStackAck::Cleared,
        retire_versions: BackendAck::Succeeded,
        sweep_retirement: BackendAck::Succeeded,
    }
}

fn no_resume_abort_ack(root: RootToken) -> AbortAck {
    AbortAck {
        root,
        clear_stack: ClearStackAck::NotAttempted,
        retire_versions: BackendAck::Succeeded,
        sweep_retirement: BackendAck::NotAttempted,
    }
}

#[test]
fn abort_releases_remaining_frames_in_exact_lifo_order_once_each() {
    let root = RootToken(7);
    let mut ledger = active_root(7, 12);
    for token in [10, 20, 30, 40] {
        ledger = enter_frame(&ledger, root, FrameToken(token))
            .expect("enter frame")
            .next;
    }
    // Normal LIFO leave of innermost frame 40, followed by a new frame 50.
    ledger = leave_frame(&ledger, root, FrameToken(40))
        .expect("leave frame 40")
        .next;
    ledger = enter_frame(&ledger, root, FrameToken(50))
        .expect("enter frame 50")
        .next;

    let before_counts = frame_pin_counts(&ledger, root).expect("active counts");
    assert_eq!(
        before_counts,
        FramePinCounts {
            entered: 5,
            released: 1,
            active: 4,
            released_on_abort: 0,
        }
    );

    let plan =
        plan_abort_root(&ledger, root, PreAbortObservation::settled_clean()).expect("abort plan");
    assert_eq!(plan.root(), root);
    assert_eq!(plan.map_epoch(), MapEpoch(12));
    // Innermost-first LIFO order; already-left frame 40 is never re-released.
    assert_eq!(
        plan.released_frames(),
        &[
            FrameToken(50),
            FrameToken(30),
            FrameToken(20),
            FrameToken(10),
        ]
    );
    assert_eq!(
        plan.counts(),
        FramePinCounts {
            entered: 5,
            released: 5,
            active: 0,
            released_on_abort: 4,
        }
    );
    assert!(!plan.actions().clear_operand_stack);
    assert!(plan.actions().retire_aborted_root_versions);
    assert!(!plan.actions().sweep_module_retirement);
    assert_eq!(
        plan.plan_unproven(),
        Some(UnprovenReason::AbortedFramePinsRetainBackendRefs)
    );

    let outcome = ack_abort_root(plan, no_resume_abort_ack(root)).expect("ack abort");
    assert_eq!(
        outcome.released_frames,
        vec![
            FrameToken(50),
            FrameToken(30),
            FrameToken(20),
            FrameToken(10),
        ]
    );
    assert_eq!(outcome.next.status, RootStatus::Aborted);
    assert!(outcome.next.frames.is_empty());
    assert!(!outcome.next.pending_dispatch);
    assert_eq!(outcome.next.entered, 5);
    assert_eq!(outcome.next.released, 5);
    assert_eq!(outcome.next.session, SessionHealth::Poisoned);
    assert_eq!(
        outcome.exit_release,
        ExitRelease::Unproven(UnprovenReason::AbortedFramePinsRetainBackendRefs)
    );
    assert_eq!(
        outcome.session_state,
        SessionState::TerminatedAfterGuestFailure
    );
}

#[test]
fn normal_lifo_leave_and_finish_root_retains_session_or_poisons_on_backend_failure() {
    let root = RootToken(3);
    let mut ledger = active_root(3, 5);
    ledger = enter_frame(&ledger, root, FrameToken(1))
        .expect("enter 1")
        .next;
    ledger = enter_frame(&ledger, root, FrameToken(2))
        .expect("enter 2")
        .next;

    // Non-LIFO leave is refused with no mutation.
    let snapshot = ledger.clone();
    assert_eq!(
        leave_frame(&ledger, root, FrameToken(1)),
        Err(Error::NonLifoFrameRelease)
    );
    assert_eq!(ledger, snapshot);

    // Pending dispatch blocks leaveFrame and finishRoot.
    let mut pending = ledger.clone();
    pending.pending_dispatch = true;
    assert_eq!(
        leave_frame(&pending, root, FrameToken(2)),
        Err(Error::PendingDispatch)
    );
    assert_eq!(
        plan_finish_root(&pending, root),
        Err(Error::ActiveFramesRemaining)
    );

    // Unwind in strict LIFO order.
    ledger = leave_frame(&ledger, root, FrameToken(2))
        .expect("leave 2")
        .next;
    assert_eq!(
        plan_finish_root(&ledger, root),
        Err(Error::ActiveFramesRemaining)
    );
    ledger = leave_frame(&ledger, root, FrameToken(1))
        .expect("leave 1")
        .next;
    assert_eq!(
        leave_frame(&ledger, root, FrameToken(1)),
        Err(Error::NonLifoFrameRelease)
    );

    // Pending dispatch on an empty frame stack still refuses finishRoot.
    let mut pending_empty = ledger.clone();
    pending_empty.pending_dispatch = true;
    assert_eq!(
        plan_finish_root(&pending_empty, root),
        Err(Error::PendingDispatch)
    );

    // Exact finish retains session.
    let plan = plan_finish_root(&ledger, root).expect("finish plan");
    assert_eq!(plan.actions().retire_map_pin, MapEpoch(5));
    assert!(plan.actions().retire_root_versions);
    assert!(plan.actions().sweep_module_retirement);
    let ok = ack_finish_root(
        plan,
        FinishAck {
            root,
            retire_versions: BackendAck::Succeeded,
            sweep_retirement: BackendAck::Succeeded,
        },
    )
    .expect("finish ack");
    assert_eq!(ok.next.status, RootStatus::Finished);
    assert_eq!(ok.next.session, SessionHealth::Healthy);
    assert_eq!(ok.exit_release, ExitRelease::Exact);
    assert_eq!(ok.session_state, SessionState::Retained);

    // Committed retirement failure poisons session even though root is finished.
    let failed_retire = ack_finish_root(
        plan,
        FinishAck {
            root,
            retire_versions: BackendAck::Failed,
            sweep_retirement: BackendAck::NotAttempted,
        },
    )
    .expect("committed retirement failure still retires root");
    assert_eq!(failed_retire.next.status, RootStatus::Finished);
    assert_eq!(failed_retire.next.session, SessionHealth::Poisoned);
    assert_eq!(
        failed_retire.exit_release,
        ExitRelease::Unproven(UnprovenReason::VersionRetirementFailed)
    );
    assert_eq!(
        failed_retire.session_state,
        SessionState::TerminatedAfterGuestFailure
    );

    let failed_sweep = ack_finish_root(
        plan,
        FinishAck {
            root,
            retire_versions: BackendAck::Succeeded,
            sweep_retirement: BackendAck::Failed,
        },
    )
    .expect("sweep failure poisons");
    assert_eq!(failed_sweep.next.session, SessionHealth::Poisoned);
    assert_eq!(
        failed_sweep.exit_release,
        ExitRelease::Unproven(UnprovenReason::RetirementSweepFailed)
    );
}

#[test]
fn refuses_inactive_stale_foreign_zero_and_poisoned_roots_without_mutation() {
    assert_eq!(
        Snapshot::pinned(RootToken(0), MapEpoch(1)),
        Err(Error::InvalidRootToken)
    );

    let ledger = enter_frame(&active_root(11, 2), RootToken(11), FrameToken(4))
        .expect("enter")
        .next;
    let baseline = ledger.clone();

    // Zero claim root refused everywhere.
    assert_eq!(
        frame_pin_counts(&ledger, RootToken(0)),
        Err(Error::InvalidRootToken)
    );
    assert_eq!(
        enter_frame(&ledger, RootToken(0), FrameToken(5)),
        Err(Error::InvalidRootToken)
    );
    assert_eq!(
        leave_frame(&ledger, RootToken(0), FrameToken(4)),
        Err(Error::InvalidRootToken)
    );
    assert_eq!(
        plan_finish_root(&ledger, RootToken(0)),
        Err(Error::InvalidRootToken)
    );
    assert_eq!(
        plan_abort_root(&ledger, RootToken(0), PreAbortObservation::settled_clean()),
        Err(Error::InvalidRootToken)
    );
    assert_eq!(ledger, baseline);

    // Foreign root refused everywhere.
    let foreign = RootToken(99);
    assert_eq!(frame_pin_counts(&ledger, foreign), Err(Error::ForeignRoot));
    assert_eq!(
        enter_frame(&ledger, foreign, FrameToken(5)),
        Err(Error::ForeignRoot)
    );
    assert_eq!(
        leave_frame(&ledger, foreign, FrameToken(4)),
        Err(Error::ForeignRoot)
    );
    assert_eq!(plan_finish_root(&ledger, foreign), Err(Error::ForeignRoot));
    assert_eq!(
        plan_abort_root(&ledger, foreign, PreAbortObservation::settled_clean()),
        Err(Error::ForeignRoot)
    );
    assert_eq!(ledger, baseline);

    // Stale root after retained exact abort refuses all further operations.
    let clean_root = active_root(12, 3);
    let plan = plan_abort_root(
        &clean_root,
        RootToken(12),
        PreAbortObservation::settled_clean(),
    )
    .expect("plan clean abort");
    // Mismatched ack root returns plan intact with no mutation.
    let rejected = ack_abort_root(plan, exact_abort_ack(RootToken(77)))
        .expect_err("foreign ack must be rejected");
    assert_eq!(rejected.error, Error::ForeignRoot);
    let rejected_zero = ack_abort_root(rejected.plan, exact_abort_ack(RootToken(0)))
        .expect_err("zero ack must be rejected");
    assert_eq!(rejected_zero.error, Error::InvalidRootToken);

    let retired = ack_abort_root(rejected_zero.plan, exact_abort_ack(RootToken(12)))
        .expect("genuine ack commits")
        .next;
    assert_eq!(retired.status, RootStatus::Aborted);
    assert_eq!(retired.session, SessionHealth::Healthy);
    assert_eq!(
        plan_abort_root(
            &retired,
            RootToken(12),
            PreAbortObservation::settled_clean()
        ),
        Err(Error::InactiveRoot)
    );
    assert_eq!(
        plan_finish_root(&retired, RootToken(12)),
        Err(Error::InactiveRoot)
    );
    assert_eq!(
        enter_frame(&retired, RootToken(12), FrameToken(1)),
        Err(Error::InactiveRoot)
    );
    assert_eq!(
        leave_frame(&retired, RootToken(12), FrameToken(1)),
        Err(Error::InactiveRoot)
    );

    // Stale root after finish refuses all further operations.
    let finished = ack_finish_root(
        plan_finish_root(&clean_root, RootToken(12)).expect("finish plan"),
        FinishAck {
            root: RootToken(12),
            retire_versions: BackendAck::Succeeded,
            sweep_retirement: BackendAck::Succeeded,
        },
    )
    .expect("finish ack")
    .next;
    assert_eq!(
        plan_abort_root(
            &finished,
            RootToken(12),
            PreAbortObservation::settled_clean()
        ),
        Err(Error::InactiveRoot)
    );

    // Poisoned session refuses every transition.
    let mut poisoned = ledger.clone();
    poisoned.session = SessionHealth::Poisoned;
    assert_eq!(
        plan_abort_root(
            &poisoned,
            RootToken(11),
            PreAbortObservation::settled_clean()
        ),
        Err(Error::SessionPoisoned)
    );
    assert_eq!(
        leave_frame(&poisoned, RootToken(11), FrameToken(4)),
        Err(Error::SessionPoisoned)
    );
    assert_eq!(
        plan_finish_root(&poisoned, RootToken(11)),
        Err(Error::SessionPoisoned)
    );
}

#[test]
fn refuses_malformed_counts_invalid_or_duplicate_frames_and_bounds() {
    let root = RootToken(5);

    // Underflow: released > entered.
    let underflow = Snapshot {
        root,
        map_epoch: MapEpoch(1),
        frames: vec![],
        entered: 1,
        released: 2,
        pending_dispatch: false,
        status: RootStatus::Active,
        session: SessionHealth::Healthy,
    };
    assert_eq!(
        plan_abort_root(&underflow, root, PreAbortObservation::settled_clean()),
        Err(Error::MalformedCounts)
    );

    // Mismatch: entered - released (2) != frames.len() (1).
    let too_few_frames = Snapshot {
        root,
        map_epoch: MapEpoch(1),
        frames: vec![FrameToken(1)],
        entered: 3,
        released: 1,
        pending_dispatch: false,
        status: RootStatus::Active,
        session: SessionHealth::Healthy,
    };
    assert_eq!(
        plan_abort_root(&too_few_frames, root, PreAbortObservation::settled_clean()),
        Err(Error::MalformedCounts)
    );

    // Mismatch: entered - released (1) != frames.len() (2).
    let too_many_frames = Snapshot {
        root,
        map_epoch: MapEpoch(1),
        frames: vec![FrameToken(1), FrameToken(2)],
        entered: 2,
        released: 1,
        pending_dispatch: false,
        status: RootStatus::Active,
        session: SessionHealth::Healthy,
    };
    assert_eq!(
        plan_abort_root(&too_many_frames, root, PreAbortObservation::settled_clean()),
        Err(Error::MalformedCounts)
    );

    // Zero frame token inside active stack.
    let zero_frame = Snapshot {
        root,
        map_epoch: MapEpoch(1),
        frames: vec![FrameToken(1), FrameToken(0)],
        entered: 2,
        released: 0,
        pending_dispatch: false,
        status: RootStatus::Active,
        session: SessionHealth::Healthy,
    };
    assert_eq!(
        plan_abort_root(&zero_frame, root, PreAbortObservation::settled_clean()),
        Err(Error::InvalidFrameToken)
    );

    // Duplicate frame token inside active stack (would release same frame twice).
    let dup_frame = Snapshot {
        root,
        map_epoch: MapEpoch(1),
        frames: vec![FrameToken(4), FrameToken(4)],
        entered: 2,
        released: 0,
        pending_dispatch: false,
        status: RootStatus::Active,
        session: SessionHealth::Healthy,
    };
    assert_eq!(
        plan_abort_root(&dup_frame, root, PreAbortObservation::settled_clean()),
        Err(Error::DuplicateOrUnorderedFrame)
    );

    // Out-of-order frame stack.
    let unordered_frame = Snapshot {
        root,
        map_epoch: MapEpoch(1),
        frames: vec![FrameToken(9), FrameToken(3)],
        entered: 2,
        released: 0,
        pending_dispatch: false,
        status: RootStatus::Active,
        session: SessionHealth::Healthy,
    };
    assert_eq!(
        plan_abort_root(&unordered_frame, root, PreAbortObservation::settled_clean()),
        Err(Error::DuplicateOrUnorderedFrame)
    );

    // Entering zero, duplicate, or decreasing token is refused.
    let one_frame = enter_frame(&active_root(5, 1), root, FrameToken(7))
        .expect("enter 7")
        .next;
    assert_eq!(
        enter_frame(&one_frame, root, FrameToken(0)),
        Err(Error::InvalidFrameToken)
    );
    assert_eq!(
        enter_frame(&one_frame, root, FrameToken(7)),
        Err(Error::DuplicateOrUnorderedFrame)
    );
    assert_eq!(
        enter_frame(&one_frame, root, FrameToken(6)),
        Err(Error::DuplicateOrUnorderedFrame)
    );

    // Entering past MAX_ACTIVE_FRAMES is refused.
    let max_count = usize::try_from(MAX_ACTIVE_FRAMES).expect("fits usize");
    let mut full_frames = Vec::with_capacity(max_count);
    for token in 1..=MAX_ACTIVE_FRAMES {
        full_frames.push(FrameToken(token));
    }
    let at_bound = Snapshot {
        root,
        map_epoch: MapEpoch(1),
        frames: full_frames,
        entered: MAX_ACTIVE_FRAMES,
        released: 0,
        pending_dispatch: false,
        status: RootStatus::Active,
        session: SessionHealth::Healthy,
    };
    assert_eq!(
        enter_frame(
            &at_bound,
            root,
            FrameToken(MAX_ACTIVE_FRAMES.saturating_add(1))
        ),
        Err(Error::FrameBoundExceeded)
    );

    // Entering past u32::MAX total entered count refuses without wrapping.
    let saturated = Snapshot {
        root,
        map_epoch: MapEpoch(1),
        frames: vec![],
        entered: u32::MAX,
        released: u32::MAX,
        pending_dispatch: false,
        status: RootStatus::Active,
        session: SessionHealth::Healthy,
    };
    assert_eq!(
        enter_frame(&saturated, root, FrameToken(1)),
        Err(Error::FrameCounterExhausted)
    );
}

#[test]
fn settled_clean_abort_retains_session_only_when_every_backend_ack_succeeds() {
    let root = RootToken(21);
    let mut ledger = active_root(21, 9);
    ledger = enter_frame(&ledger, root, FrameToken(1))
        .expect("enter 1")
        .next;
    ledger = enter_frame(&ledger, root, FrameToken(2))
        .expect("enter 2")
        .next;
    ledger = leave_frame(&ledger, root, FrameToken(2))
        .expect("leave 2")
        .next;
    ledger = leave_frame(&ledger, root, FrameToken(1))
        .expect("leave 1")
        .next;

    let plan = plan_abort_root(&ledger, root, PreAbortObservation::settled_clean())
        .expect("plan settled abort");
    assert!(plan.actions().clear_operand_stack);
    assert!(plan.actions().retire_aborted_root_versions);
    assert!(plan.actions().sweep_module_retirement);
    assert_eq!(plan.plan_unproven(), None);
    assert!(plan.released_frames().is_empty());
    assert_eq!(
        plan.counts(),
        FramePinCounts {
            entered: 2,
            released: 2,
            active: 0,
            released_on_abort: 0,
        }
    );

    // All three backend acks succeeded -> exact release, session retained.
    let exact = ack_abort_root(plan.clone(), exact_abort_ack(root)).expect("exact ack");
    assert_eq!(exact.exit_release, ExitRelease::Exact);
    assert_eq!(exact.session_state, SessionState::Retained);
    assert_eq!(exact.next.session, SessionHealth::Healthy);
    assert_eq!(exact.next.status, RootStatus::Aborted);

    // Every incomplete or failed backend ack poisons the session.
    let failure_cases = [
        (
            AbortAck {
                root,
                clear_stack: ClearStackAck::NotAttempted,
                retire_versions: BackendAck::Succeeded,
                sweep_retirement: BackendAck::Succeeded,
            },
            UnprovenReason::OperandRootsNotCleared,
        ),
        (
            AbortAck {
                root,
                clear_stack: ClearStackAck::Refused,
                retire_versions: BackendAck::Succeeded,
                sweep_retirement: BackendAck::Succeeded,
            },
            UnprovenReason::OperandRootReleaseRefused,
        ),
        (
            AbortAck {
                root,
                clear_stack: ClearStackAck::Trapped,
                retire_versions: BackendAck::Succeeded,
                sweep_retirement: BackendAck::Succeeded,
            },
            UnprovenReason::OperandRootReleaseTrapped,
        ),
        (
            AbortAck {
                root,
                clear_stack: ClearStackAck::Cleared,
                retire_versions: BackendAck::Failed,
                sweep_retirement: BackendAck::NotAttempted,
            },
            UnprovenReason::VersionRetirementFailed,
        ),
        (
            AbortAck {
                root,
                clear_stack: ClearStackAck::Cleared,
                retire_versions: BackendAck::NotAttempted,
                sweep_retirement: BackendAck::Succeeded,
            },
            UnprovenReason::VersionRetirementFailed,
        ),
        (
            AbortAck {
                root,
                clear_stack: ClearStackAck::Cleared,
                retire_versions: BackendAck::Succeeded,
                sweep_retirement: BackendAck::Failed,
            },
            UnprovenReason::RetirementSweepFailed,
        ),
        (
            AbortAck {
                root,
                clear_stack: ClearStackAck::Cleared,
                retire_versions: BackendAck::Succeeded,
                sweep_retirement: BackendAck::NotAttempted,
            },
            UnprovenReason::RetirementSweepFailed,
        ),
    ];

    for (ack, expected_reason) in failure_cases {
        let outcome = ack_abort_root(plan.clone(), ack).expect("ack completes with poison");
        assert_eq!(outcome.next.status, RootStatus::Aborted);
        assert_eq!(outcome.next.session, SessionHealth::Poisoned);
        assert_eq!(
            outcome.session_state,
            SessionState::TerminatedAfterGuestFailure
        );
        assert_eq!(outcome.exit_release, ExitRelease::Unproven(expected_reason));
    }
}

#[test]
fn unsettled_guest_and_pre_abort_failures_forbid_guest_resumption_and_poison() {
    let root = RootToken(33);
    let ledger = active_root(33, 4);

    let unsettled_guests = [
        (
            GuestObservation {
                guest_faulted: true,
                ..GuestObservation::settled()
            },
            UnprovenReason::GuestCallInterrupted,
        ),
        (
            GuestObservation {
                failure_flag: 4,
                ..GuestObservation::settled()
            },
            UnprovenReason::GuestFailureRecorded,
        ),
        (
            GuestObservation {
                continuation_depth: 1,
                ..GuestObservation::settled()
            },
            UnprovenReason::GuestWorkOpen,
        ),
        (
            GuestObservation {
                reflection_depth: 1,
                ..GuestObservation::settled()
            },
            UnprovenReason::GuestWorkOpen,
        ),
        (
            GuestObservation {
                dispatch_flag: 1,
                ..GuestObservation::settled()
            },
            UnprovenReason::GuestWorkOpen,
        ),
        (
            GuestObservation {
                current_frame_ptr: 1_077_248,
                ..GuestObservation::settled()
            },
            UnprovenReason::GuestInsideBorrowedFrame,
        ),
        (
            GuestObservation {
                live_frame_token: 3,
                ..GuestObservation::settled()
            },
            UnprovenReason::GuestInsideBorrowedFrame,
        ),
    ];

    for (guest, expected_reason) in unsettled_guests {
        assert_eq!(
            classify_guest(guest),
            GuestSettlement::Unsettled(expected_reason)
        );
        let pre_abort = PreAbortObservation {
            guest,
            ..PreAbortObservation::settled_clean()
        };
        let plan = plan_abort_root(&ledger, root, pre_abort).expect("unsettled abort plan");
        assert!(!plan.actions().clear_operand_stack);
        assert!(!plan.actions().sweep_module_retirement);
        assert_eq!(plan.plan_unproven(), Some(expected_reason));

        // Compliant adapter does not resume the guest -> poisons with exact reason.
        let compliant = ack_abort_root(plan.clone(), no_resume_abort_ack(root)).expect("ack");
        assert_eq!(compliant.next.session, SessionHealth::Poisoned);
        assert_eq!(
            compliant.exit_release,
            ExitRelease::Unproven(expected_reason)
        );
        assert_eq!(
            compliant.session_state,
            SessionState::TerminatedAfterGuestFailure
        );

        // Non-compliant adapter resuming clear_stack on an unsettled guest is caught and poisoned.
        let resumed = ack_abort_root(plan, exact_abort_ack(root)).expect("caught resumption");
        assert_eq!(resumed.next.session, SessionHealth::Poisoned);
        assert_eq!(
            resumed.exit_release,
            ExitRelease::Unproven(UnprovenReason::UnexpectedGuestResumption)
        );
    }

    // Pre-abort host cleanup failures also forbid clear_stack and poison.
    let pre_abort_failures = [
        (
            PreAbortObservation {
                replay_release_ok: false,
                ..PreAbortObservation::settled_clean()
            },
            UnprovenReason::FrozenReplayReleaseFailed,
        ),
        (
            PreAbortObservation {
                provisional_owners_release_ok: false,
                ..PreAbortObservation::settled_clean()
            },
            UnprovenReason::ProvisionalSavedOwnerReleaseFailed,
        ),
        (
            PreAbortObservation {
                trace_available: false,
                ..PreAbortObservation::settled_clean()
            },
            UnprovenReason::RootTraceUnavailable,
        ),
        (
            PreAbortObservation {
                engine_poisoned_during_root: true,
                ..PreAbortObservation::settled_clean()
            },
            UnprovenReason::SessionPoisonedDuringRoot,
        ),
    ];

    for (pre_abort, expected_reason) in pre_abort_failures {
        let plan = plan_abort_root(&ledger, root, pre_abort).expect("pre-abort failure plan");
        assert!(!plan.actions().clear_operand_stack);
        let outcome = ack_abort_root(plan, no_resume_abort_ack(root)).expect("ack");
        assert_eq!(outcome.next.session, SessionHealth::Poisoned);
        assert_eq!(outcome.exit_release, ExitRelease::Unproven(expected_reason));
    }

    // Pending dispatch with zero active frames (defensive snapshot) still forbids exact release.
    let mut pending = active_root(33, 4);
    pending.pending_dispatch = true;
    let plan = plan_abort_root(&pending, root, PreAbortObservation::settled_clean())
        .expect("pending dispatch abort plan");
    assert!(!plan.actions().clear_operand_stack);
    assert_eq!(
        plan.plan_unproven(),
        Some(UnprovenReason::PendingDispatchUnresolved)
    );
    let outcome = ack_abort_root(plan, no_resume_abort_ack(root)).expect("ack");
    assert_eq!(outcome.next.session, SessionHealth::Poisoned);
    assert_eq!(
        outcome.exit_release,
        ExitRelease::Unproven(UnprovenReason::PendingDispatchUnresolved)
    );
}
