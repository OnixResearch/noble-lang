import Interpreter.Wasm.Decoder.Wat
import Interpreter.Wasm.SmallStep
import Interpreter.WasmPilot.Subject
import Interpreter.WasmPilot.SubjectMul
import NobleContracts.Model

namespace Interpreter.WasmPilot

open Wasm.Decoder.Wat

set_option maxRecDepth 100000
set_option maxHeartbeats 5000000

/-- Decode the exact WAT text whose digest is bound in `Subject`. -/
def subjectDecodeAccepted : Bool :=
  match decodeForVerification Subject.wat with
  | .ok _ => true
  | .error _ => false

/-- Decode the full dynamic-input `x y *` module emitted by Noble. -/
def subjectMulDecodeAccepted : Bool :=
  match decodeForVerification SubjectMul.wat with
  | .ok _ => true
  | .error _ => false

/-- The existing MC1 model executes the universally quantified I64 multiply
word with wrapping BitVec arithmetic. This source-side fact is independent of
the Wasm module and does not yet establish artifact refinement. -/
theorem model_i64_mul_exec (x y : BitVec 64) :
    NobleContracts.Exec
      [.word 6]
      [.i64 x, .i64 y] [.i64 (x * y)] := by
  unfold NobleContracts.Exec
  exact NobleContracts.Run.cons NobleContracts.Step.mul NobleContracts.Run.nil

theorem model_i64_add_exec (x y : BitVec 64) :
    NobleContracts.Exec
      [.word 4]
      [.i64 x, .i64 y] [.i64 (x + y)] := by
  unfold NobleContracts.Exec
  exact NobleContracts.Run.cons NobleContracts.Step.add NobleContracts.Run.nil

/-- A correctly hashed production artifact for `+` is wrong for this `*` claim:
the MC1 source model returns 6 while the decoded operation's corresponding
source model returns 5. This counterexample is kernel-checked; it is not yet a
proof that the artifact's execution refines either MC1 program. -/
theorem model_mul_vs_add_2_3 :
    NobleContracts.Exec [.word 6]
      [.i64 (2 : BitVec 64), .i64 3]
      [.i64 ((2 : BitVec 64) * 3)] ∧
    NobleContracts.Exec [.word 4]
      [.i64 (2 : BitVec 64), .i64 3]
      [.i64 ((2 : BitVec 64) + 3)] ∧
    ((2 : BitVec 64) * 3) ≠ ((2 : BitVec 64) + 3) := by
  exact ⟨model_i64_mul_exec 2 3, model_i64_add_exec 2 3, by decide⟩

/-- MC1's linking arithmetic projection preserves the supplied Bool prefix. -/
theorem model_linking_projection_exec (retain : Bool) (x y : BitVec 64) :
    NobleContracts.Exec
      [.word 5, .lit (NobleContracts.i64 7), .word 5]
      [.bool retain, .i64 x, .i64 y]
      [.bool retain, .i64 (x - y - 7)] := by
  unfold NobleContracts.Exec
  exact NobleContracts.Run.cons NobleContracts.Step.sub
    (NobleContracts.Run.cons NobleContracts.Step.lit
      (NobleContracts.Run.cons NobleContracts.Step.sub NobleContracts.Run.nil))

/-- Source-side expected result for the retained `7 3 * 5 +` fixture. -/
theorem model_fixed_fixture_exec :
    NobleContracts.Exec
      [.lit (NobleContracts.i64 7), .lit (NobleContracts.i64 3), .word 6,
       .lit (NobleContracts.i64 5), .word 4]
      [] [NobleContracts.i64 26] := by
  unfold NobleContracts.Exec
  exact NobleContracts.Run.cons NobleContracts.Step.lit
    (NobleContracts.Run.cons NobleContracts.Step.lit
      (NobleContracts.Run.cons NobleContracts.Step.mul
        (NobleContracts.Run.cons NobleContracts.Step.lit
          (NobleContracts.Run.cons NobleContracts.Step.add NobleContracts.Run.nil))))

#print axioms model_i64_mul_exec
#print axioms model_i64_add_exec
#print axioms model_mul_vs_add_2_3
#print axioms model_linking_projection_exec
#print axioms model_fixed_fixture_exec
#print axioms Wasm.Decoder.Wat.decodeForVerification
#print axioms Wasm.SmallStep.stepChecked?_sound
#print axioms Wasm.SmallStep.stepChecked?_complete

def main : IO Unit := do
  if subjectDecodeAccepted && subjectMulDecodeAccepted then
    IO.println "subject decoder check passed"
  else
    throw <| IO.userError "Talos decodeForVerification refused the subject"

end Interpreter.WasmPilot

def main : IO Unit := Interpreter.WasmPilot.main
