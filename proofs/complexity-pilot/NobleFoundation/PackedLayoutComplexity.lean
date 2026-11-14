/-
Complexitylib proof substeps for the encoded packed-layout model (issue #80).

The first parser component has a machine-grounded polynomial-time certificate. This is not a
certificate for `validateEncoded`; the remaining decoder, validation composition and any Rust
operational refinement are tracked separately in the pilot decision.
-/
import CslibFrontier.Tactic.PolyTime
import NobleFoundation.PackedLayoutEncoded

open Turing.MultiTapeTM
open NobleFoundation.PackedLayoutEncoded

namespace NobleFoundation.PackedLayoutComplexity

def unaryPrefixOutput : (Nat × List Bool) ↪ List Bool := encPair encNat encBits

/-- Read a unary prefix by finding its first terminator and dropping the prefix in one list scan. -/
def decodeUnaryPrefixFast (bits : List Bool) : Option (Nat × List Bool) :=
  let width := bits.idxOf false
  if width = bits.length then none else some (width, bits.drop (width + 1))

/-- The terminator search is a polynomial-time Turing computation on raw bit lists. -/
theorem decodeUnaryPrefixFast_computable_in_poly_time :
    ComputableInPolyTime decodeUnaryPrefixFast encBits (encOption unaryPrefixOutput) := by
  unfold decodeUnaryPrefixFast
  polytime

/-- The list-scan implementation agrees with the model's recursive unary parser on every input,
including unterminated strings. -/
theorem decodeUnaryPrefixFast_correct (bits : List Bool) :
    decodeUnaryPrefix bits = decodeUnaryPrefixFast bits := by
  induction bits with
  | nil => simp [decodeUnaryPrefix, decodeUnaryPrefixFast]
  | cons bit rest ih =>
    cases bit with
    | false => simp [decodeUnaryPrefix, decodeUnaryPrefixFast]
    | true =>
      by_cases h : rest.idxOf false = rest.length <;>
        simp [decodeUnaryPrefix, decodeUnaryPrefixFast, ih, h]

/-- The original unary parser is therefore machine-grounded polynomial-time. -/
theorem decodeUnaryPrefix_computable_in_poly_time :
    ComputableInPolyTime decodeUnaryPrefix encBits (encOption unaryPrefixOutput) := by
  rw [show decodeUnaryPrefix = decodeUnaryPrefixFast by
    funext bits
    exact decodeUnaryPrefixFast_correct bits]
  exact decodeUnaryPrefixFast_computable_in_poly_time

end NobleFoundation.PackedLayoutComplexity
