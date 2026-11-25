import NobleKernel

/-!
# OWN-PO-6 Extracted-Ledger Refinement (`noble_kernel.ledger`)

This module proves the scoped `OWN-PO-6` host-ledger refinement directly over the
Charon/Aeneas-extracted `noble_kernel.ledger` types and transitions in
`proofs/m3/NobleKernel/{Types,Funs}.lean` (from `crates/noble-kernel/src/ledger/`):

- **Layer A (Loop specifications)**: Exact functional equations and termination (`Result.ok`)
  for `validate_frame_stack`, `copy_frame_prefix`, and `reverse_frames` via
  `Aeneas.Std.WP.loop.spec_decr_nat` and `specReturns`.
- **Layer B (`ValidSnapshot` / `ValidActiveRoot`)**: Well-formedness of active root snapshots,
  initialization via `Snapshot.pinned`, total exact characterization of `validate_active_root`,
  and explicit refusal lemmas for poisoned, zero, foreign, and inactive roots.
- **Property 1 (#9a)**: `enter_frame` / `leave_frame` LIFO frame stack invariant,
  `validate_frame_stack` preservation, `enter_leave_roundtrip`, and explicit refusal lemmas.
- **Property 2 (#9b)**: `plan_finish_root` / `ack_finish_root` balanced release invariant and
  exact-vs-poisoned session disposition.
- **Property 3 (#9c)**: `plan_abort_root` / `ack_abort_root` exact release iff settled guest +
  empty frames + `!pending_dispatch` + clean pre-abort + `clear_stack = Cleared` +
  `retire_versions = Succeeded` + `sweep_retirement = Succeeded`, else fail-closed `Poisoned` /
  `TerminatedAfterGuestFailure`, plus `UnexpectedGuestResumption` and exact LIFO
  `released_frames.val = s.frames.val.reverse`.
- **Property 4 (#9d)**: Non-wrapping `entered` / `released` `U32` counters and inductive trace
  monotonicity (`released ≤ entered ≤ U32.max`).
- **Property 5 (#9e)**: Simulation correspondence to a Mathlib-free `PinState` / `PinStep`
  mirror of `proofs/foundation/NobleFoundation/OwnershipModel.lean` continuation `.pin` frames
  (innermost-first `s.frames.val.reverse`) and no-resurrection after retirement or poisoning.
-/

open Aeneas Aeneas.Std Result WP
open noble_kernel

set_option maxHeartbeats 1000000
set_option maxRecDepth 2048

namespace NobleM3.LedgerRefinement

/-- A `WP.spec` judgement forces a normal `Result.ok` return satisfying the postcondition. -/
theorem specReturns {α : Type _} {m : Result α} {P : α → Prop} (h : WP.spec m P) :
    ∃ y, m = Result.ok y ∧ P y := by
  cases m with
  | ret y => exact ⟨y, rfl, (WP.spec_ok y).1 h⟩
  | vis e k => exact ((WP.spec_vis e k).1 h).elim
  | div => exact (WP.spec_div.1 h).elim

/-! ## Layer A: Loop Specifications and Pure Stack Predicates -/

/-- Pure reference specification for `ledger.transition.validate_frame_stack`. -/
def specValidateFrames (prev : Nat) :
    List ledger.FrameToken → core.result.Result Unit ledger.Error
  | [] => .Ok ()
  | ft :: rest =>
    if ft.val = 0 then .Err .InvalidFrameToken
    else if ft.val ≤ prev then .Err .DuplicateOrUnorderedFrame
    else specValidateFrames ft.val rest

/-- Strictly increasing sequence of `FrameToken`s bounded strictly below by `prev`. -/
def StrictlyIncreasing (prev : Nat) : List ledger.FrameToken → Prop
  | [] => True
  | ft :: rest => prev < ft.val ∧ StrictlyIncreasing ft.val rest

theorem specValidateFrames_ok_iff (prev : Nat) (l : List ledger.FrameToken) :
    specValidateFrames prev l = .Ok () ↔ StrictlyIncreasing prev l := by
  induction l generalizing prev with
  | nil => simp [specValidateFrames, StrictlyIncreasing]
  | cons ft rest ih =>
    unfold specValidateFrames StrictlyIncreasing
    by_cases hz : ft.val = 0
    · simp [hz]
    · by_cases hle : ft.val ≤ prev
      · simp [hz, hle]
      · simp only [hz, ite_false, hle, ih ft.val]
        constructor
        · intro h; exact ⟨by omega, h⟩
        · rintro ⟨_, h⟩; exact h

theorem strictlyIncreasing_mono {p q : Nat} {l : List ledger.FrameToken}
    (hpq : p ≤ q) (h : StrictlyIncreasing q l) : StrictlyIncreasing p l := by
  cases l with
  | nil => trivial
  | cons ft rest => exact ⟨Nat.lt_of_le_of_lt hpq h.1, h.2⟩

theorem strictlyIncreasing_pos {prev : Nat} {l : List ledger.FrameToken}
    (h : StrictlyIncreasing prev l) : ∀ ft ∈ l, prev < ft.val := by
  induction l generalizing prev with
  | nil => intro _ hm; simp at hm
  | cons hd tl ih =>
    intro ft hm
    simp only [List.mem_cons] at hm
    rcases hm with rfl | hmem
    · exact h.1
    · exact Nat.lt_trans h.1 (ih h.2 ft hmem)

theorem strictlyIncreasing_nodup {prev : Nat} {l : List ledger.FrameToken}
    (h : StrictlyIncreasing prev l) : l.Nodup := by
  induction l generalizing prev with
  | nil => exact List.nodup_nil
  | cons hd tl ih =>
    rw [List.nodup_cons]
    refine ⟨?_, ih h.2⟩
    intro hmem
    have hlt := strictlyIncreasing_pos h.2 hd hmem
    omega

theorem strictlyIncreasing_take {prev : Nat} {l : List ledger.FrameToken}
    (h : StrictlyIncreasing prev l) (n : Nat) : StrictlyIncreasing prev (l.take n) := by
  induction l generalizing prev n with
  | nil => simp [StrictlyIncreasing]
  | cons hd tl ih =>
    cases n with
    | zero => simp [StrictlyIncreasing]
    | succ k =>
      simp only [List.take_succ_cons, StrictlyIncreasing]
      exact ⟨h.1, ih h.2 k⟩

theorem strictlyIncreasing_getLast_lt {prev : Nat} {l : List ledger.FrameToken}
    (h : StrictlyIncreasing prev l) {last : ledger.FrameToken}
    (hl : l.getLast? = some last) : prev < last.val := by
  exact strictlyIncreasing_pos h last (List.mem_of_getLast? hl)

theorem strictlyIncreasing_append_singleton {prev : Nat} {l : List ledger.FrameToken}
    {ft : ledger.FrameToken} (h : StrictlyIncreasing prev l)
    (hpos : prev < ft.val)
    (hlast : ∀ last, l.getLast? = some last → last.val < ft.val) :
    StrictlyIncreasing prev (l ++ [ft]) := by
  induction l generalizing prev with
  | nil => simpa [StrictlyIncreasing] using hpos
  | cons hd tl ih =>
    simp only [List.cons_append, StrictlyIncreasing]
    refine ⟨h.1, ih h.2 ?_ ?_⟩
    · cases tl with
      | nil => exact hlast hd (by simp)
      | cons x xs =>
        cases hgl : (x :: xs).getLast? with
        | none =>
          exfalso
          have hnone := List.getLast?_eq_none_iff.mp hgl
          cases hnone
        | some last =>
          exact Nat.lt_trans (strictlyIncreasing_getLast_lt h.2 hgl) (hlast last (by simpa using hgl))
    · intro last hl
      apply hlast
      cases tl with
      | nil => simp at hl
      | cons _ _ => simpa using hl

private theorem list_getLast?_eq_getElem {α : Type _} (l : List α) (i : Nat)
    (h : i < l.length) (hi : i = l.length - 1) :
    l.getLast? = some (l[i]'h) := by
  subst hi
  have hne : l ≠ [] := List.ne_nil_of_length_pos (Nat.lt_of_le_of_lt (Nat.zero_le _) h)
  rw [List.getLast?_eq_some_getLast hne, List.getLast_eq_getElem]

private theorem strictlyIncreasing_mem_le_getLast {prev : Nat} {l : List ledger.FrameToken}
    (h : StrictlyIncreasing prev l) {y last : ledger.FrameToken}
    (hy : y ∈ l) (hl : l.getLast? = some last) : y.val ≤ last.val := by
  induction l generalizing prev with
  | nil => simp at hy
  | cons hd tl ih =>
    cases tl with
    | nil =>
      simp only [List.mem_cons, List.not_mem_nil, or_false] at hy
      simp only [List.getLast?_singleton, Option.some.injEq] at hl
      subst hy; subst hl; exact Nat.le_refl _
    | cons hd2 tl2 =>
      rw [List.mem_cons] at hy
      have hl' : (hd2 :: tl2).getLast? = some last := by simpa using hl
      rcases hy with rfl | hmem
      · exact Nat.le_of_lt (strictlyIncreasing_getLast_lt h.2 hl')
      · exact ih h.2 hmem hl'

/-- `validate_frame_stack` always terminates cleanly and matches `specValidateFrames`. -/
theorem validate_frame_stack_eq (frames : Slice ledger.FrameToken) :
    ledger.transition.validate_frame_stack frames =
      Result.ok (specValidateFrames 0 frames.val) := by
  have hloop :
      ledger.transition.validate_frame_stack_loop frames 0#u32 0#usize ⦃ r =>
        r = specValidateFrames 0 frames.val ⦄ := by
    unfold ledger.transition.validate_frame_stack_loop
    apply loop.spec_decr_nat (fun st : U32 × Usize => frames.length - st.2.val)
      (fun st => st.2.val ≤ frames.length ∧
        specValidateFrames st.1.val (frames.val.drop st.2.val) =
          specValidateFrames 0 frames.val)
    · rintro ⟨prev, index⟩ ⟨hidx, hsame⟩
      dsimp only at hidx hsame ⊢
      unfold ledger.transition.validate_frame_stack_loop.body
      by_cases hlt : index.val < frames.length
      · have hlt' : index < Slice.len frames := by scalar_tac
        obtain ⟨ft, hget, hft⟩ := specReturns (Slice.index_usize_spec frames index hlt)
        have hdrop : frames.val.drop index.val = ft :: frames.val.drop (index.val + 1) := by
          rw [List.drop_eq_getElem_cons hlt, ← hft]
        rw [hdrop] at hsame
        simp only [hlt', ite_true, hget, bind_ok]
        by_cases hz : ft = 0#u32
        · have hv : ft.val = 0 := by rw [hz]; rfl
          simp only [specValidateFrames, hv, ite_true] at hsame
          simp only [hz, ite_true, WP.spec_ok]
          exact hsame
        · have hv : ft.val ≠ 0 := fun h => hz ((UScalar.eq_equiv _ _).mpr (by simpa using h))
          simp only [specValidateFrames, hv, ite_false] at hsame
          simp only [hz, ite_false]
          by_cases hle : ft ≤ prev
          · have hvle : ft.val ≤ prev.val := by scalar_tac
            simp only [hvle, ite_true] at hsame
            simp only [hle, ite_true, WP.spec_ok]
            exact hsame
          · have hvgt : ¬ ft.val ≤ prev.val := by scalar_tac
            simp only [hvgt, ite_false] at hsame
            simp only [hle, ite_false]
            obtain ⟨index1, hadd, hval⟩ :=
              specReturns (UScalar.add_spec (x := index) (y := 1#usize) (by scalar_tac))
            simp only [hadd, bind_ok, WP.spec_ok]
            refine ⟨⟨by scalar_tac, ?_⟩, by scalar_tac⟩
            rw [hval]
            exact hsame
      · have hge : ¬ index < Slice.len frames := by scalar_tac
        have hend : index.val = frames.length := by omega
        rw [hend, List.drop_length] at hsame
        simp only [specValidateFrames] at hsame
        simp only [hge, ite_false, WP.spec_ok]
        exact hsame
    · exact ⟨Nat.zero_le _, by simp⟩
  obtain ⟨r, hr, hpost⟩ := specReturns hloop
  unfold ledger.transition.validate_frame_stack
  rw [hr, hpost]

/-- `validate_frame_stack` accepts a frame slice iff its tokens are strictly increasing and positive. -/
theorem validate_frame_stack_ok_iff (frames : Slice ledger.FrameToken) :
    ledger.transition.validate_frame_stack frames =
      Result.ok (core.result.Result.Ok ()) ↔ StrictlyIncreasing 0 frames.val := by
  rw [validate_frame_stack_eq, Result.ok.injEq, specValidateFrames_ok_iff]

/-- `copy_frame_prefix` always terminates cleanly and returns `frames.val.take (min count.val frames.length)`. -/
theorem copy_frame_prefix_eq (frames : Slice ledger.FrameToken) (count : Std.Usize) :
    ∃ v, ledger.transition.copy_frame_prefix frames count = Result.ok v ∧
      v.val = frames.val.take (min count.val frames.length) := by
  let plen : Usize := if count < Slice.len frames then count else Slice.len frames
  have hplen : plen.val = min count.val frames.length := by
    dsimp only [plen]
    by_cases hlt : count < Slice.len frames
    · have hlt' : count.val < frames.length := by scalar_tac
      simp only [hlt, ite_true]
      omega
    · have hge : frames.length ≤ count.val := by scalar_tac
      simp only [hlt, ite_false, Slice.len_val]
      omega
  have hloop :
      ledger.transition.copy_frame_prefix_loop frames plen
        (alloc.vec.Vec.with_capacity ledger.FrameToken plen) 0#usize ⦃ v =>
        v.val = frames.val.take plen.val ⦄ := by
    unfold ledger.transition.copy_frame_prefix_loop
    apply loop.spec_decr_nat (fun st : alloc.vec.Vec ledger.FrameToken × Usize => plen.val - st.2.val)
      (fun st => st.2.val ≤ plen.val ∧ st.1.val = frames.val.take st.2.val)
    · rintro ⟨acc, index⟩ ⟨hle, hacc⟩
      dsimp only at hle hacc ⊢
      unfold ledger.transition.copy_frame_prefix_loop.body
      by_cases hlt : index < plen
      · have hidx_plen : index.val < plen.val := by scalar_tac
        have hidx_lt : index.val < frames.length := by omega
        have hlen_lt : acc.val.length < Usize.max := by
          have hfp : frames.length ≤ Usize.max := by simp [Slice.length]
          have hidx_v : index.val < frames.val.length := by simpa [Slice.length] using hidx_lt
          have h1 : acc.val.length = index.val := by
            rw [hacc, List.length_take, Nat.min_eq_left (Nat.le_of_lt hidx_v)]
          omega
        obtain ⟨ft, hget, hft⟩ := specReturns (Slice.index_usize_spec frames index hidx_lt)
        obtain ⟨acc1, hpush, hacc1⟩ := specReturns (alloc.vec.Vec.push_spec acc ft hlen_lt)
        obtain ⟨index1, hadd, hidx1⟩ :=
          specReturns (UScalar.add_spec (x := index) (y := 1#usize) (by scalar_tac))
        have hidx1_val : index1.val = index.val + 1 := by
          have h1 : (1#usize).val = 1 := by simp
          omega
        simp only [hlt, ite_true, hget, bind_ok, hpush, hadd, WP.spec_ok]
        refine ⟨⟨by omega, ?_⟩, by omega⟩
        rw [hacc1, hacc, hidx1_val, List.take_add_one, List.getElem?_eq_getElem hidx_lt, ← hft]
        rfl
      · have hend : index.val = plen.val := by scalar_tac
        simp only [hlt, ite_false, WP.spec_ok]
        rw [hacc, hend]
    · exact ⟨Nat.zero_le _, by simp [alloc.vec.Vec.with_capacity]⟩
  obtain ⟨v, hv, hpost⟩ := specReturns hloop
  refine ⟨v, ?_, by rw [hpost, hplen]⟩
  unfold ledger.transition.copy_frame_prefix
  by_cases hlt : count < Slice.len frames
  · simp only [hlt, ite_true, bind_ok]
    have hplen_eq : plen = count := by simp [plen, hlt]
    simpa only [hplen_eq] using hv
  · simp only [hlt, ite_false, bind_ok]
    have hplen_eq : plen = Slice.len frames := by simp [plen, hlt]
    simpa only [hplen_eq] using hv

/-- `reverse_frames` always terminates cleanly and reverses the slice elements. -/
theorem reverse_frames_eq (frames : Slice ledger.FrameToken) :
    ∃ v, ledger.transition.reverse_frames frames = Result.ok v ∧
      v.val = frames.val.reverse := by
  have hloop :
      ledger.transition.reverse_frames_loop frames
        (alloc.vec.Vec.with_capacity ledger.FrameToken (Slice.len frames))
        (Slice.len frames) ⦃ v =>
        v.val = frames.val.reverse ⦄ := by
    unfold ledger.transition.reverse_frames_loop
    apply loop.spec_decr_nat (fun st : alloc.vec.Vec ledger.FrameToken × Usize => st.2.val)
      (fun st => st.2.val ≤ frames.length ∧ st.1.val = (frames.val.drop st.2.val).reverse)
    · rintro ⟨acc, rem⟩ ⟨hle, hacc⟩
      dsimp only at hle hacc ⊢
      unfold ledger.transition.reverse_frames_loop.body
      have hsub := Usize.checked_sub_bv_spec rem 1#usize
      cases hopt : Usize.checked_sub rem 1#usize with
      | none =>
        rw [hopt] at hsub
        have hrem0 : rem.val = 0 := by scalar_tac
        simp only [lift, bind_ok, WP.spec_ok]
        rw [hacc, hrem0, List.drop_zero]
      | some next =>
        rw [hopt] at hsub
        have hnext_lt : next.val < frames.length := by scalar_tac
        have hrem_eq : rem.val = next.val + 1 := by scalar_tac
        have hacc_len : acc.val.length < Usize.max := by
          have hfp : frames.length ≤ Usize.max := by
            simp [Slice.length]
          have h1 : acc.val.length = frames.length - rem.val := by
            rw [hacc, List.length_reverse, List.length_drop]
          omega
        obtain ⟨ft, hget, hft⟩ := specReturns (Slice.index_usize_spec frames next hnext_lt)
        obtain ⟨acc1, hpush, hacc1⟩ := specReturns (alloc.vec.Vec.push_spec acc ft hacc_len)
        simp only [lift, bind_ok, hget, hpush, WP.spec_ok]
        refine ⟨⟨by omega, ?_⟩, by omega⟩
        have hdrop : frames.val.drop next.val = ft :: frames.val.drop rem.val := by
          rw [hrem_eq, List.drop_eq_getElem_cons hnext_lt, ← hft]
        rw [hacc1, hacc, hdrop, List.reverse_cons]
    · refine ⟨by scalar_tac, ?_⟩
      simp [alloc.vec.Vec.with_capacity]
  obtain ⟨v, hv, hpost⟩ := specReturns hloop
  refine ⟨v, ?_, hpost⟩
  unfold ledger.transition.reverse_frames
  exact hv

/-! ## Layer B: Ledger Snapshot Invariant and `validate_active_root` -/

/-- Well-formedness invariant of an active, healthy ledger snapshot. -/
structure ValidSnapshot (s : ledger.Snapshot) : Prop where
  healthy : s.session = .Healthy
  root_nz : s.root ≠ 0#u64
  active : s.status = .Active
  released_le : s.released.val ≤ s.entered.val
  count_eq : s.entered.val - s.released.val = s.frames.val.length
  bounded : s.frames.val.length ≤ ledger.MAX_ACTIVE_FRAMES.val
  frames_strict : StrictlyIncreasing 0 s.frames.val

/-- Active root snapshot matching a claimed root token. -/
def ValidActiveRoot (s : ledger.Snapshot) (claim : ledger.RootToken) : Prop :=
  ValidSnapshot s ∧ claim = s.root

private theorem u32_one_val : (1#u32).val = 1 := by simp
private theorem usize_one_val : (1#usize).val = 1 := by simp
private theorem max_active_frames_val : ledger.MAX_ACTIVE_FRAMES.val = 14592 := by
  unfold ledger.MAX_ACTIVE_FRAMES; simp

theorem validSnapshot_active_le_u32 (s : ledger.Snapshot) (h : ValidSnapshot s) :
    s.frames.val.length ≤ U32.max := by
  have hb := h.bounded
  have hm := max_active_frames_val
  have hu := U32.max_eq
  omega

theorem validSnapshot_entered_le_u32 (s : ledger.Snapshot) : s.entered.val ≤ U32.max := by
  scalar_tac

theorem validSnapshot_released_lt_entered (s : ledger.Snapshot) (h : ValidSnapshot s)
    (hne : s.frames.val ≠ []) : s.released.val < s.entered.val := by
  have hlen : 0 < s.frames.val.length := List.length_pos_iff.mpr hne
  have := h.released_le
  have := h.count_eq
  omega

theorem validSnapshot_released_lt_u32 (s : ledger.Snapshot) (h : ValidSnapshot s)
    (hne : s.frames.val ≠ []) : s.released.val < U32.max := by
  have hlt := validSnapshot_released_lt_entered s h hne
  have hle : s.entered.val ≤ U32.max := by scalar_tac
  omega

/-- `Snapshot.pinned` initializes a valid empty active root when `root ≠ 0`. -/
theorem Snapshot.pinned_valid (root : ledger.RootToken) (epoch : ledger.MapEpoch)
    (hnz : root ≠ 0#u64) :
    ∃ s, ledger.Snapshot.pinned root epoch = Result.ok (core.result.Result.Ok s) ∧
      ValidSnapshot s ∧ s.root = root ∧ s.map_epoch = epoch ∧
      s.frames.val = [] ∧ s.entered = 0#u32 ∧ s.released = 0#u32 ∧
      s.pending_dispatch = false := by
  unfold ledger.Snapshot.pinned
  simp only [hnz, ite_false]
  refine ⟨_, rfl, ⟨rfl, hnz, rfl, by simp, by simp, by simp, by simp [StrictlyIncreasing]⟩,
    rfl, rfl, by simp, rfl, rfl, rfl⟩

/-- `Snapshot.pinned` refuses a zero root token. -/
theorem Snapshot.pinned_invalid_zero (epoch : ledger.MapEpoch) :
    ledger.Snapshot.pinned 0#u64 epoch =
      Result.ok (core.result.Result.Err ledger.Error.InvalidRootToken) := by
  simp [ledger.Snapshot.pinned]

/-- `validate_active_root` always terminates cleanly (`Result.ok`). -/
theorem validate_active_root_total (s : ledger.Snapshot) (claim : ledger.RootToken) :
    ∃ r, ledger.transition.validate_active_root s claim = Result.ok r := by
  unfold ledger.transition.validate_active_root
  cases hs : s.session with
  | Poisoned => simp
  | Healthy =>
    simp only [bind_ok]
    by_cases hrz : s.root = 0#u64
    · simp [hrz]
    · by_cases hcz : claim = 0#u64
      · simp [hrz, hcz]
      · by_cases hne : s.root != claim
        · simp [hrz, hcz, hne]
        · simp only [hrz, ite_false, hcz, hne]
          cases hst : s.status with
          | Finished => simp
          | Aborted => simp
          | Active =>
            simp only [bind_ok, ite_true, lift]
            cases hsub : U32.checked_sub s.entered s.released with
            | none => simp
            | some value =>
              obtain ⟨r, hr, _⟩ := specReturns
                (core.convert.num.ptr_try_from_impls.TryFromU32Usize.try_from.step_spec
                  (alloc.vec.Vec.len s.frames))
              rw [hr]
              simp only [bind_ok]
              cases r with
              | Err e => exact ⟨_, rfl⟩
              | Ok value1 =>
                by_cases h1 : value1 > ledger.MAX_ACTIVE_FRAMES
                · simp [h1]
                · by_cases h2 : value > ledger.MAX_ACTIVE_FRAMES
                  · simp [h1, h2]
                  · by_cases h3 : value1 != value
                    · simp [h1, h2, h3]
                    · simp only [h1, ite_false, h2, h3, alloc.vec.Vec.as_slice, bind_ok]
                      rw [validate_frame_stack_eq]
                      simp only [bind_ok]
                      cases specValidateFrames 0 s.frames.slice.val <;> exact ⟨_, rfl⟩

/-- `validate_active_root` succeeds with `active` iff `ValidActiveRoot s claim` and
`active.val = s.frames.val.length`. -/
theorem validate_active_root_ok_iff (s : ledger.Snapshot) (claim : ledger.RootToken)
    (active : Std.U32) :
    ledger.transition.validate_active_root s claim =
      Result.ok (core.result.Result.Ok active) ↔
      ValidActiveRoot s claim ∧ active.val = s.frames.val.length := by
  constructor
  · intro h
    unfold ledger.transition.validate_active_root at h
    cases hs : s.session with
    | Poisoned => rw [hs] at h; simp at h
    | Healthy =>
      rw [hs] at h
      simp only [bind_ok, Bool.false_eq_true, ite_false] at h
      by_cases hrz : s.root = 0#u64
      · simp [hrz] at h
      · by_cases hcz : claim = 0#u64
        · simp [hrz, hcz] at h
        · by_cases hne : s.root != claim
          · simp [hrz, hcz, hne] at h
          · simp only [hrz, ite_false, hcz, hne, Bool.false_eq_true] at h
            cases hst : s.status with
            | Finished => rw [hst] at h; simp at h
            | Aborted => rw [hst] at h; simp at h
            | Active =>
              rw [hst] at h
              simp only [bind_ok, ite_true, lift] at h
              have hsub_spec := U32.checked_sub_bv_spec s.entered s.released
              cases hsub : U32.checked_sub s.entered s.released with
              | none => rw [hsub] at h; simp at h
              | some value =>
                rw [hsub] at h hsub_spec
                obtain ⟨r, hr, htry⟩ := specReturns
                  (core.convert.num.ptr_try_from_impls.TryFromU32Usize.try_from.step_spec
                    (alloc.vec.Vec.len s.frames))
                rw [hr] at h
                simp only [bind_ok] at h
                cases r with
                | Err e => simp at h
                | Ok value1 =>
                  dsimp only at h
                  by_cases h1 : value1 > ledger.MAX_ACTIVE_FRAMES
                  · simp [h1] at h
                  · by_cases h2 : value > ledger.MAX_ACTIVE_FRAMES
                    · simp [h1, h2] at h
                    · by_cases h3 : value1 != value
                      · simp [h1, h2, h3] at h
                      · simp only [h1, ite_false, h2, h3, Bool.false_eq_true,
                          alloc.vec.Vec.as_slice, bind_ok] at h
                        rw [validate_frame_stack_eq] at h
                        simp only [bind_ok] at h
                        cases hstk : specValidateFrames 0 s.frames.slice.val with
                        | Err failure => rw [hstk] at h; simp at h
                        | Ok u =>
                          rw [hstk] at h
                          simp only [Result.ok.injEq, core.result.Result.Ok.injEq] at h
                          subst h
                          have hle_u32 : (alloc.vec.Vec.len s.frames).val ≤ U32.max := by
                            by_contra hgt
                            have hgt' : U32.max < (alloc.vec.Vec.len s.frames).val := by omega
                            have hbad := htry.2 hgt'
                            cases hbad
                          obtain ⟨x1, hx1_eq, hx1_val⟩ := htry.1 hle_u32
                          have hval1 : value1.val = s.frames.val.length := by
                            cases hx1_eq
                            simpa [alloc.vec.Vec.len_val] using hx1_val
                          have hclaim : claim = s.root := by
                            have hveq : s.root.val = claim.val := by simpa using hne
                            exact ((UScalar.eq_equiv _ _).mpr hveq).symm
                          have hstrict : StrictlyIncreasing 0 s.frames.val :=
                            (specValidateFrames_ok_iff 0 s.frames.val).mp hstk
                          have hveq : value1.val = value.val := by simpa using h3
                          have hmax : value.val ≤ ledger.MAX_ACTIVE_FRAMES.val := by scalar_tac
                          refine ⟨⟨⟨hs, hrz, hst, hsub_spec.1, ?_, ?_, hstrict⟩, hclaim⟩, ?_⟩
                          · rw [← hsub_spec.2.1, ← hveq, hval1]
                          · rw [← hval1, hveq]; exact hmax
                          · rw [← hveq, hval1]
  · rintro ⟨⟨⟨hs, hrz, hst, hle, hcnt, hbound, hstrict⟩, hclaim⟩, hact⟩
    unfold ledger.transition.validate_active_root
    have hcz : claim ≠ 0#u64 := by rw [hclaim]; exact hrz
    have hne : (s.root != claim) = false := by
      simp [hclaim]
    rw [hs, hst]
    simp only [bind_ok, ite_false, hrz, hcz, hne, ite_true, lift]
    have hsub_spec := U32.checked_sub_bv_spec s.entered s.released
    cases hsub : U32.checked_sub s.entered s.released with
    | none =>
      rw [hsub] at hsub_spec
      dsimp only at hsub_spec
      omega
    | some value =>
      rw [hsub] at hsub_spec
      dsimp only at hsub_spec
      have hle_u32 : (alloc.vec.Vec.len s.frames).val ≤ U32.max := by
        rw [alloc.vec.Vec.len_val]
        exact validSnapshot_active_le_u32 s ⟨hs, hrz, hst, hle, hcnt, hbound, hstrict⟩
      obtain ⟨r, hr, htry⟩ := specReturns
        (core.convert.num.ptr_try_from_impls.TryFromU32Usize.try_from.step_spec
          (alloc.vec.Vec.len s.frames))
      obtain ⟨x1, hx1_eq, hx1_val⟩ := htry.1 hle_u32
      rw [hr, hx1_eq]
      simp only [bind_ok]
      have hx1_len : x1.val = s.frames.val.length := by
        simpa [alloc.vec.Vec.len_val] using hx1_val
      have hval_len : value.val = s.frames.val.length := by
        rw [hsub_spec.2.1, hcnt]
      have h1 : ¬ (x1 > ledger.MAX_ACTIVE_FRAMES) := by scalar_tac
      have h2 : ¬ (value > ledger.MAX_ACTIVE_FRAMES) := by scalar_tac
      have h3 : (x1 != value) = false := by
        have heq : x1 = value := (UScalar.eq_equiv _ _).mpr (by omega)
        simp [heq]
      simp only [h1, ite_false, h2, h3, Bool.false_eq_true, alloc.vec.Vec.as_slice, bind_ok]
      rw [validate_frame_stack_eq]
      have hsv : s.frames.slice.val = s.frames.val := rfl
      rw [hsv, (specValidateFrames_ok_iff 0 s.frames.val).mpr hstrict]
      simp only [bind_ok, Result.ok.injEq, core.result.Result.Ok.injEq]
      exact (UScalar.eq_equiv _ _).mpr (by omega)

/-- Convenience wrapper obtaining `active` whenever `ValidActiveRoot s claim`. -/
theorem validate_active_root_of_valid (s : ledger.Snapshot) (claim : ledger.RootToken)
    (hvalid : ValidActiveRoot s claim) :
    ∃ active, ledger.transition.validate_active_root s claim =
      Result.ok (core.result.Result.Ok active) ∧ active.val = s.frames.val.length := by
  have hle : s.frames.val.length ≤ U32.max := validSnapshot_active_le_u32 s hvalid.1
  have hu := U32.max_eq
  have hpow : 2 ^ UScalarTy.U32.numBits = 4294967296 := by decide
  let active : Std.U32 := U32.ofNatCore s.frames.val.length (by omega)
  refine ⟨active, (validate_active_root_ok_iff s claim active).mpr ⟨hvalid, by simp [active]⟩, by simp [active]⟩

/-- Refusal: poisoned sessions reject every active-root check with `SessionPoisoned`. -/
theorem validate_active_root_refuses_poisoned (s : ledger.Snapshot) (claim : ledger.RootToken)
    (hp : s.session = .Poisoned) :
    ledger.transition.validate_active_root s claim =
      Result.ok (core.result.Result.Err ledger.Error.SessionPoisoned) := by
  simp [ledger.transition.validate_active_root, hp]

/-- Refusal: healthy snapshot with zero root rejects with `InvalidRootToken`. -/
theorem validate_active_root_refuses_zero_root (s : ledger.Snapshot) (claim : ledger.RootToken)
    (hh : s.session = .Healthy) (hrz : s.root = 0#u64) :
    ledger.transition.validate_active_root s claim =
      Result.ok (core.result.Result.Err ledger.Error.InvalidRootToken) := by
  simp [ledger.transition.validate_active_root, hh, hrz]

/-- Refusal: healthy nonzero snapshot with zero claim rejects with `InvalidRootToken`. -/
theorem validate_active_root_refuses_zero_claim (s : ledger.Snapshot)
    (hh : s.session = .Healthy) (hrz : s.root ≠ 0#u64) :
    ledger.transition.validate_active_root s 0#u64 =
      Result.ok (core.result.Result.Err ledger.Error.InvalidRootToken) := by
  simp [ledger.transition.validate_active_root, hh, hrz]

/-- Refusal: mismatched nonzero claim rejects with `ForeignRoot`. -/
theorem validate_active_root_refuses_foreign (s : ledger.Snapshot) (claim : ledger.RootToken)
    (hh : s.session = .Healthy) (hrz : s.root ≠ 0#u64) (hcz : claim ≠ 0#u64)
    (hne : s.root ≠ claim) :
    ledger.transition.validate_active_root s claim =
      Result.ok (core.result.Result.Err ledger.Error.ForeignRoot) := by
  have hbne : (s.root != claim) = true := by simp [hne]
  simp [ledger.transition.validate_active_root, hh, hrz, hcz, hbne]

/-- Refusal: finished or aborted root rejects with `InactiveRoot`. -/
theorem validate_active_root_refuses_inactive (s : ledger.Snapshot) (claim : ledger.RootToken)
    (hh : s.session = .Healthy) (hrz : s.root ≠ 0#u64) (hclaim : claim = s.root)
    (hinact : s.status ≠ .Active) :
    ledger.transition.validate_active_root s claim =
      Result.ok (core.result.Result.Err ledger.Error.InactiveRoot) := by
  have hcz : claim ≠ 0#u64 := by rw [hclaim]; exact hrz
  have hbne : (s.root != claim) = false := by simp [hclaim]
  cases hst : s.status with
  | Active => exact (hinact hst).elim
  | Finished => simp [ledger.transition.validate_active_root, hh, hrz, hcz, hbne, hst]
  | Aborted => simp [ledger.transition.validate_active_root, hh, hrz, hcz, hbne, hst]

/-- `frame_pin_counts` reads exact `FramePinCounts` for any valid active root. -/
theorem frame_pin_counts_spec (s : ledger.Snapshot) (claim : ledger.RootToken)
    (hvalid : ValidActiveRoot s claim) :
    ∃ c, ledger.transition.frame_pin_counts s claim = Result.ok (core.result.Result.Ok c) ∧
      c.entered = s.entered ∧ c.released = s.released ∧
      c.active.val = s.frames.val.length ∧ c.released_on_abort = 0#u32 := by
  obtain ⟨active, hact, hlen⟩ := validate_active_root_of_valid s claim hvalid
  unfold ledger.transition.frame_pin_counts
  rw [hact]
  simp only [bind_ok]
  exact ⟨_, rfl, rfl, rfl, hlen, rfl⟩

/-! ## Property 1 (#9a): `enter_frame` / `leave_frame` LIFO Stack Invariant -/

/-- `enter_frame` pushes `ft` onto the back of `s.frames`, preserves `ValidSnapshot`, and increments `entered`. -/
theorem enter_frame_pushes_lifo (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (hvalid : ValidActiveRoot s claim)
    (hft : ft ≠ 0#u32)
    (hmono : ∀ last, s.frames.val.getLast? = some last → last.val < ft.val)
    (hcap : s.frames.val.length < ledger.MAX_ACTIVE_FRAMES.val)
    (hcnt : s.entered.val < U32.max) :
    ∃ tr, ledger.transition.enter_frame s claim ft = Result.ok (core.result.Result.Ok tr) ∧
      tr.next.frames.val = s.frames.val ++ [ft] ∧
      ValidSnapshot tr.next ∧
      tr.next.root = s.root ∧
      tr.next.map_epoch = s.map_epoch ∧
      tr.next.entered.val = s.entered.val + 1 ∧
      tr.next.released = s.released ∧
      tr.next.pending_dispatch = false ∧
      tr.counts.entered = tr.next.entered ∧
      tr.counts.released = s.released ∧
      tr.counts.active.val = s.frames.val.length + 1 ∧
      tr.counts.released_on_abort = 0#u32 := by
  obtain ⟨active, hact, hlen⟩ := validate_active_root_of_valid s claim hvalid
  have hncap : ¬ (active ≥ ledger.MAX_ACTIVE_FRAMES) := by scalar_tac
  have hadd1 := U32.checked_add_bv_spec s.entered 1#u32
  cases ho1 : U32.checked_add s.entered 1#u32 with
  | none =>
    rw [ho1] at hadd1
    dsimp only at hadd1
    have h1 := u32_one_val
    omega
  | some entered1 =>
    rw [ho1] at hadd1
    dsimp only at hadd1
    have hadd2 := U32.checked_add_bv_spec active 1#u32
    cases ho2 : U32.checked_add active 1#u32 with
    | none =>
      rw [ho2] at hadd2
      dsimp only at hadd2
      have hm := max_active_frames_val
      have hu := U32.max_eq
      have h1 := u32_one_val
      omega
    | some active1 =>
      rw [ho2] at hadd2
      dsimp only at hadd2
      obtain ⟨pref, hpref, hpref_val⟩ :=
        copy_frame_prefix_eq s.frames.slice (alloc.vec.Vec.len s.frames)
      have hpref_eq : pref.val = s.frames.val := by
        have hflen : (alloc.vec.Vec.len s.frames).val = s.frames.val.length := by simp
        have hsl : s.frames.slice.length = s.frames.val.length := rfl
        have hsv : s.frames.slice.val = s.frames.val := rfl
        rw [hpref_val, hflen, hsl, hsv, Nat.min_self, List.take_length]
      have hpush_lt : pref.val.length < Usize.max := by
        rw [hpref_eq]
        have hm := max_active_frames_val
        scalar_tac
      obtain ⟨next_frames, hpush, hnext_val⟩ :=
        specReturns (alloc.vec.Vec.push_spec pref ft hpush_lt)
      have hframes_eq : next_frames.val = s.frames.val ++ [ft] := by
        rw [hnext_val, hpref_eq]
      have hft_pos : 0 < ft.val := by scalar_tac
      have hstrict' : StrictlyIncreasing 0 next_frames.val := by
        rw [hframes_eq]
        exact strictlyIncreasing_append_singleton hvalid.1.frames_strict hft_pos hmono
      have hvalid' : ValidSnapshot
          { s with
            frames := next_frames,
            entered := entered1,
            pending_dispatch := false,
            status := ledger.RootStatus.Active,
            session := ledger.SessionHealth.Healthy } :=
        { healthy := rfl
          root_nz := hvalid.1.root_nz
          active := rfl
          released_le := by
            dsimp only
            have := hvalid.1.released_le
            have h1 := u32_one_val
            rw [hadd1.2.1, h1]
            omega
          count_eq := by
            dsimp only
            have hrl := hvalid.1.released_le
            have hce := hvalid.1.count_eq
            rw [hframes_eq, List.length_append, List.length_singleton, hadd1.2.1, u32_one_val]
            omega
          bounded := by
            dsimp only
            rw [hframes_eq, List.length_append, List.length_singleton]
            omega
          frames_strict := hstrict' }
      unfold ledger.transition.enter_frame
      rw [hact]
      simp only [hft, ite_false, lift]
      have hsub := Usize.checked_sub_bv_spec (alloc.vec.Vec.len s.frames) 1#usize
      cases ho : Usize.checked_sub (alloc.vec.Vec.len s.frames) 1#usize with
      | none =>
        rw [ho] at hsub
        dsimp only at hsub
        simp only [bind_ok, hncap, ite_false, ho1, ho2, alloc.vec.Vec.as_slice, hpref, hpush]
        refine ⟨_, rfl, hframes_eq, hvalid', rfl, rfl, ?_, rfl, rfl, rfl, rfl, ?_, rfl⟩
        · dsimp only; rw [hadd1.2.1, u32_one_val]
        · dsimp only; rw [hadd2.2.1, hlen, u32_one_val]
      | some last_idx =>
        rw [ho] at hsub
        dsimp only at hsub
        have hidx_lt : last_idx.val < s.frames.length := by scalar_tac
        obtain ⟨last_ft, hget_last, hlast_val⟩ :=
          specReturns (alloc.vec.Vec.index_usize_spec s.frames last_idx hidx_lt)
        have hlidx : last_idx.val = s.frames.val.length - 1 := by
          have h1 := usize_one_val
          have hflen : (alloc.vec.Vec.len s.frames).val = s.frames.val.length := by simp
          rw [hflen] at hsub
          omega
        have hlast_opt : s.frames.val.getLast? = some last_ft := by
          rw [hlast_val]
          exact list_getLast?_eq_getElem s.frames.val last_idx.val hidx_lt hlidx
        have hnotle : ¬ (ft ≤ last_ft) := by
          have := hmono last_ft hlast_opt
          scalar_tac
        simp only [bind_ok, alloc.vec.Vec.index_slice_index, hget_last, hnotle, ite_false,
          hncap, ho1, ho2, alloc.vec.Vec.as_slice, hpref, hpush]
        refine ⟨_, rfl, hframes_eq, hvalid', rfl, rfl, ?_, rfl, rfl, rfl, rfl, ?_, rfl⟩
        · dsimp only; rw [hadd1.2.1, u32_one_val]
        · dsimp only; rw [hadd2.2.1, hlen, u32_one_val]

/-- Refusal: `enter_frame` rejects zero frame tokens with `InvalidFrameToken`. -/
theorem enter_frame_refuses_invalid_token (s : ledger.Snapshot) (claim : ledger.RootToken)
    (hvalid : ValidActiveRoot s claim) :
    ledger.transition.enter_frame s claim 0#u32 =
      Result.ok (core.result.Result.Err ledger.Error.InvalidFrameToken) := by
  obtain ⟨active, hact, _⟩ := validate_active_root_of_valid s claim hvalid
  simp [ledger.transition.enter_frame, hact]

/-- Refusal: `enter_frame` rejects duplicate or non-increasing frame tokens with `DuplicateOrUnorderedFrame`. -/
theorem enter_frame_refuses_duplicate_or_unordered (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft last : ledger.FrameToken) (hvalid : ValidActiveRoot s claim) (hft : ft ≠ 0#u32)
    (hlast : s.frames.val.getLast? = some last) (hle : ft.val ≤ last.val) :
    ledger.transition.enter_frame s claim ft =
      Result.ok (core.result.Result.Err ledger.Error.DuplicateOrUnorderedFrame) := by
  obtain ⟨active, hact, _⟩ := validate_active_root_of_valid s claim hvalid
  have hne : s.frames.val ≠ [] := by
    intro he; simp [he] at hlast
  have hlen_pos : 0 < s.frames.val.length := List.length_pos_iff.mpr hne
  have hsub := Usize.checked_sub_bv_spec (alloc.vec.Vec.len s.frames) 1#usize
  cases ho : Usize.checked_sub (alloc.vec.Vec.len s.frames) 1#usize with
  | none =>
    rw [ho] at hsub
    dsimp only at hsub
    have hflen : (alloc.vec.Vec.len s.frames).val = s.frames.val.length := by simp
    have h1 := usize_one_val
    rw [hflen] at hsub
    omega
  | some last_idx =>
    rw [ho] at hsub
    dsimp only at hsub
    have hidx_lt : last_idx.val < s.frames.length := by scalar_tac
    obtain ⟨last_ft, hget_last, hlast_val⟩ :=
      specReturns (alloc.vec.Vec.index_usize_spec s.frames last_idx hidx_lt)
    have hlidx : last_idx.val = s.frames.val.length - 1 := by
      have h1 := usize_one_val
      have hflen : (alloc.vec.Vec.len s.frames).val = s.frames.val.length := by simp
      rw [hflen] at hsub
      omega
    have hlast_eq : last_ft = last := by
      have hopt := list_getLast?_eq_getElem s.frames.val last_idx.val hidx_lt hlidx
      rw [hlast, ← hlast_val, Option.some.injEq] at hopt
      exact hopt.symm
    have hft_le_val : ft.val ≤ last_ft.val := by rw [hlast_eq]; exact hle
    unfold ledger.transition.enter_frame
    rw [hact]
    simp [hft, lift, ho, hget_last, hft_le_val]

/-- Refusal: `enter_frame` rejects exceeding `MAX_ACTIVE_FRAMES` with `FrameBoundExceeded`. -/
theorem enter_frame_refuses_bound_exceeded (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (hvalid : ValidActiveRoot s claim) (hft : ft ≠ 0#u32)
    (hmono : ∀ last, s.frames.val.getLast? = some last → last.val < ft.val)
    (hcap : ledger.MAX_ACTIVE_FRAMES.val ≤ s.frames.val.length) :
    ledger.transition.enter_frame s claim ft =
      Result.ok (core.result.Result.Err ledger.Error.FrameBoundExceeded) := by
  obtain ⟨active, hact, hlen⟩ := validate_active_root_of_valid s claim hvalid
  have hge_val : ledger.MAX_ACTIVE_FRAMES.val ≤ active.val := by omega
  unfold ledger.transition.enter_frame
  rw [hact]
  simp only [hft, ite_false, lift]
  have hsub := Usize.checked_sub_bv_spec (alloc.vec.Vec.len s.frames) 1#usize
  cases ho : Usize.checked_sub (alloc.vec.Vec.len s.frames) 1#usize with
  | none =>
    simp [hge_val]
  | some last_idx =>
    rw [ho] at hsub
    dsimp only at hsub
    have hidx_lt : last_idx.val < s.frames.length := by scalar_tac
    obtain ⟨last_ft, hget_last, hlast_val⟩ :=
      specReturns (alloc.vec.Vec.index_usize_spec s.frames last_idx hidx_lt)
    have hlidx : last_idx.val = s.frames.val.length - 1 := by
      have h1 := usize_one_val
      have hflen : (alloc.vec.Vec.len s.frames).val = s.frames.val.length := by simp
      rw [hflen] at hsub
      omega
    have hlast_opt : s.frames.val.getLast? = some last_ft := by
      rw [hlast_val]
      exact list_getLast?_eq_getElem s.frames.val last_idx.val hidx_lt hlidx
    have hgt : last_ft.val < ft.val := hmono last_ft hlast_opt
    have hnotle_val : ¬ (ft.val ≤ last_ft.val) := by omega
    simp [hget_last, hnotle_val, hge_val]

/-- Refusal: `enter_frame` rejects `entered = U32.max` with `FrameCounterExhausted` (never wraps). -/
theorem enter_frame_refuses_counter_exhausted (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (hvalid : ValidActiveRoot s claim) (hft : ft ≠ 0#u32)
    (hmono : ∀ last, s.frames.val.getLast? = some last → last.val < ft.val)
    (hcap : s.frames.val.length < ledger.MAX_ACTIVE_FRAMES.val)
    (hmax : s.entered.val = U32.max) :
    ledger.transition.enter_frame s claim ft =
      Result.ok (core.result.Result.Err ledger.Error.FrameCounterExhausted) := by
  obtain ⟨active, hact, hlen⟩ := validate_active_root_of_valid s claim hvalid
  have hlt_cap : active.val < ledger.MAX_ACTIVE_FRAMES.val := by omega
  have hnotge_val : ¬ (ledger.MAX_ACTIVE_FRAMES.val ≤ active.val) := by omega
  have hadd1 := U32.checked_add_bv_spec s.entered 1#u32
  have ho1 : U32.checked_add s.entered 1#u32 = none := by
    cases h : U32.checked_add s.entered 1#u32 with
    | none => rfl
    | some _ =>
      rw [h] at hadd1
      dsimp only at hadd1
      have h1 := u32_one_val
      omega
  unfold ledger.transition.enter_frame
  rw [hact]
  simp only [hft, ite_false, lift]
  have hsub := Usize.checked_sub_bv_spec (alloc.vec.Vec.len s.frames) 1#usize
  cases ho : Usize.checked_sub (alloc.vec.Vec.len s.frames) 1#usize with
  | none =>
    simp [hnotge_val, ho1]
  | some last_idx =>
    rw [ho] at hsub
    dsimp only at hsub
    have hidx_lt : last_idx.val < s.frames.length := by scalar_tac
    obtain ⟨last_ft, hget_last, hlast_val⟩ :=
      specReturns (alloc.vec.Vec.index_usize_spec s.frames last_idx hidx_lt)
    have hlidx : last_idx.val = s.frames.val.length - 1 := by
      have h1 := usize_one_val
      have hflen : (alloc.vec.Vec.len s.frames).val = s.frames.val.length := by simp
      rw [hflen] at hsub
      omega
    have hlast_opt : s.frames.val.getLast? = some last_ft := by
      rw [hlast_val]
      exact list_getLast?_eq_getElem s.frames.val last_idx.val hidx_lt hlidx
    have hgt : last_ft.val < ft.val := hmono last_ft hlast_opt
    have hnotle_val : ¬ (ft.val ≤ last_ft.val) := by omega
    simp [hget_last, hnotle_val, hnotge_val, ho1]

/-- `leave_frame` pops the innermost frame `ft`, preserves `ValidSnapshot`, and increments `released`. -/
theorem leave_frame_pops_lifo (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (prefix_ : List ledger.FrameToken)
    (hvalid : ValidActiveRoot s claim) (hdisp : s.pending_dispatch = false)
    (hft : ft ≠ 0#u32) (hframes : s.frames.val = prefix_ ++ [ft]) :
    ∃ tr, ledger.transition.leave_frame s claim ft = Result.ok (core.result.Result.Ok tr) ∧
      tr.next.frames.val = prefix_ ∧
      ValidSnapshot tr.next ∧
      tr.next.root = s.root ∧
      tr.next.map_epoch = s.map_epoch ∧
      tr.next.entered = s.entered ∧
      tr.next.released.val = s.released.val + 1 ∧
      tr.next.pending_dispatch = false ∧
      tr.counts.entered = s.entered ∧
      tr.counts.released = tr.next.released ∧
      tr.counts.active.val = prefix_.length ∧
      tr.counts.released_on_abort = 0#u32 := by
  obtain ⟨active, hact, hlen⟩ := validate_active_root_of_valid s claim hvalid
  have hne : s.frames.val ≠ [] := by rw [hframes]; simp
  have hrel_lt := validSnapshot_released_lt_u32 s hvalid.1 hne
  have hact_pos : 0 < active.val := by
    rw [hlen, hframes, List.length_append, List.length_singleton]; omega
  have hflen : (alloc.vec.Vec.len s.frames).val = s.frames.val.length := by simp
  have hsub_idx := Usize.checked_sub_bv_spec (alloc.vec.Vec.len s.frames) 1#usize
  cases ho : Usize.checked_sub (alloc.vec.Vec.len s.frames) 1#usize with
  | none =>
    rw [ho] at hsub_idx
    dsimp only at hsub_idx
    rw [hflen, hframes, List.length_append, List.length_singleton] at hsub_idx
    have h1 := usize_one_val
    omega
  | some idx =>
    rw [ho] at hsub_idx
    dsimp only at hsub_idx
    have hidx_val : idx.val = prefix_.length := by
      rw [hflen, hframes, List.length_append, List.length_singleton] at hsub_idx
      have h1 := usize_one_val
      omega
    have hidx_lt : idx.val < s.frames.length := by
      rw [alloc.vec.Vec.length, hframes, List.length_append, List.length_singleton]
      omega
    obtain ⟨got_ft, hget, hgot_val⟩ :=
      specReturns (alloc.vec.Vec.index_usize_spec s.frames idx hidx_lt)
    have hgot_eq : got_ft = ft := by
      rw [hgot_val]
      simp [hframes, hidx_val]
    have hadd1 := U32.checked_add_bv_spec s.released 1#u32
    cases ho1 : U32.checked_add s.released 1#u32 with
    | none =>
      rw [ho1] at hadd1
      dsimp only at hadd1
      have h1 := u32_one_val
      omega
    | some rel1 =>
      rw [ho1] at hadd1
      dsimp only at hadd1
      have hsub2 := U32.checked_sub_bv_spec active 1#u32
      cases ho2 : U32.checked_sub active 1#u32 with
      | none =>
        rw [ho2] at hsub2
        dsimp only at hsub2
        have h1 := u32_one_val
        omega
      | some act1 =>
        rw [ho2] at hsub2
        dsimp only at hsub2
        obtain ⟨next_frames, hpref, hpref_val⟩ := copy_frame_prefix_eq s.frames.slice idx
        have hframes_eq : next_frames.val = prefix_ := by
          have hsl : s.frames.slice.length = s.frames.val.length := rfl
          have hsv : s.frames.slice.val = s.frames.val := rfl
          rw [hpref_val, hidx_val, hsl, hsv, hframes, List.length_append, List.length_singleton]
          have hmin : min prefix_.length (prefix_.length + 1) = prefix_.length := by omega
          rw [hmin, List.take_left]
        have hstrict' : StrictlyIncreasing 0 next_frames.val := by
          rw [hframes_eq]
          have htake := strictlyIncreasing_take hvalid.1.frames_strict prefix_.length
          rw [hframes, List.take_left] at htake
          exact htake
        have hvalid' : ValidSnapshot
            { s with
              frames := next_frames,
              released := rel1,
              pending_dispatch := false,
              status := ledger.RootStatus.Active,
              session := ledger.SessionHealth.Healthy } :=
          { healthy := rfl
            root_nz := hvalid.1.root_nz
            active := rfl
            released_le := by
              dsimp only
              have hlt := validSnapshot_released_lt_entered s hvalid.1 hne
              have h1 := u32_one_val
              rw [hadd1.2.1, h1]
              omega
            count_eq := by
              dsimp only
              have hlt := validSnapshot_released_lt_entered s hvalid.1 hne
              have hce := hvalid.1.count_eq
              have h1 := u32_one_val
              rw [hframes_eq, hadd1.2.1, h1]
              rw [hframes, List.length_append, List.length_singleton] at hce
              omega
            bounded := by
              dsimp only
              have := hvalid.1.bounded
              rw [hframes] at this
              rw [hframes_eq]
              simp only [List.length_append, List.length_singleton] at this
              omega
            frames_strict := hstrict' }
        have hbeq_false : (got_ft != ft) = false := by simp [hgot_eq]
        unfold ledger.transition.leave_frame
        rw [hact]
        simp only [hdisp, Bool.false_eq_true, ite_false, hft, lift, ho, bind_ok,
          alloc.vec.Vec.index_slice_index, hget, hbeq_false, ho1, ho2,
          alloc.vec.Vec.as_slice, hpref]
        refine ⟨_, rfl, hframes_eq, hvalid', rfl, rfl, rfl, ?_, rfl, rfl, rfl, ?_, rfl⟩
        · dsimp only; rw [hadd1.2.1, u32_one_val]
        · dsimp only
          rw [hsub2.2.1, hlen, hframes, List.length_append, List.length_singleton, u32_one_val]
          omega

/-- Refusal: `leave_frame` rejects `pending_dispatch = true` with `PendingDispatch`. -/
theorem leave_frame_refuses_pending_dispatch (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (hvalid : ValidActiveRoot s claim)
    (hdisp : s.pending_dispatch = true) :
    ledger.transition.leave_frame s claim ft =
      Result.ok (core.result.Result.Err ledger.Error.PendingDispatch) := by
  obtain ⟨active, hact, _⟩ := validate_active_root_of_valid s claim hvalid
  simp [ledger.transition.leave_frame, hact, hdisp]

/-- Refusal: `leave_frame` rejects zero frame tokens with `InvalidFrameToken`. -/
theorem leave_frame_refuses_invalid_token (s : ledger.Snapshot) (claim : ledger.RootToken)
    (hvalid : ValidActiveRoot s claim) (hdisp : s.pending_dispatch = false) :
    ledger.transition.leave_frame s claim 0#u32 =
      Result.ok (core.result.Result.Err ledger.Error.InvalidFrameToken) := by
  obtain ⟨active, hact, _⟩ := validate_active_root_of_valid s claim hvalid
  simp [ledger.transition.leave_frame, hact, hdisp]

/-- Refusal: `leave_frame` rejects empty or non-top frame tokens with `NonLifoFrameRelease`. -/
theorem leave_frame_refuses_non_lifo (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (hvalid : ValidActiveRoot s claim)
    (hdisp : s.pending_dispatch = false) (hft : ft ≠ 0#u32)
    (hnotlast : s.frames.val.getLast? ≠ some ft) :
    ledger.transition.leave_frame s claim ft =
      Result.ok (core.result.Result.Err ledger.Error.NonLifoFrameRelease) := by
  obtain ⟨active, hact, _⟩ := validate_active_root_of_valid s claim hvalid
  unfold ledger.transition.leave_frame
  rw [hact]
  simp only [hdisp, Bool.false_eq_true, ite_false, hft, lift]
  have hsub := Usize.checked_sub_bv_spec (alloc.vec.Vec.len s.frames) 1#usize
  cases ho : Usize.checked_sub (alloc.vec.Vec.len s.frames) 1#usize with
  | none => simp [bind_ok]
  | some idx =>
    rw [ho] at hsub
    dsimp only at hsub
    have hidx_lt : idx.val < s.frames.length := by scalar_tac
    obtain ⟨got_ft, hget, hgot_val⟩ :=
      specReturns (alloc.vec.Vec.index_usize_spec s.frames idx hidx_lt)
    have hlidx : idx.val = s.frames.val.length - 1 := by
      have h1 := usize_one_val
      have hflen : (alloc.vec.Vec.len s.frames).val = s.frames.val.length := by simp
      rw [hflen] at hsub
      omega
    have hne : got_ft ≠ ft := by
      intro heq
      apply hnotlast
      rw [← heq, hgot_val]
      exact list_getLast?_eq_getElem s.frames.val idx.val hidx_lt hlidx
    have hne_val : got_ft.val ≠ ft.val := fun h => hne ((UScalar.eq_equiv _ _).mpr (by simpa using h))
    simp [hget, hne_val]

/-- Entering `ft` then immediately leaving `ft` restores the exact frame stack. -/
theorem enter_leave_roundtrip (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (hvalid : ValidActiveRoot s claim) (hft : ft ≠ 0#u32)
    (hmono : ∀ last, s.frames.val.getLast? = some last → last.val < ft.val)
    (hcap : s.frames.val.length < ledger.MAX_ACTIVE_FRAMES.val)
    (hcnt : s.entered.val < U32.max) :
    ∃ tr1 tr2,
      ledger.transition.enter_frame s claim ft = Result.ok (core.result.Result.Ok tr1) ∧
      ledger.transition.leave_frame tr1.next claim ft = Result.ok (core.result.Result.Ok tr2) ∧
      tr2.next.frames.val = s.frames.val ∧
      tr2.next.entered.val = s.entered.val + 1 ∧
      tr2.next.released.val = s.released.val + 1 ∧
      ValidSnapshot tr2.next := by
  obtain ⟨tr1, h1, hframes1, hv1, hroot1, _, hent1, hrel1, hdisp1, _, _, _, _⟩ :=
    enter_frame_pushes_lifo s claim ft hvalid hft hmono hcap hcnt
  have hvalid1 : ValidActiveRoot tr1.next claim := ⟨hv1, by rw [hroot1]; exact hvalid.2⟩
  obtain ⟨tr2, h2, hframes2, hv2, _, _, hent2, hrel2, _, _, _, _, _⟩ :=
    leave_frame_pops_lifo tr1.next claim ft s.frames.val hvalid1 hdisp1 hft hframes1
  refine ⟨tr1, tr2, h1, h2, hframes2, ?_, ?_, hv2⟩
  · rw [hent2, hent1]
  · rw [hrel2, hrel1]

/-! ## Property 2 (#9b): `plan_finish_root` / `ack_finish_root` Balanced Release -/

/-- `plan_finish_root` succeeds iff the root is valid/active, all frames are released, and no dispatch is pending. -/
theorem plan_finish_root_iff (s : ledger.Snapshot) (claim : ledger.RootToken) :
    (∃ plan, ledger.transition.plan_finish_root s claim =
      Result.ok (core.result.Result.Ok plan)) ↔
      ValidActiveRoot s claim ∧ s.frames.val = [] ∧ s.pending_dispatch = false := by
  constructor
  · rintro ⟨plan, h⟩
    unfold ledger.transition.plan_finish_root at h
    obtain ⟨r, hval⟩ := validate_active_root_total s claim
    rw [hval] at h
    simp only [bind_ok] at h
    cases r with
    | Err e => simp at h
    | Ok active =>
      dsimp only at h
      by_cases hact0 : active = 0#u32
      · by_cases hdisp : s.pending_dispatch
        · simp [hact0, hdisp] at h
        · have hok := (validate_active_root_ok_iff s claim active).mp hval
          have hlen0 : s.frames.val.length = 0 := by
            rw [← hok.2, hact0]; rfl
          exact ⟨hok.1, List.length_eq_zero_iff.mp hlen0, Bool.eq_false_iff.mpr hdisp⟩
      · have hbne : (active != 0#u32) = true := by simp [hact0]
        simp [hbne] at h
  · rintro ⟨hvalid, hempty, hdisp⟩
    obtain ⟨active, hact, hlen⟩ := validate_active_root_of_valid s claim hvalid
    have hact0 : active = 0#u32 := by
      have : active.val = 0 := by rw [hlen, hempty]; rfl
      exact (UScalar.eq_equiv _ _).mpr (by simpa using this)
    unfold ledger.transition.plan_finish_root
    rw [hact]
    simp [hact0, hdisp]

/-- Every accepted `FinishPlan` satisfies balanced frame counters (`entered = released`, `active = 0`). -/
theorem plan_finish_root_balanced (s : ledger.Snapshot) (claim : ledger.RootToken)
    (plan : ledger.FinishPlan)
    (h : ledger.transition.plan_finish_root s claim = Result.ok (core.result.Result.Ok plan)) :
    ValidActiveRoot s claim ∧
      plan.root = s.root ∧
      plan.map_epoch = s.map_epoch ∧
      plan.counts.entered = s.entered ∧
      plan.counts.released = s.released ∧
      plan.counts.entered = plan.counts.released ∧
      plan.counts.active = 0#u32 ∧
      plan.counts.released_on_abort = 0#u32 ∧
      s.frames.val = [] ∧
      s.pending_dispatch = false := by
  have ⟨hvalid, hempty, hdisp⟩ := (plan_finish_root_iff s claim).mp ⟨plan, h⟩
  obtain ⟨active, hact, hlen⟩ := validate_active_root_of_valid s claim hvalid
  have hact0 : active = 0#u32 := by
    have : active.val = 0 := by rw [hlen, hempty]; rfl
    exact (UScalar.eq_equiv _ _).mpr (by simpa using this)
  unfold ledger.transition.plan_finish_root at h
  rw [hact] at h
  simp only [bind_ok, hact0, bne_self_eq_false, Bool.false_eq_true, ite_false, hdisp,
    Result.ok.injEq, core.result.Result.Ok.injEq] at h
  subst h
  have hent_rel : s.entered = s.released := by
    have h1 := hvalid.1.released_le
    have h2 := hvalid.1.count_eq
    rw [hempty, List.length_nil] at h2
    exact (UScalar.eq_equiv _ _).mpr (by omega)
  exact ⟨hvalid, rfl, rfl, rfl, rfl, hent_rel, rfl, rfl, hempty, hdisp⟩

/-- Refusal: `plan_finish_root` rejects active frames with `ActiveFramesRemaining`. -/
theorem plan_finish_root_refuses_active_frames (s : ledger.Snapshot) (claim : ledger.RootToken)
    (hvalid : ValidActiveRoot s claim) (hne : s.frames.val ≠ []) :
    ledger.transition.plan_finish_root s claim =
      Result.ok (core.result.Result.Err ledger.Error.ActiveFramesRemaining) := by
  obtain ⟨active, hact, hlen⟩ := validate_active_root_of_valid s claim hvalid
  have hpos : 0 < s.frames.val.length := List.length_pos_iff.mpr hne
  have hne0 : active.val ≠ 0 := by omega
  unfold ledger.transition.plan_finish_root
  rw [hact]
  simp [hne0]

/-- Refusal: `plan_finish_root` rejects `pending_dispatch = true` with `PendingDispatch`. -/
theorem plan_finish_root_refuses_pending_dispatch (s : ledger.Snapshot) (claim : ledger.RootToken)
    (hvalid : ValidActiveRoot s claim) (hempty : s.frames.val = [])
    (hdisp : s.pending_dispatch = true) :
    ledger.transition.plan_finish_root s claim =
      Result.ok (core.result.Result.Err ledger.Error.PendingDispatch) := by
  obtain ⟨active, hact, hlen⟩ := validate_active_root_of_valid s claim hvalid
  have hact0 : active = 0#u32 := by
    have : active.val = 0 := by rw [hlen, hempty]; rfl
    exact (UScalar.eq_equiv _ _).mpr (by simpa using this)
  unfold ledger.transition.plan_finish_root
  rw [hact]
  simp [hact0, hdisp]

/-- Complete specification of `ack_finish_root` (rejections preserve `plan`; valid ack retires
root to `Finished` and keeps `Healthy`/`Retained` iff both backend acks succeeded). -/
theorem ack_finish_root_spec (plan : ledger.FinishPlan) (ack : ledger.FinishAck) :
    (ack.root = 0#u64 →
      ledger.transition.ack_finish_root plan ack =
        Result.ok (core.result.Result.Err { error := ledger.Error.InvalidRootToken, plan })) ∧
    (ack.root ≠ 0#u64 → ack.root ≠ plan.root →
      ledger.transition.ack_finish_root plan ack =
        Result.ok (core.result.Result.Err { error := ledger.Error.ForeignRoot, plan })) ∧
    (ack.root ≠ 0#u64 → ack.root = plan.root →
      ∃ out, ledger.transition.ack_finish_root plan ack =
          Result.ok (core.result.Result.Ok out) ∧
        out.next.root = plan.root ∧
        out.next.map_epoch = plan.map_epoch ∧
        out.next.status = ledger.RootStatus.Finished ∧
        out.next.frames.val = [] ∧
        out.next.pending_dispatch = false ∧
        out.next.entered = plan.counts.entered ∧
        out.next.released = plan.counts.released ∧
        out.counts = plan.counts ∧
        (out.exit_release = ledger.ExitRelease.Exact ↔
          ack.retire_versions = ledger.BackendAck.Succeeded ∧
          ack.sweep_retirement = ledger.BackendAck.Succeeded) ∧
        ((out.next.session = ledger.SessionHealth.Healthy ∧
          out.session_state = ledger.SessionState.Retained) ↔
          out.exit_release = ledger.ExitRelease.Exact) ∧
        (out.exit_release ≠ ledger.ExitRelease.Exact →
          out.next.session = ledger.SessionHealth.Poisoned ∧
          out.session_state = ledger.SessionState.TerminatedAfterGuestFailure)) := by
  refine ⟨?_, ?_, ?_⟩
  · intro h0
    simp [ledger.transition.ack_finish_root, h0]
  · intro h0 hne
    have hbne : (ack.root != plan.root) = true := by simp [hne]
    simp [ledger.transition.ack_finish_root, h0, hbne]
  · intro h0 heq
    have hbne : (ack.root != plan.root) = false := by simp [heq]
    unfold ledger.transition.ack_finish_root ledger.transition.disposition
    simp only [h0, ite_false, hbne, Bool.false_eq_true]
    cases hret : ack.retire_versions <;> cases hsweep : ack.sweep_retirement <;>
      simp [bind_ok]

/-! ## Property 3 (#9c): `plan_abort_root` / `ack_abort_root` Exact Release vs. Fail-Closed `Poisoned` -/

/-- Pure predicate characterizing a settled guest observation. -/
def GuestSettled (g : ledger.GuestObservation) : Prop :=
  g.guest_faulted = false ∧
  g.failure_flag = 0#u32 ∧
  g.continuation_depth = 0#u32 ∧
  g.reflection_depth = 0#u32 ∧
  g.dispatch_flag = 0#u32 ∧
  g.current_frame_ptr = 0#u32 ∧
  g.live_frame_token = 0#u32

/-- Pure predicate characterizing a clean pre-abort observation. -/
def PreAbortClean (pre : ledger.PreAbortObservation) : Prop :=
  pre.replay_release_ok = true ∧
  pre.provisional_owners_release_ok = true ∧
  pre.trace_available = true ∧
  GuestSettled pre.guest ∧
  pre.engine_poisoned_during_root = false

/-- `classify_guest` returns `Settled` iff `GuestSettled g`. -/
theorem classify_guest_settled_iff (g : ledger.GuestObservation) :
    ledger.transition.classify_guest g = Result.ok ledger.GuestSettlement.Settled ↔
      GuestSettled g := by
  unfold ledger.transition.classify_guest GuestSettled
  by_cases h1 : g.guest_faulted = true
  · simp [h1]
  · have h1f : g.guest_faulted = false := Bool.eq_false_iff.mpr h1
    by_cases h2 : g.failure_flag = 0#u32
    · by_cases h3 : g.continuation_depth = 0#u32
      · by_cases h4 : g.reflection_depth = 0#u32
        · by_cases h5 : g.dispatch_flag = 0#u32
          · by_cases h6 : g.current_frame_ptr = 0#u32
            · by_cases h7 : g.live_frame_token = 0#u32
              · simp [h1f, h2, h3, h4, h5, h6, h7]
              · simp [h1f, h2, h3, h4, h5, h6, h7]
            · simp [h1f, h2, h3, h4, h5, h6]
          · simp [h1f, h2, h3, h4, h5]
        · simp [h1f, h2, h3, h4]
      · simp [h1f, h2, h3]
    · simp [h1f, h2]

/-- `classify_guest` always terminates cleanly (`Result.ok`). -/
theorem classify_guest_total (g : ledger.GuestObservation) :
    ∃ r, ledger.transition.classify_guest g = Result.ok r := by
  unfold ledger.transition.classify_guest
  split_ifs <;> exact ⟨_, rfl⟩

/-- `first_plan_unproven` returns `none` iff pre-abort is clean, no frames remain on abort, and `!pending`. -/
theorem first_plan_unproven_none_iff (pre : ledger.PreAbortObservation)
    (released_on_abort : Std.U32) (pending_dispatch : Bool) :
    ledger.transition.first_plan_unproven pre released_on_abort pending_dispatch =
      Result.ok none ↔
      PreAbortClean pre ∧ released_on_abort = 0#u32 ∧ pending_dispatch = false := by
  unfold ledger.transition.first_plan_unproven PreAbortClean
  by_cases hrep : pre.replay_release_ok = true
  · by_cases hprov : pre.provisional_owners_release_ok = true
    · by_cases htr : pre.trace_available = true
      · obtain ⟨gs, hgs⟩ := classify_guest_total pre.guest
        rw [hgs]
        simp only [bind_ok]
        cases gs with
        | Unsettled reason =>
          have hns : ¬ GuestSettled pre.guest := by
            rw [← classify_guest_settled_iff, hgs]
            simp
          simp [hrep, hprov, htr, hns]
        | Settled =>
          have hs : GuestSettled pre.guest := (classify_guest_settled_iff pre.guest).mp hgs
          by_cases hroa : released_on_abort = 0#u32
          · by_cases hpd : pending_dispatch = true
            · simp [hrep, hprov, htr, hs, hroa, hpd]
            · have hpd' : pending_dispatch = false := Bool.eq_false_iff.mpr hpd
              by_cases hpo : pre.engine_poisoned_during_root = true
              · simp [hrep, hprov, htr, hs, hroa, hpd', hpo]
              · have hpo' : pre.engine_poisoned_during_root = false := Bool.eq_false_iff.mpr hpo
                simp [hrep, hprov, htr, hs, hroa, hpd', hpo']
          · simp [hrep, hprov, htr, hs, hroa]
      · simp [hrep, hprov, htr]
    · simp [hrep, hprov]
  · simp [hrep]

/-- `first_plan_unproven` always terminates cleanly (`Result.ok`). -/
theorem first_plan_unproven_total (pre : ledger.PreAbortObservation)
    (released_on_abort : Std.U32) (pending_dispatch : Bool) :
    ∃ r, ledger.transition.first_plan_unproven pre released_on_abort pending_dispatch = Result.ok r := by
  unfold ledger.transition.first_plan_unproven
  obtain ⟨gs, hgs⟩ := classify_guest_total pre.guest
  rw [hgs]
  simp only [bind_ok]
  cases gs <;> split_ifs <;> exact ⟨_, rfl⟩

/-- `plan_abort_root` produces an exact LIFO release list `s.frames.val.reverse`, balances counts,
and records `plan_unproven = none` iff `PreAbortClean pre ∧ s.frames.val = [] ∧ !s.pending_dispatch`. -/
theorem plan_abort_root_spec (s : ledger.Snapshot) (claim : ledger.RootToken)
    (pre : ledger.PreAbortObservation) (hvalid : ValidActiveRoot s claim) :
    ∃ plan, ledger.transition.plan_abort_root s claim pre =
        Result.ok (core.result.Result.Ok plan) ∧
      plan.root = s.root ∧
      plan.map_epoch = s.map_epoch ∧
      plan.released_frames.val = s.frames.val.reverse ∧
      plan.counts.entered = s.entered ∧
      plan.counts.released = s.entered ∧
      plan.counts.active = 0#u32 ∧
      plan.counts.released_on_abort.val = s.frames.val.length ∧
      (plan.plan_unproven = none ↔
        PreAbortClean pre ∧ s.frames.val = [] ∧ s.pending_dispatch = false) ∧
      plan.actions.clear_operand_stack = plan.plan_unproven.isNone ∧
      plan.actions.retire_map_pin = s.map_epoch ∧
      plan.actions.retire_aborted_root_versions = true ∧
      plan.actions.sweep_module_retirement = plan.plan_unproven.isNone := by
  obtain ⟨active, hact, hlen⟩ := validate_active_root_of_valid s claim hvalid
  have hadd := U32.checked_add_bv_spec s.released active
  cases ho : U32.checked_add s.released active with
  | none =>
    rw [ho] at hadd
    have hle := hvalid.1.released_le
    have hcnt := hvalid.1.count_eq
    have hent : s.entered.val ≤ U32.max := by scalar_tac
    omega
  | some val1 =>
    rw [ho] at hadd
    have hval1_eq : val1 = s.entered := by
      have hle := hvalid.1.released_le
      have hcnt := hvalid.1.count_eq
      exact (UScalar.eq_equiv _ _).mpr (by omega)
    obtain ⟨rev, hrev, hrev_val⟩ := reverse_frames_eq s.frames.slice
    obtain ⟨pu, hpu⟩ := first_plan_unproven_total pre active s.pending_dispatch
    unfold ledger.transition.plan_abort_root
    rw [hact]
    simp only [bind_ok, lift, ho, hval1_eq, ite_true, alloc.vec.Vec.as_slice, hrev, hpu]
    have hisNone : core.option.Option.is_none pu = pu.isNone := by
      cases pu <;> rfl
    have hpu_iff : pu = none ↔ PreAbortClean pre ∧ s.frames.val = [] ∧ s.pending_dispatch = false := by
      have h1 := first_plan_unproven_none_iff pre active s.pending_dispatch
      rw [hpu, Result.ok.injEq] at h1
      have hact0 : active = 0#u32 ↔ s.frames.val = [] := by
        constructor
        · intro ha
          have : s.frames.val.length = 0 := by rw [← hlen, ha]; rfl
          exact List.length_eq_zero_iff.mp this
        · intro he
          have : active.val = 0 := by rw [hlen, he]; rfl
          exact (UScalar.eq_equiv _ _).mpr (by simpa using this)
      rw [h1, hact0]
    exact ⟨_, rfl, rfl, rfl, hrev_val, rfl, rfl, rfl, hlen, hpu_iff, hisNone, rfl, rfl, hisNone⟩

/-- Specification of `ack_abort_root` for every ack (including rejected root mismatch). -/
theorem ack_abort_root_spec (plan : ledger.AbortPlan) (ack : ledger.AbortAck) :
    (ack.root = 0#u64 →
      ledger.transition.ack_abort_root plan ack =
        Result.ok (core.result.Result.Err { error := ledger.Error.InvalidRootToken, plan })) ∧
    (ack.root ≠ 0#u64 → ack.root ≠ plan.root →
      ledger.transition.ack_abort_root plan ack =
        Result.ok (core.result.Result.Err { error := ledger.Error.ForeignRoot, plan })) ∧
    (ack.root ≠ 0#u64 → ack.root = plan.root →
      ∃ out, ledger.transition.ack_abort_root plan ack =
          Result.ok (core.result.Result.Ok out) ∧
        out.next.root = plan.root ∧
        out.next.map_epoch = plan.map_epoch ∧
        out.next.status = ledger.RootStatus.Aborted ∧
        out.next.frames.val = [] ∧
        out.next.pending_dispatch = false ∧
        out.next.entered = plan.counts.entered ∧
        out.next.released = plan.counts.released ∧
        out.counts = plan.counts ∧
        out.released_frames = plan.released_frames ∧
        (out.exit_release = ledger.ExitRelease.Exact ↔
          (plan.actions.clear_operand_stack = true ∨ ack.clear_stack = ledger.ClearStackAck.NotAttempted) ∧
          plan.plan_unproven = none ∧
          ack.clear_stack = ledger.ClearStackAck.Cleared ∧
          ack.retire_versions = ledger.BackendAck.Succeeded ∧
          ack.sweep_retirement = ledger.BackendAck.Succeeded) ∧
        ((out.next.session = ledger.SessionHealth.Healthy ∧
          out.session_state = ledger.SessionState.Retained) ↔
          out.exit_release = ledger.ExitRelease.Exact) ∧
        (out.exit_release ≠ ledger.ExitRelease.Exact →
          out.next.session = ledger.SessionHealth.Poisoned ∧
          out.session_state = ledger.SessionState.TerminatedAfterGuestFailure)) := by
  refine ⟨?_, ?_, ?_⟩
  · intro h0
    simp [ledger.transition.ack_abort_root, h0]
  · intro h0 hne
    have hbne : (ack.root != plan.root) = true := by simp [hne]
    simp [ledger.transition.ack_abort_root, h0, hbne]
  · intro h0 heq
    have hbne : (ack.root != plan.root) = false := by simp [heq]
    unfold ledger.transition.ack_abort_root ledger.transition.resolve_abort_release
      ledger.transition.disposition
    simp only [h0, ite_false, hbne, Bool.false_eq_true]
    cases hclr : plan.actions.clear_operand_stack <;>
      cases hpu : plan.plan_unproven <;>
      cases hcs : ack.clear_stack <;>
      cases hret : ack.retire_versions <;>
      cases hsweep : ack.sweep_retirement <;>
      simp [bind_ok]

/-- Composed `plan_abort_root` + `ack_abort_root`: exact release iff settled guest + empty frames +
`!pending_dispatch` + clean pre-abort + `clear_stack = Cleared` + `retire_versions = Succeeded` +
`sweep_retirement = Succeeded`; all other paths fail closed to `Poisoned` / `TerminatedAfterGuestFailure`. -/
theorem ack_abort_root_exact_iff (s : ledger.Snapshot) (claim : ledger.RootToken)
    (pre : ledger.PreAbortObservation) (plan : ledger.AbortPlan)
    (ack : ledger.AbortAck) (out : ledger.AbortOutcome)
    (hvalid : ValidActiveRoot s claim)
    (hplan : ledger.transition.plan_abort_root s claim pre = Result.ok (core.result.Result.Ok plan))
    (hack : ledger.transition.ack_abort_root plan ack = Result.ok (core.result.Result.Ok out)) :
    out.next.status = ledger.RootStatus.Aborted ∧
      out.next.frames.val = [] ∧
      out.next.pending_dispatch = false ∧
      out.next.entered = s.entered ∧
      out.next.released = s.entered ∧
      out.released_frames.val = s.frames.val.reverse ∧
      (out.exit_release = ledger.ExitRelease.Exact ↔
        PreAbortClean pre ∧ s.frames.val = [] ∧ s.pending_dispatch = false ∧
        ack.clear_stack = ledger.ClearStackAck.Cleared ∧
        ack.retire_versions = ledger.BackendAck.Succeeded ∧
        ack.sweep_retirement = ledger.BackendAck.Succeeded) ∧
      ((out.next.session = ledger.SessionHealth.Healthy ∧
        out.session_state = ledger.SessionState.Retained) ↔
        out.exit_release = ledger.ExitRelease.Exact) ∧
      (out.exit_release ≠ ledger.ExitRelease.Exact →
        out.next.session = ledger.SessionHealth.Poisoned ∧
        out.session_state = ledger.SessionState.TerminatedAfterGuestFailure) := by
  obtain ⟨plan', hplan', _, _, hrev, hent, hrel, _, _, hpu_iff, hclr, _, _, _⟩ :=
    plan_abort_root_spec s claim pre hvalid
  rw [hplan, Result.ok.injEq, core.result.Result.Ok.injEq] at hplan'
  subst hplan'
  have hroot_nz : ack.root ≠ 0#u64 := by
    intro h0
    have hbad := (ack_abort_root_spec plan ack).1 h0
    rw [hack, Result.ok.injEq] at hbad
    cases hbad
  have hroot_eq : ack.root = plan.root := by
    by_contra hne
    have hbad := (ack_abort_root_spec plan ack).2.1 hroot_nz hne
    rw [hack, Result.ok.injEq] at hbad
    cases hbad
  obtain ⟨out', hack', _, _, hst, hfr, hpd, hent', hrel', _, hrelf, hexact, hdisp, hfail⟩ :=
    (ack_abort_root_spec plan ack).2.2 hroot_nz hroot_eq
  rw [hack, Result.ok.injEq, core.result.Result.Ok.injEq] at hack'
  subst hack'
  refine ⟨hst, hfr, hpd, ?_, ?_, ?_, ?_, hdisp, hfail⟩
  · rw [hent', hent]
  · rw [hrel', hrel]
  · rw [hrelf, hrev]
  · rw [hexact, hpu_iff]
    constructor
    · rintro ⟨_, hpu_none, hcs, hret, hsweep⟩
      exact ⟨hpu_none.1, hpu_none.2.1, hpu_none.2.2, hcs, hret, hsweep⟩
    · rintro ⟨hclean, hempty, hpdisp, hcs, hret, hsweep⟩
      have hpu_none : plan.plan_unproven = none := hpu_iff.mpr ⟨hclean, hempty, hpdisp⟩
      refine ⟨Or.inl ?_, ⟨hclean, hempty, hpdisp⟩, hcs, hret, hsweep⟩
      rw [hclr, hpu_none]
      rfl

/-- Fail-closed `UnexpectedGuestResumption`: if clearing operand stack was forbidden
(`clear_operand_stack = false`) yet the backend attempted `clear_stack ≠ NotAttempted`,
`ack_abort_root` poisons the session with `UnexpectedGuestResumption`. -/
theorem ack_abort_root_unexpected_resumption (plan : ledger.AbortPlan) (ack : ledger.AbortAck)
    (out : ledger.AbortOutcome) (hno : plan.actions.clear_operand_stack = false)
    (hcs : ack.clear_stack ≠ ledger.ClearStackAck.NotAttempted)
    (hack : ledger.transition.ack_abort_root plan ack = Result.ok (core.result.Result.Ok out)) :
    out.exit_release = ledger.ExitRelease.Unproven ledger.UnprovenReason.UnexpectedGuestResumption ∧
      out.next.session = ledger.SessionHealth.Poisoned ∧
      out.session_state = ledger.SessionState.TerminatedAfterGuestFailure := by
  unfold ledger.transition.ack_abort_root ledger.transition.resolve_abort_release
    ledger.transition.disposition at hack
  by_cases h0 : ack.root = 0#u64
  · simp [h0] at hack
  · by_cases hne : ack.root != plan.root
    · simp [h0, hne] at hack
    · simp only [h0, ite_false, hne, Bool.false_eq_true, hno] at hack
      cases hclr : ack.clear_stack with
      | NotAttempted => exact (hcs hclr).elim
      | Cleared =>
        simp [hclr] at hack
        subst hack
        exact ⟨rfl, rfl, rfl⟩
      | Refused =>
        simp [hclr] at hack
        subst hack
        exact ⟨rfl, rfl, rfl⟩
      | Trapped =>
        simp [hclr] at hack
        subst hack
        exact ⟨rfl, rfl, rfl⟩

/-! ## Property 4 (#9d): Non-Wrapping `entered` / `released` Counters -/

/-- `enter_frame` never wraps `entered`: at `U32.max` (when reachable) it refuses with
`FrameCounterExhausted`, and on any `.Ok` step `entered` increases by exactly 1 within `U32.max`. -/
theorem enter_frame_counter_nonwrapping (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (hvalid : ValidActiveRoot s claim) :
    (s.entered.val = U32.max →
      ∃ e, ledger.transition.enter_frame s claim ft = Result.ok (core.result.Result.Err e)) ∧
    (s.entered.val = U32.max → ft ≠ 0#u32 →
      (∀ last, s.frames.val.getLast? = some last → last.val < ft.val) →
      s.frames.val.length < ledger.MAX_ACTIVE_FRAMES.val →
      ledger.transition.enter_frame s claim ft =
        Result.ok (core.result.Result.Err ledger.Error.FrameCounterExhausted)) ∧
    (∀ tr, ledger.transition.enter_frame s claim ft = Result.ok (core.result.Result.Ok tr) →
      tr.next.entered.val = s.entered.val + 1 ∧
      tr.next.entered.val ≤ U32.max ∧
      tr.next.released = s.released ∧
      tr.next.released.val ≤ tr.next.entered.val) := by
  refine ⟨?_, ?_, ?_⟩
  · intro hmax
    by_cases hft : ft = 0#u32
    · subst hft
      exact ⟨_, enter_frame_refuses_invalid_token s claim hvalid⟩
    · by_cases hcap : ledger.MAX_ACTIVE_FRAMES.val ≤ s.frames.val.length
      · have hex := Classical.em (∀ last, s.frames.val.getLast? = some last → last.val < ft.val)
        cases hex with
        | inl hmono =>
          exact ⟨_, enter_frame_refuses_bound_exceeded s claim ft hvalid hft hmono hcap⟩
        | inr hnot =>
          have ⟨last, hlast, hle⟩ :
              ∃ last, s.frames.val.getLast? = some last ∧ ft.val ≤ last.val := by
            by_contra hnone
            apply hnot
            intro last hl
            by_contra hbad
            exact hnone ⟨last, hl, by omega⟩
          exact ⟨_, enter_frame_refuses_duplicate_or_unordered s claim ft last hvalid hft hlast hle⟩
      · have hcap' : s.frames.val.length < ledger.MAX_ACTIVE_FRAMES.val := by omega
        have hex := Classical.em (∀ last, s.frames.val.getLast? = some last → last.val < ft.val)
        cases hex with
        | inl hmono =>
          exact ⟨_, enter_frame_refuses_counter_exhausted s claim ft hvalid hft hmono hcap' hmax⟩
        | inr hnot =>
          have ⟨last, hlast, hle⟩ :
              ∃ last, s.frames.val.getLast? = some last ∧ ft.val ≤ last.val := by
            by_contra hnone
            apply hnot
            intro last hl
            by_contra hbad
            exact hnone ⟨last, hl, by omega⟩
          exact ⟨_, enter_frame_refuses_duplicate_or_unordered s claim ft last hvalid hft hlast hle⟩
  · intro hmax hft hmono hcap
    exact enter_frame_refuses_counter_exhausted s claim ft hvalid hft hmono hcap hmax
  · intro tr htr
    obtain ⟨active, hact, _⟩ := validate_active_root_of_valid s claim hvalid
    unfold ledger.transition.enter_frame at htr
    rw [hact] at htr
    simp only [bind_ok] at htr
    by_cases hft : ft = 0#u32
    · simp [hft] at htr
    · simp only [hft, ite_false, lift] at htr
      cases ho : Usize.checked_sub (alloc.vec.Vec.len s.frames) 1#usize with
      | none =>
        rw [ho] at htr
        by_cases hcap : active ≥ ledger.MAX_ACTIVE_FRAMES
        · simp [hcap] at htr
        · simp only [hcap, ite_false] at htr
          have hadd1 := U32.checked_add_bv_spec s.entered 1#u32
          cases ho1 : U32.checked_add s.entered 1#u32 with
          | none => rw [ho1] at htr; simp at htr
          | some entered1 =>
            rw [ho1] at htr hadd1
            dsimp only at hadd1
            cases ho2 : U32.checked_add active 1#u32 with
            | none => rw [ho2] at htr; simp at htr
            | some active1 =>
              rw [ho2] at htr
              simp only [alloc.vec.Vec.as_slice, bind_ok] at htr
              obtain ⟨pref, hpref, hpref_val⟩ :=
                copy_frame_prefix_eq s.frames.slice (alloc.vec.Vec.len s.frames)
              rw [hpref] at htr
              simp only [bind_ok] at htr
              have hpush_lt : pref.val.length < Usize.max := by
                have hflen : (alloc.vec.Vec.len s.frames).val = s.frames.val.length := by simp
                have hsl : s.frames.slice.length = s.frames.val.length := rfl
                have hsv : s.frames.slice.val = s.frames.val := rfl
                have hp_eq : pref.val = s.frames.val := by
                  rw [hpref_val, hflen, hsl, hsv, Nat.min_self, List.take_length]
                rw [hp_eq]
                have := validSnapshot_active_le_u32 s hvalid.1
                scalar_tac
              obtain ⟨nf, hpush, _⟩ := specReturns (alloc.vec.Vec.push_spec pref ft hpush_lt)
              rw [hpush] at htr
              simp only [bind_ok, Result.ok.injEq, core.result.Result.Ok.injEq] at htr
              subst htr
              dsimp only
              have h1 := u32_one_val
              have hrel := hvalid.1.released_le
              exact ⟨by omega, by omega, rfl, by omega⟩
      | some last_idx =>
        rw [ho] at htr
        have hsub := Usize.checked_sub_bv_spec (alloc.vec.Vec.len s.frames) 1#usize
        rw [ho] at hsub
        have hidx_lt : last_idx.val < s.frames.length := by scalar_tac
        obtain ⟨last_ft, hget, _⟩ :=
          specReturns (alloc.vec.Vec.index_usize_spec s.frames last_idx hidx_lt)
        simp only [alloc.vec.Vec.index_slice_index, hget, bind_ok] at htr
        by_cases hle : ft ≤ last_ft
        · simp [hle] at htr
        · simp only [hle, ite_false] at htr
          by_cases hcap : active ≥ ledger.MAX_ACTIVE_FRAMES
          · simp [hcap] at htr
          · simp only [hcap, ite_false] at htr
            have hadd1 := U32.checked_add_bv_spec s.entered 1#u32
            cases ho1 : U32.checked_add s.entered 1#u32 with
            | none => rw [ho1] at htr; simp at htr
            | some entered1 =>
              rw [ho1] at htr hadd1
              dsimp only at hadd1
              cases ho2 : U32.checked_add active 1#u32 with
              | none => rw [ho2] at htr; simp at htr
              | some active1 =>
                rw [ho2] at htr
                simp only [alloc.vec.Vec.as_slice, bind_ok] at htr
                obtain ⟨pref, hpref, hpref_val⟩ :=
                  copy_frame_prefix_eq s.frames.slice (alloc.vec.Vec.len s.frames)
                rw [hpref] at htr
                simp only [bind_ok] at htr
                have hpush_lt : pref.val.length < Usize.max := by
                  have hflen : (alloc.vec.Vec.len s.frames).val = s.frames.val.length := by simp
                  have hsl : s.frames.slice.length = s.frames.val.length := rfl
                  have hsv : s.frames.slice.val = s.frames.val := rfl
                  have hp_eq : pref.val = s.frames.val := by
                    rw [hpref_val, hflen, hsl, hsv, Nat.min_self, List.take_length]
                  rw [hp_eq]
                  have := validSnapshot_active_le_u32 s hvalid.1
                  scalar_tac
                obtain ⟨nf, hpush, _⟩ := specReturns (alloc.vec.Vec.push_spec pref ft hpush_lt)
                rw [hpush] at htr
                simp only [bind_ok, Result.ok.injEq, core.result.Result.Ok.injEq] at htr
                subst htr
                dsimp only
                have h1 := u32_one_val
                have hrel := hvalid.1.released_le
                exact ⟨by omega, by omega, rfl, by omega⟩

/-- `leave_frame` never wraps `released`: on any `.Ok` transition, `released` increments by 1
within `entered ≤ U32.max`. -/
theorem leave_frame_counter_nonwrapping (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (tr : ledger.FrameTransition)
    (h : ledger.transition.leave_frame s claim ft = Result.ok (core.result.Result.Ok tr)) :
    tr.next.released.val = s.released.val + 1 ∧
      tr.next.released.val ≤ tr.next.entered.val ∧
      tr.next.entered = s.entered ∧
      tr.next.entered.val ≤ U32.max := by
  unfold ledger.transition.leave_frame at h
  obtain ⟨r, hval⟩ := validate_active_root_total s claim
  rw [hval] at h
  simp only [bind_ok] at h
  cases r with
  | Err e => simp at h
  | Ok active =>
    have hok := (validate_active_root_ok_iff s claim active).mp hval
    dsimp only at h
    by_cases hdisp : s.pending_dispatch = true
    · simp [hdisp] at h
    · by_cases hft : ft = 0#u32
      · simp [hdisp, hft] at h
      · simp only [hdisp, ite_false, hft, lift] at h
        cases ho : Usize.checked_sub (alloc.vec.Vec.len s.frames) 1#usize with
        | none => rw [ho] at h; simp at h
        | some idx =>
          rw [ho] at h
          have hsub := Usize.checked_sub_bv_spec (alloc.vec.Vec.len s.frames) 1#usize
          rw [ho] at hsub
          have hidx_lt : idx.val < s.frames.length := by scalar_tac
          obtain ⟨got_ft, hget, _⟩ :=
            specReturns (alloc.vec.Vec.index_usize_spec s.frames idx hidx_lt)
          simp only [alloc.vec.Vec.index_slice_index, hget, bind_ok] at h
          by_cases hne : got_ft != ft
          · simp [hne] at h
          · simp only [hne, Bool.false_eq_true, ite_false] at h
            have hadd1 := U32.checked_add_bv_spec s.released 1#u32
            cases ho1 : U32.checked_add s.released 1#u32 with
            | none => rw [ho1] at h; simp at h
            | some rel1 =>
              rw [ho1] at h hadd1
              dsimp only at hadd1
              have hsub2 := U32.checked_sub_bv_spec active 1#u32
              cases ho2 : U32.checked_sub active 1#u32 with
              | none => rw [ho2] at h; simp at h
              | some act1 =>
                rw [ho2] at h hsub2
                simp only [alloc.vec.Vec.as_slice, bind_ok] at h
                obtain ⟨nf, hpref, _⟩ := copy_frame_prefix_eq s.frames.slice idx
                rw [hpref] at h
                simp only [bind_ok, Result.ok.injEq, core.result.Result.Ok.injEq] at h
                subst h
                dsimp only
                have h1 := u32_one_val
                have hact_pos : 0 < active.val := by omega
                have hcnt := hok.1.1.count_eq
                have hle := hok.1.1.released_le
                have hlen_act := hok.2
                have hent : s.entered.val ≤ U32.max := by scalar_tac
                exact ⟨by omega, by omega, rfl, hent⟩

private theorem enter_frame_preserves_valid (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (tr : ledger.FrameTransition) (hvalid : ValidActiveRoot s claim)
    (hstep : ledger.transition.enter_frame s claim ft = Result.ok (core.result.Result.Ok tr)) :
    ValidSnapshot tr.next ∧ tr.next.root = s.root := by
  have hft : ft ≠ 0#u32 := by
    intro h0; subst h0
    have href := enter_frame_refuses_invalid_token s claim hvalid
    rw [hstep, Result.ok.injEq] at href; cases href
  have hmono : ∀ last, s.frames.val.getLast? = some last → last.val < ft.val := by
    intro last hl
    by_contra hle
    have hle' : ft.val ≤ last.val := by omega
    have href := enter_frame_refuses_duplicate_or_unordered s claim ft last hvalid hft hl hle'
    rw [hstep, Result.ok.injEq] at href; cases href
  have hcap : s.frames.val.length < ledger.MAX_ACTIVE_FRAMES.val := by
    by_contra hge
    have hge' : ledger.MAX_ACTIVE_FRAMES.val ≤ s.frames.val.length := by omega
    have href := enter_frame_refuses_bound_exceeded s claim ft hvalid hft hmono hge'
    rw [hstep, Result.ok.injEq] at href; cases href
  have hcnt : s.entered.val < U32.max := by
    have hle : s.entered.val ≤ U32.max := by scalar_tac
    by_contra hge
    have hmax : s.entered.val = U32.max := by omega
    have href := enter_frame_refuses_counter_exhausted s claim ft hvalid hft hmono hcap hmax
    rw [hstep, Result.ok.injEq] at href; cases href
  obtain ⟨tr', htr', _, hv', hrt', _, _, _, _, _, _, _, _⟩ :=
    enter_frame_pushes_lifo s claim ft hvalid hft hmono hcap hcnt
  rw [hstep, Result.ok.injEq, core.result.Result.Ok.injEq] at htr'
  subst htr'
  exact ⟨hv', hrt'⟩

private theorem leave_frame_preserves_valid (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (tr : ledger.FrameTransition) (hvalid : ValidActiveRoot s claim)
    (hstep : ledger.transition.leave_frame s claim ft = Result.ok (core.result.Result.Ok tr)) :
    ValidSnapshot tr.next ∧ tr.next.root = s.root := by
  have hdisp : s.pending_dispatch = false := by
    by_contra hne
    have hpd : s.pending_dispatch = true := Bool.eq_true_of_not_eq_false hne
    have href := leave_frame_refuses_pending_dispatch s claim ft hvalid hpd
    rw [hstep, Result.ok.injEq] at href; cases href
  have hft : ft ≠ 0#u32 := by
    intro h0; subst h0
    have href := leave_frame_refuses_invalid_token s claim hvalid hdisp
    rw [hstep, Result.ok.injEq] at href; cases href
  have hlast : s.frames.val.getLast? = some ft := by
    by_contra hne
    have href := leave_frame_refuses_non_lifo s claim ft hvalid hdisp hft hne
    rw [hstep, Result.ok.injEq] at href; cases href
  have hne_nil : s.frames.val ≠ [] := by
    intro hem; simp [hem] at hlast
  have hframes : s.frames.val = s.frames.val.dropLast ++ [ft] := by
    have hgl : s.frames.val.getLast hne_nil = ft := by
      rw [List.getLast?_eq_some_getLast hne_nil] at hlast
      injection hlast
    rw [← hgl, List.dropLast_append_getLast hne_nil]
  obtain ⟨tr', htr', _, hv', hrt', _, _, _, _, _, _, _, _⟩ :=
    leave_frame_pops_lifo s claim ft s.frames.val.dropLast hvalid hdisp hft hframes
  rw [hstep, Result.ok.injEq, core.result.Result.Ok.injEq] at htr'
  subst htr'
  exact ⟨hv', hrt'⟩

/-- Finite inductive trace of active snapshots reachable from `Snapshot.pinned` via
`enter_frame` and `leave_frame`. -/
inductive LedgerTrace (root : ledger.RootToken) (epoch : ledger.MapEpoch) : ledger.Snapshot → Prop where
  | pinned {s : ledger.Snapshot} :
      ledger.Snapshot.pinned root epoch = Result.ok (core.result.Result.Ok s) →
      LedgerTrace root epoch s
  | enter {s : ledger.Snapshot} {ft : ledger.FrameToken} {tr : ledger.FrameTransition} :
      LedgerTrace root epoch s →
      ledger.transition.enter_frame s root ft = Result.ok (core.result.Result.Ok tr) →
      LedgerTrace root epoch tr.next
  | leave {s : ledger.Snapshot} {ft : ledger.FrameToken} {tr : ledger.FrameTransition} :
      LedgerTrace root epoch s →
      ledger.transition.leave_frame s root ft = Result.ok (core.result.Result.Ok tr) →
      LedgerTrace root epoch tr.next

/-- Every snapshot along a `LedgerTrace` satisfies `ValidActiveRoot s root`. -/
theorem trace_valid_snapshot {root : ledger.RootToken} {epoch : ledger.MapEpoch}
    {s : ledger.Snapshot} (htr : LedgerTrace root epoch s) :
    ValidActiveRoot s root := by
  induction htr with
  | pinned hp =>
    unfold ledger.Snapshot.pinned at hp
    by_cases h0 : root = 0#u64
    · simp [h0] at hp
    · simp only [h0, ite_false, Result.ok.injEq, core.result.Result.Ok.injEq] at hp
      subst hp
      exact ⟨⟨rfl, h0, rfl, by simp, by simp, by simp, by simp [StrictlyIncreasing]⟩, rfl⟩
  | @enter s_prev _ _ _ hstep ih =>
    have ⟨hv', hrt'⟩ := enter_frame_preserves_valid s_prev root _ _ ih hstep
    exact ⟨hv', by rw [hrt', ih.2]⟩
  | @leave s_prev _ _ _ hstep ih =>
    have ⟨hv', hrt'⟩ := leave_frame_preserves_valid s_prev root _ _ ih hstep
    exact ⟨hv', by rw [hrt', ih.2]⟩

/-- Every reachable snapshot in a `LedgerTrace` has non-wrapping bounded counters. -/
theorem trace_counters_monotone_nonwrapping {root : ledger.RootToken} {epoch : ledger.MapEpoch}
    {s : ledger.Snapshot} (htr : LedgerTrace root epoch s) :
    s.released.val ≤ s.entered.val ∧
      s.entered.val ≤ U32.max ∧
      s.entered.val - s.released.val = s.frames.val.length ∧
      s.frames.val.length ≤ ledger.MAX_ACTIVE_FRAMES.val := by
  have hv := (trace_valid_snapshot htr).1
  exact ⟨hv.released_le, by scalar_tac, hv.count_eq, hv.bounded⟩

/-- Every `enter_frame`/`leave_frame` step from a `LedgerTrace` state increases counters
monotonically without wrapping. -/
theorem trace_step_counters_monotone {root : ledger.RootToken} {epoch : ledger.MapEpoch}
    {s : ledger.Snapshot} (htr : LedgerTrace root epoch s) :
    (∀ ft tr, ledger.transition.enter_frame s root ft = Result.ok (core.result.Result.Ok tr) →
      s.entered.val ≤ tr.next.entered.val ∧ s.released.val ≤ tr.next.released.val ∧
      tr.next.entered.val ≤ U32.max) ∧
    (∀ ft tr, ledger.transition.leave_frame s root ft = Result.ok (core.result.Result.Ok tr) →
      s.entered.val ≤ tr.next.entered.val ∧ s.released.val ≤ tr.next.released.val ∧
      tr.next.entered.val ≤ U32.max) := by
  have hv := trace_valid_snapshot htr
  refine ⟨?_, ?_⟩
  · intro ft tr h
    have hc := (enter_frame_counter_nonwrapping s root ft hv).2.2 tr h
    have hrel : tr.next.released.val = s.released.val := by rw [hc.2.2.1]
    omega
  · intro ft tr h
    have hc := leave_frame_counter_nonwrapping s root ft tr h
    have hent : tr.next.entered.val = s.entered.val := by rw [hc.2.2.1]
    omega

/-! ## Property 5 (#9e): Correspondence to `OwnershipModel.lean` Pin Frames -/

/-- Mathlib-free abstract pin-frame state mirroring `OwnershipModel.lean` continuation `.pin`
frames (innermost-first stack) extended with `OWN-PO-6` retirement and session health. -/
structure PinState where
  pins : List Nat
  live : Bool
  healthy : Bool
  deriving DecidableEq

/-- Abstract pin-stack operations corresponding to `OwnershipModel.Step.{run, pin}` and
`OWN-PO-6` root retirement. -/
inductive PinOp where
  | pushPin (id : Nat)
  | popPin (id : Nat)
  | finish (exact : Bool)
  | abort (released : List Nat) (exact : Bool)
  deriving DecidableEq

/-- Abstract transition relation on `PinState`:
- `pushPin id` prepends `id` at the head of `pins` (matching `OwnershipModel.Step.run` prepending
  `.pin (.prog p)` at the continuation head).
- `popPin id` pops `id` from the head of `pins` (matching `OwnershipModel.Step.pin` popping `.pin v`
  from the continuation head).
- `finish exact` retires an empty active pin stack (`live := false`, `healthy := exact`).
- `abort released exact` drains all active pins in innermost-first order (`released = pins`),
  retires the root (`live := false`), and preserves `healthy = true` iff `exact = true ∧ pins = []`. -/
inductive PinStep : PinState → PinOp → PinState → Prop where
  | pushPin {st : PinState} {id : Nat} :
      st.live = true → st.healthy = true → 0 < id → (∀ x ∈ st.pins, x < id) →
      PinStep st (.pushPin id) { pins := id :: st.pins, live := true, healthy := true }
  | popPin {rest : List Nat} {id : Nat} :
      PinStep { pins := id :: rest, live := true, healthy := true } (.popPin id)
        { pins := rest, live := true, healthy := true }
  | finish {exact : Bool} :
      PinStep { pins := [], live := true, healthy := true } (.finish exact)
        { pins := [], live := false, healthy := exact }
  | abort {pins : List Nat} {exact : Bool} :
      PinStep { pins := pins, live := true, healthy := true }
        (.abort pins (exact && pins.isEmpty))
        { pins := [], live := false, healthy := exact && pins.isEmpty }

/-- Projection from an extracted `ledger.Snapshot` to `PinState`.
Because `enter_frame` pushes at the back and `leave_frame` pops from the back, `s.frames.val.reverse`
is the exact innermost-first continuation pin list matching `OwnershipModel.lean`. -/
def toPinState (s : ledger.Snapshot) : PinState :=
  { pins := s.frames.val.reverse.map (·.val)
    live := match s.status with | .Active => true | _ => false
    healthy := match s.session with | .Healthy => true | .Poisoned => false }

theorem toPinState_valid_flags {s : ledger.Snapshot} (h : ValidSnapshot s) :
    (toPinState s).live = true ∧ (toPinState s).healthy = true := by
  simp [toPinState, h.active, h.healthy]

/-- `Snapshot.pinned` initializes `toPinState s = { pins := [], live := true, healthy := true }`. -/
theorem pinned_refines_pin_model (root : ledger.RootToken) (epoch : ledger.MapEpoch)
    (s : ledger.Snapshot)
    (h : ledger.Snapshot.pinned root epoch = Result.ok (core.result.Result.Ok s)) :
    toPinState s = { pins := [], live := true, healthy := true } := by
  unfold ledger.Snapshot.pinned at h
  by_cases h0 : root = 0#u64
  · simp [h0] at h
  · simp only [h0, ite_false, Result.ok.injEq, core.result.Result.Ok.injEq] at h
    subst h
    simp [toPinState]

/-- `enter_frame` refines `PinStep.pushPin ft.val` (prepending `ft.val` at the innermost head). -/
theorem enter_frame_refines_pin_model (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (hvalid : ValidActiveRoot s claim) (hft : ft ≠ 0#u32)
    (hmono : ∀ last, s.frames.val.getLast? = some last → last.val < ft.val)
    (hcap : s.frames.val.length < ledger.MAX_ACTIVE_FRAMES.val)
    (hcnt : s.entered.val < U32.max) :
    ∃ tr, ledger.transition.enter_frame s claim ft = Result.ok (core.result.Result.Ok tr) ∧
      PinStep (toPinState s) (.pushPin ft.val) (toPinState tr.next) := by
  obtain ⟨tr, htr, hframes, hv, _, _, _, _, _, _, _, _, _⟩ :=
    enter_frame_pushes_lifo s claim ft hvalid hft hmono hcap hcnt
  refine ⟨tr, htr, ?_⟩
  have hst : toPinState s = { pins := s.frames.val.reverse.map (·.val), live := true, healthy := true } := by
    simp [toPinState, hvalid.1.active, hvalid.1.healthy]
  have hst' : toPinState tr.next =
      { pins := ft.val :: s.frames.val.reverse.map (·.val), live := true, healthy := true } := by
    simp [toPinState, hv.active, hv.healthy, hframes, List.reverse_append]
  rw [hst, hst']
  apply PinStep.pushPin rfl rfl (by scalar_tac)
  intro x hx
  simp only [List.mem_map, List.mem_reverse] at hx
  obtain ⟨y, hy, rfl⟩ := hx
  cases hl : s.frames.val.getLast? with
  | none =>
    have hem := List.getLast?_eq_none_iff.mp hl
    simp [hem] at hy
  | some last =>
    have hlt := hmono last hl
    have hle : y.val ≤ last.val :=
      strictlyIncreasing_mem_le_getLast hvalid.1.frames_strict hy hl
    omega

/-- `leave_frame` refines `PinStep.popPin ft.val` (popping `ft.val` from the innermost head). -/
theorem leave_frame_refines_pin_model (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (prefix_ : List ledger.FrameToken)
    (hvalid : ValidActiveRoot s claim) (hdisp : s.pending_dispatch = false)
    (hft : ft ≠ 0#u32) (hframes : s.frames.val = prefix_ ++ [ft]) :
    ∃ tr, ledger.transition.leave_frame s claim ft = Result.ok (core.result.Result.Ok tr) ∧
      PinStep (toPinState s) (.popPin ft.val) (toPinState tr.next) := by
  obtain ⟨tr, htr, hframes', hv, _, _, _, _, _, _, _, _, _⟩ :=
    leave_frame_pops_lifo s claim ft prefix_ hvalid hdisp hft hframes
  refine ⟨tr, htr, ?_⟩
  have hst : toPinState s =
      { pins := ft.val :: prefix_.reverse.map (·.val), live := true, healthy := true } := by
    simp [toPinState, hvalid.1.active, hvalid.1.healthy, hframes, List.reverse_append]
  have hst' : toPinState tr.next =
      { pins := prefix_.reverse.map (·.val), live := true, healthy := true } := by
    simp [toPinState, hv.active, hv.healthy, hframes']
  rw [hst, hst']
  exact PinStep.popPin

/-- `plan_finish_root` + `ack_finish_root` refines `PinStep.finish`. -/
theorem finish_root_refines_pin_model (s : ledger.Snapshot) (claim : ledger.RootToken)
    (plan : ledger.FinishPlan) (ack : ledger.FinishAck) (out : ledger.FinishOutcome)
    (hplan : ledger.transition.plan_finish_root s claim = Result.ok (core.result.Result.Ok plan))
    (hack : ledger.transition.ack_finish_root plan ack = Result.ok (core.result.Result.Ok out)) :
    ∃ exact : Bool,
      (exact = true ↔ out.exit_release = ledger.ExitRelease.Exact) ∧
      PinStep (toPinState s) (.finish exact) (toPinState out.next) := by
  have ⟨hvalid, _, _, _, _, _, _, _, hempty, _⟩ := plan_finish_root_balanced s claim plan hplan
  have hroot_nz : ack.root ≠ 0#u64 := by
    intro h0
    have hbad := (ack_finish_root_spec plan ack).1 h0
    rw [hack, Result.ok.injEq] at hbad; cases hbad
  have hroot_eq : ack.root = plan.root := by
    by_contra hne
    have hbad := (ack_finish_root_spec plan ack).2.1 hroot_nz hne
    rw [hack, Result.ok.injEq] at hbad; cases hbad
  obtain ⟨out', hack', _, _, hst, hfr, _, _, _, _, _, hdisp, hfail⟩ :=
    (ack_finish_root_spec plan ack).2.2 hroot_nz hroot_eq
  rw [hack, Result.ok.injEq, core.result.Result.Ok.injEq] at hack'
  subst hack'
  have hst_s : toPinState s = { pins := [], live := true, healthy := true } := by
    simp [toPinState, hvalid.1.active, hvalid.1.healthy, hempty]
  cases hex : out.exit_release with
  | Exact =>
    have hh := (hdisp.mpr hex).1
    have hst_out : toPinState out.next = { pins := [], live := false, healthy := true } := by
      simp [toPinState, hst, hh, hfr]
    refine ⟨true, by simp, ?_⟩
    rw [hst_s, hst_out]
    exact PinStep.finish
  | Unproven reason =>
    have hp := (hfail (by simp [hex])).1
    have hst_out : toPinState out.next = { pins := [], live := false, healthy := false } := by
      simp [toPinState, hst, hp, hfr]
    refine ⟨false, by simp, ?_⟩
    rw [hst_s, hst_out]
    exact PinStep.finish

/-- `plan_abort_root` + `ack_abort_root` refines `PinStep.abort` (draining active pins in exact
innermost-first LIFO order and poisoning unless exact release holds with empty frames). -/
theorem abort_root_refines_pin_model (s : ledger.Snapshot) (claim : ledger.RootToken)
    (pre : ledger.PreAbortObservation) (plan : ledger.AbortPlan) (ack : ledger.AbortAck)
    (out : ledger.AbortOutcome)
    (hvalid : ValidActiveRoot s claim)
    (hplan : ledger.transition.plan_abort_root s claim pre = Result.ok (core.result.Result.Ok plan))
    (hack : ledger.transition.ack_abort_root plan ack = Result.ok (core.result.Result.Ok out)) :
    ∃ exact : Bool,
      (exact && (toPinState s).pins.isEmpty = true ↔ out.exit_release = ledger.ExitRelease.Exact) ∧
      out.released_frames.val.map (·.val) = (toPinState s).pins ∧
      PinStep (toPinState s) (.abort (out.released_frames.val.map (·.val))
        (exact && (toPinState s).pins.isEmpty)) (toPinState out.next) := by
  have ⟨hst, hfr, _, _, _, hrev, hexact, hdisp, hfail⟩ :=
    ack_abort_root_exact_iff s claim pre plan ack out hvalid hplan hack
  have hst_s : toPinState s =
      { pins := s.frames.val.reverse.map (·.val), live := true, healthy := true } := by
    simp [toPinState, hvalid.1.active, hvalid.1.healthy]
  have hrel_eq : out.released_frames.val.map (·.val) = (toPinState s).pins := by
    rw [hrev, hst_s]
  cases hex : out.exit_release with
  | Exact =>
    have hempty : s.frames.val = [] := (hexact.mp hex).2.1
    have hh := (hdisp.mpr hex).1
    have hpins_empty : (toPinState s).pins = [] := by simp [hst_s, hempty]
    have hst_out : toPinState out.next = { pins := [], live := false, healthy := true } := by
      simp [toPinState, hst, hh, hfr]
    have hfem : s.frames.val.isEmpty = true := by simp [hempty]
    refine ⟨true, by simp [hpins_empty], hrel_eq, ?_⟩
    rw [hrel_eq, hst_s, hst_out]
    have hstep := @PinStep.abort (s.frames.val.reverse.map (·.val)) true
    simp only [Bool.true_and, List.isEmpty_map, List.isEmpty_reverse, hfem] at hstep
    simpa [hfem] using hstep
  | Unproven reason =>
    have hp := (hfail (by simp [hex])).1
    have hst_out : toPinState out.next = { pins := [], live := false, healthy := false } := by
      simp [toPinState, hst, hp, hfr]
    refine ⟨false, by simp, hrel_eq, ?_⟩
    rw [hrel_eq, hst_s, hst_out]
    simp only [Bool.false_and]
    exact @PinStep.abort (s.frames.val.reverse.map (·.val)) false

/-- No resurrection after retirement or poisoning: whenever `(toPinState s).live = false` or
`(toPinState s).healthy = false`, every active-root transition (`enter_frame`, `leave_frame`,
`plan_finish_root`, `plan_abort_root`) refuses with `Result.ok (.Err _)`. -/
theorem retired_or_poisoned_refuses_all (s : ledger.Snapshot) (claim : ledger.RootToken)
    (ft : ledger.FrameToken) (pre : ledger.PreAbortObservation)
    (hclosed : (toPinState s).live = false ∨ (toPinState s).healthy = false) :
    (∃ e, ledger.transition.enter_frame s claim ft = Result.ok (core.result.Result.Err e)) ∧
    (∃ e, ledger.transition.leave_frame s claim ft = Result.ok (core.result.Result.Err e)) ∧
    (∃ e, ledger.transition.plan_finish_root s claim = Result.ok (core.result.Result.Err e)) ∧
    (∃ e, ledger.transition.plan_abort_root s claim pre = Result.ok (core.result.Result.Err e)) := by
  have hval_err : ∃ e, ledger.transition.validate_active_root s claim =
      Result.ok (core.result.Result.Err e) := by
    obtain ⟨r, hr⟩ := validate_active_root_total s claim
    cases r with
    | Err e => exact ⟨e, hr⟩
    | Ok active =>
      have hvalid := (validate_active_root_ok_iff s claim active).mp hr
      have hflags := toPinState_valid_flags hvalid.1.1
      rcases hclosed with hdead | hpois
      · rw [hflags.1] at hdead; cases hdead
      · rw [hflags.2] at hpois; cases hpois
  obtain ⟨e, he⟩ := hval_err
  refine ⟨⟨e, ?_⟩, ⟨e, ?_⟩, ⟨e, ?_⟩, ⟨e, ?_⟩⟩
  · simp [ledger.transition.enter_frame, he]
  · simp [ledger.transition.leave_frame, he]
  · simp [ledger.transition.plan_finish_root, he]
  · simp [ledger.transition.plan_abort_root, he]

end NobleM3.LedgerRefinement
