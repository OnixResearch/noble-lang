import M5Resources
import HostLocality
import RegionTrace

open Aeneas Aeneas.Std Result
open noble_kernel

set_option maxHeartbeats 2000000
set_option maxRecDepth 4096

namespace RegionCutProof

/-- The lane's earlier explicit width-sensitive total model for Rust's `i64 -> usize`
    conversion, retained only as the reference the checked-in whole-crate external
    model (`proofs/m3/NobleKernel/FunsExternal.lean`) is proved to agree with. -/
def laneConversionModel (value : Std.I64) :
    Result (core.result.Result Std.Usize core.num.error.TryFromIntError) :=
  if value.val < 0 then ok (.Err ())
  else match UScalar.tryMkOpt .Usize value.val.toNat with
    | some converted => ok (.Ok converted)
    | none => ok (.Err ())

/-- `Usize.max + 1` is exactly the width's power of two, so an in-range signed
    value lies below it on the extraction target. -/
theorem usize_max_succ_pow :
    (Usize.max : Int) + 1 = (2 : Int) ^ UScalarTy.Usize.numBits := by
  have h : Usize.max + 1 = 2 ^ UScalarTy.Usize.numBits := by
    rw [UScalarTy.Usize_numBits_eq]
    exact Usize.max_succ_eq_pow
  have cast : ((Usize.max + 1 : Nat) : Int) = ((2 ^ UScalarTy.Usize.numBits : Nat) : Int) := by
    rw [h]
  push_cast at cast ⊢
  exact cast

/-- A nonnegative `i64` inside the `usize` range keeps its value when cast. -/
theorem iscalar_hcast_usize_val (value : Std.I64) (nonnegative : 0 ≤ value.val)
    (upper : value.val ≤ (Usize.max : Int)) :
    (IScalar.hcast .Usize value).val = value.val.toNat := by
  rw [IScalar.hcast_val_eq]
  have bound : value.val < (2 : Int) ^ UScalarTy.Usize.numBits := by
    have := usize_max_succ_pow
    omega
  rw [Int.emod_eq_of_lt nonnegative bound]

/-- The checked-in whole-crate external model and the lane's explicit
    width-sensitive total model are the same function on every `i64`. -/
theorem conversion_models_agree (value : Std.I64) :
    Usize.Insts.CoreConvertTryFromI64TryFromIntError.try_from value =
      laneConversionModel value := by
  unfold Usize.Insts.CoreConvertTryFromI64TryFromIntError.try_from laneConversionModel
  have powCast : ((2 ^ UScalarTy.Usize.numBits : Nat) : Int) =
      (2 : Int) ^ UScalarTy.Usize.numBits := by
    push_cast
    rfl
  by_cases low : 0 ≤ value.val
  · by_cases high : value.val ≤ (Usize.max : Int)
    · have negative : ¬ value.val < 0 := by omega
      have bound : value.val < (2 : Int) ^ UScalarTy.Usize.numBits := by
        have := usize_max_succ_pow
        omega
      have fit : UScalar.inBounds .Usize value.val.toNat := by
        show value.val.toNat < 2 ^ UScalarTy.Usize.numBits
        exact (Int.toNat_lt low).mpr (by rw [powCast]; exact bound)
      rw [ite_eq_left ⟨low, high⟩, ite_eq_right negative]
      cases converted : UScalar.tryMkOpt .Usize value.val.toNat with
      | some result =>
        have shape := UScalar.tryMkOpt_eq .Usize value.val.toNat
        rw [converted] at shape
        have hcast : IScalar.hcast .Usize value = result :=
          UScalar.eq_of_val_eq (by
            rw [iscalar_hcast_usize_val value low high]
            exact shape.1.symm)
        rw [hcast]
      | none =>
        have shape := UScalar.tryMkOpt_eq .Usize value.val.toNat
        rw [converted] at shape
        exact absurd fit shape
    · have negative : ¬ value.val < 0 := by omega
      rw [ite_eq_right (by tauto), ite_eq_right negative]
      cases converted : UScalar.tryMkOpt .Usize value.val.toNat with
      | some result =>
        have shape := UScalar.tryMkOpt_eq .Usize value.val.toNat
        rw [converted] at shape
        have boundNat : value.val < ((2 ^ UScalarTy.Usize.numBits : Nat) : Int) :=
          (Int.toNat_lt low).mp shape.2
        have bound : value.val < (2 : Int) ^ UScalarTy.Usize.numBits := by
          rw [powCast] at boundNat
          exact boundNat
        have leMax : value.val ≤ (Usize.max : Int) := by
          have := usize_max_succ_pow
          omega
        exact absurd leMax high
      | none => rfl
  · have negative : value.val < 0 := by omega
    rw [ite_eq_right (by tauto), ite_eq_left negative]

/-- The checked-in external model is exact for signed-to-unsigned conversion on
    this pinned usize width. Its success has a nonnegative source. -/
theorem conversion_success_nonnegative (x : Std.I64) (converted : Std.Usize)
    (h : Usize.Insts.CoreConvertTryFromI64TryFromIntError.try_from x =
      Result.ok (.Ok converted)) :
    0 ≤ x.val ∧ converted.val = x.val.toNat := by
  rw [conversion_models_agree] at h
  unfold laneConversionModel at h
  split at h
  · simp at h
  · split at h
    · simp only [Result.ok.injEq, core.result.Result.Ok.injEq] at h
      subst converted
      constructor
      · omega
      · simp only [UScalar.tryMkOpt] at *
        split at *
        · simp only [Option.some.injEq] at *
          subst_vars
          rfl
        · simp_all
    · simp at h

/-- A refused signed input never receives the positive branch's premise. -/
theorem conversion_refuses_negative (x : Std.I64) (negative : x.val < 0) :
    Usize.Insts.CoreConvertTryFromI64TryFromIntError.try_from x =
      Result.ok (.Err ()) := by
  rw [conversion_models_agree]
  simp [laneConversionModel, negative]

/-- The same signed arguments as the host report determine this exact
    nonnegative range; no refused input is granted any of these premises. -/
def rangeBinding (request : region_read.Request) (range : region_read.Range)
    (regionSize : Std.Usize) : Prop :=
  0 ≤ request.offset_bytes.val ∧
    0 ≤ request.length_bytes.val ∧
    range.start.val = request.offset_bytes.val.toNat ∧
    range.finish.val = (request.offset_bytes.val + request.length_bytes.val).toNat ∧
    range.finish.val ≤ regionSize.val ∧
    request.length_bytes.val.toNat ≤ region_read.MAX_REGION_BYTES.val

theorem handle_eq_self (handle : resources.Handle) :
    resources.Handle.Insts.CoreCmpPartialEqHandle.eq handle handle =
      Result.ok true := by
  simp [resources.Handle.Insts.CoreCmpPartialEqHandle.eq,
    resources.TableId.Insts.CoreCmpPartialEqTableId.eq,
    resources.Context.Insts.CoreCmpPartialEqContext.eq,
    types.ResourceKind.Insts.CoreCmpPartialEqResourceKind.eq,
    resources.Rights.Insts.CoreCmpPartialEqRights.eq]

theorem handle_eq_sound (left right : resources.Handle)
    (matching : resources.Handle.Insts.CoreCmpPartialEqHandle.eq left right =
      Result.ok true) : left = right := by
  cases left
  cases right
  simp [resources.Handle.Insts.CoreCmpPartialEqHandle.eq,
    resources.TableId.Insts.CoreCmpPartialEqTableId.eq,
    resources.Context.Insts.CoreCmpPartialEqContext.eq,
    types.ResourceKind.Insts.CoreCmpPartialEqResourceKind.eq,
    resources.Rights.Insts.CoreCmpPartialEqRights.eq] at matching ⊢
  grind

theorem handle_eq_truth (left right : resources.Handle) :
    resources.Handle.Insts.CoreCmpPartialEqHandle.eq left right =
      Result.ok (decide (left = right)) := by
  cases left
  cases right
  simp [resources.Handle.Insts.CoreCmpPartialEqHandle.eq,
    resources.TableId.Insts.CoreCmpPartialEqTableId.eq,
    resources.Context.Insts.CoreCmpPartialEqContext.eq,
    types.ResourceKind.Insts.CoreCmpPartialEqResourceKind.eq,
    resources.Rights.Insts.CoreCmpPartialEqRights.eq]
  split_ifs <;> simp_all

@[simp] theorem usize_tryMkOpt_value (n : Nat) (value : Std.Usize)
    (made : UScalar.tryMkOpt .Usize n = some value) : value.val = n := by
  unfold UScalar.tryMkOpt at made
  split at made
  · cases made
    rfl
  · simp at made

theorem successful_preflight_binds_signed_range
    (table : resources.table.Table) (required : resources.Requirement)
    (request : region_read.Request) (regionSize : Std.Usize)
    (range : region_read.Range)
    (snapshot : resources.Snapshot)
    (validated : resources.table.Table.validate table request.claim required =
      Result.ok (.Ok snapshot))
    (retained : request.retained = some request.claim)
    (accepted : region_read.preflight table required request regionSize =
      Result.ok (.Ok range)) :
    rangeBinding request range regionSize := by
  unfold region_read.preflight at accepted
  unfold rangeBinding
  split at accepted <;> try simp_all
  simp only [core.result.Result.is_err, bind_ok, pure] at accepted
  simp [core.cmp.PartialEq.ne.trait_default,
    core.cmp.PartialEq.ne.default, handle_eq_self, bind_ok, lift] at accepted
  cases addition : request.offset_bytes.checked_add request.length_bytes with
  | none =>
    rw [addition] at accepted
    simp at accepted
  | some total =>
    rw [addition] at accepted
    have exactAddition := I64.checked_add_bv_spec request.offset_bytes request.length_bytes
    change core.num.checked_add_IScalar request.offset_bytes request.length_bytes = some total at addition
    rw [addition] at exactAddition
    simp only [conversion_models_agree, laneConversionModel] at accepted
    repeat (split at accepted <;> simp_all)
    grind [usize_tryMkOpt_value]

/-- The admitted signed arguments determine the *existing RegionTrace* read
    commit's start and byte count, rather than only a second local range model.
    The wrapper's later event emission remains a separate shell observation. -/
theorem successful_preflight_matches_region_trace_commit
    (table : resources.table.Table) (required : resources.Requirement)
    (request : region_read.Request) (regionSize : Std.Usize)
    (range : region_read.Range) (snapshot : resources.Snapshot)
    (validated : resources.table.Table.validate table request.claim required =
      Result.ok (.Ok snapshot))
    (retained : request.retained = some request.claim)
    (accepted : region_read.preflight table required request regionSize =
      Result.ok (.Ok range)) :
    RegionTrace.CommitKind.regionRead range.start.val
      (range.finish.val - range.start.val) =
    RegionTrace.CommitKind.regionRead request.offset_bytes.val.toNat
      request.length_bytes.val.toNat := by
  rcases successful_preflight_binds_signed_range table required request
    regionSize range snapshot validated retained accepted with
    ⟨offsetNonnegative, lengthNonnegative, exactStart, exactFinish, _, _⟩
  have exactLength :
      range.finish.val - range.start.val =
        request.length_bytes.val.toNat := by
    omega
  rw [exactLength, exactStart]

/-- A failed native lookup cannot authorize a protected copy, even when
    externally supplied range and claim happen to look plausible. -/
theorem access_refusal_prevents_copy (failure : resources.Error)
    (claim : resources.Handle) (scope : resources.Scope)
    (range : region_read.Range) (size : Std.Usize) :
    region_read.authorize_copy (.Err failure) claim scope range size =
      Result.ok false := by
  rfl

/-- The native-access result is necessarily a successful snapshot whenever
    the actual extracted decision authorizes the protected copy. -/
theorem authorized_copy_has_native_access (access : core.result.Result
      resources.Snapshot resources.Error) (claim : resources.Handle)
    (scope : resources.Scope) (range : region_read.Range) (size : Std.Usize)
    (authorized : region_read.authorize_copy access claim scope range size =
      Result.ok true) :
    ∃ snapshot, access = .Ok snapshot := by
  cases access with
  | Ok snapshot => exact ⟨snapshot, rfl⟩
  | Err failure => simp [region_read.authorize_copy] at authorized

theorem state_eq_with_busy_total (state : resources.State) (serial : Std.U64) :
    ∃ answer, resources.State.Insts.CoreCmpPartialEqState.eq state
        (.Busy serial) = Result.ok answer := by
  cases state <;>
    simp [resources.State.Insts.CoreCmpPartialEqState.eq, lift,
      core.cmp.impls.PartialEqU64.eq]
  all_goals try tauto
  split_ifs <;> simp_all [UScalar.eq_equiv]
  all_goals tauto

theorem state_eq_busy_sound (state : resources.State) (serial : Std.U64)
    (matching : resources.State.Insts.CoreCmpPartialEqState.eq state
      (.Busy serial) = Result.ok true) :
    state = .Busy serial := by
  cases state with
  | Live =>
    simp [resources.State.Insts.CoreCmpPartialEqState.eq,
      resources.State.read_discriminant] at matching
  | Busy current =>
    simp [resources.State.Insts.CoreCmpPartialEqState.eq, lift,
      core.cmp.impls.PartialEqU64.eq, resources.State.read_discriminant] at matching
    grind [UScalar.eq_equiv]
  | Retiring current =>
    simp [resources.State.Insts.CoreCmpPartialEqState.eq,
      resources.State.read_discriminant] at matching
  | Retired =>
    simp [resources.State.Insts.CoreCmpPartialEqState.eq,
      resources.State.read_discriminant] at matching

/-- Every successful native-copy decision has the exact admitted owner in
    both the snapshot and borrow scope, the current busy scope, and an
    interval inside storage. -/
theorem authorized_copy_exact_identity_and_bounds
    (snapshot : resources.Snapshot) (claim : resources.Handle)
    (scope : resources.Scope) (range : region_read.Range)
    (regionSize : Std.Usize)
    (authorized : region_read.authorize_copy (.Ok snapshot) claim scope range
      regionSize = Result.ok true) :
    snapshot.handle = claim ∧ scope.handle = claim ∧
      snapshot.state = .Busy scope.serial ∧
      snapshot.last_scope = some scope.serial ∧
      range.start.val ≤ range.finish.val ∧ range.finish.val ≤ regionSize.val := by
  simp only [region_read.authorize_copy, handle_eq_truth, bind_ok] at authorized
  repeat (split at authorized <;> simp_all)
  all_goals obtain ⟨answer, stateCheck⟩ :=
    state_eq_with_busy_total snapshot.state scope.serial
  all_goals cases answer <;> simp [stateCheck, bind_ok] at authorized
  all_goals exact ⟨state_eq_busy_sound snapshot.state scope.serial stateCheck, authorized⟩

/-- At the borrow-completion boundary, the extracted table decision succeeds
    for a selected well-formed busy record under an authentic scope. This
    reuses the locality lemma over the actual extracted Table.decide body. -/
theorem admitted_completion_decision (table : resources.table.Table)
    (record : resources.Snapshot) (serial : Std.U64)
    (selected : table.records.val[record.handle.slot.val]? = some record)
    (wellFormed : M5Resources.valid record = true)
    (busy : record.state = .Busy serial)
    (authentic : M5Resources.scopeCheck record serial = .Ok ()) :
    resources.table.Table.decide table record.handle
        (.Complete serial .Success) =
      Result.ok (.Ok ⟨{ record with state := .Live },
        .OwnerReturned .Success⟩) :=
  (HostLocality.m5_table_complete_exact table record serial .Success
    selected wellFormed busy authentic).1

/-- A selected busy record whose native pin is still held cannot fail the
    mutable table commit for OwnerReturned. The record's presence is the
    selected-record premise from the actual retained table. -/
theorem owner_return_commit_succeeds (table : resources.table.Table)
    (record : resources.Snapshot)
    (selected : table.records.val[record.handle.slot.val]? = some record)
    (pinned : 0 < table.pins.val) :
    ∃ updated,
      resources.table.Table.commit table
          ⟨{ record with state := .Live }, .OwnerReturned .Success⟩ =
        Result.ok (.Ok (), updated) := by
  have hasPin : ∃ remaining, Usize.checked_sub table.pins 1#usize = some remaining := by
    cases sub : Usize.checked_sub table.pins 1#usize with
    | some remaining => exact ⟨remaining, rfl⟩
    | none =>
      have underflow := Usize.checked_sub_bv_spec table.pins 1#usize
      simp only [sub] at underflow
      simp at underflow
      omega
  obtain ⟨remaining, sub⟩ := hasPin
  have selectedSlice :
      table.records.slice[record.handle.slot.val]? = some record := by
    simpa only [alloc.vec.Vec.val, Slice.getElem?_Nat_eq] using selected
  simp only [Slice.getElem?_Nat_eq] at selectedSlice
  simp [resources.table.Table.commit, sub, lift, alloc.vec.Vec.deref_mut,
    core.slice.Slice.get_mut, core.slice.index.Usize.get_mut, selectedSlice]

/-- Once the real extracted Table.complete reaches an admitted record with
    its pin held, it returns exactly the retained owner. No report commit
    is needed to make this true. -/
theorem admitted_complete_restores_owner (table : resources.table.Table)
    (record : resources.Snapshot) (serial : Std.U64)
    (selected : table.records.val[record.handle.slot.val]? = some record)
    (wellFormed : M5Resources.valid record = true)
    (busy : record.state = .Busy serial)
    (authentic : M5Resources.scopeCheck record serial = .Ok ())
    (pinned : 0 < table.pins.val) :
    ∃ updated,
      resources.table.access.Table.complete table
          ⟨record.handle, serial⟩ .Success =
        Result.ok (.Ok ⟨some ⟨record.handle⟩,
          ⟨{ record with state := .Live }, .OwnerReturned .Success⟩⟩, updated) := by
  have decision := admitted_completion_decision table record serial
    selected wellFormed busy authentic
  obtain ⟨updated, committed⟩ :=
    owner_return_commit_succeeds table record selected pinned
  exact ⟨updated, by simp [resources.table.access.Table.complete, decision, committed]⟩

end RegionCutProof
