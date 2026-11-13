/-
  Per-word stack traces for the ordered ownership fragment (issue #59).

  `Ownership.run` decides a program but keeps no history. The corpus lane needs the stack after
  every word: the production acceptance machine is driven one word prefix at a time, and comparing
  the machine's derived stack after each prefix with the model's trace reports the first diverging
  word instead of only a final mismatch.

  The trace is total. It lists the incoming stack and then one entry per word the checker admitted,
  so a refused word simply ends the trace, and `traceWords_length` states that the trace has one
  more entry than the word list exactly when the checker accepts. A short trace is therefore a
  refusal, never a missing row. `traceWords_take` states the property the lane relies on: the trace
  of a word prefix is the prefix of the trace, so the trace's k-th entry is the stack the first `k`
  words leave. `runTrace` is the same trace for a straight-line `Program`; the two-arm branch and
  the ordinary error return are not word sequences and have no per-word trace.
-/
import NobleFoundation.Ownership

namespace NobleFoundation.Ownership

/-- The straight-line word sequence of a program, flattened: `none` for the branch and
    error-return alternatives, which are not word sequences. -/
def straightLine : Program → Option (List Word)
  | .word word => some [word]
  | .compose first second =>
      (straightLine first).bind fun before => (straightLine second).map fun after => before ++ after
  | .branch _ _ => none
  | .fail => none

/-- The executable checker over a word sequence: the flat form of a straight-line program. -/
def runWords : List Word → List Payload → Option (List Payload)
  | [], operands => some operands
  | word :: rest, operands => (step word operands).bind (runWords rest)

/-- The stack after each admitted word: the incoming stack first, then one entry per word the
    checker admitted. A refused word ends the trace. -/
def traceWords : List Word → List Payload → List (List Payload)
  | [], operands => [operands]
  | word :: rest, operands =>
      match step word operands with
      | none => [operands]
      | some out => operands :: traceWords rest out

/-- The per-word trace of a straight-line program; `none` when the program is not a word sequence,
    i.e. a two-arm branch or the ordinary error return. -/
def runTrace (program : Program) (operands : List Payload) : Option (List (List Payload)) :=
  (straightLine program).map (fun words => traceWords words operands)

private theorem runWords_append (first second : List Word) (operands : List Payload) :
    runWords (first ++ second) operands = (runWords first operands).bind (runWords second) := by
  induction first generalizing operands with
  | nil => simp [runWords]
  | cons word rest hypothesis =>
      simp only [List.cons_append, runWords, Option.bind_assoc]
      congr 1
      funext out
      exact hypothesis out

/-- A straight-line program is checked by `runWords` over its own word sequence. -/
theorem run_eq_runWords (program : Program) (words : List Word) (operands : List Payload)
    (straight : straightLine program = some words) :
    run program operands = runWords words operands := by
  induction program generalizing words operands with
  | word word =>
      have single : [word] = words := by simpa [straightLine] using straight
      rw [← single]
      simp [run, runWords]
  | compose first second firstInduction secondInduction =>
      simp only [straightLine] at straight
      cases firstStraight : straightLine first with
      | none => simp [firstStraight] at straight
      | some before =>
          cases secondStraight : straightLine second with
          | none => simp [firstStraight, secondStraight] at straight
          | some after =>
              simp only [firstStraight, secondStraight, Option.bind_some, Option.map_some,
                Option.some.injEq] at straight
              rw [← straight]
              unfold run
              rw [firstInduction before operands firstStraight]
              cases beforeRun : runWords before operands with
              | none =>
                  simp only [Option.bind_eq_bind, Option.bind_none]
                  rw [runWords_append, beforeRun]
                  simp
              | some middle =>
                  simp only [Option.bind_eq_bind, Option.bind_some]
                  rw [secondInduction after middle secondStraight]
                  rw [runWords_append, beforeRun]
                  simp
  | branch thenArm elseArm thenInduction elseInduction => simp [straightLine] at straight
  | fail => simp [straightLine] at straight

private theorem traceWords_ne_nil (words : List Word) (operands : List Payload) :
    traceWords words operands ≠ [] := by
  cases words with
  | nil => simp [traceWords]
  | cons word rest =>
      cases stepped : step word operands with
      | none => simp [traceWords, stepped]
      | some out => simp [traceWords, stepped]

/-- The trace has one more entry than the word list exactly when the checker accepts: a refused
    word leaves the incoming stack alone, so the count is the acceptance decision. -/
theorem traceWords_length (words : List Word) (operands : List Payload) :
    (traceWords words operands).length = words.length + 1 ↔
      (runWords words operands).isSome := by
  induction words generalizing operands with
  | nil => simp [traceWords, runWords]
  | cons word rest hypothesis =>
      cases stepped : step word operands with
      | none => simp [traceWords, runWords, stepped]
      | some out =>
          simp only [traceWords, stepped, List.length_cons, runWords, Option.bind_some,
            Nat.add_right_cancel_iff]
          exact hypothesis out

/-- The trace of a word prefix is the prefix of the trace, so entry `k` is the stack the first `k`
    words leave. This is the property the production lane's per-prefix comparison uses. -/
theorem traceWords_take (words : List Word) (operands : List Payload) (count : Nat)
    (within : count ≤ words.length) :
    traceWords (words.take count) operands = (traceWords words operands).take (count + 1) := by
  induction words generalizing operands count with
  | nil =>
      have empty : count = 0 := by simp_all
      rw [empty]
      simp [traceWords]
  | cons word rest hypothesis =>
      cases count with
      | zero => cases stepped : step word operands <;> simp [traceWords, stepped]
      | succ prior =>
          have within' : prior ≤ rest.length := by simpa using within
          cases stepped : step word operands with
          | none => simp [traceWords, stepped]
          | some out =>
              simp only [traceWords, stepped, List.take_succ_cons]
              rw [hypothesis out prior within']

/-- Every word prefix of an accepted word sequence is accepted. -/
theorem runWords_prefix_accepted (words : List Word) (operands : List Payload) (count : Nat)
    (within : count ≤ words.length) (accepted : (runWords words operands).isSome = true) :
    (runWords (words.take count) operands).isSome = true := by
  have full : (traceWords words operands).length = words.length + 1 :=
    (traceWords_length words operands).mpr accepted
  have limited : count + 1 ≤ (traceWords words operands).length := by omega
  have taken : (traceWords (words.take count) operands).length =
      (words.take count).length + 1 := by
    rw [traceWords_take words operands count within, List.length_take, Nat.min_eq_left limited,
      List.length_take, Nat.min_eq_left within]
  exact (traceWords_length (words.take count) operands).mp taken

/-- The last trace entry is the checker's result. -/
theorem traceWords_last (words : List Word) (operands out : List Payload)
    (accepted : runWords words operands = some out) :
    (traceWords words operands).getLast? = some out := by
  induction words generalizing operands with
  | nil =>
      have same : operands = out := by simpa [runWords] using accepted
      rw [← same]
      simp [traceWords]
  | cons word rest hypothesis =>
      cases stepped : step word operands with
      | none => simp [runWords, stepped] at accepted
      | some middle =>
          simp only [runWords, stepped, Option.bind_some] at accepted
          have tail : traceWords rest middle ≠ [] := traceWords_ne_nil rest middle
          rw [show traceWords (word :: rest) operands = operands :: traceWords rest middle by
            simp [traceWords, stepped]]
          rw [List.getLast?_cons_of_ne_nil tail]
          exact hypothesis middle accepted

/-- The last entry of a straight-line program's trace is the program's checker result. -/
theorem runTrace_getLast (program : Program) (operands out : List Payload)
    (trace : List (List Payload)) (recorded : runTrace program operands = some trace)
    (accepted : run program operands = some out) : trace.getLast? = some out := by
  unfold runTrace at recorded
  cases straight : straightLine program with
  | none => simp [straight] at recorded
  | some words =>
      simp only [straight, Option.map_some, Option.some.injEq] at recorded
      rw [← recorded]
      exact traceWords_last words operands out (by
        rw [← run_eq_runWords program words operands straight]
        exact accepted)

/-- A decided trace fixture: duplication then one drop, over a data head with a resource tail. -/
theorem trace_dup_drop :
    traceWords [.dup, .drop] [.data 1, .resource 7] =
      [[.data 1, .resource 7], [.data 1, .data 1, .resource 7], [.data 1, .resource 7]] := by
  decide

/-- A decided refusal fixture: a must-use resource as the constrained operand ends the trace after
    the incoming stack, so the trace is shorter than the word list by the `traceWords_length`
    criterion. -/
theorem trace_stops_at_refused_word :
    traceWords [.dup, .drop] [.resource 7] = [[.resource 7]] := by
  decide

end NobleFoundation.Ownership
