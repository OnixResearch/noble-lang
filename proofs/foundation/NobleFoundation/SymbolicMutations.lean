/-
Test-local faulty symbolic-query variants (issue #56, acceptance item 4).

Noble's discipline requires that a bounded query lane be accompanied by *test-local*
faulty variants that the retained expectations catch, so that "a search found
nothing" and "two consumers agree" can never stand in for the reference result.
Nothing here is compiled into a production path or referenced by the reference
models themselves: every variant is a separate definition, and every theorem states
that a named reference expectation detects it.

The variants are one seeded fault each, at the four places the issue names: the
operation, an operand, a constant, and the ordered output list. Two further
variants corrupt the *encoding* instead of the graph: a shifted encoder that
records bits at the wrong slot (with a matching decoder, so both consumers of that
one faulty encoding agree) and an output-blind verdict that inspects only the first
ordered result.

A variant that escaped would mean the retained family cannot distinguish it; the
lane would then need a new distinguishing case rather than a weakened variant.
-/
import NobleFoundation.SymbolicSolver

namespace NobleFoundation.SymbolicPair.Mutations

open NobleFoundation.PackedSchedule
open NobleFoundation.SymbolicPair
open NobleFoundation.SymbolicSolver

/-! ### Seeded graph faults

Each face below differs from a reference face in exactly one place. -/

/-- Fault 1: the operation. The reference face adds its operands, this one subtracts
them in the same order. -/
def opMutated : Graph where
  arity := 3
  output := 2
  node := fun index =>
    if index = 0 then .constant 5
    else if index = 1 then .constant 3
    else .sub 1 0

/-- Fault 2: an operand. The reference face adds two distinct constants, this one
reaches the same node twice. -/
def operandMutated : Graph where
  arity := 3
  output := 2
  node := fun index =>
    if index = 0 then .constant 5
    else if index = 1 then .constant 3
    else .add 1 1

/-- Fault 3: a constant. The reference face holds three, this one holds six. -/
def constantMutated : Graph where
  arity := 3
  output := 2
  node := fun index =>
    if index = 0 then .constant 6
    else if index = 1 then .constant 3
    else .add 1 0

/-- Two ordered outputs that agree on the first and differ on the second. -/
def twoOutLeft : Graph where
  arity := 3
  output := 2
  node := fun index =>
    if index = 0 then .constant 3
    else if index = 1 then .constant 5
    else .add 0 1

def twoOutRight : Graph where
  arity := 3
  output := 2
  node := fun index =>
    if index = 0 then .constant 3
    else if index = 1 then .constant 5
    else .sub 1 0

/-- Fault 4: the ordered output list. The right face is asked for its outputs in the
other order, so the ordered results need to differ even where the graphs agree. -/
def twoOutSwapped : Pair where
  left := twoOutLeft
  right := twoOutRight
  outputs := [2, 0]

/-- The reference comparison over both ordered outputs: the pair differs at the
second ordered result. -/
def twoOutPair : Pair where
  left := twoOutLeft
  right := twoOutRight
  outputs := [0, 2]

/-! ### Corrupted encodings

A shifted encoder records each slot's bits at the next slot. A consumer that reads
the shifted slot finds the bits there, so two consumers of that one faulty encoding
agree; the graph, however, reads slot zero, and the reference expectation says its
bits are the candidate's. -/

/-- Fault 5: the encoder. Records each slot's exact bits at the next slot. -/
def encodeShifted (slots : List Nat) (inputs : Nat → BitVec 64) : List (Nat × BitVec 64) :=
  slots.map (fun slot => (slot + 1, inputs slot))

/-- Fault 6: the verdict. Compares only the first ordered result, so a difference in
any later output is invisible to it. -/
def refutedFirstOnly (pair : Pair) (inputs : Nat → BitVec 64) : Bool :=
  match pair.outputs with
  | [] => false
  | out :: _ => decide (value pair.left inputs out ≠ value pair.right inputs out)

/-! ### Detection

Each theorem names the reference expectation that catches its variant. -/

/-- Fault 1 is caught: the subtraction face refutes the addition face that the
reference pair keeps in agreement. -/
theorem wrong_op_is_caught :
    refuted ⟨agreeLeft, opMutated, [2]⟩ (fun _ => 0) = true
    ∧ refuted agreePair (fun _ => 0) = false := by
  decide

/-- Fault 2 is caught: `5 + 5` is not `3 + 5`. -/
theorem wrong_operand_is_caught :
    refuted ⟨agreeLeft, operandMutated, [2]⟩ (fun _ => 0) = true
    ∧ refuted agreePair (fun _ => 0) = false := by
  decide

/-- Fault 3 is caught: `6 + 3` is not `3 + 5`. -/
theorem wrong_constant_is_caught :
    refuted ⟨agreeLeft, constantMutated, [2]⟩ (fun _ => 0) = true
    ∧ refuted agreePair (fun _ => 0) = false := by
  decide

/-- Fault 4 is caught: asking the face for its outputs in the other order changes the
ordered results, and the reference over the declared order catches it. -/
theorem wrong_output_order_is_caught :
    refuted twoOutSwapped (fun _ => 0) = true
    ∧ replayLeft twoOutSwapped (fun _ => 0) ≠ replayLeft twoOutPair (fun _ => 0) := by
  decide

/-- Fault 5 is caught: the corrupted witness round-trips at the slot it recorded, and
the graph reads a different slot, so the verdict flips from agreement to refutation
against the reference replay of the candidate's own bits. -/
theorem shifted_encoding_is_caught :
    decodeInputs (encodeShifted [0] (fun _ => 6)) 0 1 = 6
    ∧ decodeInputs (encodeShifted [0] (fun _ => 6)) 0 0 ≠ 6
    ∧ decodeInputs (encode [0] (fun _ => 6)) 0 0 = 6
    ∧ refuted inputPair (decodeInputs (encodeShifted [0] (fun _ => 6)) 7) = true
    ∧ refuted inputPair (decodeInputs (encode [0] (fun _ => 6)) 7) = false := by
  decide

/-- Fault 6 is caught: the output-blind verdict reports agreement on a pair whose
second ordered result differs, and the reference comparison reports the refutation. -/
theorem output_blind_verdict_is_caught :
    refutedFirstOnly twoOutPair (fun _ => 0) = false
    ∧ refuted twoOutPair (fun _ => 0) = true := by
  decide

/-- The mutation lane is closed over the declared variants: each fault is caught by
the same-named reference expectation above, and none escapes. -/
theorem all_declared_mutations_are_caught :
    (refuted ⟨agreeLeft, opMutated, [2]⟩ (fun _ => 0))
    && (refuted ⟨agreeLeft, operandMutated, [2]⟩ (fun _ => 0))
    && (refuted ⟨agreeLeft, constantMutated, [2]⟩ (fun _ => 0))
    && (refuted twoOutSwapped (fun _ => 0))
    && (decodeInputs (encodeShifted [0] (fun _ => 6)) 0 0 != 6)
    && (!refutedFirstOnly twoOutPair (fun _ => 0))
    && (refuted twoOutPair (fun _ => 0)) = true := by
  decide

end NobleFoundation.SymbolicPair.Mutations
