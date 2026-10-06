import NobleContracts.NamedV2

namespace NobleContracts.NamedV2.Examples
open NobleContracts.NamedV1
private abbrev Op := NamedV1.Op
private abbrev Stack := NamedV1.Stack
private abbrev CodeTyped := NamedV1.CodeTyped
private abbrev OpTyped := NamedV1.OpTyped
private abbrev Run := NamedV1.Run
private abbrev Step := NamedV1.Step
private abbrev Exec := NamedV1.Exec

/-! Synthetic semantic fixture, NOT accepted source. Prospective native units:
```
module Definitions@5 [ export step def step [ 1 + ] ]
import Definitions@5 as d
module Subject@3 [ def twice [ d.step d.step ] ]
```
The claim is over the checked `twice` *body*, not the submission's enclosing
invocation; the host must independently authenticate that one-root invocation
and derive the accepted body, exact bytes, both fresh specialization slots,
actual occurrence node IDs and lexical positions, resolution, and every
environment row. The example ordinals/bytes are illustrative, not authentic. -/
private def caller : Provenance :=
  { moduleName := "Subject", moduleVersion := 3, moduleSource := [1, 2],
    definitionSlot := 0, definitionName := "twice", definitionSource := [2] }
private def callee : Provenance :=
  { moduleName := "Definitions", moduleVersion := 5, moduleSource := [3, 4],
    definitionSlot := 1, definitionName := "step", definitionSource := [4] }
private def useAt (occurrence slot : Nat) : Use :=
  { slot, occurrence, caller, origin := callee,
    input := [.i64], output := [.i64] }
private def stepAt (slot : Nat) : Entry :=
  { slot, origin := callee, input := [.i64], output := [.i64],
    body := [.lit (.i64 1), .word 4], dependencies := [], rank := 0 }
private def env : Environment := { entries := [stepAt 7, stepAt 8] }
private def root : List Op := [.call (useAt 11 7), .call (useAt 12 8)]

private theorem closure : ExactClosure env caller root [7, 8] 1 := by
  simp [ExactClosure, env, stepAt, root,
    usesCode, usesOp, usesValue, useAt, Environment.lookup, Entry.matches,
    caller, callee]

private theorem step_typed (slot : Nat) : CodeTyped env (stepAt slot).body [.i64] [.i64] := by
  exact .cons (.lit .i64) (.cons (NamedV1.OpTyped.add (s := [])) .nil)

private theorem root_typed : CodeTyped env root [.i64] [.i64] := by
  have typedCall (occurrence slot : Nat) (hlookup : env.lookup slot = some (stepAt slot)) :
      OpTyped env (.call (useAt occurrence slot)) [.i64] [.i64] := by
    apply OpTyped.call (entry := stepAt slot) (s := [])
    · exact hlookup
    · simp [Entry.matches, stepAt, useAt]
    · exact step_typed slot
  exact .cons (typedCall 11 7 rfl) (.cons (typedCall 12 8 rfl) .nil)

/-- Unlike the weaker NamedV1.Satisfies, the checked body interface and all
closure/entry premises reside in this synthetic subject, and the v2 claim
quantifies all tails, inputs, ghost parameters, and normal results. -/
def subject : Subject :=
  { provenance := caller, env, root, dependencies := [7, 8], rank := 1,
    closure, checkedEntries := by
      intro entry he
      simp only [env, List.mem_cons, List.mem_nil_iff, or_false] at he
      rcases he with rfl | rfl
      · exact step_typed 7
      · exact step_typed 8,
    input := [.i64], output := [.i64], typed := root_typed }

/-- Concrete generated-claim *style* (not a claim emitted by a source exporter):
requires `true`, ensures `(eq (out y) (add (in x) 2))`. -/
def claim : Prop :=
  exportedNamedClaim subject []
    (fun before after params => Holds₂ subject.env (.bool true) before after params)
    (fun before after params =>
      Holds₂ subject.env (.eq (.output 0) (.add (.input 0) (.i64 2)))
        before after params)

private theorem increment_run (s : Stack) (n : BitVec 64) (final : Stack)
    (slot : Nat) (h : Run env (stepAt slot).body (.i64 n :: s) final) :
    final = .i64 (n + 1) :: s := by
  change Run env [.lit (.i64 1), .word 4] (.i64 n :: s) final at h
  cases h with
  | cons hfirst hrest =>
    cases hfirst
    cases hrest with
    | cons hsecond hnil =>
      cases hsecond
      cases hnil
      rfl

private theorem increment_call (occurrence slot : Nat)
    (hlookup : env.lookup slot = some (stepAt slot)) (s : Stack)
    (n : BitVec 64) (final : Stack)
    (h : Step env (.call (useAt occurrence slot)) (.i64 n :: s) final) :
    final = .i64 (n + 1) :: s := by
  cases h with
  | call lookup _ execution =>
    change env.lookup slot = some _ at lookup
    rw [hlookup] at lookup
    cases lookup
    exact increment_run s n final slot execution

private theorem run_nil_eq {s t : Stack} (h : Run env [] s t) : t = s := by
  cases h
  rfl

/-- Exact semantics of two separate named calls, preserving an arbitrary
bottom prefix, including its values/types. Addition is BitVec wrapping. -/
theorem two_calls (tail : Stack) (n : BitVec 64) (final : Stack)
    (h : Exec env root (tail ++ [.i64 n]) final) :
    final = tail ++ [.i64 (n + 2)] := by
  have hrun : Run env root (.i64 n :: tail.reverse) final.reverse := by
    change Run env root (tail ++ [NamedV1.Value.i64 n]).reverse final.reverse at h
    simpa using h
  cases hrun with
  | cons first rest =>
    have first_result := increment_call 11 7 rfl tail.reverse n _ first
    subst_vars
    cases rest with
    | cons second done =>
      have second_result := increment_call 12 8 rfl tail.reverse (n + 1) _ second
      subst_vars
      have hfinal : final.reverse = .i64 ((n + 1) + 1) :: tail.reverse :=
        run_nil_eq done
      have flipped := congrArg List.reverse hfinal
      simpa [List.reverse_cons, BitVec.add_assoc] using flipped

/-- MC1-strength named result: exact typed I64 input/output, empty ghost
parameter stack, true precondition on the empty output snapshot, arbitrary
tail, and the precise wrapping +2 postcondition on every normal result. -/
theorem proof : claim := by
  unfold claim
  apply twoStepClaim subject (useAt 11 7) (useAt 12 8) (stepAt 7) (stepAt 8)
  · rfl
  · rfl
  · rfl
  · decide
  · decide
  · rfl
  · rfl
  · simp [Entry.matches, stepAt, useAt]
  · simp [Entry.matches, stepAt, useAt]
  · rfl
  · rfl

/-- The named program value may itself occur in a logical `maps` assertion:
the result is proved from actual named execution, not a guessed evaluator. -/
theorem maps_two_calls (n : BitVec 64) :
    Holds₂ env (.maps (.input 0) (.param 0) (.output 0))
      [.program root] [.i64 (n + 2)] [.i64 n] := by
  have hmap : ∀ result, Exec env root [.i64 n] result →
      result = [.i64 (n + 2)] := by
    intro result execution
    simpa using two_calls [] n result execution
  simpa [Holds₂, evaluate₂] using hmap

/-- A forged immutable callee version has no resolved normal call, even when
the slot and executable bytes resemble the true specialization. -/
private def forged : Use :=
  { useAt 11 7 with origin := { callee with moduleVersion := 6 } }

theorem forged_call_has_no_result (n : BitVec 64) :
    ¬ ∃ final, Step env (.call forged) [.i64 n] final := by
  rintro ⟨final, execution⟩
  cases execution with
  | call lookup hmatches _ =>
    change some (stepAt 7) = some _ at lookup
    cases lookup
    have bad := hmatches.2.1
    simp [stepAt, forged, callee] at bad

/-- A wrong postcondition cannot be proved by vacuity: a concrete normal
return exists and falsifies its exact output. -/
private theorem concrete_return : Exec env root [.i64 0] [.i64 2] := by
  change Run env root [.i64 0] [.i64 2]
  have callStep (occurrence slot : Nat)
      (hlookup : env.lookup slot = some (stepAt slot)) (n : BitVec 64) :
      Step env (.call (useAt occurrence slot)) [.i64 n] [.i64 (n + 1)] := by
    apply NamedV1.Step.call (entry := stepAt slot)
    · exact hlookup
    · simp [Entry.matches, stepAt, useAt]
    · exact .cons .lit (.cons .add .nil)
  simpa [root] using
    (NamedV1.Run.cons (callStep 11 7 rfl 0)
      (NamedV1.Run.cons (callStep 12 8 rfl 1) NamedV1.Run.nil))

private def wrongClaim : Prop :=
  exportedNamedClaim subject []
    (fun before after params => Holds₂ subject.env (.bool true) before after params)
    (fun before after params =>
      Holds₂ subject.env (.eq (.output 0) (.add (.input 0) (.i64 3)))
        before after params)

theorem wrong_output_refuted : ¬ wrongClaim := by
  intro purported
  obtain ⟨after, heq, _, post⟩ :=
    purported [] [.i64 0] [] (.cons .i64 .nil) .nil
      (by simp [Holds₂, evaluate₂]) [.i64 2] concrete_return
  have ha : after = [.i64 2] := by simpa using heq.symm
  subst after
  simp [Holds₂, evaluate₂, NamedV2.binaryI64] at post

#print axioms twoStepClaim
#print axioms NobleContracts.NamedV2.capturedIncrementConstruction
#print axioms NobleContracts.NamedV2.capturedIncrementResult
#print axioms NobleContracts.NamedV2.capturedIncrementClaim
#print axioms NobleContracts.NamedV2.capturedTwoNotOne
#print axioms proof
#print axioms maps_two_calls
#print axioms forged_call_has_no_result
#print axioms wrong_output_refuted
end NobleContracts.NamedV2.Examples
