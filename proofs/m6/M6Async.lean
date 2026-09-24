import NobleKernel

open Aeneas Aeneas.Std
open noble_kernel
open noble_kernel.async_tasks.domain

set_option maxHeartbeats 4000000
set_option maxRecDepth 8192
set_option Aeneas.customDoElab false

namespace M6Async

abbrev Checked (α : Type) := core.result.Result α async_tasks.domain.Error

/- The reference functions below do not call extracted async functions. The outer
   Result records checked machine arithmetic, not an additional lifecycle outcome.
   Correspondence preserves that envelope: it does not assume panic freedom for
   arbitrary fabricated snapshots or claim physical native destruction. -/

def emptyObligations : Obligations := ⟨0#u64, 0#u64, 0#u8⟩

def emptyAccounting : Accounting :=
  ⟨emptyObligations, 0#u64, 0#u64, emptyObligations, emptyObligations,
    emptyObligations, 0#u64, 0#u64, 0#usize, false, false, false, false⟩

def unchanged (record : Snapshot) (action : Action := .Duplicate) : Decision :=
  ⟨record, action, emptyAccounting⟩

def isError {α : Type} : Checked α → Bool
  | .Ok _ => false
  | .Err _ => true

def maxBytes : Result Usize := .ok 67108864#usize

def mask (count : U8) : Result U64 := do
  if count = 0#u8 then return 0#u64
  if count >= 64#u8 then return 18446744073709551615#u64
  let top ← 1#u64 <<< count
  top - 1#u64

/-- Checked arithmetic remains explicit even for a request that has not yet
    passed admission. Only `footprint` applies the public reservation guards. -/
def reservationFootprint (request : async_tasks.bounds.Request) :
    Result async_tasks.bounds.Footprint := do
  let inputAndResult ← request.input_bytes + request.result_bytes
  let bytes ← inputAndResult + request.parked_bytes
  let inputBuffer := if request.input_bytes = 0#usize then 0#usize else 1#usize
  let resultBuffer := if request.result_bytes = 0#usize then 0#usize else 1#usize
  let parked := if request.parked_bytes = 0#usize then 0#usize else 1#usize
  let owners ← UScalar.cast .Usize request.inputs + UScalar.cast .Usize request.results
  let withInput ← owners + inputBuffer
  let withResult ← withInput + resultBuffer
  let withParked ← withResult + parked
  let retirementWork ← withParked + 1#usize
  .ok ⟨1#usize, 1#usize, bytes, parked, UScalar.cast .Usize request.pins,
    UScalar.cast .U64 request.wakeups, retirementWork⟩

def footprint (request : async_tasks.bounds.Request) :
    Result (Checked async_tasks.bounds.Footprint) := do
  if request.inputs > 64#u8 then return .Err .InvalidRequest
  if request.results > 64#u8 then return .Err .InvalidRequest
  if request.pins > 64#u8 then return .Err .InvalidRequest
  if request.native = 0#u64 then return .Err .InvalidRequest
  let capacity ← maxBytes
  if request.input_bytes > capacity then return .Err .InvalidRequest
  if request.result_bytes > capacity then return .Err .InvalidRequest
  if request.parked_bytes > capacity then return .Err .InvalidRequest
  if UScalar.cast .U64 request.wakeups > 65536#u64 then return .Err .InvalidRequest
  let value ← reservationFootprint request
  if value.bytes > capacity then return .Err .ByteCapacity
  .ok (.Ok value)

def settled (record : Snapshot) : Bool :=
  record.native_stopped && record.pins == 0#u64 && record.retiring_inputs == 0#u64 &&
    record.returned_inputs == 0#u64 && record.results == 0#u64 && record.buffers == 0#u8

def stateValid (record : Snapshot) : Bool :=
  match record.state with
  | .Pending => record.failure.isNone && record.completion.isNone &&
      !record.completion_closed && !record.finalized && record.retiring_inputs == 0#u64
  | .Ready => match record.completion with
    | none => false
    | some completion => record.failure.isNone && !record.finalized &&
        decide (completion.bytes <= record.reservation.result_bytes) &&
        record.returned_inputs == completion.inputs.returned &&
        record.results == completion.produced &&
        (record.retiring_inputs &&& (~~~completion.inputs.retired)) == 0#u64 &&
        ((record.buffers &&& 2#u8) != 0#u8) == (completion.bytes != 0#usize)
  | .Delivered => record.failure.isNone && record.completion.isSome &&
      record.returned_inputs == 0#u64 && record.results == 0#u64 &&
      (record.buffers &&& 2#u8) == 0#u8
  | .Retiring => record.failure.isSome && !record.finalized &&
      record.returned_inputs == 0#u64 && !record.wake_pending
  | .Retired => record.failure.isSome && record.finalized && record.completion_closed &&
      settled record && !record.wake_pending

def dispositionCheck (record : Snapshot) (completion : Completion) : Result (Checked Unit) := do
  if completion.inputs.returned &&& completion.inputs.consumed != 0#u64 then
    return .Err .InvalidDisposition
  if completion.inputs.returned &&& completion.inputs.retired != 0#u64 then
    return .Err .InvalidDisposition
  if completion.inputs.consumed &&& completion.inputs.retired != 0#u64 then
    return .Err .InvalidDisposition
  let inputs ← mask record.reservation.inputs
  if completion.inputs.returned ||| completion.inputs.consumed ||| completion.inputs.retired != inputs then
    return .Err .InvalidDisposition
  let results ← mask record.reservation.results
  if completion.produced &&& (~~~results) != 0#u64 then return .Err .ResultCapacity
  .ok (.Ok ())

def ownersValid (record : Snapshot) : Result Bool := do
  match record.completion with
  | none =>
    if !record.outcome.isNone then return false
    if record.returned_inputs != 0#u64 then return false
    if record.results != 0#u64 then return false
    if record.buffers &&& 2#u8 != 0#u8 then return false
    let inputs ← mask record.reservation.inputs
    if record.retiring_inputs &&& (~~~inputs) != 0#u64 then return false
    if record.completion_closed then .ok record.native_stopped else .ok true
  | some completion =>
    let checked ← dispositionCheck record completion
    if isError checked then return false
    if !record.completion_closed then return false
    if record.outcome.isNone then return false
    if record.returned_inputs &&& (~~~completion.inputs.returned) != 0#u64 then return false
    if record.retiring_inputs &&& (~~~(completion.inputs.retired ||| completion.inputs.returned)) != 0#u64 then
      return false
    if record.results &&& (~~~completion.produced) != 0#u64 then return false
    if record.buffers &&& 2#u8 != 0#u8 then
      if completion.bytes = 0#usize then return false
      if completion.bytes > record.reservation.result_bytes then return false
    .ok true

def recordCheck (record : Snapshot) : Result (Checked Unit) := do
  let reservation ← footprint record.reservation
  if isError reservation then return .Err .InvalidRecord
  if record.handle.generation = 0#u64 then return .Err .InvalidRecord
  if record.handle.slot >= 256#usize then return .Err .InvalidRecord
  if record.native != record.reservation.native then return .Err .InvalidRecord
  if record.handle.context != record.reservation.context then return .Err .InvalidRecord
  let pins ← mask record.reservation.pins
  if record.pins &&& (~~~pins) != 0#u64 then return .Err .InvalidRecord
  if record.wakeups_remaining > record.reservation.wakeups then return .Err .InvalidRecord
  if record.buffers &&& (~~~7#u8) != 0#u8 then return .Err .InvalidRecord
  if record.buffers &&& 1#u8 != 0#u8 then
    if record.reservation.input_bytes = 0#usize then return .Err .InvalidRecord
  if record.buffers &&& 4#u8 != 0#u8 then
    if record.reservation.parked_bytes = 0#usize then return .Err .InvalidRecord
  if record.returned_inputs &&& record.retiring_inputs != 0#u64 then return .Err .InvalidRecord
  let owners ← ownersValid record
  if owners then
    if stateValid record then
      if record.finalized then
        if settled record then
          if record.wake_pending then .ok (.Err .InvalidRecord) else .ok (.Ok ())
        else .ok (.Err .InvalidRecord)
      else .ok (.Ok ())
    else .ok (.Err .InvalidRecord)
  else .ok (.Err .InvalidRecord)

def handleCheck (retained claim : Handle) : Checked Unit :=
  if retained.table != claim.table then .Err .InvalidHandle
  else if retained.slot != claim.slot then .Err .InvalidHandle
  else if retained.context != claim.context then .Err .WrongContext
  else if retained.generation != claim.generation then .Err .WrongGeneration
  else .Ok ()

def nativeCheck (retained : NativeId) (event : Event) : Checked Unit :=
  let supplied := match event with
    | .CompleteSuccess native _ | .CompleteDomainError native _ | .NativeStopped native
      | .SettlePins native _ | .Wake native => some native
    | .Inspect | .Deliver | .Cancel | .Trap | .Deadline | .Budget | .InternalFailure
      | .Cleanup _ | .TakeWake | .Finish => none
  match supplied with
  | none => .Ok ()
  | some native => if native != retained then .Err .WrongNative else .Ok ()

def failureRule (state : State) (failure : Failure) : Checked async_tasks.schema.Rule :=
  match state with
  | .Pending | .Ready => .Ok (.Retire failure)
  | .Delivered | .Retiring | .Retired => .Ok .Duplicate

def classify (state : State) : Event → Checked async_tasks.schema.Rule
  | .Inspect => .Ok .Inspect
  | .CompleteSuccess _ completion => match state with
    | .Pending | .Retiring => .Ok (.Complete .Success completion)
    | .Ready | .Delivered | .Retired => .Ok .Duplicate
  | .CompleteDomainError _ completion => match state with
    | .Pending | .Retiring => .Ok (.Complete .DomainError completion)
    | .Ready | .Delivered | .Retired => .Ok .Duplicate
  | .Deliver => match state with
    | .Pending => .Err .Pending
    | .Ready => .Ok .Deliver
    | .Delivered => .Err .Delivered
    | .Retiring => .Err .Retiring
    | .Retired => .Err .Retired
  | .Cancel => failureRule state .Cancelled
  | .Trap => failureRule state .Trap
  | .Deadline => failureRule state .Deadline
  | .Budget => failureRule state .Budget
  | .InternalFailure => failureRule state .Internal
  | .NativeStopped _ => match state with
    | .Pending | .Ready | .Delivered | .Retiring => .Ok .ObserveStop
    | .Retired => .Ok .Duplicate
  | .SettlePins _ pins => match state with
    | .Pending | .Ready | .Delivered | .Retiring => .Ok (.SettlePins pins)
    | .Retired => .Err .Retired
  | .Cleanup obligations => match state with
    | .Pending => .Err .Pending
    | .Ready | .Delivered | .Retiring => .Ok (.Cleanup obligations)
    | .Retired => .Err .Retired
  | .Wake _ => match state with
    | .Pending | .Ready => .Ok .Wake
    | .Delivered => .Err .Delivered
    | .Retiring => .Err .Retiring
    | .Retired => .Err .Retired
  | .TakeWake => match state with
    | .Pending | .Ready => .Ok .TakeWake
    | .Delivered => .Err .Delivered
    | .Retiring => .Err .Retiring
    | .Retired => .Err .Retired
  | .Finish => match state with
    | .Pending => .Err .Pending
    | .Ready => .Err .Ready
    | .Delivered | .Retiring => .Ok .Finish
    | .Retired => .Ok .Duplicate

def retireWith (record : Snapshot) (reason : Failure) (inputs : U64) : Decision :=
  { record := { record with
      state := .Retiring, failure := some reason,
      completion_closed := if record.native_stopped then true else record.completion_closed,
      returned_inputs := 0#u64, retiring_inputs := record.retiring_inputs ||| inputs,
      wake_pending := false },
    action := match reason with
      | .Cancelled => .CancellationAcknowledged
      | .Trap | .Deadline | .Budget | .Internal => .FailureRecorded reason,
    accounting := { emptyAccounting with
      retired := ⟨inputs, record.results, record.buffers &&& 2#u8⟩,
      wake_removed := record.wake_pending } }

def retire (record : Snapshot) (reason : Failure) : Result Decision := do
  let inputs ← match record.state with
    | .Pending => mask record.reservation.inputs
    | .Ready => .ok record.returned_inputs
    | .Delivered | .Retiring | .Retired => .ok 0#u64
  .ok (retireWith record reason inputs)

def deliver (record : Snapshot) : Decision :=
  { record := { record with
      state := .Delivered, returned_inputs := 0#u64,
      results := 0#u64, buffers := record.buffers &&& (~~~2#u8), wake_pending := false },
    action := .ResultDelivered,
    accounting := { emptyAccounting with
      delivered := ⟨record.returned_inputs, record.results, record.buffers &&& 2#u8⟩,
      wake_removed := record.wake_pending } }

def nativeStop (record : Snapshot) : Decision :=
  if record.native_stopped then unchanged record else
    unchanged { record with
      native_stopped := true,
      completion_closed := match record.state with
        | .Retiring | .Retired => true
        | .Pending | .Ready | .Delivered => record.completion_closed } .NativeStopObserved

def wake (record : Snapshot) : Result Decision :=
  if record.wake_pending then .ok (unchanged record .WakeCoalesced)
  else if record.wakeups_remaining = 0#u32 then retire record .Budget
  else do
    let remaining ← record.wakeups_remaining - 1#u32
    .ok { unchanged { record with wake_pending := true, wakeups_remaining := remaining } .WakeQueued
      with accounting := { emptyAccounting with wake_queued := true } }

def takeWake (record : Snapshot) : Checked Decision :=
  if record.wake_pending then
    .Ok { unchanged { record with wake_pending := false } .WakeTaken
      with accounting := { emptyAccounting with wake_removed := true } }
  else .Err .NoWakeup

/-- All accepted completion metadata, including masks, remains task-owned until
    delivery or named retirement cleanup. A callback never changes native stop. -/
def completeAccepted (record : Snapshot) (outcome : Outcome) (completion : Completion) : Decision :=
  let oversized := decide (completion.bytes > record.reservation.result_bytes)
  let wasRetiring := match record.state with | .Retiring => true | _ => false
  let hasBuffer := completion.bytes != 0#usize && !oversized
  let buffers := if hasBuffer then record.buffers ||| 2#u8 else record.buffers
  let retiring := wasRetiring || oversized
  let retiringInputs := if retiring then completion.inputs.retired ||| completion.inputs.returned
    else completion.inputs.retired
  { record := { record with
      state := if retiring then .Retiring else .Ready,
      completion := some completion, outcome := some outcome,
      failure := if oversized && record.failure.isNone then some .Budget else record.failure,
      completion_closed := true,
      returned_inputs := if retiring then 0#u64 else completion.inputs.returned,
      retiring_inputs := retiringInputs, results := completion.produced, buffers,
      wake_pending := if retiring then false else record.wake_pending },
    action := if oversized then .OversizedResult
      else if wasRetiring then .CompletionRetired outcome else .ResultReady outcome,
    accounting := { emptyAccounting with
      acquired := ⟨0#u64, completion.produced, if hasBuffer then 2#u8 else 0#u8⟩,
      consumed_inputs := completion.inputs.consumed,
      retirement_withdrawn_inputs := record.retiring_inputs &&& completion.inputs.consumed,
      retired := ⟨retiringInputs &&& (~~~record.retiring_inputs),
        if retiring then completion.produced else 0#u64,
        if retiring then buffers &&& 2#u8 else 0#u8⟩,
      rejected_result_bytes := if oversized then completion.bytes else 0#usize,
      completion_accepted := true,
      wake_removed := if retiring then record.wake_pending else false } }

def complete (record : Snapshot) (outcome : Outcome) (completion : Completion) :
    Result (Checked Decision) := do
  if record.completion_closed then return .Ok (unchanged record)
  let check ← dispositionCheck record completion
  match check with
  | .Err error => .ok (.Err error)
  | .Ok _ => .ok (.Ok (completeAccepted record outcome completion))

def settlePins (record : Snapshot) (pins : U64) : Checked Decision :=
  if !record.native_stopped then .Err .NativeStillRunning
  else if pins = 0#u64 ∨ pins &&& (~~~record.pins) != 0#u64 then .Err .InvalidPins
  else .Ok { unchanged { record with pins := record.pins &&& (~~~pins) } .PinsSettled
    with accounting := { emptyAccounting with pins_released := pins } }

def cleanupAvailable (record : Snapshot) : Option Obligations :=
  match record.state with
  | .Ready => some ⟨record.retiring_inputs, 0#u64, record.buffers &&& (~~~2#u8)⟩
  | .Delivered | .Retiring => some ⟨record.retiring_inputs, record.results, record.buffers⟩
  | .Pending | .Retired => none

def cleanup (record : Snapshot) (work : Obligations) : Checked Decision :=
  if !record.native_stopped then .Err .NativeStillRunning
  else if record.pins != 0#u64 then .Err .PinsOutstanding
  else match cleanupAvailable record with
  | none => .Err .InvalidCleanup
  | some available =>
    if work.inputs = 0#u64 ∧ work.results = 0#u64 ∧ work.buffers = 0#u8 then .Err .InvalidCleanup
    else if work.inputs &&& (~~~available.inputs) != 0#u64 ∨
        work.results &&& (~~~available.results) != 0#u64 ∨
        work.buffers &&& (~~~available.buffers) != 0#u8 then .Err .InvalidCleanup
    else .Ok { unchanged { record with
        retiring_inputs := record.retiring_inputs &&& (~~~work.inputs),
        results := record.results &&& (~~~work.results),
        buffers := record.buffers &&& (~~~work.buffers) } .CleanupSettled
      with accounting := { emptyAccounting with cleaned := work } }

def finalDecision (record : Snapshot) (state : State) (action : Action) : Decision :=
  { unchanged { record with state, finalized := true } action
    with accounting := { emptyAccounting with reservation_released := true } }

def finish (record : Snapshot) : Checked Decision :=
  if record.finalized then .Ok (unchanged record)
  else if !record.native_stopped then .Err .NativeStillRunning
  else if record.pins != 0#u64 then .Err .PinsOutstanding
  else if !settled record then .Err .CleanupOutstanding
  else match record.state with
    | .Delivered => .Ok (finalDecision record .Delivered .DeliverySettled)
    | .Retiring => .Ok (finalDecision record .Retired .RetirementCompleted)
    | .Pending | .Ready | .Retired => .Err .InvalidRecord

def applyRule (record : Snapshot) : async_tasks.schema.Rule → Result (Checked Decision)
  | .Inspect => .ok (.Ok (unchanged record .Inspected))
  | .Complete outcome completion => complete record outcome completion
  | .Deliver => .ok (.Ok (deliver record))
  | .Retire reason => do
    let decision ← retire record reason
    .ok (.Ok decision)
  | .ObserveStop => .ok (.Ok (nativeStop record))
  | .SettlePins pins => .ok (settlePins record pins)
  | .Cleanup work => .ok (cleanup record work)
  | .Wake => do
    let decision ← wake record
    .ok (.Ok decision)
  | .TakeWake => .ok (takeWake record)
  | .Finish => .ok (finish record)
  | .Duplicate => .ok (.Ok (unchanged record))

def applyEvent (record : Snapshot) (event : Event) : Result (Checked Decision) :=
  match classify record.state event with
  | .Err error => .ok (.Err error)
  | .Ok rule => applyRule record rule

def step (record : Snapshot) (claim : Handle) (event : Event) : Result (Checked Decision) := do
  let valid ← recordCheck record
  match valid with
  | .Err error => .ok (.Err error)
  | .Ok () => match handleCheck record.handle claim with
    | .Err error => .ok (.Err error)
    | .Ok () => match nativeCheck record.native event with
      | .Err error => .ok (.Err error)
      | .Ok () => applyEvent record event

def tableStep (table : async_tasks.table.Table) (claim : Handle) (event : Event) :
    Result (Checked Decision) :=
  if claim.table != table.id then .ok (.Err .InvalidHandle)
  else match table.records.val[claim.slot.val]? with
    | none => .ok (.Err .InvalidHandle)
    | some record => step record claim event

/- Correspondence to the actual Charon → Aeneas declarations. -/

@[simp] theorem resultPure {α : Type} (value : α) :
    (pure value : Result α) = Result.ok value := by rfl

@[simp] theorem scalarBEq {ty : UScalarTy} (left right : UScalar ty) :
    (left == right) = decide (left = right) := by
  change (left.bv == right.bv) = decide (left = right)
  apply Bool.eq_iff_iff.mpr
  simp [UScalar.eq_equiv_bv_eq]

@[simp] theorem boolBEq (left right : Bool) : (left == right) = decide (left = right) := by
  cases left <;> cases right <;> rfl

@[simp] theorem isError_refines {α : Type} (value : Checked α) :
    core.result.Result.is_err value = Result.ok (isError value) := by
  cases value <;> rfl

@[simp] theorem emptyObligations_refines :
    async_tasks.domain.Obligations.empty = Result.ok emptyObligations := by rfl

@[simp] theorem emptyAccounting_refines :
    async_tasks.accounting.Accounting.empty = Result.ok emptyAccounting := by
  simp [async_tasks.accounting.Accounting.empty, emptyAccounting]

@[simp] theorem unchanged_refines (record : Snapshot) (action : Action) :
    async_tasks.accounting.Decision.unchanged record action = Result.ok (unchanged record action) := by
  simp [async_tasks.accounting.Decision.unchanged, unchanged]

@[simp] theorem mask_refines (count : U8) : async_tasks.bounds.mask count = mask count := by
  simp [async_tasks.bounds.mask, mask, async_tasks.bounds.MAX_OBLIGATIONS, core.num.U64.MAX, U64.rMax]

@[simp] theorem reservationFootprint_refines (request : async_tasks.bounds.Request) :
    async_tasks.bounds.Request.validated_footprint request = reservationFootprint request := by
  by_cases inputBuffer : request.input_bytes = 0#usize <;>
    by_cases resultBuffer : request.result_bytes = 0#usize <;>
    by_cases parkedBuffer : request.parked_bytes = 0#usize <;>
    simp only [async_tasks.bounds.Request.validated_footprint, reservationFootprint,
      lift, bind_tc_ok,
      inputBuffer, resultBuffer, parkedBuffer, ↓reduceIte]

@[simp] theorem footprint_refines (request : async_tasks.bounds.Request) :
    async_tasks.bounds.Request.footprint request = footprint request := by
  simp only [async_tasks.bounds.Request.footprint, footprint, maxBytes,
    reservationFootprint_refines, async_tasks.bounds.MAX_BYTES,
    async_tasks.bounds.MAX_OBLIGATIONS, async_tasks.bounds.MAX_WAKEUPS,
    lift, resultPure, bind_tc_ok]

@[simp] theorem settled_refines (record : Snapshot) :
    async_tasks.transition.validation.settled record = Result.ok (settled record) := by
  simp [async_tasks.transition.validation.settled, settled]
  all_goals split_ifs <;> simp_all

@[simp] theorem stateValid_refines (record : Snapshot) :
    async_tasks.transition.validation.state_valid record = Result.ok (stateValid record) := by
  cases state : record.state <;> cases completion : record.completion <;>
    cases failure : record.failure <;>
    simp [async_tasks.transition.validation.state_valid, stateValid, state, completion, failure,
      core.option.Option.is_none, core.option.Option.is_some, lift]
  all_goals split_ifs <;> simp_all

@[simp] theorem dispositionCheck_refines (record : Snapshot) (completion : Completion) :
    async_tasks.transition.validation.disposition record completion = dispositionCheck record completion := by
  simp [async_tasks.transition.validation.disposition, dispositionCheck, lift]

@[simp] theorem ownersValid_refines (record : Snapshot) :
    async_tasks.transition.validation.owners_valid record = ownersValid record := by
  cases completion : record.completion <;>
    simp [async_tasks.transition.validation.owners_valid, ownersValid, completion,
      core.option.Option.is_none, lift]
  all_goals split_ifs <;> simp_all [UScalar.eq_equiv]

@[simp] theorem recordCheck_refines (record : Snapshot) :
    async_tasks.transition.validation.record record = recordCheck record := by
  simp only [async_tasks.transition.validation.record, recordCheck, footprint_refines,
    mask_refines, ownersValid_refines, stateValid_refines, settled_refines, isError_refines,
    async_tasks.bounds.MAX_SLOTS, lift, resultPure, bind_tc_ok]

@[simp] theorem handleCheck_refines (retained claim : Handle) :
    async_tasks.transition.validation.handle retained claim = Result.ok (handleCheck retained claim) := by
  simp [async_tasks.transition.validation.handle, handleCheck]
  split_ifs <;> simp_all

@[simp] theorem nativeCheck_refines (retained : NativeId) (event : Event) :
    async_tasks.transition.validation.native retained event = Result.ok (nativeCheck retained event) := by
  cases event <;> simp [async_tasks.transition.validation.native, nativeCheck]
  all_goals split_ifs <;> simp_all

/-- All five states and fifteen event constructors, with arbitrary payloads. -/
theorem classify_refines (state : State) (event : Event) :
    async_tasks.schema.classify state event = Result.ok (classify state event) := by
  cases state <;> cases event <;>
    simp only [async_tasks.schema.classify, async_tasks.schema.completion_rule,
      async_tasks.schema.retirement_rule, async_tasks.schema.delivery_rule, classify, failureRule,
      bind_tc_ok]

@[simp] theorem retire_refines (record : Snapshot) (reason : Failure) :
    async_tasks.transition.lifecycle.retire record reason = retire record reason := by
  cases state : record.state <;> cases reason <;> cases stopped : record.native_stopped <;>
    simp [async_tasks.transition.lifecycle.retire, retire, retireWith, state, stopped, lift]

@[simp] theorem deliver_refines (record : Snapshot) :
    async_tasks.transition.lifecycle.deliver record = Result.ok (deliver record) := by
  simp [async_tasks.transition.lifecycle.deliver, deliver, lift]

@[simp] theorem nativeStop_refines (record : Snapshot) :
    async_tasks.transition.lifecycle.native_stop record = Result.ok (nativeStop record) := by
  cases state : record.state <;>
    simp [async_tasks.transition.lifecycle.native_stop, nativeStop, state]
  all_goals split_ifs <;> simp_all

@[simp] theorem wake_refines (record : Snapshot) :
    async_tasks.transition.lifecycle.wake record = wake record := by
  simp [async_tasks.transition.lifecycle.wake, wake, unchanged]

@[simp] theorem takeWake_refines (record : Snapshot) :
    async_tasks.transition.lifecycle.take_wake record = Result.ok (takeWake record) := by
  simp [async_tasks.transition.lifecycle.take_wake, takeWake, unchanged]
  all_goals split_ifs <;> simp_all

@[simp] theorem exceedsLimit_refines (completion : Completion) (limit : Usize) :
    async_tasks.transition.completion.exceeds_limit completion limit =
      Result.ok (decide (completion.bytes > limit)) := by rfl

@[simp] theorem complete_refines (record : Snapshot) (outcome : Outcome) (completion : Completion) :
    async_tasks.transition.completion.complete record outcome completion = complete record outcome completion := by
  cases closed : record.completion_closed with
  | true => simp [async_tasks.transition.completion.complete, complete, closed]
  | false =>
    simp [async_tasks.transition.completion.complete, complete, closed]
    congr 1
    funext check
    cases check with
    | Err error => rfl
    | Ok value =>
      cases value
      cases state : record.state <;>
        by_cases oversized : record.reservation.result_bytes.val < completion.bytes.val <;>
        by_cases zeroBytes : completion.bytes.val = 0 <;>
        cases failure : record.failure <;>
        simp [completeAccepted, state, oversized, zeroBytes, failure, UScalar.eq_equiv,
          lift, emptyAccounting, emptyObligations]

@[simp] theorem settlePins_refines (record : Snapshot) (pins : U64) :
    async_tasks.transition.cleanup.pins record pins = Result.ok (settlePins record pins) := by
  simp [async_tasks.transition.cleanup.pins, settlePins, unchanged, lift]
  all_goals split_ifs <;> simp_all

@[simp] theorem cleanup_refines (record : Snapshot) (work : Obligations) :
    async_tasks.transition.cleanup.settle record work = Result.ok (cleanup record work) := by
  cases state : record.state <;>
    simp [async_tasks.transition.cleanup.settle, cleanup, cleanupAvailable, state, unchanged, lift,
      ite_and, ite_or]
  all_goals split_ifs <;> simp_all

@[simp] theorem finish_refines (record : Snapshot) :
    async_tasks.transition.cleanup.finish record = Result.ok (finish record) := by
  cases state : record.state <;>
    simp [async_tasks.transition.cleanup.finish, finish, finalDecision, state, unchanged]
  all_goals split_ifs <;> simp_all

/-- Universal equality includes rejection and checked-machine failure paths. -/
theorem transition_refines (record : Snapshot) (claim : Handle) (event : Event) :
    async_tasks.transition.transition record claim event = step record claim event := by
  simp [async_tasks.transition.transition, step, applyEvent, classify_refines]
  congr 1
  funext valid
  cases valid <;> simp
  cases handleCheck record.handle claim <;> simp
  cases nativeCheck record.native event <;> simp
  cases classify record.state event <;> simp
  rename_i rule
  cases rule <;> simp [applyRule]

/-- This is the retained production Table.decide, including its namespace check
    and actual retained slot lookup, not a model-only transition wrapper. -/
theorem table_decide_refines (table : async_tasks.table.Table) (claim : Handle) (event : Event) :
    async_tasks.table.Table.decide table claim event = tableStep table claim event := by
  simp [async_tasks.table.Table.decide, tableStep, core.slice.Slice.get, alloc.vec.Vec.deref]
  split_ifs <;> try simp_all
  cases table.records.val[claim.slot.val]? <;> simp [transition_refines]

@[simp] theorem handleCheck_self (handle : Handle) : handleCheck handle handle = .Ok () := by
  simp [handleCheck]

/-- The guard is exactly successful independent record validation, including
    ownership and reservation checks. It is never assumed for arbitrary records. -/
def WellFormed (record : Snapshot) : Prop := recordCheck record = .ok (.Ok ())

def completionEvent (native : NativeId) (outcome : Outcome) (completion : Completion) : Event :=
  match outcome with
  | .Success => .CompleteSuccess native completion
  | .DomainError => .CompleteDomainError native completion

@[simp] theorem nativeCheck_completionEvent (record : Snapshot) (outcome : Outcome)
    (completion : Completion) : nativeCheck record.native (completionEvent record.native outcome completion) = .Ok () := by
  cases outcome <;> simp [completionEvent, nativeCheck]

 theorem authenticated_step (record : Snapshot) (event : Event)
    (valid : WellFormed record) (authentic : nativeCheck record.native event = .Ok ()) :
    async_tasks.transition.transition record record.handle event = applyEvent record event := by
  change recordCheck record = .ok (.Ok ()) at valid
  simp [transition_refines, step, valid, authentic]

/- Semantic roots follow below. Success premises mean a committed local decision;
   no theorem turns a supplied callback id into independent shell authority. -/

theorem complete_success_cases (record : Snapshot) (outcome : Outcome)
    (completion : Completion) (decision : Decision)
    (accepted : complete record outcome completion = .ok (.Ok decision)) :
    decision = unchanged record ∨ decision = completeAccepted record outcome completion := by
  by_cases closed : record.completion_closed
  · left
    simpa [complete, closed] using accepted.symm
  · cases checked : dispositionCheck record completion with
    | ret check =>
      cases check with
      | Ok value =>
        cases value
        right
        simpa [complete, closed, checked] using accepted.symm
      | Err error => simp [complete, closed, checked] at accepted
    | vis effect continuation => simp [complete, closed, checked] at accepted
    | div => simp [complete, closed, checked] at accepted

/-- Accepted input dispositions are a disjoint, exhaustive partition, and
    produced result identities fit the separately reserved result mask. -/
theorem accepted_disposition_masks (record : Snapshot) (completion : Completion)
    (accepted : async_tasks.transition.validation.disposition record completion = .ok (.Ok ())) :
    ∃ inputs results,
      mask record.reservation.inputs = .ok inputs ∧
      mask record.reservation.results = .ok results ∧
      completion.inputs.returned &&& completion.inputs.consumed = 0#u64 ∧
      completion.inputs.returned &&& completion.inputs.retired = 0#u64 ∧
      completion.inputs.consumed &&& completion.inputs.retired = 0#u64 ∧
      completion.inputs.returned ||| completion.inputs.consumed ||| completion.inputs.retired = inputs ∧
      completion.produced &&& (~~~results) = 0#u64 := by
  rw [dispositionCheck_refines] at accepted
  by_cases returnedConsumed : completion.inputs.returned &&& completion.inputs.consumed = 0#u64
  · by_cases returnedRetired : completion.inputs.returned &&& completion.inputs.retired = 0#u64
    · by_cases consumedRetired : completion.inputs.consumed &&& completion.inputs.retired = 0#u64
      · simp [dispositionCheck, returnedConsumed, returnedRetired, consumedRetired] at accepted
        cases inputMask : mask record.reservation.inputs with
        | ret inputs =>
          by_cases exhaustive :
              completion.inputs.returned ||| completion.inputs.consumed ||| completion.inputs.retired = inputs
          · simp_all [UScalar.eq_equiv]
            cases resultMask : mask record.reservation.results with
            | ret results =>
              by_cases bounded : completion.produced &&& (~~~results) = 0#u64
              · refine ⟨results, ?_⟩
                simp_all [UScalar.eq_equiv]
              · simp_all [UScalar.eq_equiv]
            | vis effect continuation => simp_all
            | div => simp_all
          · simp_all [UScalar.eq_equiv]
        | vis effect continuation => simp [inputMask] at accepted
        | div => simp [inputMask] at accepted
      · simp [dispositionCheck, returnedConsumed, returnedRetired, consumedRetired] at accepted
    · simp [dispositionCheck, returnedConsumed, returnedRetired] at accepted
  · simp [dispositionCheck, returnedConsumed] at accepted

theorem retire_success_fields (record : Snapshot) (reason : Failure) (decision : Decision)
    (accepted : retire record reason = .ok decision) :
    decision.record.state = .Retiring ∧
    decision.record.native_stopped = record.native_stopped ∧
    decision.record.pins = record.pins ∧ decision.accounting.pins_released = 0#u64 ∧
    decision.accounting.reservation_released = false ∧
    decision.record.returned_inputs = 0#u64 ∧
    decision.record.retiring_inputs = record.retiring_inputs ||| decision.accounting.retired.inputs ∧
    decision.accounting.retired.results = record.results ∧
    decision.accounting.retired.buffers = record.buffers &&& 2#u8 ∧
    decision.accounting.delivered = emptyObligations ∧
    decision.record.wake_pending = false := by
  cases state : record.state
  all_goals simp [retire, state] at accepted
  case Pending =>
    cases checked : mask record.reservation.inputs <;> simp [checked] at accepted
    subst decision
    simp [retireWith, emptyAccounting]
  all_goals
    subst decision
    simp [retireWith, emptyAccounting]

/-- Cancellation moves Pending/Ready ownership into retirement. It neither
    observes native stop, settles a native pin, delivers owners, nor releases
    admission capacity. The successful-decision premise retains arithmetic
    failures instead of silently treating them as cancellation success. -/
theorem cancellation_preserves_native_pins (record : Snapshot) (decision : Decision)
    (valid : WellFormed record)
    (cancellable : record.state = .Pending ∨ record.state = .Ready)
    (accepted : async_tasks.transition.transition record record.handle .Cancel = .ok (.Ok decision)) :
    decision.record.state = .Retiring ∧
    decision.record.native_stopped = record.native_stopped ∧
    decision.record.pins = record.pins ∧ decision.accounting.pins_released = 0#u64 ∧
    decision.accounting.reservation_released = false ∧
    decision.record.returned_inputs = 0#u64 ∧
    decision.record.retiring_inputs = record.retiring_inputs ||| decision.accounting.retired.inputs ∧
    decision.accounting.retired.results = record.results ∧
    decision.accounting.retired.buffers = record.buffers &&& 2#u8 ∧
    decision.accounting.delivered = emptyObligations ∧
    decision.record.wake_pending = false := by
  rw [authenticated_step record .Cancel valid (by rfl)] at accepted
  rcases cancellable with state | state
  all_goals
    simp [applyEvent, classify, failureRule, state, applyRule] at accepted
    cases result : retire record .Cancelled <;> simp_all
    apply retire_success_fields record .Cancelled decision
    simpa using result

theorem completion_step (record : Snapshot) (outcome : Outcome) (completion : Completion)
    (valid : WellFormed record) :
    async_tasks.transition.transition record record.handle
      (completionEvent record.native outcome completion) =
      match record.state with
      | .Pending | .Retiring => complete record outcome completion
      | .Ready | .Delivered | .Retired => .ok (.Ok (unchanged record)) := by
  rw [authenticated_step record _ valid (nativeCheck_completionEvent record outcome completion)]
  cases state : record.state <;> cases outcome <;>
    simp [applyEvent, classify, completionEvent, state, applyRule]

/-- Both ordinary outcomes retain exact returned inputs, produced results and
    bounded result bytes in Ready. Delivery accounting remains empty. -/
theorem bounded_completion_owns_results (record : Snapshot) (outcome : Outcome)
    (completion : Completion) (valid : WellFormed record)
    (pending : record.state = .Pending) (openCompletion : record.completion_closed = false)
    (metadata : dispositionCheck record completion = .ok (.Ok ()))
    (bounded : completion.bytes <= record.reservation.result_bytes) :
    async_tasks.transition.transition record record.handle
      (completionEvent record.native outcome completion) =
        .ok (.Ok (completeAccepted record outcome completion)) ∧
    (completeAccepted record outcome completion).record.state = .Ready ∧
    (completeAccepted record outcome completion).record.returned_inputs = completion.inputs.returned ∧
    (completeAccepted record outcome completion).record.results = completion.produced ∧
    (completeAccepted record outcome completion).record.completion = some completion ∧
    (completeAccepted record outcome completion).accounting.acquired.results = completion.produced ∧
    (completeAccepted record outcome completion).accounting.delivered = emptyObligations ∧
    (completeAccepted record outcome completion).accounting.completion_accepted = true := by
  constructor
  · rw [completion_step record outcome completion valid]
    simp [pending, complete, openCompletion, metadata]
  · simp [completeAccepted, pending, not_lt_of_ge bounded, emptyAccounting]

/-- Success and domain-error callbacks have identical pin/stop discipline.
    This also covers callbacks already classified as duplicates. -/
theorem completion_does_not_stop_native (record : Snapshot) (outcome : Outcome)
    (completion : Completion) (decision : Decision) (valid : WellFormed record)
    (accepted : async_tasks.transition.transition record record.handle
      (completionEvent record.native outcome completion) = .ok (.Ok decision)) :
    decision.record.native_stopped = record.native_stopped ∧
    decision.record.pins = record.pins ∧
    decision.accounting.pins_released = 0#u64 ∧
    decision.accounting.reservation_released = false ∧
    decision.accounting.delivered = emptyObligations := by
  have completeFields (success : complete record outcome completion = .ok (.Ok decision)) :
      decision.record.native_stopped = record.native_stopped ∧
      decision.record.pins = record.pins ∧
      decision.accounting.pins_released = 0#u64 ∧
      decision.accounting.reservation_released = false ∧
      decision.accounting.delivered = emptyObligations := by
    rcases complete_success_cases record outcome completion decision success with rfl | rfl
    all_goals simp [unchanged, completeAccepted, emptyAccounting]
  rw [completion_step record outcome completion valid] at accepted
  cases state : record.state <;> simp only [state] at accepted
  case Pending => exact completeFields accepted
  case Retiring => exact completeFields accepted
  all_goals
    simp at accepted
    subst decision
    simp [unchanged, emptyAccounting]

/-- A result arriving after cancellation/failure remains in retirement even
    when its bytes are oversized. It cannot be delivered by that callback. -/
theorem late_completion_not_delivered (record : Snapshot) (outcome : Outcome)
    (completion : Completion) (decision : Decision) (valid : WellFormed record)
    (retiring : record.state = .Retiring)
    (accepted : async_tasks.transition.transition record record.handle
      (completionEvent record.native outcome completion) = .ok (.Ok decision)) :
    decision.record.state = .Retiring ∧
    decision.accounting.delivered = emptyObligations ∧
    decision.accounting.pins_released = 0#u64 ∧
    decision.accounting.reservation_released = false := by
  rw [completion_step record outcome completion valid] at accepted
  simp [retiring] at accepted
  rcases complete_success_cases record outcome completion decision accepted with rfl | rfl
  all_goals simp [unchanged, completeAccepted, retiring, emptyAccounting]

/-- Delivery is the cancellation boundary: cancel cannot revoke owners already
    delivered or manufacture a second retirement/accounting delta. -/
theorem delivered_cancel_idempotent (record : Snapshot) (valid : WellFormed record)
    (delivered : record.state = .Delivered) :
    async_tasks.transition.transition record record.handle .Cancel =
      .ok (.Ok (unchanged record)) ∧
    (unchanged record).accounting = emptyAccounting := by
  constructor
  · rw [authenticated_step record .Cancel valid (by rfl)]
    simp [applyEvent, classify, failureRule, delivered, applyRule]
  · rfl

/-- Completed-state duplicates ignore replacement metadata after identity
    validation; no second owner acquisition or delivery is reported. -/
theorem duplicate_completion_idempotent (record : Snapshot) (outcome : Outcome)
    (completion : Completion) (valid : WellFormed record)
    (completed : record.state = .Ready ∨ record.state = .Delivered ∨ record.state = .Retired) :
    async_tasks.transition.transition record record.handle
      (completionEvent record.native outcome completion) = .ok (.Ok (unchanged record)) ∧
    (unchanged record).accounting = emptyAccounting := by
  constructor
  · rw [completion_step record outcome completion valid]
    rcases completed with state | state | state <;> simp [state]
  · rfl

/-- Even authentic pin-settlement requests are rejected while native execution
    is running. Retired records are excluded because they reject as Retired. -/
theorem pins_require_native_stop (record : Snapshot) (pins : U64)
    (valid : WellFormed record) (active : record.state ≠ .Retired)
    (running : record.native_stopped = false) :
    async_tasks.transition.transition record record.handle (.SettlePins record.native pins) =
      .ok (.Err .NativeStillRunning) := by
  rw [authenticated_step record _ valid (by simp [nativeCheck])]
  cases state : record.state <;>
    simp_all [applyEvent, classify, applyRule, settlePins]

/-- Fresh successful finalization requires native stop, no native pins and no
    undischarged owners/buffers. It emits exactly one reservation-release bit. -/
theorem finish_requires_settlement (record : Snapshot) (decision : Decision)
    (valid : WellFormed record) (fresh : record.finalized = false)
    (eligible : record.state = .Delivered ∨ record.state = .Retiring)
    (accepted : async_tasks.transition.transition record record.handle .Finish = .ok (.Ok decision)) :
    settled record = true ∧ decision.record.finalized = true ∧
    decision.accounting.reservation_released = true := by
  rw [authenticated_step record .Finish valid (by rfl)] at accepted
  rcases eligible with state | state
  all_goals
    simp [applyEvent, classify, state, applyRule, finish, fresh] at accepted
    split_ifs at accepted; try simp_all [finalDecision, unchanged, emptyAccounting]
  all_goals
    subst decision
    simp_all

@[simp] theorem and_not_zero_u64 (value : U64) : value &&& (~~~(0#u64)) = value := by
  apply (UScalar.eq_equiv_bv_eq _ _).mpr
  simp

theorem ready_cleanup_preserves_results (record : Snapshot) (work : Obligations)
    (decision : Decision) (ready : record.state = .Ready)
    (accepted : cleanup record work = .Ok decision) :
    decision.record.results = record.results ∧
    decision.accounting.cleaned.results = 0#u64 := by
  simp only [cleanup, cleanupAvailable, ready, and_not_zero_u64] at accepted
  split_ifs at accepted; try simp_all [unchanged, emptyAccounting]
  all_goals
    have noResults : work.results = 0#u64 := by
      apply (UScalar.eq_equiv _ _).mpr
      simp_all
    subst decision
    change record.results &&& (~~~work.results) = record.results ∧ work.results = 0#u64
    refine ⟨?_, noResults⟩
    rw [noResults]
    exact and_not_zero_u64 record.results

/-- Ready owners and the result-buffer bit are transferred by Deliver, never
    counted as delivered at completion. Named Ready cleanup cannot consume any
    result owner; its available buffer mask excludes the result-buffer bit. -/
theorem ready_results_owned_until_delivery (record : Snapshot) (valid : WellFormed record)
    (ready : record.state = .Ready) :
    async_tasks.transition.transition record record.handle .Deliver = .ok (.Ok (deliver record)) ∧
    (deliver record).accounting.delivered =
      ⟨record.returned_inputs, record.results, record.buffers &&& 2#u8⟩ ∧
    (deliver record).record.returned_inputs = 0#u64 ∧
    (deliver record).record.results = 0#u64 ∧
    (deliver record).record.native_stopped = record.native_stopped ∧
    (deliver record).record.pins = record.pins ∧
    (deliver record).accounting.reservation_released = false ∧
    cleanupAvailable record =
      some ⟨record.retiring_inputs, 0#u64, record.buffers &&& (~~~2#u8)⟩ ∧
    (∀ work decision,
      async_tasks.transition.transition record record.handle (.Cleanup work) = .ok (.Ok decision) →
      decision.record.results = record.results ∧ decision.accounting.cleaned.results = 0#u64) := by
  refine ⟨?_, rfl, rfl, rfl, rfl, rfl, rfl, ?_, ?_⟩
  · rw [authenticated_step record .Deliver valid (by rfl)]
    simp [applyEvent, classify, ready, applyRule]
  · simp [cleanupAvailable, ready]
  · intro work decision accepted
    rw [authenticated_step record (.Cleanup work) valid (by rfl)] at accepted
    simp [applyEvent, classify, ready, applyRule] at accepted
    exact ready_cleanup_preserves_results record work decision ready accepted

theorem complete_preserves_primary (record : Snapshot) (outcome : Outcome)
    (completion : Completion) (decision : Decision) (primary : Failure)
    (failed : record.failure = some primary)
    (accepted : complete record outcome completion = .ok (.Ok decision)) :
    decision.record.failure = some primary := by
  rcases complete_success_cases record outcome completion decision accepted with rfl | rfl
  all_goals simp [unchanged, completeAccepted, failed]

theorem retiring_apply_preserves_primary (record : Snapshot) (event : Event)
    (decision : Decision) (primary : Failure)
    (retiring : record.state = .Retiring) (failed : record.failure = some primary)
    (accepted : applyEvent record event = .ok (.Ok decision)) :
    decision.record.failure = some primary := by
  cases event <;>
    simp [applyEvent, classify, failureRule, retiring, applyRule] at accepted
  case CompleteSuccess native completion =>
    exact complete_preserves_primary record .Success completion decision primary failed accepted
  case CompleteDomainError native completion =>
    exact complete_preserves_primary record .DomainError completion decision primary failed accepted
  all_goals try
    simp_all [nativeStop, settlePins, cleanup, cleanupAvailable, finish, finalDecision, unchanged]
  all_goals try (split_ifs at accepted)
  all_goals try simp_all
  all_goals subst decision
  all_goals simp_all

/-- Once abnormal retirement has recorded a primary failure, every successful
    later event preserves it, including oversized late results and cleanup.
    It intentionally does not assert this of fabricated Pending/Ready failures. -/
theorem failure_preserves_primary (record : Snapshot) (event : Event)
    (decision : Decision) (primary : Failure) (valid : WellFormed record)
    (retiring : record.state = .Retiring ∨ record.state = .Retired)
    (failed : record.failure = some primary)
    (accepted : async_tasks.transition.transition record record.handle event = .ok (.Ok decision)) :
    decision.record.failure = some primary := by
  rw [transition_refines] at accepted
  change recordCheck record = .ok (.Ok ()) at valid
  simp [step, valid] at accepted
  cases authentic : nativeCheck record.native event with
  | Err error => simp [authentic] at accepted
  | Ok value =>
    cases value
    simp [authentic] at accepted
    rcases retiring with retiring | retired
    · exact retiring_apply_preserves_primary record event decision primary retiring failed accepted
    · cases event <;>
        simp [applyEvent, classify, failureRule, retired, applyRule, unchanged] at accepted
      all_goals subst decision
      all_goals exact failed

theorem successful_step_wellFormed (record : Snapshot) (claim : Handle)
    (event : Event) (decision : Decision)
    (accepted : async_tasks.transition.transition record claim event = .ok (.Ok decision)) :
    WellFormed record := by
  rw [transition_refines] at accepted
  cases checked : recordCheck record with
  | ret check =>
    cases check with
    | Ok value => cases value; exact checked
    | Err error => simp [step, checked] at accepted
  | vis effect continuation => simp [step, checked] at accepted
  | div => simp [step, checked] at accepted

/-- With two committed Finish decisions, the first fresh one releases the
    reservation and finalizes the record; the second is an unchanged duplicate.
    A rejected second call is not a release either, and is not falsely claimed
    to be a successful transition. -/
theorem release_once (record : Snapshot) (first second : Decision)
    (valid : WellFormed record) (fresh : record.finalized = false)
    (eligible : record.state = .Delivered ∨ record.state = .Retiring)
    (firstAccepted : async_tasks.transition.transition record record.handle .Finish = .ok (.Ok first))
    (secondAccepted : async_tasks.transition.transition first.record first.record.handle .Finish = .ok (.Ok second)) :
    first.accounting.reservation_released = true ∧
    first.record.finalized = true ∧
    second = unchanged first.record ∧
    second.accounting.reservation_released = false := by
  have firstFields := finish_requires_settlement record first valid fresh eligible firstAccepted
  have secondValid := successful_step_wellFormed first.record first.record.handle .Finish second secondAccepted
  rw [authenticated_step first.record .Finish secondValid (by rfl)] at secondAccepted
  have secondUnchanged : second = unchanged first.record := by
    cases state : first.record.state <;>
      simp [applyEvent, classify, state, applyRule, finish, firstFields.2.1] at secondAccepted
    all_goals exact secondAccepted.symm
  refine ⟨firstFields.2.2, firstFields.2.1, secondUnchanged, ?_⟩
  simp [secondUnchanged, unchanged, emptyAccounting]

end M6Async
