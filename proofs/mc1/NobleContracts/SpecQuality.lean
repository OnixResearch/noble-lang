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


/-- A retained witness for one relation: the admissible assignment together with a
satisfying output. It carries no solver status, so a searched-but-unreplayed model
cannot be presented as a witness. -/
structure Witness (spec : Spec) (after : Stack) where
  before : Stack
  params : Stack
  admissible : Admissible spec before params
  satisfies : spec.post before after params

/-- Rechecking a witness recomputes the admissible assignment and the satisfaction
from the raw stacks rather than trusting a producer's claim. The result is the
witness itself, so a *failed* recheck cannot produce one. -/
def Recheck (spec : Spec) (before after params : Stack) : Prop :=
  Admissible spec before params ∧ spec.post before after params

def recheck_yields_witness {spec : Spec} {before after params : Stack}
    (checked : Recheck spec before after params) : Witness spec after :=
  ⟨before, params, checked.1, checked.2⟩

theorem witness_recheck_recovers_the_assignment {spec : Spec} {after : Stack}
    (witness : Witness spec after) :
    Recheck spec witness.before after witness.params :=
  ⟨witness.admissible, witness.satisfies⟩

/-- A witness for the underconstrained relation exists while determinacy is refused:
the witness is not evidence of uniqueness. -/
theorem witness_is_not_uniqueness :
    Nonempty (Witness underconstrainedSpec [Value.bool true]) ∧ ¬ Deterministic underconstrainedSpec := by
  refine ⟨⟨[.i64 0], [], ⟨StackTyped.cons HasType.i64 StackTyped.nil, StackTyped.nil, trivial⟩,
    trivial⟩, ?_⟩
  intro deterministic
  have same := deterministic [.i64 0] [] [Value.bool true] [Value.bool false]
    ⟨StackTyped.cons HasType.i64 StackTyped.nil, StackTyped.nil, trivial⟩ trivial trivial
  cases same

/-! ### Submitted evidence is derived, not trusted (issue #65 residual)

A doc comment cannot attach to a mutual block, so this section is documented as prose.
Structural equality of scalar types is an explicit traversal rather than a derived instance, so a
submitted observation relation cannot be read as equal to the retained one by an instance that
silently changes. -/
mutual
  def sameType : Ty → Ty → Bool
    | .unit, .unit => true
    | .bool, .bool => true
    | .i64, .i64 => true
    | .text, .text => true
    | .syntax, .syntax => true
    | .pair left right, .pair otherLeft otherRight =>
        sameType left otherLeft && sameType right otherRight
    | .sum left right, .sum otherLeft otherRight =>
        sameType left otherLeft && sameType right otherRight
    | .list item, .list other => sameType item other
    | .program inputs outputs, .program otherInputs otherOutputs =>
        sameTypes inputs otherInputs && sameTypes outputs otherOutputs
    | _, _ => false
  def sameTypes : List Ty → List Ty → Bool
    | [], [] => true
    | head :: tail, other :: otherTail => sameType head other && sameTypes tail otherTail
    | _, _ => false
end

/-- A submitted evidence artifact (issue #65 residual): it *claims* which obligation it discharges,
which retained relation it is about — kind, observation relation (input/output types) and parameter
binding — and which assignment it asserts. Nothing in it is trusted: acceptance recomputes every
one of those from the retained `Spec` and the raw stacks. -/
structure Submission where
  obligation : Obligation
  input : List Ty
  output : List Ty
  params : List Ty
  before : Stack
  after : Stack
  ghosts : Stack

/-- A submission is evidence only when its claimed kind matches, every claimed observation-relation
and parameter-binding field equals the retained relation's, **and** the raw stacks recheck. A
submission that merely asserts satisfaction satisfies no conjunct of `Recheck` and so is refused. -/
def Accepts (spec : Spec) (submission : Submission) : Prop :=
  submission.obligation = Obligation.specificationQuality ∧
    submission.input = spec.input ∧
    submission.output = spec.output ∧
    submission.params = spec.params ∧
    Recheck spec submission.before submission.after submission.ghosts

/-- The artifact an accepted submission yields is the **derived** witness of the retained
relation, not the submitted claim. -/
def acceptedWitness {spec : Spec} {submission : Submission} (accepted : Accepts spec submission) :
    Witness spec submission.after :=
  recheck_yields_witness accepted.2.2.2.2

/-- A matching increment submission whose raw stacks really recheck is evidence. -/
def incrementSubmission : Submission where
  obligation := Obligation.specificationQuality
  input := [.i64]
  output := [.i64]
  params := []
  before := [.i64 0]
  after := [.i64 1]
  ghosts := []

theorem accepted_submission_yields_the_derived_witness :
    Accepts (incrementSpec fun _ _ _ => True) incrementSubmission := by
  refine ⟨rfl, rfl, rfl, rfl, ?_⟩
  refine ⟨⟨StackTyped.cons HasType.i64 StackTyped.nil, StackTyped.nil, trivial⟩, ?_⟩
  exact ⟨0, rfl, rfl⟩

/-- Wrong **obligation**: evidence submitted for implementation satisfaction cannot be read as
specification-quality evidence for the same relation. -/
theorem wrong_obligation_cannot_be_accepted :
    ¬ Accepts (incrementSpec fun _ _ _ => True)
      { incrementSubmission with obligation := Obligation.implementationSatisfaction } := by
  intro accepted
  cases accepted.1

/-- Wrong **observation relation**: a submission whose claimed output types are not the retained
relation's output types is refused, even when its claimed input, parameters and stacks all match
and do recheck. -/
theorem wrong_observation_relation_cannot_be_accepted :
    ¬ Accepts (incrementSpec fun _ _ _ => True)
      { incrementSubmission with output := [.bool] } := by
  intro accepted
  cases accepted.2.2.1

/-- Wrong **parameter binding**: a submission that binds a ghost parameter the retained relation
does not declare is refused, so an extra parameter cannot stand in for the relation's own. -/
theorem wrong_parameter_binding_cannot_be_accepted :
    ¬ Accepts (incrementSpec fun _ _ _ => True)
      { incrementSubmission with params := [.i64] } := by
  intro accepted
  cases accepted.2.2.2.1

/-- Assertion is not evidence: a submission whose types match but whose raw stacks do not satisfy
the postcondition yields no artifact, because acceptance requires the recomputed `Recheck` rather
than the producer's claim. -/
theorem asserted_satisfaction_yields_no_artifact :
    ¬ Accepts contradictorySpec
      { incrementSubmission with
        before := [.i64 0], after := [.i64 1], ghosts := [] } := by
  intro accepted
  exact accepted.2.2.2.2.2

/-! ### Recheck during shrinking (issue #65 residual)

A witness is only evidence for the assignment it rechecks at. A shrinker that hands back a shorter
ordered input stack must therefore hand it back as a *candidate*: the same `Recheck` applies to the
shrunk stacks, and in this exact ordered profile no strictly shorter assignment can satisfy it.

Refuted first draft: "shrinking preserves the witness", i.e. that a prefix of a witnessing assignment
witnesses the same proposition. Prefixes of an exact ordered stack are typed against a shorter type
list, never the retained one, so the retained relation's own `Admissible` refuses them; the theorem
below states that refusal rather than assuming shrinking away. -/

/-- A shrink proposal: a strictly shorter ordered input stack than the witness used. -/
def Shrinks (before shrunk : Stack) : Prop :=
  shrunk.length < before.length

/-- Exact ordered typing pins the stack length, so a producer cannot choose an assignment length. -/
theorem StackTyped.length_eq {stack : Stack} {types : List Ty} (typed : StackTyped stack types) :
    stack.length = types.length := by
  induction typed with
  | nil => rfl
  | cons _ _ ih => simp only [List.length_cons, List.length_cons, ih]

/-- An admissible assignment's stacks are exactly as long as their declared type lists, so its own
length is pinned by the retained relation rather than by the producer. -/
theorem admissible_pins_its_lengths {spec : Spec} {before params : Stack}
    (admissible : Admissible spec before params) :
    before.length = spec.input.length ∧ params.length = spec.params.length :=
  ⟨StackTyped.length_eq admissible.1, StackTyped.length_eq admissible.2.1⟩

/-- A strictly shorter assignment never rechecks, so a producer cannot shrink a witness and keep the
same evidence: the shrink has to be refused, not trusted. -/
theorem a_shrunk_assignment_never_rechecks {spec : Spec} {before shrunk after params : Stack}
    (shrinks : Shrinks before shrunk) (admissible : Admissible spec before params) :
    ¬ Recheck spec shrunk after params := by
  intro rechecked
  have shrunkLength : shrunk.length = spec.input.length :=
    (admissible_pins_its_lengths rechecked.1).1
  have witnessLength : before.length = spec.input.length :=
    (admissible_pins_its_lengths admissible).1
  unfold Shrinks at shrinks
  rw [shrunkLength, witnessLength] at shrinks
  exact Nat.lt_irrefl spec.input.length shrinks

/-- A prefix candidate examined by a shrinker: the witness assignment with `offset` inputs kept. -/
theorem a_prefix_candidate_is_refused {spec : Spec} {before after params : Stack}
    (admissible : Admissible spec before params) {offset : Nat} (hShorter : offset < before.length) :
    ¬ Recheck spec (before.take offset) after params := by
  have hLength : (before.take offset).length = offset := by
    rw [List.length_take, Nat.min_eq_left (by omega)]
  refine a_shrunk_assignment_never_rechecks ?_ admissible
  unfold Shrinks
  rw [hLength]
  exact hShorter

/-- **Recheck during shrinking, measured**: every strictly shorter candidate assignment of a witness
is refused by the retained relation, so a shrinker may propose candidates but never report a smaller
one as evidence. The statement ranges over all shorter stacks rather than only the prefixes a scan
would visit. -/
theorem every_shorter_candidate_is_refused (spec : Spec) (before after params : Stack)
    (admissible : Admissible spec before params) :
    ∀ candidate : Stack, Shrinks before candidate → ¬ Recheck spec candidate after params :=
  fun _candidate shrinks => a_shrunk_assignment_never_rechecks shrinks admissible

end NobleContracts.SpecQuality
