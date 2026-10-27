import NobleContracts.Examples
import NobleContracts.Obligation

/-! Specification-quality claims kept strictly separate from implementation
satisfaction (`issue 65`). These are claims about the *retained relation*, not
about any implementation, and a relational claim here never asserts executable
termination, absence of traps, success under quotas, or intended meaning.

Bounded profile: pure `I64`/`Bool` exact ordered stacks, the existing predicate
semantics and wrapping arithmetic. `maps`, program/syntax values, resources,
effects, concurrency and arbitrary recursion are out of scope. -/
namespace NobleContracts.SpecQuality
open NobleContracts

/-- One retained relation: exact ordered input/output/parameter types plus the
two predicate stacks. The implementation claim is deliberately not a field, so
no specification-quality obligation can be discharged by an implementation
proof. -/
structure Spec where
  code : List Op
  input : List Ty
  output : List Ty
  params : List Ty
  pre : Stack → Stack → Stack → Prop
  post : Stack → Stack → Stack → Prop

/-- An admissible assignment: both stacks are exactly typed and the
precondition holds. Ghost parameters never enter execution. -/
def Admissible (spec : Spec) (before params : Stack) : Prop :=
  StackTyped before spec.input ∧ StackTyped params spec.params ∧ spec.pre before [] params

/-- 1. Domain inhabitation: some well-typed assignment satisfies the precondition. -/
def Inhabited (spec : Spec) : Prop :=
  ∃ before params, Admissible spec before params

/-- 2. Relational realizability: every admissible assignment has some exactly
typed output satisfying the postcondition. This is *not* an execution claim. -/
def Realizable (spec : Spec) : Prop :=
  ∀ before params, Admissible spec before params →
    ∃ after, StackTyped after spec.output ∧ spec.post before after params

/-- 3. Output determinacy under the exact scalar equality: two satisfying outputs
for the same assignment are equal. A coarser observational quotient is not
available here, so a producer cannot pick one to make determinacy pass. -/
def Deterministic (spec : Spec) : Prop :=
  ∀ before params after other, Admissible spec before params →
    spec.post before after params → spec.post before other params → after = other

/-- 4. Implementation satisfaction is the retained `exportedClaim`, unchanged. -/
def Satisfied (spec : Spec) : Prop :=
  exportedClaim spec.code spec.input spec.output spec.params spec.pre spec.post

/-- The exact wrapping increment with a true precondition. -/
def incrementSpec (pre : Stack → Stack → Stack → Prop) : Spec where
  code := increment
  input := [.i64]
  output := [.i64]
  params := []
  pre := pre
  post := fun before after _ => ∃ n, before = [.i64 n] ∧ after = [.i64 (n + 1)]

theorem increment_inhabited : Inhabited (incrementSpec fun _ _ _ => True) := by
  refine ⟨[.i64 0], [], ?_, StackTyped.nil, trivial⟩
  exact StackTyped.cons HasType.i64 StackTyped.nil

theorem increment_realizable : Realizable (incrementSpec fun _ _ _ => True) := by
  intro before params admissible
  obtain ⟨n, rfl⟩ := (stackTyped_i64_iff before).mp admissible.1
  refine ⟨[.i64 (n + 1)], StackTyped.cons HasType.i64 StackTyped.nil, n, rfl, rfl⟩

theorem increment_deterministic : Deterministic (incrementSpec fun _ _ _ => True) := by
  intro before params after other _ admissibleAfter admissibleOther
  obtain ⟨n, rfl, hAfter⟩ := admissibleAfter
  obtain ⟨m, hBefore, hOther⟩ := admissibleOther
  cases hBefore
  rw [hOther, hAfter]

/-- 2'. Checked non-realizability evidence: with a contradictory postcondition
every admissible assignment fails, so an admissible input plus a checked
argument refuses realizability. A failed output search would not be evidence. -/
def contradictorySpec : Spec where
  code := increment
  input := [.i64]
  output := [.i64]
  params := []
  pre := fun _ _ _ => True
  post := fun _ _ _ => False

theorem contradictory_not_realizable :
    ¬ Realizable contradictorySpec ∧ Admissible contradictorySpec [.i64 0] [] := by
  refine ⟨?_, ?_⟩
  · intro realizable
    obtain ⟨after, _, falsehood⟩ := realizable [.i64 0] []
      ⟨StackTyped.cons HasType.i64 StackTyped.nil, StackTyped.nil, trivial⟩
    exact falsehood
  · exact ⟨StackTyped.cons HasType.i64 StackTyped.nil, StackTyped.nil, trivial⟩

/-- 4'. A false precondition makes the universal obligation vacuous, and the
report must say so rather than present the claim as establishment. -/
theorem false_precondition_is_vacuous :
    Satisfied (incrementSpec fun _ _ _ => False) ∧
      ¬ Inhabited (incrementSpec fun _ _ _ => False) := by
  refine ⟨?_, ?_⟩
  · intro tail before params _ _ falsehood
    exact False.elim falsehood
  · rintro ⟨before, params, admissible⟩
    exact admissible.2.2

end NobleContracts.SpecQuality
