/-
  Ownership accounting oracle for the ordered stack fragment (issue #59).

  The fragment separates two components of one configuration:

  * an opaque ordered *segment* that no word in this fragment inspects, and
  * an ordered *operand stack* that `dup`, `drop`, `swap` and `quote` touch.

  A payload is either a data reference or a must-use resource, and `wrapped`
  nests payloads so eligibility is recursive. The data-constraining words
  `dup`, `drop` and `quote` require a duplicable operand; `swap` only reorders
  its two operands and has no eligibility check. A payload is duplicable
  exactly when it contains no resource at any depth. Dropping a data reference
  is therefore not releasing a host resource, and no word in this fragment can
  release a must-use obligation.

  The executable checker `run` and the declarative judgment `Accounts` are
  written independently of the production acceptance machine: this file names
  the mathematics it claims, and the Rust side compares against it rather than
  re-deriving an expectation from the implementation under test.
-/
namespace NobleFoundation.Ownership

/-- One ordered-stack payload. -/
inductive Payload where
  | data (tag : Nat)
  | resource (tag : Nat)
  | wrapped (inner : Payload)
deriving DecidableEq, Repr

/-- Whether any must-use resource occurs at any wrapping depth. -/
def containsResource : Payload → Bool
  | .data _ => false
  | .resource _ => true
  | .wrapped inner => containsResource inner

/-- The word fragment: no reborrows, aggregates, loops or suspension. -/
inductive Word where
  | dup
  | drop
  | swap
  | quote
deriving DecidableEq, Repr

/-- A payload is duplicable exactly when it contains no must-use resource. -/
def duplicable (payload : Payload) : Bool :=
  decide (containsResource payload = false)

/-- One checked step over the operand stack; the head is the operand a word
    constrains. A step rejects rather than inventing an obligation when the
    operand it would touch is not duplicable. -/
def step : Word → List Payload → Option (List Payload)
  | .dup, payload :: rest =>
      if duplicable payload then some (payload :: payload :: rest) else none
  | .drop, payload :: rest =>
      if duplicable payload then some rest else none
  | .swap, left :: right :: rest =>
      some (right :: left :: rest)
  | .quote, payload :: rest =>
      if duplicable payload then some (payload :: rest) else none
  | _, _ => none

/-- The finite two-arm fragment: words, composition, a two-arm branch and the
    ordinary error-return alternative. -/
inductive Program where
  | word (w : Word)
  | compose (first second : Program)
  | branch (thenArm elseArm : Program)
  | fail
deriving DecidableEq, Repr

/-- The executable checker. A branch is admitted only when both arms leave the
    same operand accounting. A normal error return is admitted only when no
    operand obligation is outstanding, so it never licenses abandoning one. -/
def run : Program → List Payload → Option (List Payload)
  | .word w, operands => step w operands
  | .compose first second, operands => run first operands >>= run second
  | .branch thenArm elseArm, operands =>
      if run thenArm operands = run elseArm operands then run thenArm operands else none
  | .fail, operands => if operands.isEmpty then some operands else none

/-- The declarative judgment the checker answers. -/
inductive Accounts : Program → List Payload → List Payload → Prop where
  | word {w operands out} : step w operands = some out → Accounts (.word w) operands out
  | compose {first second start middle out} :
      Accounts first start middle →
      Accounts second middle out →
      Accounts (.compose first second) start out
  | branch {thenArm elseArm start out} :
      Accounts thenArm start out →
      Accounts elseArm start out →
      Accounts (.branch thenArm elseArm) start out
  | fail : Accounts .fail [] []

/-- A payload is duplicable exactly when no resource occurs at any depth. -/
theorem duplicable_iff (payload : Payload) :
    duplicable payload = true ↔ containsResource payload = false := by
  unfold duplicable
  exact decide_eq_true_iff

theorem data_duplicable (tag : Nat) : duplicable (.data tag) = true := by
  rw [duplicable_iff]
  rfl

theorem resource_not_duplicable (tag : Nat) : duplicable (.resource tag) = false := by
  unfold duplicable containsResource
  rfl

/-- Eligibility is recursive: wrapping a payload never changes it. -/
theorem wrapped_duplicable (inner : Payload) :
    duplicable (.wrapped inner) = duplicable inner := by
  simp [duplicable, containsResource]

theorem wrapped_data_duplicable (tag : Nat) :
    duplicable (.wrapped (.data tag)) = true := by
  rw [wrapped_duplicable, data_duplicable]

theorem wrapped_resource_not_duplicable (tag : Nat) :
    duplicable (.wrapped (.resource tag)) = false := by
  rw [wrapped_duplicable, resource_not_duplicable]

/-- No wrapping depth makes a resource eligible. -/
def wrapDepth : Nat → Payload → Payload
  | 0, payload => payload
  | depth + 1, payload => .wrapped (wrapDepth depth payload)

theorem wrapDepth_preserves_eligibility (depth : Nat) (payload : Payload) :
    duplicable (wrapDepth depth payload) = duplicable payload := by
  induction depth with
  | zero => rfl
  | succ depth hypothesis =>
      unfold wrapDepth
      rw [wrapped_duplicable, hypothesis]

theorem wrapDepth_resource_not_duplicable (depth : Nat) (tag : Nat) :
    duplicable (wrapDepth depth (.resource tag)) = false := by
  rw [wrapDepth_preserves_eligibility, resource_not_duplicable]

/-- A must-use resource as the constrained operand blocks every data-constraining word. -/
theorem resource_operand_blocks_non_swap_word (word : Word) (tag : Nat)
    (rest : List Payload) (notSwap : word ≠ .swap) :
    step word (.resource tag :: rest) = none := by
  cases word with
  | dup => simp [step, resource_not_duplicable]
  | drop => simp [step, resource_not_duplicable]
  | swap => exact (notSwap rfl).elim
  | quote => simp [step, resource_not_duplicable]

/-- Dropping consumes exactly the duplicable operand it inspected. -/
theorem drop_consumes_one (payload : Payload) (rest : List Payload)
    (eligible : duplicable payload = true) : step .drop (payload :: rest) = some rest := by
  simp [step, eligible]

/-- Duplication preserves both copies of the duplicable operand. -/
theorem dup_duplicates_one (payload : Payload) (rest : List Payload)
    (eligible : duplicable payload = true) :
    step .dup (payload :: rest) = some (payload :: payload :: rest) := by
  simp [step, eligible]

/-- Quoting retains the obligation it inspected rather than consuming it. -/
theorem quote_retains_one (payload : Payload) (rest : List Payload)
    (eligible : duplicable payload = true) : step .quote (payload :: rest) = some (payload :: rest) := by
  simp [step, eligible]

/-- Swapping permutes two operands and consumes neither, including resources. -/
theorem swap_permutes_two (left right : Payload) (rest : List Payload)
    : step .swap (left :: right :: rest) = some (right :: left :: rest) := by
  simp [step]

/-- A decided fixture shows that swap moves a resource without an eligibility check. -/
theorem swap_moves_a_resource :
    run (.word .swap) [.resource 1, .data 2] = some [.data 2, .resource 1] := by
  decide

/-- The checker is sound for the declarative judgment. -/
theorem run_sound (program : Program) :
    ∀ (operands out : List Payload), run program operands = some out → Accounts program operands out := by
  induction program with
  | word word => intro operands out accepted; exact Accounts.word accepted
  | compose first second firstSound secondSound =>
      intro operands out accepted
      unfold run at accepted
      cases firstRun : run first operands with
      | none => simp [firstRun] at accepted
      | some middle =>
          simp only [firstRun] at accepted
          exact Accounts.compose (firstSound operands middle firstRun)
            (secondSound middle out accepted)
  | branch thenArm elseArm thenSound elseSound =>
      intro operands out accepted
      unfold run at accepted
      cases thenRun : run thenArm operands with
      | none =>
          cases elseRun : run elseArm operands <;> simp [thenRun, elseRun] at accepted
      | some thenOut =>
          cases elseRun : run elseArm operands with
          | none => simp [thenRun, elseRun] at accepted
          | some elseOut =>
              simp [thenRun, elseRun] at accepted
              obtain ⟨sameThen, sameElse⟩ := accepted
              subst sameThen
              subst sameElse
              exact Accounts.branch (thenSound operands thenOut thenRun)
                (elseSound operands thenOut elseRun)
  | fail =>
      intro operands out accepted
      cases operands with
      | nil =>
          simp [run] at accepted
          subst accepted
          exact Accounts.fail
      | cons payload rest => simp [run] at accepted

/-- A branch accounts for the same obligations on both arms. -/
theorem branch_accounts_both_arms (thenArm elseArm : Program) (start out : List Payload)
    (accounts : Accounts (.branch thenArm elseArm) start out) :
    Accounts thenArm start out ∧ Accounts elseArm start out := by
  cases accounts with
  | branch thenAccounts elseAccounts => exact ⟨thenAccounts, elseAccounts⟩

/-- A normal error return is admitted only with no outstanding operand. -/
theorem fail_requires_empty_operands (start out : List Payload)
    (accounts : Accounts .fail start out) : start = [] ∧ out = [] := by
  cases accounts with
  | fail => exact ⟨rfl, rfl⟩

/-- An outstanding operand is never abandoned by the error-return alternative. -/
theorem fail_rejects_outstanding_operand (payload : Payload) (rest : List Payload) :
    run .fail (payload :: rest) = none := by
  unfold run
  simp

/-- The opaque segment is untouched: the fragment separates the segment from
    the operand stack by construction, so the must-use obligations it carries
    are conserved by every admitted program. -/
def runConfiguration (segment : List Payload) (program : Program)
    (operands : List Payload) : Option (List Payload × List Payload) :=
  match run program operands with
  | some out => some (segment, out)
  | none => none

theorem segment_conserved (segment : List Payload) (program : Program)
    (operands out : List Payload)
    (accepted : runConfiguration segment program operands = some (segment, out)) :
    run program operands = some out := by
  unfold runConfiguration at accepted
  cases programRun : run program operands with
  | none => simp [programRun] at accepted
  | some actual =>
      simp [programRun] at accepted
      subst accepted
      rfl

/-! ## Branch join: the abstract stack two accepted arms share (issue #59 joins)

A join is *not* "both branches have the same number of resources". It is a position-wise record of
**both** arms' payloads, so an owner identity is never collapsed into another one and a nested
payload is never forgotten because it is eligible or because the counts agree.

The joined stack is built from both arms, so it represents each of them by construction; `mergePosition`
is the per-position merge, and the controls show it never loses an arm's identity. The substantive
theorem is `joined_word_admission_agrees`: over the straight-line fragment a data-constraining
word's admission depends on per-position eligibility, while swap only needs two positions, so a
stack pair whose positions agree on eligibility is admitted in both arms or in neither.

`OwnershipJudgment.lean` lifts the same simulation over whole straight-line word sequences
(`joined_sequence_admission_agrees`), where each step keeps the intermediate stacks agreeing, and
then over the whole fragment: result agreement holds for every program
(`accepted_results_agree`), and admission agreement for every branch-free one, the error return
included (`branch_free_admission_agrees`). Branch admission does not lift
(`branch_admission_does_not_lift`): the branch rule compares the two arms' actual results, so a join
can never be used to skip accounting one arm (`branch_accounts_both_arms` and the mutation controls
cover that case).
-/

/-- One joined position: the same payload in both arms, or two conflicting ones. -/
inductive Merged where
  | same (payload : Payload)
  | conflicting (left right : Payload)
deriving DecidableEq, Repr

/-- Merge one position. Identical payloads collapse to `same`; anything else, including a data
reference against a wrapped payload, is recorded as `conflicting`. -/
def mergePosition (left right : Payload) : Merged :=
  if left = right then .same left else .conflicting left right

/-- The abstract stack two accepted arms share: both arm results, at equal length. The length premise is
the join obligation — a position one arm lacks is not a shared obligation and is never padded. -/
structure BranchJoin where
  leftArm : List Payload
  rightArm : List Payload
  sameLength : leftArm.length = rightArm.length

/-- The position-wise merge of both arms. -/
def BranchJoin.merged (join : BranchJoin) : List Merged :=
  List.zipWith (fun left right => mergePosition left right) join.leftArm join.rightArm

/-- **Representation**: a merge never loses either arm's payload at a position. -/
theorem merge_represents_each_arm (left right : Payload) :
    ((∃ payload, mergePosition left right = .same payload ∧ payload = left ∧ payload = right) ∨
      mergePosition left right = .conflicting left right) := by
  unfold mergePosition
  by_cases same : left = right
  · rw [if_pos same]
    exact Or.inl ⟨left, rfl, rfl, same⟩
  · rw [if_neg same]
    exact Or.inr rfl

/-- **Distinct owners are never conflated**: two different payloads at the same position are recorded
as conflicting rather than merged, so a join cannot pick one arm's owner and drop the other's. -/
theorem distinct_owners_are_not_merged (left right : Payload) (different : left ≠ right) :
    mergePosition left right = .conflicting left right := by
  unfold mergePosition
  rw [if_neg different]

/-- The seeded one-armed join — keeping only the first arm at every position — is refuted wherever the
arms differ, so it cannot be accepted as a join. -/
theorem one_armed_join_is_refuted (left right : Payload) (different : left ≠ right) :
    (Merged.same left) ≠ mergePosition left right := by
  rw [distinct_owners_are_not_merged left right different]
  simp only [ne_eq, reduceCtorEq, not_false_eq_true]

/-- Equal counts with different identities are the control a "same number of resources" join fails:
one resource against another is conflicting. -/
theorem same_count_different_identity_is_conflicting :
    mergePosition (.resource 1) (.resource 2) = .conflicting (.resource 1) (.resource 2) := by
  decide

/-- A nested payload is not forgotten because it is eligible: a wrapped data reference against a plain
one is conflicting, even though both are duplicable. -/
theorem nested_payload_is_not_merged_with_its_unwrapping :
    mergePosition (.wrapped (.data 1)) (.data 1) = .conflicting (.wrapped (.data 1)) (.data 1) := by
  decide

/-- A wrapped must-use resource is never merged with a plain payload. -/
theorem nested_resource_is_not_merged :
    mergePosition (.wrapped (.resource 7)) (.data 1) =
      .conflicting (.wrapped (.resource 7)) (.data 1) := by
  decide

/-- A join of arms of different lengths is not a `BranchJoin` at all: the premise is not optional and
cannot be discharged by agreeing counts. -/
theorem unequal_arms_cannot_be_joined :
    ([.data 1] : List Payload).length ≠ ([.data 1, .data 2] : List Payload).length := by
  decide

/-- The eligibility class of an ordered stack: one decision per position, in order. -/
def eligibilityClass (operands : List Payload) : List Bool := operands.map duplicable

/-- Position-wise agreement on eligibility, at the same ordered positions and with the same length. -/
def EligibilityAgrees (left right : List Payload) : Prop :=
  eligibilityClass left = eligibilityClass right

/-- **Word admission is decided by arity and eligibility**: two stacks whose positions agree on
eligibility are admitted by one data-constraining word in both arms or in neither, while swap only
needs its two operands. Thus the joined abstract stack preserves this fragment's accounting decision
rather than inventing one. -/
theorem joined_word_admission_agrees (word : Word) (left right : List Payload)
    (agree : EligibilityAgrees left right) :
    (step word left).isSome = (step word right).isSome := by
  cases word with
  | dup =>
      cases left with
      | nil => cases right <;> simp_all [EligibilityAgrees, eligibilityClass]
      | cons head rest =>
          cases right with
          | nil => simp_all [EligibilityAgrees, eligibilityClass]
          | cons other tail =>
              simp only [EligibilityAgrees, eligibilityClass, List.map_cons, List.cons.injEq] at agree
              simp only [step]
              rw [agree.1]
              split <;> rfl
  | drop =>
      cases left with
      | nil => cases right <;> simp_all [EligibilityAgrees, eligibilityClass]
      | cons head rest =>
          cases right with
          | nil => simp_all [EligibilityAgrees, eligibilityClass]
          | cons other tail =>
              simp only [EligibilityAgrees, eligibilityClass, List.map_cons, List.cons.injEq] at agree
              simp only [step]
              rw [agree.1]
              split <;> rfl
  | quote =>
      cases left with
      | nil => cases right <;> simp_all [EligibilityAgrees, eligibilityClass]
      | cons head rest =>
          cases right with
          | nil => simp_all [EligibilityAgrees, eligibilityClass]
          | cons other tail =>
              simp only [EligibilityAgrees, eligibilityClass, List.map_cons, List.cons.injEq] at agree
              simp only [step]
              rw [agree.1]
              split <;> rfl
  | swap =>
      cases left with
      | nil => cases right <;> simp_all [EligibilityAgrees, eligibilityClass]
      | cons head rest =>
          cases rest with
          | nil =>
              cases right with
              | nil => rfl
              | cons _ remaining =>
                  cases remaining with
                  | nil => simp only [step, Option.isSome_none]
                  | cons _ _ => simp_all [EligibilityAgrees, eligibilityClass]
          | cons second tail =>
              cases right with
              | nil => simp_all [EligibilityAgrees, eligibilityClass]
              | cons other remaining =>
                  cases remaining with
                  | nil => simp_all [EligibilityAgrees, eligibilityClass]
                  | cons otherSecond otherTail =>
                      simp only [step, Option.isSome_some]

end NobleFoundation.Ownership
