/-
Bounded solver and witness-encoder correspondence for pure I64/Bool graph pairs
(issue #56, phase 2).

Phase 1 (`SymbolicPair`) proved what a *declared finite* search can and cannot
establish. This file adds the two things it left open: an encoder/decoder
correspondence for the witness representation, and a *first-match* bounded search.

The earliest-witness property is stated structurally: a reported index comes with
a decomposition of the declared universe into a prefix of candidates that do **not**
refute, of that exact length, followed by a candidate that does. So the reported
witness can never be a later, avoidable one, and the statement never relies on
positional lookup into the candidate list.

Nothing here is a SAT/SMT explorer, a general symbolic executor, or a claim about
`crates/noble-ngraph/src/extract/symbolic.rs`, the Wasm legs or any emitted
artifact. Guards, input facts, `F32`, heap/resource/effect reasoning, recursion and
live slots remain outside the slice, as in phase 1.
-/
import NobleFoundation.SymbolicPair

namespace NobleFoundation.SymbolicSolver

open NobleFoundation.PackedSchedule
open NobleFoundation.SymbolicPair

/-! ### Encoder correspondence

A witness records a *declared slot list*, in order, with the candidate's exact
bits. Decoding returns exactly those bits at those slots, so an encoder cannot
smuggle a value the candidate did not have. -/

/-- Encode a candidate's exact bits at each declared slot, in the declared order. -/
def encode (slots : List Nat) (inputs : Nat → BitVec 64) : List (Nat × BitVec 64) :=
  slots.map (fun slot => (slot, inputs slot))

/-- The encoder records exactly one pair per declared slot. -/
theorem encode_length (slots : List Nat) (inputs : Nat → BitVec 64) :
    (encode slots inputs).length = slots.length := by
  simp [encode]

/-- Every declared slot decodes back to the candidate's own bits. -/
theorem decode_encode_replays (slots : List Nat) (inputs : Nat → BitVec 64)
    (fallback : BitVec 64) (slot : Nat) (hmem : slot ∈ slots) :
    decodeInputs (encode slots inputs) fallback slot = inputs slot := by
  have hlookup : lookup (encode slots inputs) slot = some (inputs slot) := by
    unfold encode
    induction slots with
    | nil => simp at hmem
    | cons head rest ih =>
        simp only [List.map_cons]
        by_cases hhead : head = slot
        · rw [lookup, if_pos hhead, hhead]
        · simp only [lookup, hhead]
          exact ih (List.mem_of_ne_of_mem (fun h => hhead h.symm) hmem)
  simp [decodeInputs, hlookup]

/-- A corrupted witness is a different assignment at the corrupted slot: the
decoder returns the corrupted bits and not the candidate's. -/
theorem corrupted_witness_shifts_bits (slot : Nat) (value shifted : BitVec 64)
    (fallback : BitVec 64) (hne : shifted ≠ value) :
    decodeInputs [(slot, shifted)] fallback slot = shifted ∧
      decodeInputs [(slot, shifted)] fallback slot ≠ value := by
  simp [decodeInputs, lookup, hne]

/-! ### First-match bounded search

`firstIndex` walks the declared universe in declared order and reports the index of
the first refuting candidate. -/

/-- Index of the first refuting candidate in the declared universe. -/
def firstIndex (pair : Pair) : List (Nat → BitVec 64) → Option Nat
  | [] => none
  | candidate :: rest =>
      if refuted pair candidate = true then
        some 0
      else
        (firstIndex pair rest).map (fun index => index + 1)

/-- A failed search establishes agreement on the declared universe, and nothing
beyond it. -/
theorem firstIndex_none_agrees (pair : Pair) (candidates : List (Nat → BitVec 64))
    (hsearch : firstIndex pair candidates = none) :
    ∀ inputs ∈ candidates, agreesOn pair inputs := by
  induction candidates with
  | nil => intro inputs hmem; simp at hmem
  | cons candidate rest ih =>
      intro inputs hmem
      simp only [firstIndex] at hsearch
      by_cases hhead : refuted pair candidate = true
      · rw [if_pos hhead] at hsearch
        simp at hsearch
      · rw [if_neg hhead] at hsearch
        cases hrest : firstIndex pair rest with
        | none =>
            rcases List.mem_cons.mp hmem with hsame | hlater
            · rw [hsame]
              cases hbool : refuted pair candidate with
              | false => exact (not_refuted_iff_agrees pair candidate).mp hbool
              | true => exact absurd hbool hhead
            · exact ih hrest inputs hlater
        | some found =>
            rw [hrest] at hsearch
            simp at hsearch

/-- A reported index comes with the *earliest* structural decomposition: the
declared universe is a prefix of candidates that do not refute, of exactly the
reported length, followed by a candidate that does. -/
theorem firstIndex_some_decomposes (pair : Pair) (candidates : List (Nat → BitVec 64))
    (index : Nat) (hsearch : firstIndex pair candidates = some index) :
    ∃ (earlier : List (Nat → BitVec 64)) (candidate : Nat → BitVec 64)
      (rest : List (Nat → BitVec 64)),
      candidates = earlier ++ candidate :: rest ∧
        earlier.length = index ∧
        (∀ inputs ∈ earlier, agreesOn pair inputs) ∧
        refuted pair candidate = true := by
  induction candidates generalizing index with
  | nil => simp [firstIndex] at hsearch
  | cons head tail ih =>
      simp only [firstIndex] at hsearch
      by_cases hhead : refuted pair head = true
      · rw [if_pos hhead] at hsearch
        have hzero : index = 0 := by
          cases index with
          | zero => rfl
          | succ k => simp at hsearch
        subst hzero
        exact ⟨[], head, tail, by simp, by simp, by simp, hhead⟩
      · rw [if_neg hhead] at hsearch
        cases hrest : firstIndex pair tail with
        | none => rw [hrest] at hsearch; simp at hsearch
        | some found =>
            rw [hrest] at hsearch
            have hindex : index = found + 1 := by
              cases index with
              | zero => simp at hsearch
              | succ k =>
                  have hk : found = k := by
                    have hsome := Option.some.inj hsearch
                    exact Nat.succ.inj hsome
                  rw [← hk]
            subst hindex
            obtain ⟨earlier, candidate, rest, hsplit, hlen, hprefix, hrefutes⟩ :=
              ih found hrest
            refine ⟨head :: earlier, candidate, rest, ?_, ?_, ?_, hrefutes⟩
            · rw [hsplit]
              simp
            · simp [hlen]
            · intro inputs hmem
              rcases List.mem_cons.mp hmem with hsame | hlater
              · rw [hsame]
                cases hbool : refuted pair head with
                | false => exact (not_refuted_iff_agrees pair head).mp hbool
                | true => exact absurd hbool hhead
              · exact hprefix inputs hlater

/-! ### Fixtures

The three phase-1 fixture pairs, run through the first-match search over the same
declared two-candidate universe. -/

/-- The same-sum pair has no refuting candidate in the declared universe. -/
theorem same_sum_first_none : firstIndex agreePair candidates = none := by
  decide

/-- The subtraction pair is refuted at the earliest declared candidate, index zero. -/
theorem subtraction_first_index_zero : firstIndex differPair candidates = some 0 := by
  decide

/-- The input-dependent pair is refuted at the earliest declared candidate: the
input face computes `0 + 1` and the other face is the exact constant seven. -/
theorem input_first_index_zero : firstIndex inputPair candidates = some 0 := by
  decide

/-- The declared candidate at index zero really refutes the input-dependent pair,
which is why the search reports it rather than a later candidate. -/
theorem input_zero_candidate_refutes : refuted inputPair (fun _ => 0) = true := by
  decide

/-- Encoded witnesses replay their candidate and a corrupted one does not, on the
same input-dependent pair: `(0, 6)` leaves the face at `7` and agrees, `(0, 3)`
reaches `4` and refutes. -/
theorem encoded_witness_decides :
    refuted inputPair (decodeInputs (encode [0] (fun _ => 6)) 0) = false ∧
      refuted inputPair (decodeInputs (encode [0] (fun _ => 3)) 0) = true := by
  decide

end NobleFoundation.SymbolicSolver
