//! Pure host-ledger transitions for live-slot root and frame-pin accounting.
//!
//! # Scope (Owner Decision 57 extraction precursor)
//!
//! This module is a tested, Aeneas-extractable precursor for the first
//! `OWN-PO-6` host-ledger accounting unit under Owner Decision 57
//! (`move-to-extracted-rust`). It is **not** yet a shipping JavaScript
//! authority path and does **not** maintain shadow state over authoritative JS
//! state; the live-slot JS runtime remains authoritative until the synchronous
//! worker boundary cutover is wired and measured.
//!
//! # Production anchors
//!
//! - `SlotRegistry` (`crates/noble-cli/src/core/runtime/slot-registry.mjs`):
//!   `pinRoot`, `enterFrame`, `leaveFrame`, `framePinCounts`, `finishRoot`,
//!   `abortRoot`, and `#poisoned` / `#check`.
//! - `SlotEngine` (`crates/noble-cli/src/core/runtime/slot-engine.mjs`):
//!   `invoke` exception cleanup (`catch` block) and `unsettledGuest`.
//! - `live-borrows.wat` (`crates/noble-wasm/runtime/live-borrows.wat`):
//!   frame sidecar bounds `[1077248, 1310720)` with minimum 16-byte records,
//!   yielding [`MAX_ACTIVE_FRAMES`].
//!
//! # JavaScript adapter interface contract (for future cutover)
//!
//! When the JS worker cuts over to this extracted subject, the adapter MUST:
//!
//! 1. Translate only its own authenticated opaque identities (JS root token,
//!    monotonically issued frame token, pinned map epoch) into [`RootToken`],
//!    [`FrameToken`], and [`MapEpoch`]. Guest integers and public JSONL fields
//!    are never ledger tokens.
//! 2. Serialize all transitions for a root with **no guest re-entry** between
//!    planning (`plan_abort_root` / `plan_finish_root`), executing the planned
//!    backend actions, and acknowledging (`ack_abort_root` / `ack_finish_root`).
//! 3. On abnormal exit (`SlotEngine.invoke` `catch`):
//!    - Release any provisional frozen replay owner and provisional saved
//!      owners, capture the root trace, and read the host-owned guest
//!      settlement words (`guestFaulted`, `failure`, `cp`, `rp`,
//!      `GUEST_DISPATCH_FLAG`, `GUEST_CURRENT_FRAME`, `live_frame`) without
//!      entering the guest.
//!    - Call [`plan_abort_root`]. On `Err`, leave the ledger untouched and fail
//!      closed.
//!    - Execute **only** the actions authorized by [`AbortPlan::actions`]:
//!      - Call `runtime.clear_stack()` **only** when
//!        [`AbortActions::clear_operand_stack`] is `true`; otherwise record
//!        [`ClearStackAck::NotAttempted`] (never resume an unsettled guest).
//!      - Release the root's map pin and run version retirement when
//!        [`AbortActions::retire_aborted_root_versions`] is `true`.
//!      - Run module retirement sweep only when
//!        [`AbortActions::sweep_module_retirement`] is `true` and prior
//!        backend steps succeeded.
//!    - Call [`ack_abort_root`] with the observed [`AbortAck`], commit
//!      [`AbortOutcome::next`], retain [`AbortOutcome::counts`], and report
//!      [`AbortOutcome::exit_release`] and [`AbortOutcome::session_state`].
//!
//! # Non-claims
//!
//! This unit covers root/frame-pin accounting and exact-release classification
//! only. CAS/retained maps, saved-program graphs, code-span retirement, Lean
//! refinement theorems over the extracted subject, and worker correspondence
//! remain open obligations for `OWN-PO-6` and `OWN-PO-7`.

mod transition;

pub use transition::{
    ack_abort_root, ack_finish_root, classify_guest, enter_frame, frame_pin_counts, leave_frame,
    plan_abort_root, plan_finish_root,
};

/// Maximum concurrent active borrowed frames for one root.
///
/// Derived from the live-slot frame sidecar `[1077248, 1310720)` (233,472
/// bytes) in `live-borrows.wat` and `slot-registry.mjs` divided by the minimum
/// 16-byte frame record header.
pub const MAX_ACTIVE_FRAMES: u32 = 14_592;

/// Host-assigned opaque root identity (`1..=u64::MAX`; `0` is invalid).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootToken(pub u64);

/// Host-assigned monotonic frame-pin identity (`1..=u32::MAX`; `0` is invalid).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameToken(pub u32);

/// Pinned registry map epoch retained by a root until finish or abort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapEpoch(pub u64);

/// Lifecycle state of one root in the host ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[tigerstyle::state_machine_enum]
pub enum RootStatus {
    Active,
    Finished,
    Aborted,
}

/// Host ledger / session health (`SlotRegistry.#poisoned`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[tigerstyle::state_machine_enum]
pub enum SessionHealth {
    Healthy,
    Poisoned,
}

/// Operator-observable per-root frame-pin counts (`SlotRegistry.framePinCounts`
/// and `SlotRegistry.abortRoot`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramePinCounts {
    pub entered: u32,
    pub released: u32,
    pub active: u32,
    pub released_on_abort: u32,
}

/// Complete pure host-ledger snapshot for one root and its frame-pin stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub root: RootToken,
    pub map_epoch: MapEpoch,
    pub frames: alloc::vec::Vec<FrameToken>,
    pub entered: u32,
    pub released: u32,
    pub pending_dispatch: bool,
    pub status: RootStatus,
    pub session: SessionHealth,
}

impl Snapshot {
    /// Construct a fresh active root pinned to `map_epoch`.
    ///
    /// Refuses `RootToken(0)`, matching `SlotRegistry.pinRoot` which issues
    /// strictly positive identities starting at `1`.
    pub const fn pinned(root: RootToken, map_epoch: MapEpoch) -> Result<Self, Error> {
        if root.0 == 0 {
            return Err(Error::InvalidRootToken);
        }
        Ok(Self {
            root,
            map_epoch,
            frames: alloc::vec::Vec::new(),
            entered: 0,
            released: 0,
            pending_dispatch: false,
            status: RootStatus::Active,
            session: SessionHealth::Healthy,
        })
    }
}

/// Explicit typed refusal. A refusal never mutates the snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[tigerstyle::state_machine_enum]
pub enum Error {
    SessionPoisoned,
    InvalidRootToken,
    ForeignRoot,
    InactiveRoot,
    InvalidFrameToken,
    DuplicateOrUnorderedFrame,
    FrameBoundExceeded,
    FrameCounterExhausted,
    MalformedCounts,
    PendingDispatch,
    NonLifoFrameRelease,
    ActiveFramesRemaining,
}

/// Exact reason why backend release could not be proved on exit (Decision 6 /
/// `SlotEngine.invoke` `exit_release.reason`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[tigerstyle::state_machine_enum]
pub enum UnprovenReason {
    FrozenReplayReleaseFailed,
    ProvisionalSavedOwnerReleaseFailed,
    RootTraceUnavailable,
    GuestCallInterrupted,
    GuestFailureRecorded,
    GuestWorkOpen,
    GuestInsideBorrowedFrame,
    AbortedFramePinsRetainBackendRefs,
    PendingDispatchUnresolved,
    SessionPoisonedDuringRoot,
    UnexpectedGuestResumption,
    OperandRootsNotCleared,
    OperandRootReleaseRefused,
    OperandRootReleaseTrapped,
    VersionRetirementFailed,
    RetirementSweepFailed,
}

/// Result of inspecting the host-owned guest runtime words without re-entering
/// the guest (`SlotEngine.unsettledGuest`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[tigerstyle::state_machine_enum]
pub enum GuestSettlement {
    Settled,
    Unsettled(UnprovenReason),
}

/// Host-read guest state after an abnormal root exit (`SlotEngine.unsettledGuest`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuestObservation {
    /// True if a Wasm guest call threw/trapped (`guestFaulted`).
    pub guest_faulted: bool,
    /// Shared global `failure` (`this.shared.failure.value`).
    pub failure_flag: u32,
    /// Shared global `cp` (`this.shared.cp.value`).
    pub continuation_depth: u32,
    /// Shared global `rp` (`this.shared.rp.value`).
    pub reflection_depth: u32,
    /// Memory word `GUEST_DISPATCH_FLAG` (address 57352).
    pub dispatch_flag: u32,
    /// Memory word `GUEST_CURRENT_FRAME` (address 1050624).
    pub current_frame_ptr: u32,
    /// Shared global `live_frame` (`this.shared.live_frame.value`).
    pub live_frame_token: u32,
}

impl GuestObservation {
    /// A completely settled guest: no trap, zero failure flag, and no open
    /// continuation, reflection, dispatch, or borrowed frame.
    pub const fn settled() -> Self {
        Self {
            guest_faulted: false,
            failure_flag: 0,
            continuation_depth: 0,
            reflection_depth: 0,
            dispatch_flag: 0,
            current_frame_ptr: 0,
            live_frame_token: 0,
        }
    }
}

/// Host observations gathered in `SlotEngine.invoke`'s `catch` block before
/// planning the root abort transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreAbortObservation {
    /// Whether releasing any provisional frozen replay owner succeeded.
    pub replay_release_ok: bool,
    /// Whether releasing every provisional saved Program owner succeeded.
    pub provisional_owners_release_ok: bool,
    /// Whether root trace inspection (`trace` and `traceSize`) succeeded.
    pub trace_available: bool,
    /// Whether the engine had already poisoned itself during the root
    /// (for example on a checkpoint channel failure).
    pub engine_poisoned_during_root: bool,
    /// Host-read guest settlement words.
    pub guest: GuestObservation,
}

impl PreAbortObservation {
    /// Clean pre-abort host state with a settled guest (for example a refusal
    /// while saving outputs after normal guest return).
    pub const fn settled_clean() -> Self {
        Self {
            replay_release_ok: true,
            provisional_owners_release_ok: true,
            trace_available: true,
            engine_poisoned_during_root: false,
            guest: GuestObservation::settled(),
        }
    }
}

/// Adapter acknowledgement of the backend operand-root `clear_stack` step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[tigerstyle::state_machine_enum]
pub enum ClearStackAck {
    NotAttempted,
    Cleared,
    Refused,
    Trapped,
}

/// Adapter acknowledgement of a backend retirement step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[tigerstyle::state_machine_enum]
pub enum BackendAck {
    NotAttempted,
    Succeeded,
    Failed,
}

/// Whether backend release was proved exact (`exit_release` in `SlotEngine`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[tigerstyle::state_machine_enum]
pub enum ExitRelease {
    Exact,
    Unproven(UnprovenReason),
}

/// Resulting host session disposition (`session_state` in `SlotEngine`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[tigerstyle::state_machine_enum]
pub enum SessionState {
    Retained,
    TerminatedAfterGuestFailure,
}

/// Committed result of an `enter_frame` or `leave_frame` transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameTransition {
    pub next: Snapshot,
    pub counts: FramePinCounts,
}

/// Backend actions required after planning `finish_root`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinishActions {
    pub retire_map_pin: MapEpoch,
    pub retire_root_versions: bool,
    pub sweep_module_retirement: bool,
}

/// Opaque plan produced by [`plan_finish_root`]; consumed once by
/// [`ack_finish_root`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinishPlan {
    pub(crate) root: RootToken,
    pub(crate) map_epoch: MapEpoch,
    pub(crate) counts: FramePinCounts,
    pub(crate) actions: FinishActions,
}

impl FinishPlan {
    pub const fn root(&self) -> RootToken {
        self.root
    }

    pub const fn map_epoch(&self) -> MapEpoch {
        self.map_epoch
    }

    pub const fn counts(&self) -> FramePinCounts {
        self.counts
    }

    pub const fn actions(&self) -> FinishActions {
        self.actions
    }
}

/// Adapter acknowledgement for [`ack_finish_root`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinishAck {
    pub root: RootToken,
    pub retire_versions: BackendAck,
    pub sweep_retirement: BackendAck,
}

/// Committed result of [`ack_finish_root`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinishOutcome {
    pub next: Snapshot,
    pub counts: FramePinCounts,
    pub exit_release: ExitRelease,
    pub session_state: SessionState,
}

/// Backend actions authorized by [`plan_abort_root`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbortActions {
    /// True only when the guest is settled and no prior condition already
    /// forces session poisoning. When `false`, the adapter MUST NOT call
    /// `runtime.clear_stack()`.
    pub clear_operand_stack: bool,
    /// Map epoch whose root pin is retired from the host ledger.
    pub retire_map_pin: MapEpoch,
    /// Always `true`: `SlotRegistry.abortRoot` retires unreachable versions
    /// after releasing the root's map pin.
    pub retire_aborted_root_versions: bool,
    /// True only when exact release can still be proved (`SlotEngine.invoke`).
    pub sweep_module_retirement: bool,
}

/// Opaque plan produced by [`plan_abort_root`]; consumed once by
/// [`ack_abort_root`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbortPlan {
    pub(crate) root: RootToken,
    pub(crate) map_epoch: MapEpoch,
    pub(crate) counts: FramePinCounts,
    pub(crate) released_frames: alloc::vec::Vec<FrameToken>,
    pub(crate) actions: AbortActions,
    pub(crate) plan_unproven: Option<UnprovenReason>,
}

impl AbortPlan {
    pub const fn root(&self) -> RootToken {
        self.root
    }

    pub const fn map_epoch(&self) -> MapEpoch {
        self.map_epoch
    }

    pub const fn counts(&self) -> FramePinCounts {
        self.counts
    }

    pub const fn actions(&self) -> AbortActions {
        self.actions
    }

    pub const fn plan_unproven(&self) -> Option<UnprovenReason> {
        self.plan_unproven
    }

    /// Remaining frame pins to release from the host ledger, in exact LIFO
    /// (innermost-first) order.
    pub fn released_frames(&self) -> &[FrameToken] {
        self.released_frames.as_slice()
    }
}

/// Adapter acknowledgement for [`ack_abort_root`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbortAck {
    pub root: RootToken,
    pub clear_stack: ClearStackAck,
    pub retire_versions: BackendAck,
    pub sweep_retirement: BackendAck,
}

/// Committed result of [`ack_abort_root`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbortOutcome {
    pub next: Snapshot,
    pub counts: FramePinCounts,
    pub released_frames: alloc::vec::Vec<FrameToken>,
    pub exit_release: ExitRelease,
    pub session_state: SessionState,
}

/// Returned when an acknowledgement targets the wrong or invalid root; the
/// original plan is returned intact with no mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RejectedAck<T> {
    pub error: Error,
    pub plan: T,
}
