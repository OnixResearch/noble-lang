import NobleFoundation.Instr

/-!
# Transcribed I64 runtime helpers versus MC1 (issue #15, Phase 3, staged)

This is a Lean *transcription* of the managed-linear-memory runtime helpers that
implement words 4/5/6. It is not a decoding of the Wasm bytes. The transcribed
WAT functions are:
- `$op_add`, `$op_sub`, `$op_mul` (`crates/noble-wasm/runtime/operations.wat`);
- `$need`, `$slot_kind`, `$push_tag`, `$push_i64`, `$take`, `$pop_kind`
  (`stack.wat`);
- `$fail` (`accounting.wat`).

`verification/foundation/check.mjs` pins the exact text of those functions.
A changed helper therefore invalidates this model instead of silently
diverging from it.

What is modelled:
- the operand slots, top first (slot `sp-1` at the head), with the slot tag
  and the `i64` payload. Tag 1 is I64 (`$push_i64`);
- the first-failure code (`$fail` keeps the first nonzero code);
- the `$operand_limit` quota check of `$push_tag`.

What is not modelled:
- byte addresses, zeroing of popped slots and peak accounting;
- the quota *code*, which depends on the global `$phase`. Any nonzero code
  is allowed.

What is proved: on a well-formed slot stack, the transcribed helper refines
MC1 `Step` for words 4/5/6, including operand order, the untouched remainder
and failure on a non-I64 operand. Lowering, the assembler, the optimizer and
the engine stay outside (see `proofs/foundation/README.md`, Phase 3 matrix).
-/

namespace NobleFoundation

open NobleContracts

/-- One 16-byte operand slot: `tag:i32, padding:i32, payload:i64`. -/
structure Slot where
  tag : Nat
  payload : BitVec 64
  deriving DecidableEq, Repr

/-- The helper-visible machine state. -/
structure WState where
  slots : List Slot
  failure : Nat
  deriving DecidableEq, Repr

/-- `$fail`: keep the first failure code. -/
def wFail (code : Nat) (s : WState) : WState :=
  if s.failure = 0 then { s with failure := code } else s

/-- `$pop_kind expected`: `$need 1`, the `$slot_kind` check, then `$take`. -/
def popKind (expected : Nat) (s : WState) : BitVec 64 × WState :=
  if s.failure ≠ 0 then (0, s) else
  match s.slots with
  | [] => (0, wFail 4 s)
  | top :: rest => if top.tag ≠ expected then (0, wFail 4 s) else (top.payload, { s with slots := rest })

/-- `$push_tag kind value` with operand limit `limit` (bytes) and an arbitrary
nonzero quota code `quota`. -/
def pushTag (limit quota : Nat) (kind : Nat) (value : BitVec 64) (s : WState) : WState :=
  if s.failure ≠ 0 then s else
  if (s.slots.length + 1) * 16 > limit then wFail quota s else
  { s with slots := ⟨kind, value⟩ :: s.slots }

/-- `$op_add`/`$op_sub`/`$op_mul`: pop `$right` (the top) first, then `$left`,
push `f left right` as I64. -/
def opBinary (limit quota : Nat) (f : BitVec 64 → BitVec 64 → BitVec 64) (s : WState) : WState :=
  let (right, s1) := popKind 1 s
  let (left, s2) := popKind 1 s1
  pushTag limit quota 1 (f left right) s2

/-- The helper selected by `crates/noble-wasm/src/operations.rs` for a word. -/
def helper (limit quota : Nat) : Nat → Option (WState → WState)
  | 4 => some (opBinary limit quota (· + ·))
  | 5 => some (opBinary limit quota (· - ·))
  | 6 => some (opBinary limit quota (· * ·))
  | _ => none

/-- Layout relation between an internal MC1 stack and the slot stack. I64
values occupy tag-1 slots carrying their bits. Every other value occupies a
slot whose tag is not 1. The arithmetic helpers never read those payloads, so
their encoding is left abstract. -/
inductive SlotsRel : Stack → List Slot → Prop where
  | nil : SlotsRel [] []
  | i64 {x vs ss} : SlotsRel vs ss → SlotsRel (.i64 x :: vs) (⟨1, x⟩ :: ss)
  | other {v vs ss} (tag : Nat) (payload : BitVec 64) :
      (∀ x, v ≠ .i64 x) → tag ≠ 1 → SlotsRel vs ss → SlotsRel (v :: vs) (⟨tag, payload⟩ :: ss)

theorem SlotsRel.length_eq {vs ss} (h : SlotsRel vs ss) : vs.length = ss.length := by
  induction h <;> simp_all

/-- **Helper refinement (normal case).** On a non-failed state whose slots
encode the MC1 stack and fit the operand limit, each arithmetic helper
produces exactly the slots of MC1's unique successor stack. The remainder is
untouched, and the payload is `left ∘ right` with `left` the deeper operand. -/
theorem helper_refines_step {limit quota w : Nat} {f : WState → WState}
    (hw : helper limit quota w = some f)
    {a b : BitVec 64} {vs : Stack} {ss : List Slot} (hrel : SlotsRel vs ss)
    (hlim : (ss.length + 2) * 16 ≤ limit) :
    ∃ t, Step (.word w) (.i64 b :: .i64 a :: vs) t ∧
      (∀ t', Step (.word w) (.i64 b :: .i64 a :: vs) t' → t' = t) ∧
      ∃ ss', f ⟨⟨1, b⟩ :: ⟨1, a⟩ :: ss, 0⟩ = ⟨ss', 0⟩ ∧ SlotsRel t ss' := by
  have hfit : ¬ (ss.length + 1) * 16 > limit := by omega
  match w, hw with
  | 4, hw =>
      cases hw
      refine ⟨_, .add, fun t' h => (Step.add_iff.mp h), _, ?_, .i64 hrel⟩
      simp [opBinary, popKind, pushTag, hfit]
  | 5, hw =>
      cases hw
      refine ⟨_, .sub, fun t' h => (Step.sub_iff.mp h), _, ?_, .i64 hrel⟩
      simp [opBinary, popKind, pushTag, hfit]
  | 6, hw =>
      cases hw
      refine ⟨_, .mul, fun t' h => (Step.mul_iff.mp h), _, ?_, .i64 hrel⟩
      simp [opBinary, popKind, pushTag, hfit]

/-- **Helper refinement (stuck case).** If either of the top two MC1 values is
not an I64, MC1 has no transition, and the helper records failure code 4 on a
non-failed state. Ill-typed operands are refused, never coerced. -/
theorem helper_fails_when_stuck {limit quota w : Nat} {f : WState → WState}
    (hw : helper limit quota w = some f)
    {u v : Value} {vs : Stack} {su sv : Slot} {ss : List Slot}
    (hrel : SlotsRel (v :: u :: vs) (sv :: su :: ss))
    (hbad : (∀ x, v ≠ .i64 x) ∨ (∀ x, u ≠ .i64 x)) :
    (∀ t, ¬ Step (.word w) (v :: u :: vs) t) ∧
      (f ⟨sv :: su :: ss, 0⟩).failure = 4 := by
  have hw' : w = 4 ∨ w = 5 ∨ w = 6 := by
    match w, hw with
    | 4, _ => exact Or.inl rfl
    | 5, _ => exact Or.inr (Or.inl rfl)
    | 6, _ => exact Or.inr (Or.inr rfl)
  constructor
  · intro t h
    rcases hw' with rfl | rfl | rfl <;> cases h <;> rcases hbad with h | h <;> exact h _ rfl
  · cases hrel with
    | i64 hrest =>
        cases hrest with
        | i64 _ =>
            exfalso; rcases hbad with h | h <;> exact h _ rfl
        | other tag payload _ htag _ =>
            match w, hw with
            | 4, hw | 5, hw | 6, hw =>
                cases hw; simp [opBinary, popKind, pushTag, wFail, htag]
    | other tag payload _ htag _ =>
        match w, hw with
        | 4, hw | 5, hw | 6, hw =>
            cases hw; simp [opBinary, popKind, pushTag, wFail, htag]

/-- Mutation control: a helper that popped `left` first (operands swapped)
would disagree with MC1 subtraction on `3 8 -`. The pop order is load-bearing. -/
def opBinarySwapped (limit quota : Nat) (f : BitVec 64 → BitVec 64 → BitVec 64)
    (s : WState) : WState :=
  let (left, s1) := popKind 1 s
  let (right, s2) := popKind 1 s1
  pushTag limit quota 1 (f left right) s2

theorem swapped_helper_refuted :
    opBinary 64 1 (· - ·) ⟨[⟨1, 8⟩, ⟨1, 3⟩], 0⟩ = ⟨[⟨1, BitVec.ofInt 64 (-5)⟩], 0⟩ ∧
    opBinarySwapped 64 1 (· - ·) ⟨[⟨1, 8⟩, ⟨1, 3⟩], 0⟩ = ⟨[⟨1, 5⟩], 0⟩ ∧
    BitVec.ofInt 64 (-5) ≠ 5 := by
  refine ⟨by decide, by decide, by decide⟩

end NobleFoundation
