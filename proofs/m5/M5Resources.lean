import NobleKernel

open Aeneas Aeneas.Std
open noble_kernel
open noble_kernel.resources

set_option maxHeartbeats 2000000
set_option maxRecDepth 4096

namespace M5Resources

abbrev Checked (α : Type) := core.result.Result α resources.Error

def valid (record : Snapshot) : Bool :=
  if record.handle.generation = 0#u64 then false
  else if record.last_scope = some 0#u64 then false
  else match record.state with
  | .Live => record.retirement.isNone
  | .Busy serial => serial != 0#u64 && decide (record.last_scope = some serial) && record.retirement.isNone
  | .Retiring serial => serial != 0#u64 && decide (record.last_scope = some serial) && record.retirement.isSome
  | .Retired => true

def handleCheck (retained claim : Handle) : Checked Unit :=
  if retained.table ≠ claim.table ∨ retained.slot ≠ claim.slot then .Err .InvalidHandle
  else if retained.context ≠ claim.context then .Err .WrongContext
  else if retained.kind ≠ claim.kind then .Err .WrongKind
  else if retained.generation ≠ claim.generation then .Err .WrongGeneration
  else if retained.rights ≠ claim.rights then .Err .WrongRights
  else .Ok ()

def liveCheck (record : Snapshot) (required : Requirement) : Checked Unit :=
  if record.handle.context ≠ required.context then .Err .WrongContext
  else if record.handle.kind ≠ required.kind then .Err .WrongKind
  else if record.handle.rights &&& required.rights != required.rights then .Err .WrongRights
  else match record.state with
  | .Live => .Ok ()
  | .Busy _ => .Err .Busy
  | .Retiring _ => .Err .Retiring
  | .Retired => .Err .Retired

def scopeCheck (record : Snapshot) (serial : U64) : Checked Unit :=
  if serial = 0#u64 ∨ record.last_scope ≠ some serial then .Err .WrongScope else .Ok ()

def afterCheck (check : Checked Unit) (result : Checked Decision) : Checked Decision :=
  match check with
  | .Ok () => result
  | .Err error => .Err error

def unchanged (record : Snapshot) : Decision := ⟨record, .Unchanged⟩

def retire (record : Snapshot) (reason : Retirement) : Decision :=
  match record.state with
  | .Live => ⟨{ record with state := .Retired, retirement := some reason }, .LocalRelease⟩
  | .Busy serial => ⟨{ record with state := .Retiring serial, retirement := some reason }, .AccessRevoked⟩
  | .Retiring _ | .Retired => unchanged record

def inspect (record : Snapshot) (required : Requirement) : Checked Decision :=
  afterCheck (liveCheck record required) (.Ok (unchanged record))

def begin (record : Snapshot) (required : Requirement) (serial : U64) : Checked Decision :=
  afterCheck (liveCheck record required)
    (if serial = 0#u64 ∨ record.last_scope.any (fun previous => serial <= previous) then .Err .WrongScope
     else .Ok ⟨{ record with state := .Busy serial, last_scope := some serial }, .BorrowAdmitted⟩)

def access (record : Snapshot) (serial : U64) : Checked Decision :=
  afterCheck (scopeCheck record serial) (match record.state with
    | .Live => .Err .WrongScope
    | .Busy _ => .Ok (unchanged record)
    | .Retiring _ => .Err .Retiring
    | .Retired => .Err .Retired)

def complete (record : Snapshot) (serial : U64) (outcome : Completion) : Checked Decision :=
  afterCheck (scopeCheck record serial) (match record.state with
    | .Live | .Retired => .Ok (unchanged record)
    | .Busy _ => .Ok ⟨{ record with state := .Live }, .OwnerReturned outcome⟩
    | .Retiring _ => .Ok ⟨{ record with state := .Retired }, .RetirementCompleted⟩)

def revoke (record : Snapshot) (serial : U64) (reason : Retirement) : Checked Decision :=
  afterCheck (scopeCheck record serial) (match record.state with
    | .Live | .Retired => .Ok (unchanged record)
    | .Busy _ | .Retiring _ => .Ok (retire record reason))

def release (record : Snapshot) (required : Requirement) : Checked Decision :=
  afterCheck (liveCheck record required) (.Ok ⟨{ record with state := .Retired }, .LocalRelease⟩)

def transfer (record : Snapshot) (required : Requirement) (receiver : Context) (generation : U64) : Checked Decision :=
  afterCheck (liveCheck record required)
    (if generation <= record.handle.generation then .Err .WrongGeneration
     else .Ok ⟨{ record with handle := { record.handle with generation, context := receiver }, last_scope := none }, .OwnerTransferred⟩)

def applyEvent (record : Snapshot) : Event → Checked Decision
  | .Inspect required => inspect record required
  | .Begin required serial => begin record required serial
  | .Access serial => access record serial
  | .Complete serial outcome => complete record serial outcome
  | .Revoke serial reason => revoke record serial reason
  | .Retire reason => .Ok (retire record reason)
  | .Release required => release record required
  | .Transfer required receiver generation => transfer record required receiver generation

def step (record : Snapshot) (claim : Handle) (event : Event) : Checked Decision :=
  if valid record then afterCheck (handleCheck record.handle claim) (applyEvent record event)
  else .Err .InvalidRecord

@[simp] theorem option_eq (a b : Option U64) :
    noble_kernel.core.option.Option.Insts.CoreCmpPartialEqOption.eq core.cmp.PartialEqU64 a b =
      Result.ok (decide (a = b)) := by
  cases a <;> cases b <;>
    simp [noble_kernel.core.option.Option.Insts.CoreCmpPartialEqOption.eq,
      core.cmp.impls.PartialEqU64.eq]

@[simp] theorem scopeCheck_refines (record : Snapshot) (serial : U64) :
    resources.transition.require_scope record serial = Result.ok (scopeCheck record serial) := by
  cases scope : record.last_scope <;>
    simp [resources.transition.require_scope, scopeCheck, scope]
  all_goals split_ifs <;> simp_all

@[simp] theorem retire_refines (record : Snapshot) (reason : Retirement) :
    resources.transition.retire record reason = Result.ok (retire record reason) := by
  cases state : record.state <;> simp [resources.transition.retire, retire, unchanged, state]

@[simp] theorem liveCheck_refines (record : Snapshot) (required : Requirement) :
    resources.transition.require_live record required = Result.ok (liveCheck record required) := by
  simp [resources.transition.require_live, liveCheck, lift]
  split_ifs <;> try simp_all
  all_goals cases state : record.state <;> simp_all

@[simp] theorem handleCheck_refines (retained claim : Handle) :
    resources.transition.validate_handle retained claim = Result.ok (handleCheck retained claim) := by
  simp [resources.transition.validate_handle, handleCheck]
  split_ifs <;> simp_all

@[simp] theorem valid_refines (record : Snapshot) :
    resources.transition.valid_record record = Result.ok (valid record) := by
  by_cases generation : record.handle.generation = 0#u64
  · simp [resources.transition.valid_record, valid, generation]
  cases scope : record.last_scope with
  | none =>
    cases state : record.state <;>
      simp [resources.transition.valid_record, valid, generation, scope, state,
        scopeCheck, core.result.Result.is_ok]
  | some previous =>
    by_cases zero : previous = 0#u64
    · simp [resources.transition.valid_record, valid, generation, scope, zero]
      split <;> simp_all
      all_goals simp_all only [UScalar.val, UScalarTy.U64_numBits_eq, BitVec.toNat_ofNat,
        Nat.zero_mod, not_true_eq_false]
    cases state : record.state <;>
      simp [resources.transition.valid_record, valid, generation, scope, zero, state,
        scopeCheck, core.result.Result.is_ok]
    all_goals split <;> simp_all
    all_goals simp_all only [UScalar.val, UScalarTy.U64_numBits_eq, BitVec.toNat_ofNat,
      Nat.zero_mod, not_true_eq_false]
    all_goals split_ifs <;> simp_all [UScalar.eq_equiv]
    all_goals tauto

@[simp] theorem inspect_refines (record : Snapshot) (required : Requirement) :
    resources.transition.inspect record required = Result.ok (inspect record required) := by
  simp [resources.transition.inspect, inspect]
  cases liveCheck record required <;> rfl

@[simp] theorem release_refines (record : Snapshot) (required : Requirement) :
    resources.transition.release record required = Result.ok (release record required) := by
  simp [resources.transition.release, release]
  cases liveCheck record required <;> rfl

@[simp] theorem transfer_refines (record : Snapshot) (required : Requirement)
    (receiver : Context) (generation : U64) :
    resources.transition.transfer record required receiver generation =
      Result.ok (transfer record required receiver generation) := by
  simp [resources.transition.transfer, transfer]
  cases liveCheck record required <;> simp [afterCheck]
  split <;> rfl

@[simp] theorem access_refines (record : Snapshot) (serial : U64) :
    resources.transition.access record serial = Result.ok (access record serial) := by
  simp [resources.transition.access, access]
  cases scopeCheck record serial <;> simp [afterCheck]
  cases state : record.state <;> simp [unchanged]

@[simp] theorem complete_refines (record : Snapshot) (serial : U64) (outcome : Completion) :
    resources.transition.complete record serial outcome = Result.ok (complete record serial outcome) := by
  simp [resources.transition.complete, complete]
  cases scopeCheck record serial <;> simp [afterCheck]
  cases state : record.state <;> simp [unchanged]

@[simp] theorem revoke_refines (record : Snapshot) (serial : U64) (reason : Retirement) :
    resources.transition.revoke record serial reason = Result.ok (revoke record serial reason) := by
  simp [resources.transition.revoke, revoke]
  cases scopeCheck record serial <;> simp [afterCheck]
  cases state : record.state <;> simp [unchanged]

@[simp] theorem begin_refines (record : Snapshot) (required : Requirement) (serial : U64) :
    resources.transition.begin record required serial = Result.ok (begin record required serial) := by
  simp [resources.transition.begin, begin]
  cases liveCheck record required <;> simp [afterCheck]
  split <;> simp_all
  cases scope : record.last_scope <;> simp
  split <;> simp_all

theorem transition_refines (record : Snapshot) (claim : Handle) (event : Event) :
    resources.transition.transition record claim event = Result.ok (step record claim event) := by
  simp [resources.transition.transition, step]
  split_ifs <;> try simp_all
  cases handleCheck record.handle claim <;> simp [afterCheck]
  cases event <;> simp [applyEvent]

def tableStep (table : resources.table.Table) (claim : Handle) (event : Event) : Checked Decision :=
  match table.records.val[claim.slot.val]? with
  | none => .Err .InvalidHandle
  | some retained => step retained claim event

theorem table_decide_refines (table : resources.table.Table) (claim : Handle) (event : Event) :
    resources.table.Table.decide table claim event = Result.ok (tableStep table claim event) := by
  simp [resources.table.Table.decide, tableStep, core.slice.Slice.get,
    alloc.vec.Vec.deref]
  cases table.records.val[claim.slot.val]? <;> simp [transition_refines]

@[simp] theorem handleCheck_self (handle : Handle) : handleCheck handle handle = .Ok () := by
  simp [handleCheck]

/-- Cancellation revokes access without returning an owner or releasing the native pin.
    Physical release remains a shell obligation; this theorem concerns the Rust decision. -/
theorem cancelled_pin_retained (record : Snapshot) (serial : U64) (reason : Retirement)
    (wellFormed : valid record = true) (busy : record.state = .Busy serial)
    (authentic : scopeCheck record serial = .Ok ()) :
    resources.transition.transition record record.handle (.Revoke serial reason) =
      Result.ok (.Ok ⟨{ record with state := .Retiring serial, retirement := some reason }, .AccessRevoked⟩) ∧
    resources.Decision.accounting
      ⟨{ record with state := .Retiring serial, retirement := some reason }, .AccessRevoked⟩ =
      Result.ok ⟨0#usize, 0#usize, 0#usize, 0#usize, 0#usize⟩ := by
  constructor
  · simp [transition_refines, step, wellFormed, afterCheck, applyEvent, revoke, authentic, retire, busy]
  · rfl

/-- After revocation, the sole authentic completion performs one release, never an owner return. -/
theorem late_completion_once (record : Snapshot) (serial : U64) (outcome : Completion)
    (wellFormed : valid record = true) (retiring : record.state = .Retiring serial)
    (authentic : scopeCheck record serial = .Ok ()) :
    resources.transition.transition record record.handle (.Complete serial outcome) =
      Result.ok (.Ok ⟨{ record with state := .Retired }, .RetirementCompleted⟩) ∧
    resources.Decision.accounting
      ⟨{ record with state := .Retired }, .RetirementCompleted⟩ =
      Result.ok ⟨0#usize, 1#usize, 0#usize, 0#usize, 1#usize⟩ := by
  constructor
  · simp [transition_refines, step, wellFormed, afterCheck, applyEvent, complete, authentic, retiring]
  · rfl

theorem retired_completion_idempotent (record : Snapshot) (serial : U64) (outcome : Completion)
    (wellFormed : valid record = true) (retired : record.state = .Retired)
    (authentic : scopeCheck record serial = .Ok ()) :
    resources.transition.transition record record.handle (.Complete serial outcome) =
      Result.ok (.Ok (unchanged record)) ∧
    resources.Decision.accounting (unchanged record) =
      Result.ok ⟨0#usize, 0#usize, 0#usize, 0#usize, 0#usize⟩ := by
  constructor
  · simp [transition_refines, step, wellFormed, afterCheck, applyEvent, complete, authentic, retired]
  · rfl

/-- Both normal result alternatives return the same owning handle and release exactly its pin. -/
theorem normal_completion_owner_once (record : Snapshot) (serial : U64) (outcome : Completion)
    (wellFormed : valid record = true) (busy : record.state = .Busy serial)
    (authentic : scopeCheck record serial = .Ok ()) :
    resources.transition.transition record record.handle (.Complete serial outcome) =
      Result.ok (.Ok ⟨{ record with state := .Live }, .OwnerReturned outcome⟩) ∧
    resources.Decision.accounting
      ⟨{ record with state := .Live }, .OwnerReturned outcome⟩ =
      Result.ok ⟨0#usize, 1#usize, 1#usize, 0#usize, 0#usize⟩ := by
  constructor
  · simp [transition_refines, step, wellFormed, afterCheck, applyEvent, complete, authentic, busy]
  · rfl

/-- Even a perfectly bound claim cannot inspect, release, borrow, or transfer a busy owner. -/
theorem busy_owner_not_reusable (record : Snapshot) (serial next : U64)
    (receiver : Context) (generation : U64)
    (wellFormed : valid record = true) (busy : record.state = .Busy serial) :
    let required : Requirement := ⟨record.handle.context, record.handle.kind, record.handle.rights⟩
    resources.transition.transition record record.handle (.Inspect required) = Result.ok (.Err .Busy) ∧
    resources.transition.transition record record.handle (.Release required) = Result.ok (.Err .Busy) ∧
    resources.transition.transition record record.handle (.Begin required next) = Result.ok (.Err .Busy) ∧
    resources.transition.transition record record.handle (.Transfer required receiver generation) = Result.ok (.Err .Busy) := by
  simp [transition_refines, step, wellFormed, afterCheck, applyEvent, inspect, release, begin,
    transfer, liveCheck, busy]

end M5Resources
