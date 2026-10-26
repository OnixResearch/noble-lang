/-
  Ownership accounting oracle for the ordered stack fragment (issue #59).

  The fragment separates two components of one configuration:

  * an opaque ordered *segment* that no word in this fragment inspects, and
  * an ordered *operand stack* that `dup`, `drop`, `swap` and `quote` touch.

  A payload is either a data reference or a must-use resource, and `wrapped`
  nests payloads so eligibility is recursive. Every word that constrains a data
  operand requires that operand to be duplicable, and a payload is duplicable
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
      if duplicable left && duplicable right then some (right :: left :: rest) else none
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

/-- A must-use resource as the constrained operand blocks every word. -/
theorem resource_operand_blocks_every_word (word : Word) (tag : Nat)
    (rest : List Payload) : step word (.resource tag :: rest) = none := by
  cases word <;> cases rest <;> simp [step, resource_not_duplicable]

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

/-- Swapping permutes two duplicable operands and consumes neither. -/
theorem swap_permutes_two (left right : Payload) (rest : List Payload)
    (leftEligible : duplicable left = true) (rightEligible : duplicable right = true) :
    step .swap (left :: right :: rest) = some (right :: left :: rest) := by
  simp [step, leftEligible, rightEligible]

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

end NobleFoundation.Ownership
