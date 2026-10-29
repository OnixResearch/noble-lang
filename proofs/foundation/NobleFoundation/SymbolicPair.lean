/-
Bounded query model for fixed pure I64/Bool graph pairs (issue #56, phase 1).

Search is untrusted here in the same way the scheduler is untrusted elsewhere:
this file models a *declared finite input universe*, an exhaustive search over it,
and the replay of an exact-bit witness, and proves what such a search can and
cannot establish. It is not a SAT/SMT path explorer, not a general symbolic
executor, and no solver is involved in phase 1.

Contents: witness decoding that only ever returns recorded pairs (never invented
bits), exhaustive-search agreement over the declared universe, witness refutation,
and a corrupted-witness control. Equivalence and inequivalence fixtures use the
same graph denotation as `PackedSchedule`, and `GraphRewrite`'s lifting interface is
untouched.

Guards, input-fact assumptions, `F32`, heap/resource/effect reasoning, live slots,
recursion, dynamic calls and interaction-net state are outside the slice. Nothing
here is a claim about `crates/noble-ngraph/src/extract/symbolic.rs`, the B2 I64
harness legs or any emitted Wasm.
-/
import NobleFoundation.GraphRewrite

namespace NobleFoundation.SymbolicPair

open NobleFoundation.PackedSchedule

/-! ### Witness encoding

A witness is a recorded association list of exact 64-bit input values plus an
explicit fallback for slots it does not mention. Decoding is proved to return
only recorded pairs, so a witness cannot acquire bits it did not carry. -/

/-- First-match lookup in a recorded assignment. -/
def lookup (assign : List (Nat × BitVec 64)) (slot : Nat) : Option (BitVec 64) :=
  match assign with
  | [] => none
  | pair :: rest => if pair.1 = slot then some pair.2 else lookup rest slot

/-- Decode a witness into the graph's input function. Unrecorded slots take the
declared fallback. -/
def decodeInputs (assign : List (Nat × BitVec 64)) (fallback : BitVec 64) :
    Nat → BitVec 64 :=
  fun slot =>
    match lookup assign slot with
    | some value => value
    | none => fallback

/-- A witness only ever yields a pair it recorded. -/
theorem lookup_returns_recorded (assign : List (Nat × BitVec 64)) (slot : Nat)
    (value : BitVec 64) (hlookup : lookup assign slot = some value) :
    (slot, value) ∈ assign := by
  induction assign with
  | nil => simp [lookup] at hlookup
  | cons pair rest ih =>
      by_cases hhit : pair.1 = slot
      · rw [lookup, if_pos hhit] at hlookup
        have hval : pair.2 = value := Option.some.inj hlookup
        rw [← hval, ← hhit]
        exact List.mem_cons_self
      · simp only [lookup, hhit] at hlookup
        exact List.mem_cons_of_mem _ (ih hlookup)

/-- An unrecorded slot takes exactly the declared fallback. -/
theorem decode_uses_fallback (assign : List (Nat × BitVec 64)) (fallback : BitVec 64)
    (slot : Nat) (hlookup : lookup assign slot = none) :
    decodeInputs assign fallback slot = fallback := by
  simp [decodeInputs, hlookup]

/-- A recorded slot replays exactly its recorded bits. -/
theorem decode_replays_recorded (assign : List (Nat × BitVec 64)) (fallback : BitVec 64)
    (slot : Nat) (value : BitVec 64) (hlookup : lookup assign slot = some value) :
    decodeInputs assign fallback slot = value := by
  simp [decodeInputs, hlookup]

/-! ### Ordered outputs and differences -/

/-- A fixed pair of graphs with identical arity and an ordered output list. -/
structure Pair where
  left : Graph
  right : Graph
  outputs : List Nat

/-- Complete ordered results of the left graph for one input assignment. -/
def replayLeft (pair : Pair) (inputs : Nat → BitVec 64) : List (Option Val) :=
  pair.outputs.map (value pair.left inputs)

/-- Complete ordered results of the right graph for one input assignment. -/
def replayRight (pair : Pair) (inputs : Nat → BitVec 64) : List (Option Val) :=
  pair.outputs.map (value pair.right inputs)

/-- Refutation: some ordered output's results differ under one assignment. -/
def refuted (pair : Pair) (inputs : Nat → BitVec 64) : Bool :=
  pair.outputs.any (fun out => decide (value pair.left inputs out ≠ value pair.right inputs out))

/-- Agreement: every ordered output's results coincide under one assignment. -/
def agreesOn (pair : Pair) (inputs : Nat → BitVec 64) : Prop :=
  ∀ out ∈ pair.outputs, value pair.left inputs out = value pair.right inputs out

/-- No refutation is exactly agreement over the ordered outputs. -/
theorem not_refuted_iff_agrees (pair : Pair) (inputs : Nat → BitVec 64) :
    refuted pair inputs = false ↔ agreesOn pair inputs := by
  unfold refuted agreesOn
  rw [List.any_eq_false]
  constructor
  · intro h out hmem
    have hnot : ¬ (value pair.left inputs out ≠ value pair.right inputs out) := by
      intro hne
      exact h out hmem (by simp [hne])
    by_cases hbool : value pair.left inputs out = value pair.right inputs out
    · exact hbool
    · exact absurd hbool hnot
  · intro h out hmem hdec
    have hpair : value pair.left inputs out = value pair.right inputs out := h out hmem
    rw [hpair] at hdec
    simp at hdec

/-! ### Bounded exhaustive search

The declared universe is an explicit finite list of input assignments. Nothing is
claimed about assignments outside it, and the search is not a solver: agreement for
every candidate is agreement *on the declared universe* only. -/

/-- First assignment in the declared universe that refutes the pair, if any. -/
def search (pair : Pair) (candidates : List (Nat → BitVec 64)) :
    Option (Nat → BitVec 64) :=
  candidates.find? (fun inputs => refuted pair inputs)

/-- A failed search establishes agreement on the declared universe, and nothing
beyond it. -/
theorem search_none_sound (pair : Pair) (candidates : List (Nat → BitVec 64))
    (hsearch : search pair candidates = none) :
    ∀ inputs ∈ candidates, agreesOn pair inputs := by
  unfold search at hsearch
  rw [List.find?_eq_none] at hsearch
  intro inputs hmem
  have hnot : ¬ refuted pair inputs = true := hsearch inputs hmem
  cases hbool : refuted pair inputs with
  | false => exact (not_refuted_iff_agrees pair inputs).mp hbool
  | true => exact absurd hbool hnot

/-- A found candidate is a genuine refutation of the ordered results. -/
theorem search_some_refutes (pair : Pair) (candidates : List (Nat → BitVec 64))
    (witness : Nat → BitVec 64) (hsearch : search pair candidates = some witness) :
    ¬ agreesOn pair witness := by
  unfold search at hsearch
  have hrefuted : refuted pair witness = true := List.find?_some hsearch
  intro hagrees
  rw [← not_refuted_iff_agrees] at hagrees
  rw [hagrees] at hrefuted
  simp at hrefuted

/-! ### Fixtures and controls

Two faces of the same constant expression differ only in how they are written; a
subtraction face resists the addition; and one face reads an input slot, so the
decoded witness bits decide the outcome. -/

/-- `3 + 5` written as `add 0 1` over constants in one order. -/
def agreeLeft : Graph where
  arity := 3
  output := 2
  node := fun index =>
    if index = 0 then .constant 3
    else if index = 1 then .constant 5
    else .add 0 1

/-- The same sum written as `add 1 0` over constants in the other order. -/
def agreeRight : Graph where
  arity := 3
  output := 2
  node := fun index =>
    if index = 0 then .constant 5
    else if index = 1 then .constant 3
    else .add 1 0

/-- The addition's result. -/
def agreePair : Pair where
  left := agreeLeft
  right := agreeRight
  outputs := [2]

/-- A subtraction face on the same operand order. -/
def differRight : Graph where
  arity := 3
  output := 2
  node := fun index =>
    if index = 0 then .constant 5
    else if index = 1 then .constant 3
    else .sub 1 0

/-- The pair whose right face subtracts: the ordered results differ. -/
def differPair : Pair where
  left := agreeLeft
  right := differRight
  outputs := [2]

/-- One face reads input slot zero; the other is the exact constant seven. -/
def inputLeft : Graph where
  arity := 3
  output := 2
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .constant 1
    else .add 0 1

def inputRight : Graph where
  arity := 3
  output := 2
  node := fun _ => .constant 7

def inputPair : Pair where
  left := inputLeft
  right := inputRight
  outputs := [2]

/-- Two declared candidates. Both graphs above are insensitive to them except the
input-dependent pair, whose witness carries the bits. -/
def candidates : List (Nat → BitVec 64) := [fun _ => 0, fun _ => 7]

/-- `search` on the same-sum pair finds nothing in the declared universe. -/
theorem same_sum_search_none : search agreePair candidates = none := by
  decide

/-- Consequently the pair agrees on every declared candidate, and nothing beyond
them is claimed. -/
theorem same_sum_agrees (inputs : Nat → BitVec 64)
    (hmem : inputs ∈ candidates) : agreesOn agreePair inputs :=
  search_none_sound agreePair candidates same_sum_search_none inputs hmem

/-- The subtraction pair is genuinely refuted at one declared candidate: the left face
computes `3 + 5 = 8`, while the right face's `sub 1 0` takes node 0 (`5`) from node 1 (`3`), so
`3 - 5` wraps to `2^64 - 2`. -/
theorem subtraction_refuted : ¬ agreesOn differPair (fun _ => 0) := by
  intro hagrees
  have hnot : refuted differPair (fun _ => 0) = false :=
    (not_refuted_iff_agrees differPair (fun _ => 0)).mpr hagrees
  have htrue : refuted differPair (fun _ => 0) = true := by
    decide
  rw [htrue] at hnot
  simp at hnot

/-- The exact decoded bits decide the outcome: the witness `(0, 6)` leaves the
input face at `7` and agrees, while `(0, 3)` reaches `4` and refutes. -/
theorem witness_bits_decide :
    decodeInputs [(0, 6)] 0 0 = 6 ∧
      refuted inputPair (decodeInputs [(0, 6)] 0) = false ∧
      refuted inputPair (decodeInputs [(0, 3)] 0) = true := by
  decide

/-- An unrecorded slot takes the declared fallback rather than inventing bits. -/
theorem unrecorded_slot_falls_back :
    decodeInputs [(0, 6)] 11 1 = 11 ∧ lookup [(0, 6)] 1 = none := by
  decide

/-- A corrupted witness is a different assignment, and it can flip the verdict. -/
theorem corrupted_witness_changes_the_verdict :
    refuted inputPair (decodeInputs [(0, 6)] 0) ≠
      refuted inputPair (decodeInputs [(0, 3)] 0) := by
  decide

end NobleFoundation.SymbolicPair
