import NobleContracts.NamedSubject
import NobleContracts.Expression

/-! A separate MC1-strength claim over the NamedV1 normal-return semantics.
This is a semantic model, not source acceptance, correspondence, or guest authority. -/
namespace NobleContracts.NamedV2
open NobleContracts

/-- Exact ordered type checking for the *named* values, independent of MC1-v1
`StackTyped`. No extra input or output values are admitted at this boundary. -/
inductive StackTyped (env : NamedV1.Environment) : NamedV1.Stack → List Ty → Prop where
  | nil : StackTyped env [] []
  | cons {v vs ty tys} : NamedV1.HasType env v ty → StackTyped env vs tys →
      StackTyped env (v :: vs) (ty :: tys)

/-- The exact, partial-correctness public boundary for a named root. Subject
input/output types are used directly, so a caller cannot substitute another
interface for the checked root. Ghost parameters do not enter execution; the
precondition sees the *empty* output snapshot. Only normal results are claimed. -/
def exportedNamedClaim (subject : NamedV1.Subject) (paramTypes : List Ty)
    (pre post : NamedV1.Stack → NamedV1.Stack → NamedV1.Stack → Prop) : Prop :=
  ∀ tail before params,
    StackTyped subject.env before subject.input →
    StackTyped subject.env params paramTypes →
    pre before [] params →
    ∀ final, NamedV1.Exec subject.env subject.root (tail ++ before) final →
      ∃ after, final = tail ++ after ∧
        StackTyped subject.env after subject.output ∧ post before after params

private def unaryBool (f : Bool → Bool) : Option NamedV1.Value → Option NamedV1.Value
  | some (.bool b) => some (.bool (f b))
  | _ => none
private def binaryBool (f : Bool → Bool → Bool) :
    Option NamedV1.Value → Option NamedV1.Value → Option NamedV1.Value
  | some (.bool a), some (.bool b) => some (.bool (f a b))
  | _, _ => none
def binaryI64 (f : BitVec 64 → BitVec 64 → BitVec 64) :
    Option NamedV1.Value → Option NamedV1.Value → Option NamedV1.Value
  | some (.i64 a), some (.i64 b) => some (.i64 (f a b))
  | _, _ => none
private def compareI64 (f : Int → Int → Bool) :
    Option NamedV1.Value → Option NamedV1.Value → Option NamedV1.Value
  | some (.i64 a), some (.i64 b) => some (.bool (f a.toInt b.toInt))
  | _, _ => none

/-- Full MC1 Term vocabulary interpreted with named values. In particular,
`maps` is partial correctness of actual NamedV1 execution, not a guessed
success for a missing program. Undefined operands remain undefined. -/
noncomputable def evaluate₂ (env : NamedV1.Environment) :
    Term → NamedV1.Stack → NamedV1.Stack → NamedV1.Stack → Option NamedV1.Value
  | .i64 n, _, _, _ => some (.i64 (BitVec.ofInt 64 n))
  | .bool b, _, _, _ => some (.bool b)
  | .unit, _, _, _ => some .unit
  | .input i, before, _, _ => before[i]?
  | .output i, _, after, _ => after[i]?
  | .param i, _, _, params => params[i]?
  | .definition _ body, before, after, params => evaluate₂ env body before after params
  | .not body, before, after, params =>
      unaryBool Bool.not (evaluate₂ env body before after params)
  | .and a b, before, after, params =>
      binaryBool Bool.and (evaluate₂ env a before after params) (evaluate₂ env b before after params)
  | .or a b, before, after, params =>
      binaryBool Bool.or (evaluate₂ env a before after params) (evaluate₂ env b before after params)
  | .implies a b, before, after, params =>
      binaryBool (fun x y => !x || y) (evaluate₂ env a before after params)
        (evaluate₂ env b before after params)
  | .eq a b, before, after, params =>
      match evaluate₂ env a before after params, evaluate₂ env b before after params with
      | some x, some y => some (.bool (@decide (x = y) (Classical.propDecidable _)))
      | _, _ => none
  | .lt a b, before, after, params =>
      compareI64 (fun x y => decide (x < y)) (evaluate₂ env a before after params)
        (evaluate₂ env b before after params)
  | .le a b, before, after, params =>
      compareI64 (fun x y => decide (x ≤ y)) (evaluate₂ env a before after params)
        (evaluate₂ env b before after params)
  | .add a b, before, after, params =>
      binaryI64 (· + ·) (evaluate₂ env a before after params) (evaluate₂ env b before after params)
  | .sub a b, before, after, params =>
      binaryI64 (· - ·) (evaluate₂ env a before after params) (evaluate₂ env b before after params)
  | .mul a b, before, after, params =>
      binaryI64 (· * ·) (evaluate₂ env a before after params) (evaluate₂ env b before after params)
  | .pair a b, before, after, params =>
      match evaluate₂ env a before after params, evaluate₂ env b before after params with
      | some x, some y => some (.pair x y)
      | _, _ => none
  | .first body, before, after, params =>
      match evaluate₂ env body before after params with
      | some (.pair a _) => some a
      | _ => none
  | .second body, before, after, params =>
      match evaluate₂ env body before after params with
      | some (.pair _ b) => some b
      | _ => none
  | .inl body, before, after, params =>
      (evaluate₂ env body before after params).map NamedV1.Value.inl
  | .inr body, before, after, params =>
      (evaluate₂ env body before after params).map NamedV1.Value.inr
  | .isLeft body, before, after, params =>
      match evaluate₂ env body before after params with
      | some (.inl _) => some (.bool true)
      | some (.inr _) => some (.bool false)
      | _ => none
  | .left body, before, after, params =>
      match evaluate₂ env body before after params with
      | some (.inl a) => some a
      | _ => none
  | .right body, before, after, params =>
      match evaluate₂ env body before after params with
      | some (.inr b) => some b
      | _ => none
  | .nil, _, _, _ => some (.list [])
  | .cons a b, before, after, params =>
      match evaluate₂ env a before after params, evaluate₂ env b before after params with
      | some x, some (.list xs) => some (.list (x :: xs))
      | _, _ => none
  | .isNil body, before, after, params =>
      match evaluate₂ env body before after params with
      | some (.list xs) => some (.bool xs.isEmpty)
      | _ => none
  | .head body, before, after, params =>
      match evaluate₂ env body before after params with
      | some (.list xs) => xs.head?
      | _ => none
  | .tail body, before, after, params =>
      match evaluate₂ env body before after params with
      | some (.list (_ :: xs)) => some (.list xs)
      | _ => none
  | .length body, before, after, params =>
      match evaluate₂ env body before after params with
      | some (.list xs) => some (.i64 (BitVec.ofNat 64 xs.length))
      | _ => none
  | .maps p a b, before, after, params =>
      match evaluate₂ env p before after params, evaluate₂ env a before after params,
        evaluate₂ env b before after params with
      | some (.program code), some x, some y =>
          some (.bool (@decide
            (∀ result, NamedV1.Exec env code [x] result → result = [y])
            (Classical.propDecidable _)))
      | _, _, _ => none

/-- A false or undefined expression cannot establish an assertion. -/
def Holds₂ (env : NamedV1.Environment) (term : Term)
    (before after params : NamedV1.Stack) : Prop :=
  evaluate₂ env term before after params = some (.bool true)

@[simp] theorem undefined_not_head_nil (env : NamedV1.Environment)
    (before after params : NamedV1.Stack) :
    evaluate₂ env (.not (.head .nil)) before after params = none := rfl

@[simp] theorem absent_output_not_true (env : NamedV1.Environment)
    (before params : NamedV1.Stack) : ¬ Holds₂ env (.not (.output 0)) before [] params := by
  simp [Holds₂, evaluate₂, unaryBool]

/-- Body-local inversion for one accepted immutable `[1 +]` specialization.
Lookup and full use matching are explicit premises; no source identity is
inferred from a manually supplied `Use`. -/
private theorem incrementBody (env : NamedV1.Environment)
    (s : NamedV1.Stack) (n : BitVec 64) (final : NamedV1.Stack)
    (h : NamedV1.Run env [.lit (.i64 1), .word 4] (.i64 n :: s) final) :
    final = .i64 (n + 1) :: s := by
  cases h with
  | cons first rest =>
    cases first
    cases rest with
    | cons second done =>
      cases second
      cases done
      rfl

private theorem incrementCall (env : NamedV1.Environment) (use : NamedV1.Use)
    (entry : NamedV1.Entry) (s : NamedV1.Stack) (n : BitVec 64)
    (final : NamedV1.Stack)
    (lookup : env.lookup use.slot = some entry)
    (body : entry.body = [.lit (.i64 1), .word 4])
    (execution : NamedV1.Step env (.call use) (.i64 n :: s) final) :
    final = .i64 (n + 1) :: s := by
  cases execution with
  | call actual _ run =>
    rw [lookup] at actual
    cases actual
    rw [body] at run
    exact incrementBody env s n final run

private theorem emptyRun (env : NamedV1.Environment) {s t : NamedV1.Stack}
    (execution : NamedV1.Run env [] s t) : t = s := by
  cases execution
  rfl

/-- Reviewed semantic rule for TWO ordered named calls, with two independently
resolved entries. This works with actual dynamically assigned slots and node
occurrences; source-side authentication of those records remains separate. -/
theorem twoStepResult (subject : NamedV1.Subject)
    (useA useB : NamedV1.Use) (entryA entryB : NamedV1.Entry)
    (root : subject.root = [.call useA, .call useB])
    (lookupA : subject.env.lookup useA.slot = some entryA)
    (lookupB : subject.env.lookup useB.slot = some entryB)
    (_matchA : entryA.matches useA) (_matchB : entryB.matches useB)
    (bodyA : entryA.body = [.lit (.i64 1), .word 4])
    (bodyB : entryB.body = [.lit (.i64 1), .word 4])
    (tail : NamedV1.Stack) (n : BitVec 64) (final : NamedV1.Stack)
    (execution : NamedV1.Exec subject.env subject.root (tail ++ [.i64 n]) final) :
    final = tail ++ [.i64 (n + 2)] := by
  have run : NamedV1.Run subject.env [.call useA, .call useB]
      (.i64 n :: tail.reverse) final.reverse := by
    rw [root] at execution
    change NamedV1.Run subject.env [.call useA, .call useB]
      (tail ++ [NamedV1.Value.i64 n]).reverse final.reverse at execution
    simpa using execution
  cases run with
  | cons first rest =>
    have firstResult := incrementCall subject.env useA entryA tail.reverse n _
      lookupA bodyA first
    subst_vars
    cases rest with
    | cons second done =>
      have secondResult := incrementCall subject.env useB entryB tail.reverse (n + 1) _
        lookupB bodyB second
      subst_vars
      have endEq : final.reverse = .i64 ((n + 1) + 1) :: tail.reverse :=
        emptyRun subject.env done
      have flipped := congrArg List.reverse endEq
      simpa [List.reverse_cons, BitVec.add_assoc] using flipped

/-- Exact generated-style MC1-strength rule over any independently checked
NamedV1 subject whose accepted *definition body* is the ordered pair of
resolved step calls. The rule proves normal-result behavior, not provenance,
acceptance of a submission root, or termination. -/
theorem twoStepClaim (subject : NamedV1.Subject)
    (useA useB : NamedV1.Use) (entryA entryB : NamedV1.Entry)
    (input : subject.input = [.i64]) (output : subject.output = [.i64])
    (root : subject.root = [.call useA, .call useB])
    (_differentSlots : useA.slot ≠ useB.slot)
    (_differentOccurrences : useA.occurrence ≠ useB.occurrence)
    (lookupA : subject.env.lookup useA.slot = some entryA)
    (lookupB : subject.env.lookup useB.slot = some entryB)
    (matchA : entryA.matches useA) (matchB : entryB.matches useB)
    (bodyA : entryA.body = [.lit (.i64 1), .word 4])
    (bodyB : entryB.body = [.lit (.i64 1), .word 4]) :
    exportedNamedClaim subject []
      (fun before after params => Holds₂ subject.env (.bool true) before after params)
      (fun before after params =>
        Holds₂ subject.env (.eq (.output 0) (.add (.input 0) (.i64 2)))
          before after params) := by
  intro tail before params hi hp _ final execution
  rw [input] at hi
  cases hi with
  | cons hv rest =>
    cases hv with
    | i64 =>
      cases rest
      cases hp
      refine ⟨[.i64 (_ + 2)], twoStepResult subject useA useB entryA entryB
        root lookupA lookupB matchA matchB bodyA bodyB tail _ final execution,
        ?_, ?_⟩
      · rw [output]
        exact .cons .i64 .nil
      · simp [Holds₂, evaluate₂, binaryI64]

/-- The normal result of a quotation whose I64 payload was captured at
construction time. The capture is a BitVec value, not a numeral selected by
the proof producer; its addition has the language's wrapping semantics.
Binding this recipe to a concrete source occurrence and installed Program
remains a separate correspondence obligation. -/
theorem capturedIncrementConstruction (env : NamedV1.Environment)
    (capture : BitVec 64) (tail : NamedV1.Stack) :
    NamedV1.Step env (.word 8) (.i64 capture :: tail)
      (.program [.lit (.i64 capture)] :: tail) ∧
    NamedV1.Step env (.word 9)
      (.program [.word 4] :: .program [.lit (.i64 capture)] :: tail)
      (.program [.lit (.i64 capture), .word 4] :: tail) := by
  exact ⟨.quote, .compose⟩

theorem capturedIncrementResult (env : NamedV1.Environment)
    (capture input : BitVec 64) (tail final : NamedV1.Stack)
    (execution : NamedV1.Exec env
      [.lit (.i64 capture), .word 4] (tail ++ [.i64 input]) final) :
    final = tail ++ [.i64 (input + capture)] := by
  have run : NamedV1.Run env [.lit (.i64 capture), .word 4]
      (.i64 input :: tail.reverse) final.reverse := by
    change NamedV1.Run env [.lit (.i64 capture), .word 4]
      (tail ++ [NamedV1.Value.i64 input]).reverse final.reverse at execution
    simpa using execution
  cases run with
  | cons first rest =>
    cases first
    cases rest with
    | cons second done =>
      cases second
      have endEq : final.reverse = .i64 (input + capture) :: tail.reverse :=
        emptyRun env done
      have flipped := congrArg List.reverse endEq
      simpa [List.reverse_cons] using flipped

/-- A captured program's exact claim is indexed by its own immutable payload.
The proof cannot be reused at another captured value without establishing
equality of the actual value and this index. This is only normal-return
semantics, not a source/installed-Wasm certificate. -/
theorem capturedIncrementClaim (subject : NamedV1.Subject)
    (capture : BitVec 64)
    (input : subject.input = [.i64]) (output : subject.output = [.i64])
    (root : subject.root = [.lit (.i64 capture), .word 4]) :
    exportedNamedClaim subject []
      (fun before after params => Holds₂ subject.env (.bool true) before after params)
      (fun before after params =>
        Holds₂ subject.env
          (.eq (.output 0) (.add (.input 0) (.i64 capture.toInt)))
          before after params) := by
  intro tail before params hi hp _ final execution
  rw [input] at hi
  cases hi with
  | cons hv rest =>
    cases hv with
    | i64 =>
      cases rest
      cases hp
      rename_i inputValue hpre
      refine ⟨[.i64 (inputValue + capture)], ?_, ?_, ?_⟩
      · rw [root] at execution
        exact capturedIncrementResult subject.env capture _ tail final execution
      · rw [output]
        exact .cons .i64 .nil
      · simp [Holds₂, evaluate₂, binaryI64, BitVec.ofInt_toInt]

/-- A normal result witnesses why changing the capture from 2 to 1 cannot
inherit the same arithmetic claim, even though the program shape is equal. -/
theorem capturedTwoNotOne (env : NamedV1.Environment) :
    ¬ (∀ final, NamedV1.Exec env [.lit (.i64 2), .word 4]
        [.i64 0] final → final = [.i64 1]) := by
  intro alleged
  have run : NamedV1.Exec env [.lit (.i64 2), .word 4]
      [.i64 0] [.i64 2] :=
    NamedV1.Run.cons .lit (NamedV1.Run.cons .add .nil)
  have wrong := alleged [.i64 2] run
  simp at wrong

end NobleContracts.NamedV2
