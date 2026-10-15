import NobleFoundation.Sequence
import NobleFoundation.Literal

/-!
# Kernel-checked regressions (issue #15, Phase 1)

Concrete instances of the universal theorems.  Every reference result is
computed by `decide` from the oracle's `MathlibI64.wrap`, never typed in as an
expected bit pattern of the implementation.
-/

namespace NobleFoundation

open NobleContracts

/-- Max/min/zero/negative boundaries of the oracle, through the MC1 bridge. -/
theorem oracle_boundaries :
    MathlibI64.wrap (9223372036854775807 + 1) = -9223372036854775808 ∧
    MathlibI64.wrap (-9223372036854775808 - 1) = 9223372036854775807 ∧
    MathlibI64.wrap (-9223372036854775808 * -1) = -9223372036854775808 ∧
    MathlibI64.wrap (9223372036854775807 * 2) = -2 ∧
    MathlibI64.wrap (-9223372036854775808 + -1) = 9223372036854775807 ∧
    MathlibI64.wrap (9223372036854775807 - -1) = -9223372036854775808 ∧
    MathlibI64.wrap (0 + 0) = 0 ∧
    MathlibI64.wrap (-9223372036854775808 * 0) = 0 ∧
    MathlibI64.wrap (-7 + -13) = -20 ∧
    MathlibI64.wrap (-7 - -13) = 6 ∧
    MathlibI64.wrap (-7 * -13) = 91 := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩ <;> decide

/-- Subtraction order: `3 8 -` is `-5`; the swapped program gives `5`; and the
two differ, so an implementation that swapped operands is refuted. -/
theorem sub_operand_order (pre t : Stack) :
    (Exec (lowerAll [.push 3, .push 8, .sub]) pre t ↔ t = pre ++ [i64v (-5)]) ∧
    (Exec (lowerAll [.push 8, .push 3, .sub]) pre t ↔ t = pre ++ [i64v 5]) ∧
    i64v (-5) ≠ i64v 5 := by
  refine ⟨?_, ?_, by simp only [i64v, ne_eq, Value.i64.injEq]; decide⟩
  · have h : refRun [.push 3, .push 8, .sub] [] = some [-5] := by decide
    simpa [enc] using exec_lowerAll h pre t
  · have h : refRun [.push 8, .push 3, .sub] [] = some [5] := by decide
    simpa [enc] using exec_lowerAll h pre t

/-- Wrapping addition at the maximum, with an untouched non-I64 prefix. -/
theorem add_max_wraps_preserving_prefix (t : Stack) :
    Exec (lowerAll [.push 9223372036854775807, .push 1, .add])
        [.bool true, .text "kept"] t ↔
      t = [.bool true, .text "kept", i64v (-9223372036854775808)] := by
  have h : refRun [.push 9223372036854775807, .push 1, .add] [] =
      some [-9223372036854775808] := by decide
  simpa [enc] using exec_lowerAll h [.bool true, .text "kept"] t

/-- An out-of-range literal makes the reference machine refuse instead of
wrapping, even though MC1's lowered `BitVec.ofInt` literal would wrap. -/
theorem out_of_range_push_refused :
    refRun [.push 9223372036854775808] [] = none ∧
    AOp.lower (.push 9223372036854775808) = AOp.lower (.push (-9223372036854775808)) := by
  refine ⟨by decide, ?_⟩
  simp only [AOp.lower, i64v]
  congr 2

/-- quote/compose/run: `5 quote [ 1 + ] compose run` leaves `6`, and the
maximum composed with `[ 1 + ]` wraps to the minimum, preserving the prefix. -/
theorem quote_compose_run_examples (pre t : Stack) :
    (Exec ((PExpr.compose (.quoted 5) (.arith [.push 1, .add])).code ++ [.word 10]) pre t ↔
      t = pre ++ [i64v 6]) ∧
    (Exec ((PExpr.compose (.quoted 9223372036854775807)
        (.arith [.push 1, .add])).code ++ [.word 10]) pre t ↔
      t = pre ++ [i64v (-9223372036854775808)]) := by
  constructor
  · have h : refRun (PExpr.compose (.quoted 5) (.arith [.push 1, .add])).flat [] =
        some [6] := by decide
    simpa [enc] using PExpr.exec_run_ref _ h pre t
  · have h : refRun (PExpr.compose (.quoted 9223372036854775807)
        (.arith [.push 1, .add])).flat [] = some [-9223372036854775808] := by decide
    simpa [enc] using PExpr.exec_run_ref _ h pre t

/-- Composition order matters: `[ 2 * ] [ 1 + ] compose` and its reverse
differ on 5 (12 vs 11). -/
theorem compose_order_matters (pre t : Stack) :
    (Exec ((PExpr.compose (.arith [.push 2, .mul]) (.arith [.push 1, .add])).code ++
        [.word 10]) (pre ++ [i64v 5]) t ↔ t = pre ++ [i64v 11]) ∧
    (Exec ((PExpr.compose (.arith [.push 1, .add]) (.arith [.push 2, .mul])).code ++
        [.word 10]) (pre ++ [i64v 5]) t ↔ t = pre ++ [i64v 12]) ∧
    i64v 11 ≠ i64v 12 := by
  refine ⟨?_, ?_, by simp only [i64v, ne_eq, Value.i64.injEq]; decide⟩
  · have h : refRun (PExpr.compose (.arith [.push 2, .mul]) (.arith [.push 1, .add])).flat [5] =
        some [11] := by decide
    simpa [enc] using PExpr.exec_run_ref _ h pre t
  · have h : refRun (PExpr.compose (.arith [.push 1, .add]) (.arith [.push 2, .mul])).flat [5] =
        some [12] := by decide
    simpa [enc] using PExpr.exec_run_ref _ h pre t

/-- Subtraction is not commutative in the oracle, so the order theorem has
content. -/
theorem wrap_sub_noncommutative : MathlibI64.wrap (3 - 8) ≠ MathlibI64.wrap (8 - 3) := by
  decide

end NobleFoundation
