import NobleContracts.Rules

namespace NobleContracts

/-- Semantic program behavior used by `maps` in the contract expression
language.  This is partial correctness, not a claim that a normal result exists.
A non-program value is false rather than a vacuously successful contract. -/
def Maps (program : Value) (arguments expected : Stack) : Prop :=
  match program with
  | .program code => ∀ actual, Exec code arguments actual → actual = expected
  | _ => False

@[simp] theorem maps_program_iff {code arguments expected} :
    Maps (.program code) arguments expected ↔
      ∀ actual, Exec code arguments actual → actual = expected := Iff.rfl

/-- The public exported-claim boundary.  Logical parameters are not executable
inputs.  `pre` sees no output stack.  The arbitrary untouched bottom prefix is
quantified outside the execution, and a normal result must preserve that prefix
and have the declared, exact output types. -/
def exportedClaim (code : List Op) (inputTypes outputTypes paramTypes : List Ty)
    (pre post : Stack → Stack → Stack → Prop) : Prop :=
  ∀ tail before params,
    StackTyped before inputTypes → StackTyped params paramTypes →
    pre before [] params →
    ∀ final, Exec code (tail ++ before) final →
      ∃ after, final = tail ++ after ∧ StackTyped after outputTypes ∧
        post before after params

/-- Keep the output boundary while strengthening the input assumption and
weakening the postcondition.  The implications are logical, not textual. -/
theorem exportedClaim_consequence {code inputTypes outputTypes paramTypes pre post pre' post'}
    (proof : exportedClaim code inputTypes outputTypes paramTypes pre post)
    (hpre : ∀ before params, pre' before [] params → pre before [] params)
    (hpost : ∀ before after params,
      pre' before [] params → post before after params → post' before after params) :
    exportedClaim code inputTypes outputTypes paramTypes pre' post' := by
  intro tail before params hi hp hreq final he
  obtain ⟨after, hout, ht, hensure⟩ := proof tail before params hi hp (hpre _ _ hreq) final he
  exact ⟨after, hout, ht, hpost _ _ _ hreq hensure⟩

/-- An exact semantic rule is enough for the corresponding `maps` assertion.
Unlike this helper's premise, `Maps` itself makes no termination assertion. -/
theorem maps_of_exec_iff {code arguments expected}
    (rule : ∀ actual, Exec code arguments actual ↔ actual = expected) :
    Maps (.program code) arguments expected := by
  intro actual he
  exact (rule actual).mp he

/-- Composition of higher-order behavior specifications uses the exact
intermediate value.  The second program's input cannot be silently replaced by
an unrelated precondition of the same type. -/
theorem maps_append {first second input middle output}
    (left : Maps (.program first) input middle)
    (right : Maps (.program second) middle output) :
    Maps (.program (first ++ second)) input output := by
  intro actual he
  obtain ⟨mid, hfirst, hsecond⟩ := exec_append_iff.mp he
  have hmid := left mid hfirst
  subst mid
  exact right actual hsecond

/-- A convenient bridge for universally quantified, tail-preserving I64
functions.  Arithmetic is left symbolic and stays wrapping at every operation. -/
theorem exportedClaim_unaryI64 {code : List Op} {f : BitVec 64 → BitVec 64}
    (rule : ∀ tail n final,
      Exec code (tail ++ [.i64 n]) final → final = tail ++ [.i64 (f n)]) :
    exportedClaim code [.i64] [.i64] []
      (fun _ _ _ => True)
      (fun before after _ => ∃ n, before = [.i64 n] ∧ after = [.i64 (f n)]) := by
  intro tail before params hi hp hpre final he
  obtain ⟨n, rfl⟩ := (stackTyped_i64_iff before).mp hi
  refine ⟨[.i64 (f n)], rule tail n final he, ?_, n, rfl, rfl⟩
  exact StackTyped.cons HasType.i64 StackTyped.nil

/-- Symbolic constructor inversion for generated, concrete operation lists.
Clients unfold their generated claim/expression definitions first.  Higher-order
unknown programs remain proof obligations rather than receiving guessed specs. -/
macro "noble_contract" : tactic =>
  `(tactic| (repeat intro <;>
    simp_all [NobleContracts.Exec, NobleContracts.Maps,
      List.reverse_append, List.append_assoc]))

end NobleContracts
