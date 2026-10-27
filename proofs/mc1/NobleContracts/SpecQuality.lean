import NobleContracts.Examples
import NobleContracts.Obligation
import NobleContracts.NamedV2

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


/-- An underconstrained Bool-output relation over the same input type. It makes
no implementation claim and is intentionally relational in its outputs, so
determinacy must be *refused* here rather than assumed. -/
def underconstrainedSpec : Spec where
  code := []
  input := [.i64]
  output := [.bool]
  params := []
  pre := fun _ _ _ => True
  post := fun _ _ _ => True

/-- Criterion 3: one admissible input with two checked, distinct, exactly typed
satisfying outputs. Distinctness is decided, so this is evidence rather than a
statement that some other output might exist. -/
theorem underconstrained_has_two_outputs :
    Admissible underconstrainedSpec [.i64 0] [] ∧
      StackTyped ([Value.bool true] : Stack) underconstrainedSpec.output ∧
      StackTyped ([Value.bool false] : Stack) underconstrainedSpec.output ∧
      underconstrainedSpec.post [.i64 0] [Value.bool true] [] ∧
      underconstrainedSpec.post [.i64 0] [Value.bool false] [] ∧
      ¬ Deterministic underconstrainedSpec := by
  refine ⟨⟨StackTyped.cons HasType.i64 StackTyped.nil, StackTyped.nil, trivial⟩,
    StackTyped.cons HasType.bool StackTyped.nil,
    StackTyped.cons HasType.bool StackTyped.nil, trivial, trivial, ?_⟩
  intro deterministic
  have same := deterministic [.i64 0] [] [Value.bool true] [Value.bool false]
    ⟨StackTyped.cons HasType.i64 StackTyped.nil, StackTyped.nil, trivial⟩ trivial trivial
  cases same

/-- Criterion 7, Lean side: an undefined predicate is never satisfied, so a
relation whose postcondition is that predicate is not realizable given an
admissible input. Exhaustion and unsupported syntax are refused by the same
rule — a missing value is never a successful check. -/
theorem undefined_predicate_never_satisfied (before after params : Stack) :
    ¬ Holds (.not (.head .nil)) before after params := by
  intro holds
  simp [Holds] at holds
  nomatch holds

/-- A relation whose postcondition is an undefined predicate is not realizable,
and the admissible input is retained as the witness. -/
def undefinedPostSpec : Spec where
  code := increment
  input := [.i64]
  output := [.i64]
  params := []
  pre := fun _ _ _ => True
  post := fun before after params => Holds (.not (.head .nil)) before after params

theorem undefined_post_not_realizable :
    ¬ Realizable undefinedPostSpec ∧ Admissible undefinedPostSpec [.i64 0] [] := by
  refine ⟨?_, ⟨StackTyped.cons HasType.i64 StackTyped.nil, StackTyped.nil, trivial⟩⟩
  intro realizable
  obtain ⟨after, _, holds⟩ := realizable [.i64 0] []
    ⟨StackTyped.cons HasType.i64 StackTyped.nil, StackTyped.nil, trivial⟩
  exact undefined_predicate_never_satisfied [.i64 0] after [] holds


/-- Criterion 6, typing half: every obligation carries its own kind, so evidence
submitted for one obligation cannot be read as evidence for another. A consumer
selects the kind explicitly; nothing here lets an implementation proof stand in
for a specification-quality proof. -/
inductive Obligation where
  | specificationQuality
  | implementationSatisfaction

/-- Evidence is indexed by the obligation it discharges and by the proposition
it actually proves. -/
structure Claim (obligation : Obligation) (prop : Prop) : Prop where
  evidence : prop

theorem obligation_kinds_distinct :
    Obligation.specificationQuality ≠ Obligation.implementationSatisfaction := by
  intro same
  cases same

/-- The same retained relation carries two separately typed evidence records:
determinacy refusal is a *specification-quality* claim, while the implementation
claim stays the unchanged `exportedClaim` at *implementation-satisfaction* kind.
Neither can be substituted for the other because both the kind and the proved
proposition differ. -/
theorem separate_evidence_for_separate_obligations :
    Claim .specificationQuality (¬ Deterministic underconstrainedSpec) ∧
      Claim .implementationSatisfaction
        (Satisfied (incrementSpec fun _ _ _ => False)) := by
  refine ⟨⟨?_⟩, ⟨?_⟩⟩
  · intro deterministic
    have same := deterministic [.i64 0] [] [Value.bool true] [Value.bool false]
      ⟨StackTyped.cons HasType.i64 StackTyped.nil, StackTyped.nil, trivial⟩ trivial trivial
    cases same
  · intro tail before params _ _ falsehood
    exact False.elim falsehood

end NobleContracts.SpecQuality
