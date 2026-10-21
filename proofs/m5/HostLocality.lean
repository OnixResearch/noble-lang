import M5Resources

open Aeneas Aeneas.Std
open noble_kernel

set_option maxHeartbeats 2000000
set_option maxRecDepth 4096

namespace HostLocality

/-!
This is a bounded decision model, not a proof of the handwritten Rust or Wasmtime
wrappers.  The source edges are `Host::read`/`Host::release` in
`crates/noble-cli/src/component/bounded_region.rs` and `invoke`/`restore_borrows`
in `crates/noble-wasm/src/component/lower/calls.rs`: the host validates the
representation, retained exact handle and bounds before borrowing; the lowerer
captures and validates the result before restoring declared borrows.  In
particular, `restore_borrows` only mutates compiler state and emits no runtime
instruction; `invoke` places that compilation step after returned-value
validation and before compiling later guest instructions.  The
`WrapperResponseRefines` and `HostAdapterRefinesRead` premises below remain the
open source-refinement edges.  Signed-ABI conversion, emitted-code refusal,
Wasmtime behavior, cleanup failure and actual execution require separate
source/lowering/finite-execution evidence.
-/

abbrev Handle := resources.Handle

deriving instance DecidableEq for resources.Handle

def successTag : Nat := 0
def errorTag : Nat := 1
def maxRegionBytes : Nat := 4096

def invalidHandleBytes : List Nat :=
  [105, 110, 118, 97, 108, 105, 100, 45, 104, 97, 110, 100, 108, 101]

def boundsRejectBytes : List Nat :=
  [98, 111, 117, 110, 100, 115, 45, 114, 101, 106, 101, 99, 116]

structure WireResponse where
  tag : Nat
  length : Nat
  payload : List Nat
  deriving DecidableEq, Repr

def responseWellFormed (response : WireResponse) : Prop :=
  (response.tag = successTag ∨ response.tag = errorTag) ∧
    response.length = response.payload.length

instance (response : WireResponse) : Decidable (responseWellFormed response) := by
  unfold responseWellFormed
  infer_instance

inductive ReadDecision where
  | success (bytes : List Nat)
  | domainError (message : List Nat)
  deriving DecidableEq, Repr

def encodeDecision : ReadDecision → WireResponse
  | .success bytes => ⟨successTag, bytes.length, bytes⟩
  | .domainError message => ⟨errorTag, message.length, message⟩

theorem encodeDecision_wellFormed (decision : ReadDecision) :
    responseWellFormed (encodeDecision decision) := by
  cases decision <;> simp [responseWellFormed, encodeDecision, successTag, errorTag]

structure ReadRequest where
  representation : Nat
  claim : Handle
  offset : Nat
  length : Nat

structure Region where
  identity : Handle
  bytes : List Nat
  hostBound : bytes.length ≤ maxRegionBytes

def withinBounds (region : Region) (request : ReadRequest) : Prop :=
  request.offset ≤ region.bytes.length ∧
    request.length ≤ region.bytes.length - request.offset ∧
    request.length ≤ maxRegionBytes

instance (region : Region) (request : ReadRequest) :
    Decidable (withinBounds region request) := by
  unfold withinBounds
  infer_instance

def readDecision (region : Region) (request : ReadRequest) (accessAllowed : Bool) :
    ReadDecision :=
  if request.representation ≠ 1 then
    .domainError invalidHandleBytes
  else if request.claim ≠ region.identity then
    .domainError invalidHandleBytes
  else if withinBounds region request then
    if accessAllowed then
      .success ((region.bytes.drop request.offset).take request.length)
    else
      .domainError invalidHandleBytes
  else
    .domainError boundsRejectBytes

def completionOf : ReadDecision → resources.Completion
  | .success _ => .Success
  | .domainError _ => .DomainError

theorem readDecision_success (region : Region) (request : ReadRequest)
    (representation : request.representation = 1)
    (identity : request.claim = region.identity)
    (bounds : withinBounds region request)
    (accessAllowed : Bool) (allowed : accessAllowed = true) :
    readDecision region request accessAllowed =
      .success ((region.bytes.drop request.offset).take request.length) := by
  simp [readDecision, representation, identity, bounds, allowed]

theorem readDecision_domainError (region : Region) (request : ReadRequest)
    (representation : request.representation = 1)
    (identity : request.claim = region.identity)
    (bounds : withinBounds region request)
    (accessAllowed : Bool) (accessDenied : accessAllowed = false) :
    readDecision region request accessAllowed = .domainError invalidHandleBytes := by
  simp [readDecision, representation, identity, bounds, accessDenied]

inductive StackCell where
  | natural (value : Nat)
  | owner (handle : Handle)
  | response (value : WireResponse)

structure BorrowState where
  owner : Handle
  «prefix» : List StackCell
  position : Nat
  position_is_prefix_length : position = «prefix».length

inductive GuestPhase where
  | suspended
  | validated
  | continuing
  | aborted
  deriving DecidableEq, Repr

structure GuestState where
  stack : List StackCell
  borrowed : Option BorrowState
  phase : GuestPhase
  response : Option WireResponse

inductive HostExit where
  | returned (response : WireResponse)
  | trapped

def guestCallStack (borrow : BorrowState) (request : ReadRequest) : List StackCell :=
  borrow.prefix ++
    [.owner borrow.owner, .natural request.offset, .natural request.length]

def collectGuest (borrow : BorrowState) : GuestState :=
  ⟨borrow.prefix, some borrow, .suspended, none⟩

def validateExit (state : GuestState) : HostExit → GuestState
  | .trapped => { state with phase := .aborted, response := none }
  | .returned response =>
    if responseWellFormed response then
      { state with phase := .validated, response := some response }
    else
      { state with phase := .aborted, response := none }

def restoreBorrow (state : GuestState) : GuestState :=
  match state.phase, state.borrowed, state.response with
  | .validated, some borrow, some response =>
      { state with
        stack := state.stack ++ [.owner borrow.owner, .response response]
        borrowed := none
        phase := .continuing }
  | _, _, _ => state

def restoredGuest (borrow : BorrowState) (response : WireResponse) : GuestState :=
  ⟨borrow.prefix ++ [.owner borrow.owner, .response response], none, .continuing,
    some response⟩

def WrapperResponseRefines (decision : ReadDecision) (response : WireResponse) : Prop :=
  response = encodeDecision decision ∧ responseWellFormed response

theorem t5_decision_logic_relation (borrow : BorrowState) (decision : ReadDecision)
    (response : WireResponse)
    (wrapperRefinement : WrapperResponseRefines decision response) :
    response = encodeDecision decision ∧
    restoreBorrow (validateExit (collectGuest borrow) (.returned response)) =
        restoredGuest borrow response ∧
      (restoredGuest borrow response).stack =
        borrow.prefix ++ [.owner borrow.owner, .response response] ∧
      borrow.position = borrow.prefix.length := by
  rcases wrapperRefinement with ⟨responseExact, responseValid⟩
  refine ⟨responseExact, ?_, rfl, borrow.position_is_prefix_length⟩
  simp [restoreBorrow, validateExit, collectGuest, restoredGuest, responseValid]

theorem guest_call_collection_exact (borrow : BorrowState) (request : ReadRequest) :
    guestCallStack borrow request = borrow.prefix ++
      [.owner borrow.owner, .natural request.offset, .natural request.length] ∧
    (collectGuest borrow).stack = borrow.prefix ∧
    (collectGuest borrow).borrowed = some borrow ∧
    borrow.position = borrow.prefix.length := by
  exact ⟨rfl, rfl, rfl, borrow.position_is_prefix_length⟩

/-- A stage-ordering model control only; it does not claim that moving a compiler
    helper call alone changes emitted Wasm instructions. -/
theorem restoration_before_validation_is_noop (borrow : BorrowState) :
    restoreBorrow (collectGuest borrow) = collectGuest borrow ∧
      (restoreBorrow (collectGuest borrow)).stack = borrow.prefix ∧
      (restoreBorrow (collectGuest borrow)).phase = .suspended ∧
      (restoreBorrow (collectGuest borrow)).stack ≠
        borrow.prefix ++ [.owner borrow.owner] := by
  refine ⟨rfl, rfl, rfl, ?_⟩
  intro equality
  have lengthEquality := congrArg List.length equality
  simp [restoreBorrow, collectGuest] at lengthEquality

theorem malformed_return_has_no_guest_continuation (borrow : BorrowState)
    (response : WireResponse) (malformed : ¬ responseWellFormed response) :
    let after := restoreBorrow
      (validateExit (collectGuest borrow) (.returned response))
    after.phase = .aborted ∧ after.stack = borrow.prefix ∧
      after.borrowed = some borrow ∧ after.response = none := by
  simp [restoreBorrow, validateExit, collectGuest, malformed]

theorem trapped_call_has_no_guest_continuation (borrow : BorrowState) :
    let after := restoreBorrow (validateExit (collectGuest borrow) .trapped)
    after.phase = .aborted ∧ after.stack = borrow.prefix ∧
      after.borrowed = some borrow ∧ after.response = none := by
  exact ⟨rfl, rfl, rfl, rfl⟩

def malformedTagFixture : WireResponse := ⟨2, 0, []⟩
def alteredLengthFixture : WireResponse := ⟨successTag, 1, []⟩

theorem malformed_tag_rejected : ¬ responseWellFormed malformedTagFixture := by
  simp [responseWellFormed, malformedTagFixture, successTag, errorTag]

theorem altered_result_length_rejected : ¬ responseWellFormed alteredLengthFixture := by
  simp [responseWellFormed, alteredLengthFixture, successTag, errorTag]

theorem altered_result_tag_breaks_wrapper_refinement (decision : ReadDecision)
    (expected altered : WireResponse)
    (expectedRefinement : WrapperResponseRefines decision expected)
    (alteredRefinement : WrapperResponseRefines decision altered)
    (tagChanged : altered.tag ≠ expected.tag) : False := by
  apply tagChanged
  have exactResponse : altered = expected :=
    alteredRefinement.1.trans expectedRefinement.1.symm
  exact congrArg WireResponse.tag exactResponse

theorem altered_result_length_breaks_wrapper_refinement (decision : ReadDecision)
    (expected altered : WireResponse)
    (expectedRefinement : WrapperResponseRefines decision expected)
    (alteredRefinement : WrapperResponseRefines decision altered)
    (lengthChanged : altered.length ≠ expected.length) : False := by
  apply lengthChanged
  have exactResponse : altered = expected :=
    alteredRefinement.1.trans expectedRefinement.1.symm
  exact congrArg WireResponse.length exactResponse

theorem malformed_tag_does_not_restore (borrow : BorrowState) :
    let after := restoreBorrow
      (validateExit (collectGuest borrow) (.returned malformedTagFixture))
    after.phase = .aborted ∧ after.stack = borrow.prefix ∧
      after.borrowed = some borrow := by
  have result := malformed_return_has_no_guest_continuation borrow malformedTagFixture
    malformed_tag_rejected
  exact ⟨result.1, result.2.1, result.2.2.1⟩

theorem altered_length_does_not_restore (borrow : BorrowState) :
    let after := restoreBorrow
      (validateExit (collectGuest borrow) (.returned alteredLengthFixture))
    after.phase = .aborted ∧ after.stack = borrow.prefix ∧
      after.borrowed = some borrow := by
  have result := malformed_return_has_no_guest_continuation borrow alteredLengthFixture
    altered_result_length_rejected
  exact ⟨result.1, result.2.1, result.2.2.1⟩

structure ProtectedCounters where
  protectedOperations : Nat
  regionReads : Nat
  releases : Nat

/-- Counters are mathematical naturals here; boundary scenarios state explicit
    one-increment room so they refine the host's `u64` counters without wrap. -/
def readCounterRoom (counters : ProtectedCounters) : Prop :=
  counters.protectedOperations + 1 < 2 ^ 64 ∧ counters.regionReads + 1 < 2 ^ 64

def releaseCounterRoom (counters : ProtectedCounters) : Prop :=
  counters.releases + 1 < 2 ^ 64

def countersAfterRead (before : ProtectedCounters) : ReadDecision → ProtectedCounters
  | .success _ =>
      { before with
        protectedOperations := before.protectedOperations + 1
        regionReads := before.regionReads + 1 }
  | .domainError _ => before

structure AdapterState where
  region : Region
  record : resources.Snapshot
  owner : Option resources.Owner
  counters : ProtectedCounters

def adapterReadAfter (before : AdapterState) (decision : ReadDecision) : AdapterState :=
  { before with
    record := { before.record with state := .Live }
    owner := some ⟨before.record.handle⟩
    counters := countersAfterRead before.counters decision }

/-- This premise is the still-open Rust-wrapper refinement, not a model theorem. -/
def HostAdapterRefinesRead (before after : AdapterState) (decision : ReadDecision) : Prop :=
  after = adapterReadAfter before decision

structure FrameProjection where
  unrelatedOwner : resources.Owner
  unrelatedState : resources.Snapshot
  protectedCells : List Nat

def frameProjection (frame : FrameProjection) :=
  (frame.unrelatedOwner.handle, frame.unrelatedState, frame.protectedCells)

/- The modeled adapter can read only its `Region` and request and can update only
   `AdapterState` (one optional owner); neither model function receives the
   separate `FrameProjection`.  The actual Rust host has process memory access,
   so every boundary theorem still takes `PreservesFrame` as an independent
   premise rather than deriving actual isolation from Wasm. -/
def PreservesFrame (before after : FrameProjection) : Prop :=
  before.unrelatedOwner.handle = after.unrelatedOwner.handle ∧
  before.unrelatedState = after.unrelatedState ∧
  before.protectedCells = after.protectedCells

def FrameSeparated (target : Handle) (frame : FrameProjection) : Prop :=
  frame.unrelatedOwner.handle ≠ target ∧ frame.unrelatedState.handle ≠ target

theorem host_locality_preserves_projection (before after : FrameProjection)
    (locality : PreservesFrame before after) :
    frameProjection before = frameProjection after := by
  rcases locality with ⟨owner, state, cells⟩
  simp [frameProjection, owner, state, cells]

theorem m5_table_complete_exact (table : resources.table.Table)
    (record : resources.Snapshot) (serial : Std.U64)
    (outcome : resources.Completion)
    (selected : table.records.val[record.handle.slot.val]? = some record)
    (wellFormed : M5Resources.valid record = true)
    (busy : record.state = .Busy serial)
    (authentic : M5Resources.scopeCheck record serial = .Ok ()) :
    resources.table.Table.decide table record.handle (.Complete serial outcome) =
      Result.ok (.Ok ⟨{ record with state := .Live }, .OwnerReturned outcome⟩) ∧
    resources.Decision.accounting
      ⟨{ record with state := .Live }, .OwnerReturned outcome⟩ =
      Result.ok ⟨0#usize, 1#usize, 1#usize, 0#usize, 0#usize⟩ := by
  have normal := M5Resources.normal_completion_owner_once record serial outcome
    wellFormed busy authentic
  have stepExact : M5Resources.step record record.handle (.Complete serial outcome) =
      .Ok ⟨{ record with state := .Live }, .OwnerReturned outcome⟩ := by
    simp [M5Resources.step, wellFormed, M5Resources.handleCheck_self,
      M5Resources.applyEvent, M5Resources.complete, M5Resources.afterCheck, busy, authentic]
  constructor
  · calc
      resources.table.Table.decide table record.handle (.Complete serial outcome) =
          Result.ok (M5Resources.tableStep table record.handle (.Complete serial outcome)) :=
        M5Resources.table_decide_refines table record.handle (.Complete serial outcome)
      _ = Result.ok (.Ok ⟨{ record with state := .Live }, .OwnerReturned outcome⟩) := by
        simp [M5Resources.tableStep, selected, stepExact]
  · exact normal.2

structure CompletedReadScenario where
  table : resources.table.Table
  record : resources.Snapshot
  serial : Std.U64
  region : Region
  request : ReadRequest
  accessAllowed : Bool
  response : WireResponse
  borrow : BorrowState
  adapterBefore : AdapterState
  readCounterRoom : readCounterRoom adapterBefore.counters
  adapterAfter : AdapterState
  frameBefore : FrameProjection
  frameAfter : FrameProjection
  selected : table.records.val[record.handle.slot.val]? = some record
  recordValid : M5Resources.valid record = true
  recordBusy : record.state = .Busy serial
  scopeAuthentic : M5Resources.scopeCheck record serial = .Ok ()
  adapterRecord : adapterBefore.record = record
  adapterRegion : adapterBefore.region = region
  adapterOwnerTaken : adapterBefore.owner = none
  hostAdapterRefinement : HostAdapterRefinesRead adapterBefore adapterAfter
    (readDecision region request accessAllowed)
  representationValid : request.representation = 1
  exactClaim : request.claim = record.handle
  exactRegion : region.identity = record.handle
  bounded : withinBounds region request
  borrowedOwnerExact : borrow.owner = record.handle
  frameSeparated : FrameSeparated record.handle frameBefore
  hostLocality : PreservesFrame frameBefore frameAfter
  wrapperResponseRefinement : WrapperResponseRefines
    (readDecision region request accessAllowed) response

theorem bounded_read_frame_and_restoration (scenario : CompletedReadScenario) :
    frameProjection scenario.frameBefore = frameProjection scenario.frameAfter ∧
    scenario.adapterAfter.region = scenario.adapterBefore.region ∧
    scenario.adapterBefore.region = scenario.region ∧
    scenario.adapterBefore.owner = none ∧
    scenario.adapterAfter.record = { scenario.record with state := .Live } ∧
    scenario.adapterAfter.owner = some ⟨scenario.record.handle⟩ ∧
    scenario.adapterAfter.counters = countersAfterRead scenario.adapterBefore.counters
      (readDecision scenario.region scenario.request scenario.accessAllowed) ∧
    scenario.adapterAfter.counters.releases = scenario.adapterBefore.counters.releases ∧
    readCounterRoom scenario.adapterBefore.counters ∧
    restoreBorrow (validateExit (collectGuest scenario.borrow)
      (.returned scenario.response)) = restoredGuest scenario.borrow scenario.response ∧
    scenario.borrow.owner = scenario.record.handle ∧
    scenario.response = encodeDecision
      (readDecision scenario.region scenario.request scenario.accessAllowed) ∧
    FrameSeparated scenario.record.handle scenario.frameAfter ∧
    resources.table.Table.decide scenario.table scenario.record.handle
      (.Complete scenario.serial
        (completionOf (readDecision scenario.region scenario.request scenario.accessAllowed))) =
      Result.ok (.Ok ⟨{ scenario.record with state := .Live },
        .OwnerReturned (completionOf
          (readDecision scenario.region scenario.request scenario.accessAllowed))⟩) ∧
    resources.Decision.accounting
      ⟨{ scenario.record with state := .Live }, .OwnerReturned
        (completionOf (readDecision scenario.region scenario.request scenario.accessAllowed))⟩ =
      Result.ok ⟨0#usize, 1#usize, 1#usize, 0#usize, 0#usize⟩ := by
  have projection := host_locality_preserves_projection scenario.frameBefore
    scenario.frameAfter scenario.hostLocality
  have m5 := m5_table_complete_exact scenario.table scenario.record scenario.serial
    (completionOf (readDecision scenario.region scenario.request scenario.accessAllowed))
    scenario.selected scenario.recordValid scenario.recordBusy scenario.scopeAuthentic
  have guest := t5_decision_logic_relation scenario.borrow
    (readDecision scenario.region scenario.request scenario.accessAllowed)
    scenario.response scenario.wrapperResponseRefinement
  have adapterExact : scenario.adapterAfter = adapterReadAfter scenario.adapterBefore
      (readDecision scenario.region scenario.request scenario.accessAllowed) :=
    scenario.hostAdapterRefinement
  refine ⟨projection, ?_, scenario.adapterRegion, scenario.adapterOwnerTaken,
    ?_, ?_, ?_, ?_,
    scenario.readCounterRoom, guest.2.1,
    scenario.borrowedOwnerExact,
    scenario.wrapperResponseRefinement.1,
    ?_, m5.1, m5.2⟩
  · rw [adapterExact]; rfl
  · rw [adapterExact]; simp [adapterReadAfter, scenario.adapterRecord]
  · rw [adapterExact]; simp [adapterReadAfter, scenario.adapterRecord]
  · rw [adapterExact]; rfl
  · rw [adapterExact]
    cases readDecision scenario.region scenario.request scenario.accessAllowed <;> rfl
  · rcases scenario.frameSeparated with ⟨owner, state⟩
    rcases scenario.hostLocality with ⟨ownerSame, stateSame, _⟩
    exact ⟨by intro equal; exact owner (ownerSame.trans equal),
      by intro equal; apply state; rw [stateSame]; exact equal⟩

theorem successful_read_frame_and_restoration (scenario : CompletedReadScenario)
    (accessAllowed : scenario.accessAllowed = true) :
    frameProjection scenario.frameBefore = frameProjection scenario.frameAfter ∧
    scenario.adapterAfter.region = scenario.adapterBefore.region ∧
    scenario.adapterBefore.region = scenario.region ∧
    scenario.adapterBefore.owner = none ∧
    scenario.adapterAfter.record = { scenario.record with state := .Live } ∧
    scenario.adapterAfter.owner = some ⟨scenario.record.handle⟩ ∧
    scenario.adapterAfter.counters = countersAfterRead scenario.adapterBefore.counters
      (.success ((scenario.region.bytes.drop scenario.request.offset).take
        scenario.request.length)) ∧
    scenario.adapterAfter.counters.releases = scenario.adapterBefore.counters.releases ∧
    readCounterRoom scenario.adapterBefore.counters ∧
    restoreBorrow (validateExit (collectGuest scenario.borrow)
      (.returned scenario.response)) = restoredGuest scenario.borrow scenario.response ∧
    scenario.borrow.owner = scenario.record.handle ∧
    scenario.response = encodeDecision (.success
      ((scenario.region.bytes.drop scenario.request.offset).take scenario.request.length)) ∧
    FrameSeparated scenario.record.handle scenario.frameAfter ∧
    resources.table.Table.decide scenario.table scenario.record.handle
      (.Complete scenario.serial .Success) =
      Result.ok (.Ok ⟨{ scenario.record with state := .Live }, .OwnerReturned .Success⟩) ∧
    resources.Decision.accounting
      ⟨{ scenario.record with state := .Live }, .OwnerReturned .Success⟩ =
      Result.ok ⟨0#usize, 1#usize, 1#usize, 0#usize, 0#usize⟩ := by
  have decisionExact := readDecision_success scenario.region scenario.request
    scenario.representationValid (by simp [scenario.exactClaim, scenario.exactRegion])
    scenario.bounded scenario.accessAllowed accessAllowed
  simpa [decisionExact, completionOf] using bounded_read_frame_and_restoration scenario

theorem domain_error_read_frame_and_restoration (scenario : CompletedReadScenario)
    (accessDenied : scenario.accessAllowed = false) :
    frameProjection scenario.frameBefore = frameProjection scenario.frameAfter ∧
    scenario.adapterAfter.region = scenario.adapterBefore.region ∧
    scenario.adapterBefore.region = scenario.region ∧
    scenario.adapterBefore.owner = none ∧
    scenario.adapterAfter.record = { scenario.record with state := .Live } ∧
    scenario.adapterAfter.owner = some ⟨scenario.record.handle⟩ ∧
    scenario.adapterAfter.counters = scenario.adapterBefore.counters ∧
    scenario.adapterAfter.counters.releases = scenario.adapterBefore.counters.releases ∧
    readCounterRoom scenario.adapterBefore.counters ∧
    restoreBorrow (validateExit (collectGuest scenario.borrow)
      (.returned scenario.response)) = restoredGuest scenario.borrow scenario.response ∧
    scenario.borrow.owner = scenario.record.handle ∧
    scenario.response = encodeDecision (.domainError invalidHandleBytes) ∧
    FrameSeparated scenario.record.handle scenario.frameAfter ∧
    resources.table.Table.decide scenario.table scenario.record.handle
      (.Complete scenario.serial .DomainError) =
      Result.ok (.Ok ⟨{ scenario.record with state := .Live },
        .OwnerReturned .DomainError⟩) ∧
    resources.Decision.accounting
      ⟨{ scenario.record with state := .Live }, .OwnerReturned .DomainError⟩ =
      Result.ok ⟨0#usize, 1#usize, 1#usize, 0#usize, 0#usize⟩ := by
  have decisionExact := readDecision_domainError scenario.region scenario.request
    scenario.representationValid (by simp [scenario.exactClaim, scenario.exactRegion])
    scenario.bounded scenario.accessAllowed accessDenied
  simpa [decisionExact, completionOf, countersAfterRead] using
    bounded_read_frame_and_restoration scenario

/-- Host validation failures occur before `owner.take`; no M5 completion is claimed. -/
def HostAdapterRefinesPreflight (before after : AdapterState) : Prop :=
  after = before

structure PreflightReadScenario where
  adapterBefore : AdapterState
  adapterAfter : AdapterState
  frameBefore : FrameProjection
  frameAfter : FrameProjection
  region : Region
  request : ReadRequest
  accessAllowed : Bool
  response : WireResponse
  borrow : BorrowState
  preflightFailure : request.representation ≠ 1 ∨
    request.claim ≠ region.identity ∨ ¬ withinBounds region request
  borrowedClaimExact : borrow.owner = request.claim
  hostAdapterRefinement : HostAdapterRefinesPreflight adapterBefore adapterAfter
  hostLocality : PreservesFrame frameBefore frameAfter
  frameSeparated : FrameSeparated adapterBefore.record.handle frameBefore
  wrapperResponseRefinement : WrapperResponseRefines
    (readDecision region request accessAllowed) response

theorem preflight_domain_error_frame_and_restoration
    (scenario : PreflightReadScenario) :
    frameProjection scenario.frameBefore = frameProjection scenario.frameAfter ∧
    scenario.adapterAfter = scenario.adapterBefore ∧
    scenario.adapterAfter.counters = scenario.adapterBefore.counters ∧
    restoreBorrow (validateExit (collectGuest scenario.borrow)
      (.returned scenario.response)) = restoredGuest scenario.borrow scenario.response ∧
    scenario.borrow.owner = scenario.request.claim ∧
    scenario.response = encodeDecision
      (readDecision scenario.region scenario.request scenario.accessAllowed) ∧
    (∃ message, readDecision scenario.region scenario.request scenario.accessAllowed =
      .domainError message) ∧
    FrameSeparated scenario.adapterBefore.record.handle scenario.frameAfter := by
  have projection := host_locality_preserves_projection scenario.frameBefore
    scenario.frameAfter scenario.hostLocality
  have guest := t5_decision_logic_relation scenario.borrow
    (readDecision scenario.region scenario.request scenario.accessAllowed)
    scenario.response scenario.wrapperResponseRefinement
  have decisionError : ∃ message, readDecision scenario.region scenario.request
      scenario.accessAllowed = .domainError message := by
    by_cases representation : scenario.request.representation = 1
    · by_cases identity : scenario.request.claim = scenario.region.identity
      · have bounds : ¬ withinBounds scenario.region scenario.request := by
          rcases scenario.preflightFailure with badRepresentation | badIdentity | badBounds
          · exact False.elim (badRepresentation representation)
          · exact False.elim (badIdentity identity)
          · exact badBounds
        exact ⟨boundsRejectBytes, by
          simp [readDecision, representation, identity, bounds]⟩
      · exact ⟨invalidHandleBytes, by
          simp [readDecision, representation, identity]⟩
    · exact ⟨invalidHandleBytes, by simp [readDecision, representation]⟩
  have separated : FrameSeparated scenario.adapterBefore.record.handle scenario.frameAfter := by
    rcases scenario.frameSeparated with ⟨owner, state⟩
    rcases scenario.hostLocality with ⟨ownerSame, stateSame, _⟩
    exact ⟨by intro equal; exact owner (ownerSame.trans equal),
      by intro equal; apply state; rw [stateSame]; exact equal⟩
  exact ⟨projection, scenario.hostAdapterRefinement,
    by rw [scenario.hostAdapterRefinement], guest.2.1,
    scenario.borrowedClaimExact,
    scenario.wrapperResponseRefinement.1, decisionError, separated⟩

def requirementFor (handle : Handle) : resources.Requirement :=
  ⟨handle.context, handle.kind, handle.rights⟩

theorem m5_table_release_exact (table : resources.table.Table)
    (record : resources.Snapshot)
    (selected : table.records.val[record.handle.slot.val]? = some record)
    (wellFormed : M5Resources.valid record = true)
    (_live : record.state = .Live)
    (liveCheck : M5Resources.liveCheck record (requirementFor record.handle) = .Ok ()) :
    resources.table.Table.decide table record.handle
      (.Release (requirementFor record.handle)) =
        Result.ok (.Ok ⟨{ record with state := .Retired }, .LocalRelease⟩) ∧
    resources.transition.release record (requirementFor record.handle) =
      Result.ok (.Ok ⟨{ record with state := .Retired }, .LocalRelease⟩) ∧
    resources.Decision.accounting ⟨{ record with state := .Retired }, .LocalRelease⟩ =
      Result.ok ⟨0#usize, 0#usize, 0#usize, 0#usize, 1#usize⟩ := by
  have releaseRefinement := M5Resources.release_refines record (requirementFor record.handle)
  have releaseModel : M5Resources.release record (requirementFor record.handle) =
      .Ok ⟨{ record with state := .Retired }, .LocalRelease⟩ := by
    simp [M5Resources.release, M5Resources.afterCheck, liveCheck]
  have stepExact : M5Resources.step record record.handle
      (.Release (requirementFor record.handle)) =
        .Ok ⟨{ record with state := .Retired }, .LocalRelease⟩ := by
    simp [M5Resources.step, wellFormed, M5Resources.handleCheck_self,
      M5Resources.applyEvent, M5Resources.afterCheck, releaseModel]
  constructor
  · calc
      resources.table.Table.decide table record.handle
          (.Release (requirementFor record.handle)) =
          Result.ok (M5Resources.tableStep table record.handle
            (.Release (requirementFor record.handle))) :=
        M5Resources.table_decide_refines table record.handle
          (.Release (requirementFor record.handle))
      _ = Result.ok (.Ok ⟨{ record with state := .Retired }, .LocalRelease⟩) := by
        simp [M5Resources.tableStep, selected, stepExact]
  constructor
  · calc
      resources.transition.release record (requirementFor record.handle) =
          Result.ok (M5Resources.release record (requirementFor record.handle)) :=
        releaseRefinement
      _ = Result.ok (.Ok ⟨{ record with state := .Retired }, .LocalRelease⟩) := by
        rw [releaseModel]
  · rfl

def countersAfterRelease (before : ProtectedCounters) : ProtectedCounters :=
  { before with releases := before.releases + 1 }

def adapterReleaseAfter (before : AdapterState) : AdapterState :=
  { before with
    record := { before.record with state := .Retired }
    owner := none
    counters := countersAfterRelease before.counters }

/-- Source-refinement premise for the successful consuming `Host::release` path. -/
def HostAdapterRefinesRelease (before after : AdapterState) : Prop :=
  after = adapterReleaseAfter before

def consumeGuestOwner (borrow : BorrowState) : GuestState :=
  ⟨borrow.prefix, none, .continuing, none⟩

theorem consuming_release_does_not_restore_guest_owner (borrow : BorrowState) :
    (consumeGuestOwner borrow).stack = borrow.prefix ∧
    (consumeGuestOwner borrow).borrowed = none ∧
    (consumeGuestOwner borrow).phase = .continuing ∧
    (consumeGuestOwner borrow).stack ≠
      borrow.prefix ++ [.owner borrow.owner] := by
  refine ⟨rfl, rfl, rfl, ?_⟩
  intro equality
  have lengthEquality := congrArg List.length equality
  simp [consumeGuestOwner] at lengthEquality

structure ConsumingReleaseScenario where
  table : resources.table.Table
  record : resources.Snapshot
  adapterBefore : AdapterState
  adapterAfter : AdapterState
  borrow : BorrowState
  frameBefore : FrameProjection
  frameAfter : FrameProjection
  selected : table.records.val[record.handle.slot.val]? = some record
  recordValid : M5Resources.valid record = true
  recordLive : record.state = .Live
  liveCheck : M5Resources.liveCheck record (requirementFor record.handle) = .Ok ()
  adapterRecord : adapterBefore.record = record
  adapterOwnerExact : adapterBefore.owner = some ⟨record.handle⟩
  adapterRefinement : HostAdapterRefinesRelease adapterBefore adapterAfter
  borrowedOwnerExact : borrow.owner = record.handle
  releaseCounterRoom : releaseCounterRoom adapterBefore.counters
  frameSeparated : FrameSeparated record.handle frameBefore
  hostLocality : PreservesFrame frameBefore frameAfter

theorem consuming_release_boundary_does_not_restore_and_accounts_once
    (scenario : ConsumingReleaseScenario) :
    frameProjection scenario.frameBefore = frameProjection scenario.frameAfter ∧
    scenario.adapterAfter.region = scenario.adapterBefore.region ∧
    scenario.adapterAfter.record = { scenario.record with state := .Retired } ∧
    scenario.adapterAfter.owner = none ∧
    scenario.adapterBefore.owner = some ⟨scenario.record.handle⟩ ∧
    scenario.adapterAfter.counters.protectedOperations =
      scenario.adapterBefore.counters.protectedOperations ∧
    scenario.adapterAfter.counters.regionReads = scenario.adapterBefore.counters.regionReads ∧
    scenario.adapterAfter.counters.releases = scenario.adapterBefore.counters.releases + 1 ∧
    releaseCounterRoom scenario.adapterBefore.counters ∧
    (consumeGuestOwner scenario.borrow).stack = scenario.borrow.prefix ∧
    (consumeGuestOwner scenario.borrow).borrowed = none ∧
    (consumeGuestOwner scenario.borrow).phase = .continuing ∧
    (consumeGuestOwner scenario.borrow).stack ≠
      scenario.borrow.prefix ++ [.owner scenario.borrow.owner] ∧
    scenario.borrow.owner = scenario.record.handle ∧
    FrameSeparated scenario.record.handle scenario.frameAfter ∧
    resources.table.Table.decide scenario.table scenario.record.handle
      (.Release (requirementFor scenario.record.handle)) =
        Result.ok (.Ok ⟨{ scenario.record with state := .Retired }, .LocalRelease⟩) ∧
    resources.transition.release scenario.record (requirementFor scenario.record.handle) =
      Result.ok (.Ok ⟨{ scenario.record with state := .Retired }, .LocalRelease⟩) ∧
    resources.Decision.accounting
      ⟨{ scenario.record with state := .Retired }, .LocalRelease⟩ =
      Result.ok ⟨0#usize, 0#usize, 0#usize, 0#usize, 1#usize⟩ := by
  have projection := host_locality_preserves_projection scenario.frameBefore
    scenario.frameAfter scenario.hostLocality
  have guest := consuming_release_does_not_restore_guest_owner scenario.borrow
  have m5 := m5_table_release_exact scenario.table scenario.record scenario.selected
    scenario.recordValid scenario.recordLive scenario.liveCheck
  have adapterExact : scenario.adapterAfter = adapterReleaseAfter scenario.adapterBefore :=
    scenario.adapterRefinement
  have separated : FrameSeparated scenario.record.handle scenario.frameAfter := by
    rcases scenario.frameSeparated with ⟨owner, state⟩
    rcases scenario.hostLocality with ⟨ownerSame, stateSame, _⟩
    exact ⟨by intro equal; exact owner (ownerSame.trans equal),
      by intro equal; apply state; rw [stateSame]; exact equal⟩
  refine ⟨projection, ?_, ?_, ?_, scenario.adapterOwnerExact, ?_, ?_, ?_,
    scenario.releaseCounterRoom, guest.1, guest.2.1, guest.2.2.1, guest.2.2.2,
    scenario.borrowedOwnerExact,
    separated, m5.1, m5.2.1, m5.2.2⟩
  · rw [adapterExact]; rfl
  · rw [adapterExact]; simp [adapterReleaseAfter, scenario.adapterRecord]
  · rw [adapterExact]; rfl
  · rw [adapterExact]; rfl
  · rw [adapterExact]; rfl
  · rw [adapterExact]; rfl

theorem consuming_release_does_not_restore (table : resources.table.Table)
    (record : resources.Snapshot)
    (selected : table.records.val[record.handle.slot.val]? = some record)
    (wellFormed : M5Resources.valid record = true)
    (live : record.state = .Live)
    (liveCheck : M5Resources.liveCheck record (requirementFor record.handle) = .Ok ())
    (borrow : BorrowState) (borrowMatches : borrow.owner = record.handle) :
    (consumeGuestOwner borrow).stack = borrow.prefix ∧
    (consumeGuestOwner borrow).borrowed = none ∧
    (consumeGuestOwner borrow).phase = .continuing ∧
    (consumeGuestOwner borrow).stack ≠ borrow.prefix ++ [.owner borrow.owner] ∧
    borrow.owner = record.handle ∧
    resources.table.Table.decide table record.handle
      (.Release (requirementFor record.handle)) =
        Result.ok (.Ok ⟨{ record with state := .Retired }, .LocalRelease⟩) ∧
    resources.transition.release record (requirementFor record.handle) =
      Result.ok (.Ok ⟨{ record with state := .Retired }, .LocalRelease⟩) ∧
    resources.Decision.accounting ⟨{ record with state := .Retired }, .LocalRelease⟩ =
      Result.ok ⟨0#usize, 0#usize, 0#usize, 0#usize, 1#usize⟩ := by
  rcases consuming_release_does_not_restore_guest_owner borrow with
    ⟨stack, borrowed, phase, notRestored⟩
  rcases m5_table_release_exact table record selected wellFormed live liveCheck with
    ⟨decision, release, accounting⟩
  exact ⟨stack, borrowed, phase, notRestored, borrowMatches, decision, release, accounting⟩

inductive CleanupSelection where
  | skip
  | release (owner : resources.Owner)

inductive CleanupInvocation where
  | notInvoked
  | invokeRelease (owner : resources.Owner)

inductive CleanupOutcome where
  | skipped
  | succeeded
  | failed
  deriving DecidableEq, Repr

structure CleanupTrace where
  selection : CleanupSelection
  invocation : CleanupInvocation
  outcome : CleanupOutcome
  ownerAfter : Option resources.Owner

def cleanupReleaseDelta (trace : CleanupTrace) : Nat :=
  match trace.invocation, trace.outcome with
  | .invokeRelease _, .succeeded => 1
  | _, _ => 0

def selectCleanup : Option resources.Owner → CleanupSelection
  | none => .skip
  | some owner => .release owner

def invokeCleanup : CleanupSelection → CleanupInvocation
  | .skip => .notInvoked
  | .release owner => .invokeRelease owner

def finishCleanup (invocation : CleanupInvocation) (succeeded : Bool) : CleanupOutcome :=
  match invocation with
  | .notInvoked => .skipped
  | .invokeRelease _ => if succeeded then .succeeded else .failed

def cleanupOwnerAfter (invocation : CleanupInvocation) (outcome : CleanupOutcome) :
    Option resources.Owner :=
  match invocation, outcome with
  | .notInvoked, _ => none
  | .invokeRelease _, .succeeded => none
  | .invokeRelease owner, .failed => some owner
  | .invokeRelease owner, .skipped => some owner

def runCleanup (owner : Option resources.Owner) (succeeded : Bool) : CleanupTrace :=
  let selection := selectCleanup owner
  let invocation := invokeCleanup selection
  let outcome := finishCleanup invocation succeeded
  ⟨selection, invocation, outcome, cleanupOwnerAfter invocation outcome⟩

theorem cleanup_success_is_selected_invoked_and_consuming (owner : resources.Owner) :
    runCleanup (some owner) true =
      ⟨.release owner, .invokeRelease owner, .succeeded, none⟩ := by
  rfl

theorem cleanup_failure_is_selected_invoked_and_retained (owner : resources.Owner) :
    runCleanup (some owner) false =
      ⟨.release owner, .invokeRelease owner, .failed, some owner⟩ := by
  rfl

theorem cleanup_success_counts_one_release (owner : resources.Owner) :
    cleanupReleaseDelta (runCleanup (some owner) true) = 1 := by
  rfl

theorem cleanup_failure_counts_no_release (owner : resources.Owner) :
    cleanupReleaseDelta (runCleanup (some owner) false) = 0 := by
  rfl

theorem cleanup_without_owner_is_skipped :
    runCleanup none true = ⟨.skip, .notInvoked, .skipped, none⟩ := by
  rfl

def applyCleanup (guest : GuestState) (owner : Option resources.Owner)
    (succeeded : Bool) : GuestState × CleanupTrace :=
  (guest, runCleanup owner succeeded)

theorem abnormal_cleanup_does_not_restore_guest (borrow : BorrowState)
    (response : WireResponse) (malformed : ¬ responseWellFormed response)
    (owner : Option resources.Owner) (cleanupSucceeded : Bool) :
    let guest := restoreBorrow
      (validateExit (collectGuest borrow) (.returned response))
    let result := applyCleanup guest owner cleanupSucceeded
    result.1.phase = .aborted ∧ result.1.stack = borrow.prefix ∧
      result.1.borrowed = some borrow := by
  simp [applyCleanup, restoreBorrow, validateExit, collectGuest, malformed]

theorem trapped_cleanup_does_not_restore_guest (borrow : BorrowState)
    (owner : Option resources.Owner) (cleanupSucceeded : Bool) :
    let guest := restoreBorrow (validateExit (collectGuest borrow) .trapped)
    let result := applyCleanup guest owner cleanupSucceeded
    result.1.phase = .aborted ∧ result.1.stack = borrow.prefix ∧
      result.1.borrowed = some borrow := by
  exact ⟨rfl, rfl, rfl⟩

def cleanupTwice (owner : Option resources.Owner) (firstSucceeded secondSucceeded : Bool) :=
  let first := runCleanup owner firstSucceeded
  (first, runCleanup first.ownerAfter secondSucceeded)

theorem cleanup_success_is_not_selected_twice (owner : resources.Owner) :
    let traces := cleanupTwice (some owner) true true
    traces.1.selection = .release owner ∧
    traces.1.invocation = .invokeRelease owner ∧
    traces.1.outcome = .succeeded ∧
    traces.2.selection = .skip ∧
    traces.2.invocation = .notInvoked ∧
    traces.2.outcome = .skipped ∧
    cleanupReleaseDelta traces.1 = 1 ∧ cleanupReleaseDelta traces.2 = 0 := by
  exact ⟨rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩

theorem m5_double_retirement_has_no_second_release
    (record : resources.Snapshot) (firstReason secondReason : resources.Retirement)
    (wellFormed : M5Resources.valid record = true)
    (live : record.state = .Live) :
    resources.transition.transition record record.handle (.Retire firstReason) =
      Result.ok (.Ok ⟨{ record with state := .Retired, retirement := some firstReason }, .LocalRelease⟩) ∧
    resources.transition.transition
      { record with state := .Retired, retirement := some firstReason }
      record.handle (.Retire secondReason) =
        Result.ok (.Ok (M5Resources.unchanged
          { record with state := .Retired, retirement := some firstReason })) ∧
    resources.Decision.accounting (M5Resources.unchanged
      { record with state := .Retired, retirement := some firstReason }) =
        Result.ok ⟨0#usize, 0#usize, 0#usize, 0#usize, 0#usize⟩ := by
  let retired : resources.Snapshot :=
    { record with state := .Retired, retirement := some firstReason }
  have generationNonzero : record.handle.generation ≠ 0#u64 := by
    intro generationZero
    simp [M5Resources.valid, generationZero] at wellFormed
  have scopeNonzero : record.last_scope ≠ some 0#u64 := by
    intro scopeZero
    simp [M5Resources.valid, generationNonzero, scopeZero] at wellFormed
  have retiredValid : M5Resources.valid retired = true := by
    simp [M5Resources.valid, generationNonzero, scopeNonzero, retired]
  have firstStep : M5Resources.step record record.handle (.Retire firstReason) =
      .Ok ⟨retired, .LocalRelease⟩ := by
    simp [M5Resources.step, wellFormed, M5Resources.handleCheck_self,
      M5Resources.applyEvent, M5Resources.afterCheck, M5Resources.retire, live, retired]
  have secondStep : M5Resources.step retired record.handle (.Retire secondReason) =
      .Ok (M5Resources.unchanged retired) := by
    simp [M5Resources.step, retiredValid, M5Resources.handleCheck_self,
      M5Resources.applyEvent, M5Resources.afterCheck, M5Resources.retire,
      M5Resources.unchanged, retired]
  constructor
  · calc
      resources.transition.transition record record.handle (.Retire firstReason) =
          Result.ok (M5Resources.step record record.handle (.Retire firstReason)) :=
        M5Resources.transition_refines record record.handle (.Retire firstReason)
      _ = Result.ok (.Ok ⟨retired, .LocalRelease⟩) := by rw [firstStep]
  constructor
  · calc
      resources.transition.transition retired record.handle (.Retire secondReason) =
          Result.ok (M5Resources.step retired record.handle (.Retire secondReason)) :=
        M5Resources.transition_refines retired record.handle (.Retire secondReason)
      _ = Result.ok (.Ok (M5Resources.unchanged retired)) := by rw [secondStep]
  · rfl

theorem abnormal_late_completion_is_idempotent (record : resources.Snapshot)
    (serial : Std.U64) (outcome : resources.Completion)
    (wellFormed : M5Resources.valid record = true)
    (retired : record.state = .Retired)
    (authentic : M5Resources.scopeCheck record serial = .Ok ()) :
    resources.transition.transition record record.handle (.Complete serial outcome) =
      Result.ok (.Ok (M5Resources.unchanged record)) ∧
    resources.Decision.accounting (M5Resources.unchanged record) =
      Result.ok ⟨0#usize, 0#usize, 0#usize, 0#usize, 0#usize⟩ :=
  M5Resources.retired_completion_idempotent record serial outcome wellFormed retired authentic

theorem wrong_table_identity_rejected (retained claim : Handle)
    (wrong : retained.table ≠ claim.table) :
    M5Resources.handleCheck retained claim = .Err .InvalidHandle := by
  simp [M5Resources.handleCheck, wrong]

theorem wrong_slot_identity_rejected (retained claim : Handle)
    (tableSame : retained.table = claim.table)
    (wrong : retained.slot ≠ claim.slot) :
    M5Resources.handleCheck retained claim = .Err .InvalidHandle := by
  simp [M5Resources.handleCheck, tableSame, wrong]

theorem wrong_context_identity_rejected (retained claim : Handle)
    (tableSame : retained.table = claim.table)
    (slotSame : retained.slot = claim.slot)
    (wrong : retained.context ≠ claim.context) :
    M5Resources.handleCheck retained claim = .Err .WrongContext := by
  simp [M5Resources.handleCheck, tableSame, slotSame, wrong]

theorem wrong_generation_identity_rejected (retained claim : Handle)
    (tableSame : retained.table = claim.table)
    (slotSame : retained.slot = claim.slot)
    (contextSame : retained.context = claim.context)
    (kindSame : retained.kind = claim.kind)
    (wrong : retained.generation ≠ claim.generation) :
    M5Resources.handleCheck retained claim = .Err .WrongGeneration := by
  simp [M5Resources.handleCheck, tableSame, slotSame, contextSame, kindSame, wrong]

theorem wrong_kind_identity_rejected (retained claim : Handle)
    (tableSame : retained.table = claim.table)
    (slotSame : retained.slot = claim.slot)
    (contextSame : retained.context = claim.context)
    (wrong : retained.kind ≠ claim.kind) :
    M5Resources.handleCheck retained claim = .Err .WrongKind := by
  simp [M5Resources.handleCheck, tableSame, slotSame, contextSame, wrong]

theorem wrong_rights_identity_rejected (retained claim : Handle)
    (tableSame : retained.table = claim.table)
    (slotSame : retained.slot = claim.slot)
    (contextSame : retained.context = claim.context)
    (generationSame : retained.generation = claim.generation)
    (kindSame : retained.kind = claim.kind)
    (wrong : retained.rights ≠ claim.rights) :
    M5Resources.handleCheck retained claim = .Err .WrongRights := by
  simp [M5Resources.handleCheck, tableSame, slotSame, contextSame, generationSame,
    kindSame, wrong]

theorem unrelated_frame_mutation_rejected (before after : FrameProjection)
    (ownerSame : before.unrelatedOwner.handle = after.unrelatedOwner.handle)
    (stateSame : before.unrelatedState = after.unrelatedState)
    (cellsChanged : before.protectedCells ≠ after.protectedCells) :
    ¬ PreservesFrame before after := by
  simp [PreservesFrame, ownerSame, stateSame, cellsChanged]

theorem unrelated_owner_mutation_rejected (before after : FrameProjection)
    (ownerChanged : before.unrelatedOwner.handle ≠ after.unrelatedOwner.handle) :
    ¬ PreservesFrame before after := by
  simp [PreservesFrame, ownerChanged]

theorem unrelated_state_mutation_rejected (before after : FrameProjection)
    (stateChanged : before.unrelatedState ≠ after.unrelatedState) :
    ¬ PreservesFrame before after := by
  simp [PreservesFrame, stateChanged]

end HostLocality
