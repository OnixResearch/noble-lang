import NobleContracts.NamedSubject

namespace NobleContracts.NamedV1.Tests

-- These are semantic fixtures, not accepted-source or compiler correspondence evidence.
private def caller : Provenance :=
  { moduleName := "subject", moduleVersion := 3, moduleSource := [1, 2],
    definitionSlot := 0, definitionName := "subject", definitionSource := [2] }
private def callee : Provenance :=
  { moduleName := "definitions", moduleVersion := 5, moduleSource := [3, 4],
    definitionSlot := 1, definitionName := "increment", definitionSource := [4] }
private def callAt (occurrence : Nat) : Use :=
  { slot := 7, caller, occurrence, origin := callee, input := [.i64], output := [.i64] }
private def increment : Entry :=
  { slot := 7, origin := callee, input := [.i64], output := [.i64],
    body := [.lit (.i64 1), .word 4], dependencies := [], rank := 0 }
private def environment : Environment := { entries := [increment] }
private def root : List Op := [.call (callAt 11), .call (callAt 12)]

example : (callAt 11).occurrence ≠ (callAt 12).occurrence := by decide
example : ExactClosure environment caller root [7] 1 := by
  simp [ExactClosure, environment, increment, root, usesCode, usesOp, usesValue,
    callAt, Environment.lookup, Entry.matches, caller, callee]

example : ¬ ExactClosure environment caller root [] 1 := by
  intro h
  have hused : ∃ use ∈ usesCode root, use.slot = 7 :=
    ⟨callAt 11, by simp [root, usesCode, usesOp], rfl⟩
  have hmissing : 7 ∈ ([] : List Nat) :=
    (root_dependencies_exact h 7).2 hused
  cases hmissing

example : ¬ ExactClosure environment caller root [7] 0 := by
  intro h
  have hused : callAt 11 ∈ usesCode root := by simp [root, usesCode, usesOp]
  obtain ⟨target, heq, _, hlt⟩ := (h.2.2.2.2 (callAt 11) hused).2.2
  have htarget : target = increment := by
    simpa [Environment.lookup, environment, increment, callAt] using heq.symm
  subst target
  exact Nat.lt_irrefl 0 hlt

private def forgedCall : Use :=
  { callAt 11 with origin := { callee with moduleVersion := 6 } }

example : ¬ ExactClosure environment caller [.call forgedCall] [7] 1 := by
  intro h
  have hused : forgedCall ∈ usesCode [.call forgedCall] := by
    simp [usesCode, usesOp]
  obtain ⟨target, heq, hmatch, _⟩ := (h.2.2.2.2 forgedCall hused).2.2
  have htarget : target = increment := by
    simpa [Environment.lookup, environment, increment, forgedCall, callAt] using heq.symm
  subst target
  have hversion := hmatch.2.1
  simp [increment, forgedCall, callAt, callee] at hversion

example : ¬ ∃ after, Run ({ entries := [] } : Environment)
    [.call (callAt 11)] [.i64 7] after := by
  rintro ⟨after, execution⟩
  cases execution with
  | cons first _ =>
    cases first with
    | call lookup _ _ => simp [Environment.lookup] at lookup

private theorem increment_typed :
    CodeTyped environment increment.body [.i64] [.i64] := by
  exact .cons (.lit .i64) (.cons (OpTyped.add (s := [])) .nil)

example : CodeTyped environment root [.i64] [.i64] := by
  have typedCall (occurrence : Nat) :
      OpTyped environment (.call (callAt occurrence)) [.i64] [.i64] := by
    apply OpTyped.call (entry := increment) (s := [])
    · rfl
    · simp [Entry.matches, increment, callAt]
    · exact increment_typed
  exact .cons (typedCall 11) (.cons (typedCall 12) .nil)

private theorem increment_step (n : BitVec 64) (occurrence : Nat) :
    Step environment (.call (callAt occurrence)) [.i64 n] [.i64 (n + 1)] := by
  apply Step.call (entry := increment)
  · rfl
  · simp [Entry.matches, increment, callAt]
  · exact .cons .lit (.cons .add .nil)

example : Exec environment root [.i64 7] [.i64 9] := by
  change Run environment root [.i64 7] [.i64 9]
  have h : (8 : BitVec 64) + 1 = 9 := by decide
  simpa only [root, h] using
    (Run.cons (increment_step 7 11) (Run.cons (increment_step 8 12) Run.nil))

example : usesCode [.block [.call (callAt 11)]] = [callAt 11] := by
  simp [usesCode, usesOp]
example : Exec environment [.block [.call (callAt 11)], .word 11] []
    [.syntax [.call (callAt 11)]] := by
  exact .cons .block (.cons .reflect .nil)

end NobleContracts.NamedV1.Tests
