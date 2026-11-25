//! Pure, Aeneas-extractable root and frame-pin transition functions.

/// Classify whether the guest settled cleanly using only host-owned words
/// (`SlotEngine.unsettledGuest`).
pub const fn classify_guest(observation: super::GuestObservation) -> super::GuestSettlement {
    if observation.guest_faulted {
        return super::GuestSettlement::Unsettled(super::UnprovenReason::GuestCallInterrupted);
    }
    if observation.failure_flag != 0 {
        return super::GuestSettlement::Unsettled(super::UnprovenReason::GuestFailureRecorded);
    }
    if observation.continuation_depth != 0
        || observation.reflection_depth != 0
        || observation.dispatch_flag != 0
    {
        return super::GuestSettlement::Unsettled(super::UnprovenReason::GuestWorkOpen);
    }
    if observation.current_frame_ptr != 0 || observation.live_frame_token != 0 {
        return super::GuestSettlement::Unsettled(super::UnprovenReason::GuestInsideBorrowedFrame);
    }
    super::GuestSettlement::Settled
}

/// Read the active root's current frame-pin counts (`SlotRegistry.framePinCounts`).
pub fn frame_pin_counts(
    snapshot: &super::Snapshot,
    claim_root: super::RootToken,
) -> Result<super::FramePinCounts, super::Error> {
    let active = attempt!(validate_active_root(snapshot, claim_root));
    Ok(super::FramePinCounts {
        entered: snapshot.entered,
        released: snapshot.released,
        active,
        released_on_abort: 0,
    })
}

/// Record one entered borrowed frame pin (`SlotRegistry.enterFrame`).
///
/// Refuses without mutating `snapshot` on any invalid, stale, foreign,
/// poisoned, malformed, duplicate/non-monotonic, or bound-exceeding transition.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every untrusted snapshot and token precondition is checked through typed errors rather than panicking assertions."
)]
pub fn enter_frame(
    snapshot: &super::Snapshot,
    claim_root: super::RootToken,
    frame_token: super::FrameToken,
) -> Result<super::FrameTransition, super::Error> {
    let active = attempt!(validate_active_root(snapshot, claim_root));
    if frame_token.0 == 0 {
        return Err(super::Error::InvalidFrameToken);
    }
    let len = snapshot.frames.len();
    if let Some(last_index) = len.checked_sub(1) {
        if frame_token.0 <= snapshot.frames[last_index].0 {
            return Err(super::Error::DuplicateOrUnorderedFrame);
        }
    }
    if active >= super::MAX_ACTIVE_FRAMES {
        return Err(super::Error::FrameBoundExceeded);
    }
    let next_entered = match snapshot.entered.checked_add(1) {
        Some(value) => value,
        None => return Err(super::Error::FrameCounterExhausted),
    };
    let next_active = match active.checked_add(1) {
        Some(value) => value,
        None => return Err(super::Error::FrameBoundExceeded),
    };
    let mut next_frames = copy_frame_prefix(snapshot.frames.as_slice(), len);
    next_frames.push(frame_token);
    let next = super::Snapshot {
        root: snapshot.root,
        map_epoch: snapshot.map_epoch,
        frames: next_frames,
        entered: next_entered,
        released: snapshot.released,
        pending_dispatch: false,
        status: super::RootStatus::Active,
        session: super::SessionHealth::Healthy,
    };
    let counts = super::FramePinCounts {
        entered: next_entered,
        released: snapshot.released,
        active: next_active,
        released_on_abort: 0,
    };
    Ok(super::FrameTransition { next, counts })
}

/// Release the innermost active frame pin (`SlotRegistry.leaveFrame`).
///
/// Refuses without mutating `snapshot` if a dispatch is pending, if the frame
/// stack is empty, if `frame_token` is not the innermost active frame (strict
/// LIFO), or if any ledger invariant fails.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every LIFO and snapshot invariant is enforced through typed refusal rather than panicking assertions."
)]
pub fn leave_frame(
    snapshot: &super::Snapshot,
    claim_root: super::RootToken,
    frame_token: super::FrameToken,
) -> Result<super::FrameTransition, super::Error> {
    let active = attempt!(validate_active_root(snapshot, claim_root));
    if snapshot.pending_dispatch {
        return Err(super::Error::PendingDispatch);
    }
    if frame_token.0 == 0 {
        return Err(super::Error::InvalidFrameToken);
    }
    let last_index = match snapshot.frames.len().checked_sub(1) {
        Some(index) => index,
        None => return Err(super::Error::NonLifoFrameRelease),
    };
    if snapshot.frames[last_index].0 != frame_token.0 {
        return Err(super::Error::NonLifoFrameRelease);
    }
    let next_released = match snapshot.released.checked_add(1) {
        Some(value) => value,
        None => return Err(super::Error::MalformedCounts),
    };
    let next_active = match active.checked_sub(1) {
        Some(value) => value,
        None => return Err(super::Error::MalformedCounts),
    };
    let next_frames = copy_frame_prefix(snapshot.frames.as_slice(), last_index);
    let next = super::Snapshot {
        root: snapshot.root,
        map_epoch: snapshot.map_epoch,
        frames: next_frames,
        entered: snapshot.entered,
        released: next_released,
        pending_dispatch: false,
        status: super::RootStatus::Active,
        session: super::SessionHealth::Healthy,
    };
    let counts = super::FramePinCounts {
        entered: snapshot.entered,
        released: next_released,
        active: next_active,
        released_on_abort: 0,
    };
    Ok(super::FrameTransition { next, counts })
}

/// Plan normal root retirement (`SlotRegistry.finishRoot`).
///
/// Refuses without mutating `snapshot` if any active frame remains, a dispatch
/// is pending, counts are malformed, or the root is inactive/foreign/poisoned.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; plan_finish_root is a total refusal gate whose preconditions return typed errors instead of panicking."
)]
pub fn plan_finish_root(
    snapshot: &super::Snapshot,
    claim_root: super::RootToken,
) -> Result<super::FinishPlan, super::Error> {
    let active = attempt!(validate_active_root(snapshot, claim_root));
    if active != 0 {
        return Err(super::Error::ActiveFramesRemaining);
    }
    if snapshot.pending_dispatch {
        return Err(super::Error::PendingDispatch);
    }
    let counts = super::FramePinCounts {
        entered: snapshot.entered,
        released: snapshot.released,
        active: 0,
        released_on_abort: 0,
    };
    let actions = super::FinishActions {
        retire_map_pin: snapshot.map_epoch,
        retire_root_versions: true,
        sweep_module_retirement: true,
    };
    Ok(super::FinishPlan {
        root: snapshot.root,
        map_epoch: snapshot.map_epoch,
        counts,
        actions,
    })
}

/// Commit a planned normal root retirement using actual backend
/// acknowledgements. Any failed or missing backend retirement poisons the
/// session (`CommittedRetirementError`).
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; ack_finish_root checks token identity via RejectedAck and maps every backend acknowledgement exhaustively without panicking."
)]
pub fn ack_finish_root(
    plan: super::FinishPlan,
    ack: super::FinishAck,
) -> Result<super::FinishOutcome, super::RejectedAck<super::FinishPlan>> {
    if ack.root.0 == 0 {
        return Err(super::RejectedAck {
            error: super::Error::InvalidRootToken,
            plan,
        });
    }
    if ack.root.0 != plan.root.0 {
        return Err(super::RejectedAck {
            error: super::Error::ForeignRoot,
            plan,
        });
    }
    let exit_release = match ack.retire_versions {
        super::BackendAck::Succeeded => match ack.sweep_retirement {
            super::BackendAck::Succeeded => super::ExitRelease::Exact,
            super::BackendAck::NotAttempted | super::BackendAck::Failed => {
                super::ExitRelease::Unproven(super::UnprovenReason::RetirementSweepFailed)
            }
        },
        super::BackendAck::NotAttempted | super::BackendAck::Failed => {
            super::ExitRelease::Unproven(super::UnprovenReason::VersionRetirementFailed)
        }
    };
    let (session, session_state) = disposition(exit_release);
    let next = super::Snapshot {
        root: plan.root,
        map_epoch: plan.map_epoch,
        frames: alloc::vec::Vec::new(),
        entered: plan.counts.entered,
        released: plan.counts.released,
        pending_dispatch: false,
        status: super::RootStatus::Finished,
        session,
    };
    Ok(super::FinishOutcome {
        next,
        counts: plan.counts,
        exit_release,
        session_state,
    })
}

/// Plan abnormal root retirement (`SlotRegistry.abortRoot` + `SlotEngine`
/// exception cleanup).
///
/// Refuses without mutating `snapshot` on poisoned, invalid, foreign, stale, or
/// malformed states. On success, returns an [`super::AbortPlan`] listing every
/// remaining frame pin in exact LIFO (innermost-first) order, balanced
/// post-abort counts (`active == 0`, `released == entered`), and the exact
/// backend actions permitted without resuming an unsettled guest.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every refusal precondition is validated through typed errors before constructing the abort plan."
)]
pub fn plan_abort_root(
    snapshot: &super::Snapshot,
    claim_root: super::RootToken,
    pre_abort: super::PreAbortObservation,
) -> Result<super::AbortPlan, super::Error> {
    let active = attempt!(validate_active_root(snapshot, claim_root));
    let total_released = match snapshot.released.checked_add(active) {
        Some(value) if value == snapshot.entered => value,
        Some(_) | None => return Err(super::Error::MalformedCounts),
    };
    let released_frames = reverse_frames(snapshot.frames.as_slice());
    let plan_unproven = first_plan_unproven(pre_abort, active, snapshot.pending_dispatch);
    let can_prove_exact = plan_unproven.is_none();
    let counts = super::FramePinCounts {
        entered: snapshot.entered,
        released: total_released,
        active: 0,
        released_on_abort: active,
    };
    let actions = super::AbortActions {
        clear_operand_stack: can_prove_exact,
        retire_map_pin: snapshot.map_epoch,
        retire_aborted_root_versions: true,
        sweep_module_retirement: can_prove_exact,
    };
    Ok(super::AbortPlan {
        root: snapshot.root,
        map_epoch: snapshot.map_epoch,
        counts,
        released_frames,
        actions,
        plan_unproven,
    })
}

/// Commit a planned abnormal root retirement using actual backend
/// acknowledgements.
///
/// The root is always retired (`RootStatus::Aborted`) with all frame pins
/// drained and `pending_dispatch` cleared. The session stays
/// `SessionHealth::Healthy` / `SessionState::Retained` **only** when exact
/// backend release is proved (settled guest, zero host-aborted frame pins,
/// operand roots cleared, version retirement and sweep succeeded); otherwise
/// the session transitions to `SessionHealth::Poisoned` /
/// `SessionState::TerminatedAfterGuestFailure`.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; ack_abort_root checks token identity via RejectedAck and resolves every backend acknowledgement into an explicit exact or poisoned outcome."
)]
pub fn ack_abort_root(
    plan: super::AbortPlan,
    ack: super::AbortAck,
) -> Result<super::AbortOutcome, super::RejectedAck<super::AbortPlan>> {
    if ack.root.0 == 0 {
        return Err(super::RejectedAck {
            error: super::Error::InvalidRootToken,
            plan,
        });
    }
    if ack.root.0 != plan.root.0 {
        return Err(super::RejectedAck {
            error: super::Error::ForeignRoot,
            plan,
        });
    }
    let exit_release = resolve_abort_release(plan.actions, plan.plan_unproven, ack);
    let (session, session_state) = disposition(exit_release);
    let next = super::Snapshot {
        root: plan.root,
        map_epoch: plan.map_epoch,
        frames: alloc::vec::Vec::new(),
        entered: plan.counts.entered,
        released: plan.counts.released,
        pending_dispatch: false,
        status: super::RootStatus::Aborted,
        session,
    };
    Ok(super::AbortOutcome {
        next,
        counts: plan.counts,
        released_frames: plan.released_frames,
        exit_release,
        session_state,
    })
}

const fn disposition(
    exit_release: super::ExitRelease,
) -> (super::SessionHealth, super::SessionState) {
    match exit_release {
        super::ExitRelease::Exact => (super::SessionHealth::Healthy, super::SessionState::Retained),
        super::ExitRelease::Unproven(_) => (
            super::SessionHealth::Poisoned,
            super::SessionState::TerminatedAfterGuestFailure,
        ),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; first_plan_unproven is a total priority classifier returning Option<UnprovenReason> without panic paths."
)]
const fn first_plan_unproven(
    pre_abort: super::PreAbortObservation,
    released_on_abort: u32,
    pending_dispatch: bool,
) -> Option<super::UnprovenReason> {
    if !pre_abort.replay_release_ok {
        return Some(super::UnprovenReason::FrozenReplayReleaseFailed);
    }
    if !pre_abort.provisional_owners_release_ok {
        return Some(super::UnprovenReason::ProvisionalSavedOwnerReleaseFailed);
    }
    if !pre_abort.trace_available {
        return Some(super::UnprovenReason::RootTraceUnavailable);
    }
    match classify_guest(pre_abort.guest) {
        super::GuestSettlement::Settled => {}
        super::GuestSettlement::Unsettled(reason) => return Some(reason),
    }
    if released_on_abort != 0 {
        return Some(super::UnprovenReason::AbortedFramePinsRetainBackendRefs);
    }
    if pending_dispatch {
        return Some(super::UnprovenReason::PendingDispatchUnresolved);
    }
    if pre_abort.engine_poisoned_during_root {
        return Some(super::UnprovenReason::SessionPoisonedDuringRoot);
    }
    None
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; resolve_abort_release is a total exhaustive match over plan actions and backend acknowledgements without panic paths."
)]
const fn resolve_abort_release(
    actions: super::AbortActions,
    plan_unproven: Option<super::UnprovenReason>,
    ack: super::AbortAck,
) -> super::ExitRelease {
    if !actions.clear_operand_stack
        && !matches!(ack.clear_stack, super::ClearStackAck::NotAttempted)
    {
        return super::ExitRelease::Unproven(super::UnprovenReason::UnexpectedGuestResumption);
    }
    if let Some(reason) = plan_unproven {
        return super::ExitRelease::Unproven(reason);
    }
    match ack.clear_stack {
        super::ClearStackAck::Cleared => {}
        super::ClearStackAck::NotAttempted => {
            return super::ExitRelease::Unproven(super::UnprovenReason::OperandRootsNotCleared);
        }
        super::ClearStackAck::Refused => {
            return super::ExitRelease::Unproven(super::UnprovenReason::OperandRootReleaseRefused);
        }
        super::ClearStackAck::Trapped => {
            return super::ExitRelease::Unproven(super::UnprovenReason::OperandRootReleaseTrapped);
        }
    }
    match ack.retire_versions {
        super::BackendAck::Succeeded => {}
        super::BackendAck::NotAttempted | super::BackendAck::Failed => {
            return super::ExitRelease::Unproven(super::UnprovenReason::VersionRetirementFailed);
        }
    }
    match ack.sweep_retirement {
        super::BackendAck::Succeeded => super::ExitRelease::Exact,
        super::BackendAck::NotAttempted | super::BackendAck::Failed => {
            super::ExitRelease::Unproven(super::UnprovenReason::RetirementSweepFailed)
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; validate_active_root is the total refusal gate for untrusted snapshots; assertions would convert hostile inputs into panics."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; u32::try_from and Vec::as_slice are non-const on the pinned compiler."
)]
fn validate_active_root(
    snapshot: &super::Snapshot,
    claim_root: super::RootToken,
) -> Result<u32, super::Error> {
    if matches!(snapshot.session, super::SessionHealth::Poisoned) {
        return Err(super::Error::SessionPoisoned);
    }
    if snapshot.root.0 == 0 || claim_root.0 == 0 {
        return Err(super::Error::InvalidRootToken);
    }
    if snapshot.root.0 != claim_root.0 {
        return Err(super::Error::ForeignRoot);
    }
    if !matches!(snapshot.status, super::RootStatus::Active) {
        return Err(super::Error::InactiveRoot);
    }
    let active = match snapshot.entered.checked_sub(snapshot.released) {
        Some(value) => value,
        None => return Err(super::Error::MalformedCounts),
    };
    let stack_len = match u32::try_from(snapshot.frames.len()) {
        Ok(value) => value,
        Err(_) => return Err(super::Error::FrameBoundExceeded),
    };
    if stack_len > super::MAX_ACTIVE_FRAMES || active > super::MAX_ACTIVE_FRAMES {
        return Err(super::Error::FrameBoundExceeded);
    }
    if stack_len != active {
        return Err(super::Error::MalformedCounts);
    }
    attempt!(validate_frame_stack(snapshot.frames.as_slice()));
    Ok(active)
}

const fn validate_frame_stack(frames: &[super::FrameToken]) -> Result<(), super::Error> {
    let mut previous: u32 = 0;
    let mut index: usize = 0;
    while index < frames.len() {
        let token = frames[index].0;
        if token == 0 {
            return Err(super::Error::InvalidFrameToken);
        }
        if token <= previous {
            return Err(super::Error::DuplicateOrUnorderedFrame);
        }
        previous = token;
        index += 1;
    }
    Ok(())
}

fn copy_frame_prefix(
    frames: &[super::FrameToken],
    count: usize,
) -> alloc::vec::Vec<super::FrameToken> {
    let prefix_len = if count < frames.len() {
        count
    } else {
        frames.len()
    };
    let mut result = alloc::vec::Vec::with_capacity(prefix_len);
    let mut index: usize = 0;
    while index < prefix_len {
        result.push(frames[index]);
        index += 1;
    }
    result
}

fn reverse_frames(frames: &[super::FrameToken]) -> alloc::vec::Vec<super::FrameToken> {
    let mut result = alloc::vec::Vec::with_capacity(frames.len());
    let mut remaining = frames.len();
    while let Some(next) = remaining.checked_sub(1) {
        remaining = next;
        result.push(frames[remaining]);
    }
    result
}
