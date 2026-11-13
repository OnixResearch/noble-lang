/-
  Separate duplication and discard permissions alongside capture eligibility (issue #112).

  The current contract has one eligibility predicate, `Data(a)`, granting both operations at once,
  plus a resource class granting neither. This module models the alternative the issue asks to
  evaluate: two independent bits, so four classes, and asks what the split can and cannot buy.

  * `Permission` is the pair `⟨dup, drop⟩`. `unrestricted` is today's `Data`, `mustUse` today's
    resource class, and `atMostOnce` / `atLeastOnce` are the two cells the two-class model cannot
    name. `split_adds_no_permission` proves the split is *below* `Data`: no cell grants an operation
    that `Data` does not, so the only effect of the split is to withhold one of the two.
  * `combine` is the recursive rule for `Pair`, `Sum`, nominal wrappers and lists. It is the
    pointwise conjunction, so a compound may duplicate or discard exactly when every payload may,
    and no wrapper launders a must-use payload into data.
  * `capturable` is `dup && drop`, and on a two-bit lattice that is exactly `unrestricted`
    (`capturable_iff_unrestricted`): a separate `Capture` predicate adds no cell beyond `Data`.
  * `step` is the ordered-stack table for `dup`, `drop`, `swap`, `dip`, `quote`, `compose` and
    `run`. `dip`, `compose` and `run` only reorder or consume program positions and constrain no
    permission; program positions are always unrestricted, whatever their environment names.
  * `signatureStack`/`signatureStep`/`signatureRun` state that a step and a whole run depend only on
    each position's `(permission, kind)` signature, so a joined stack that agrees positionwise is
    accepted by the same words and leaves agreeing results (`run_admission_agrees`,
    `run_results_agree`).
  * `BranchJoin`/`mergePosition` is the two-arm join: a missing position is not padded, and a
    duplicated obligation is not merged. `fail` is the ordinary error return: it discards the
    stack, so it is allowed only when every position is discardable. This rule is deliberately
    wider than `Ownership.fail`, which refuses every non-empty stack; the two rules are separate
    models and neither relabels the other (`ownership_fail_refuses_data_stack` records the
    difference as a theorem rather than leaving it to prose).
  * `matrix` is a kernel-checked table of retained decisions. Each `seeded*` rule is deliberately
    wrong and every theorem names the retained control that refutes it, the forbidden
    capture-then-duplicate chain included.

  This is a model, not a statement about the selected profile: no source form carries a separate
  duplicate or discard permission, the production kernel has one `is_data` predicate, and
  `verification/use-permissions/` records the finite comparison and the decision. This file does
  not edit or relabel `Ownership*.lean`, which remains the single authority for resource
  accounting.
-/
import NobleFoundation.Ownership

namespace NobleFoundation.UsePermissions

/-- One permission pair: the two operations the current `Data` predicate grants together. -/
structure Permission where
  /-- Permission to duplicate the value. -/
  dup : Bool
  /-- Permission to discard the value. -/
  drop : Bool
deriving DecidableEq, Repr, BEq

/-- Today's `Data` class: both bits set. -/
def Permission.unrestricted : Permission := ⟨true, true⟩

/-- Today's resource class: neither bit set, so exactly-once. -/
def Permission.mustUse : Permission := ⟨false, false⟩

/-- Affine: discardable but not duplicable. Absent from today's two-class model. -/
def Permission.atMostOnce : Permission := ⟨false, true⟩

/-- Must-handle: duplicable but not discardable. Absent from today's two-class model. -/
def Permission.atLeastOnce : Permission := ⟨true, false⟩

/-- Recursive eligibility for `Pair`, `Sum`, nominal wrappers and lists: the pointwise
conjunction, so a compound is as permissive as its stricter payload. -/
def Permission.combine (left right : Permission) : Permission :=
  ⟨left.dup && right.dup, left.drop && right.drop⟩

theorem combine_commutative (left right : Permission) :
    left.combine right = right.combine left := by
  obtain ⟨leftDup, leftDrop⟩ := left
  obtain ⟨rightDup, rightDrop⟩ := right
  cases leftDup <;> cases rightDup <;> cases leftDrop <;> cases rightDrop <;> rfl

theorem combine_associative (first second third : Permission) :
    (first.combine second).combine third = first.combine (second.combine third) := by
  obtain ⟨firstDup, firstDrop⟩ := first
  obtain ⟨secondDup, secondDrop⟩ := second
  obtain ⟨thirdDup, thirdDrop⟩ := third
  cases firstDup <;> cases secondDup <;> cases thirdDup <;>
    cases firstDrop <;> cases secondDrop <;> cases thirdDrop <;> rfl

theorem combine_idempotent (permission : Permission) :
    permission.combine permission = permission := by
  obtain ⟨dup, drop⟩ := permission
  cases dup <;> cases drop <;> rfl

/-- The unrestricted pair is the combine identity, so a payload adds no restriction. -/
theorem combine_unrestricted_left (permission : Permission) :
    Permission.unrestricted.combine permission = permission := by
  obtain ⟨dup, drop⟩ := permission
  cases dup <;> cases drop <;> rfl

theorem combine_unrestricted_right (permission : Permission) :
    permission.combine Permission.unrestricted = permission := by
  obtain ⟨dup, drop⟩ := permission
  cases dup <;> cases drop <;> rfl

/-- A must-use payload makes the whole compound must-use: a wrapper cannot launder it. -/
theorem combine_must_use_left (permission : Permission) :
    Permission.mustUse.combine permission = Permission.mustUse := by
  obtain ⟨dup, drop⟩ := permission
  cases dup <;> cases drop <;> rfl

theorem combine_must_use_right (permission : Permission) :
    permission.combine Permission.mustUse = Permission.mustUse := by
  obtain ⟨dup, drop⟩ := permission
  cases dup <;> cases drop <;> rfl

/-- A compound is unrestricted exactly when both payloads are. -/
theorem combine_unrestricted_iff (left right : Permission) :
    left.combine right = Permission.unrestricted ↔
      (left = Permission.unrestricted ∧ right = Permission.unrestricted) := by
  obtain ⟨leftDup, leftDrop⟩ := left
  obtain ⟨rightDup, rightDrop⟩ := right
  cases leftDup <;> cases rightDup <;> cases leftDrop <;> cases rightDrop <;> decide

/-- The four cells, and only those four, are permissions. -/
theorem cases_of_permission (permission : Permission) :
    permission = Permission.unrestricted ∨ permission = Permission.mustUse ∨
      permission = Permission.atMostOnce ∨ permission = Permission.atLeastOnce := by
  obtain ⟨dup, drop⟩ := permission
  cases dup <;> cases drop <;> decide

/-- **The split grants nothing new.** `Data` was already the maximum of the two bits, so every
permission is below it: a four-way model can only withhold duplication or discard, never add an
operation that `Data` does not already allow. -/
theorem split_adds_no_permission (permission : Permission) :
    permission.dup = true → permission.drop = true → permission = Permission.unrestricted := by
  obtain ⟨dup, drop⟩ := permission
  intro _dup
  intro _drop
  cases dup <;> cases drop <;> simp_all [Permission.unrestricted]

/-- Capture eligibility: both bits, exactly as `Data` is defined. -/
def Permission.capturable (permission : Permission) : Bool := permission.dup && permission.drop

/-- On a two-bit lattice, capture eligibility *is* `Data`: a separate capture predicate adds no
cell, which is why the production kernel routes `quote` through `is_data`. -/
theorem capturable_iff_unrestricted (permission : Permission) :
    permission.capturable = true ↔ permission = Permission.unrestricted := by
  obtain ⟨dup, drop⟩ := permission
  cases dup <;> cases drop <;> decide

theorem capturable_iff_bits (permission : Permission) :
    permission.capturable = true ↔ (permission.dup = true ∧ permission.drop = true) := by
  obtain ⟨dup, drop⟩ := permission
  cases dup <;> cases drop <;> decide

theorem must_use_not_capturable : Permission.mustUse.capturable = false := rfl

theorem at_most_once_not_capturable : Permission.atMostOnce.capturable = false := rfl

theorem at_least_once_not_capturable : Permission.atLeastOnce.capturable = false := rfl

/-- An opaque nominal wrapper exposes exactly its payload's permission. -/
def opaqueWrapper (inner : Permission) : Permission := inner

/-- A product requires both payloads to be eligible for the corresponding operation. -/
def pairOf (left right : Permission) : Permission := left.combine right

/-- Either arm of a sum may be exposed, so both arms must be eligible. -/
def sumOf (left right : Permission) : Permission := left.combine right

/-- A two-arm nominal wrapper combines both arms, because elimination may expose either. -/
def variantWrapper (left right : Permission) : Permission := left.combine right

/-- A list exposes its element's permission, as `List<a>` inherits the predicate from `a`. -/
def listOf (element : Permission) : Permission := element

theorem opaque_wrapper_preserves (inner : Permission) : opaqueWrapper inner = inner := rfl

theorem variant_wrapper_of_must_use (right : Permission) :
    variantWrapper Permission.mustUse right = Permission.mustUse :=
  combine_must_use_left right

theorem pair_of_must_use_is_must_use (right : Permission) :
    pairOf Permission.mustUse right = Permission.mustUse :=
  combine_must_use_left right

theorem sum_of_must_use_is_must_use (right : Permission) :
    sumOf Permission.mustUse right = Permission.mustUse :=
  combine_must_use_left right

theorem list_of_must_use_is_must_use : listOf Permission.mustUse = Permission.mustUse := rfl

/-- Nested Pair/Sum/nominal/List eligibility remains recursive; no layer launders a must-use
payload into an unrestricted value. -/
theorem nested_pair_sum_list_must_use :
    listOf (opaqueWrapper (pairOf Permission.unrestricted
      (sumOf (variantWrapper Permission.mustUse Permission.unrestricted)
        Permission.unrestricted))) = Permission.mustUse := by
  decide

/-- Wrapping depth: `opaqueWrapper` composed with itself. -/
def wrapDepth : Nat → Permission → Permission
  | 0, permission => permission
  | depth + 1, permission => opaqueWrapper (wrapDepth depth permission)

/-- No wrapping depth changes a permission (K-TYPE-02 in the permission model). -/
theorem wrap_depth_preserves (depth : Nat) (permission : Permission) :
    wrapDepth depth permission = permission := by
  induction depth with
  | zero => rfl
  | succ depth hypothesis => simp only [wrapDepth, opaqueWrapper, hypothesis]

theorem wrapped_must_use_is_must_use (depth : Nat) :
    wrapDepth depth Permission.mustUse = Permission.mustUse :=
  wrap_depth_preserves depth Permission.mustUse

/-- An opaque wrapper around a must-use payload is not capturable, so no wrapper earns capture. -/
theorem wrapped_must_use_not_capturable (depth : Nat) :
    (wrapDepth depth Permission.mustUse).capturable = false := by
  rw [wrap_depth_preserves]
  rfl

/-- One ordered-stack position is a value or a program. -/
inductive Kind where
  | value
  | program
deriving DecidableEq, Repr, BEq

/-- One ordered-stack position: an observation tag, its kind and its permission. The tag exists so
a join can see that two positions are different without seeing a permission difference. -/
structure Item where
  kind : Kind
  tag : Nat
  permission : Permission
deriving DecidableEq, Repr, BEq

def Item.data (tag : Nat) : Item := ⟨Kind.value, tag, Permission.unrestricted⟩

def Item.mustUse (tag : Nat) : Item := ⟨Kind.value, tag, Permission.mustUse⟩

def Item.atMostOnce (tag : Nat) : Item := ⟨Kind.value, tag, Permission.atMostOnce⟩

def Item.atLeastOnce (tag : Nat) : Item := ⟨Kind.value, tag, Permission.atLeastOnce⟩

def Item.program (tag : Nat) : Item := ⟨Kind.program, tag, Permission.unrestricted⟩

def Item.isProgram (item : Item) : Bool := decide (item.kind = Kind.program)

/-- The observable part of a position: what a step and a join can depend on. -/
def Item.signature (item : Item) : Permission × Kind := (item.permission, item.kind)

def signatureStack (stack : List Item) : List (Permission × Kind) := stack.map Item.signature

@[simp] theorem signatureStack_nil : signatureStack [] = [] := rfl

@[simp] theorem signatureStack_cons (item : Item) (rest : List Item) :
    signatureStack (item :: rest) = Item.signature item :: signatureStack rest := rfl

/-- A program value carries the permissions of the payloads its environment captured. -/
structure ProgramValue where
  environment : List Permission
deriving DecidableEq, Repr

/-- Every valid program is data and capturable, whatever its input interface names (SPEC §4.3). -/
def ProgramValue.permission (_ : ProgramValue) : Permission := Permission.unrestricted

theorem program_is_unrestricted (value : ProgramValue) :
    value.permission = Permission.unrestricted := rfl

theorem program_is_capturable (value : ProgramValue) : value.permission.capturable = true := rfl

/-- The word fragment. `dip`, `compose` and `run` take program positions; no word in this fragment
releases a host object. -/
inductive Word where
  | dup
  | drop
  | swap
  | dip
  | quote
  | compose
  | run
deriving DecidableEq, Repr

/-- The checked ordered-stack table.

`dup` and `drop` require their own bit. `swap` only reorders two positions and constrains no
permission. `dip` (`S a Program<S,T,e> -- T a ! e`) holds one position and consumes the program
position without constraining either. `quote` requires capture and produces a program position,
which *is* unrestricted: the captured permission is replaced by the program's, never carried.
`compose` (`S Program Program -- S Program`) and `run` consume program positions and constrain no
permission. -/
def step : Word → List Item → Option (List Item)
  | .dup, item :: rest =>
      if item.permission.dup then some (item :: item :: rest) else none
  | .drop, item :: rest => if item.permission.drop then some rest else none
  | .swap, left :: right :: rest => some (right :: left :: rest)
  | .dip, held :: program :: rest => if program.isProgram then some (held :: rest) else none
  | .quote, item :: rest =>
      if item.permission.capturable then some (Item.program item.tag :: rest) else none
  | .compose, top :: under :: rest =>
      if top.isProgram then
        if under.isProgram then some (Item.program top.tag :: rest) else none
      else none
  | .run, program :: rest => if program.isProgram then some rest else none
  | _, _ => none

/-- The ordinary error return: it discards the stack, so every position must be discardable. -/
def fail (stack : List Item) : Option (List Item) :=
  if stack.all (fun item => item.permission.drop) then some [] else none

/-- A straight-line word sequence. -/
def run : List Word → List Item → Option (List Item)
  | [], stack => some stack
  | word :: rest, stack => step word stack >>= run rest

/-- The two-arm branch: each arm runs from the same stack, and the branch is accepted only when both
arms succeed and leave the identical stack. A join therefore cannot skip accounting one arm. -/
def branch (thenArm elseArm : List Word) (start : List Item) : Option (List Item) :=
  match run thenArm start, run elseArm start with
  | some left, some right => if left = right then some left else none
  | _, _ => none

/-- The step on signatures only: the same table with the tag erased. -/
def signatureStep : Word → List (Permission × Kind) → Option (List (Permission × Kind))
  | .dup, signature :: rest =>
      if signature.1.dup then some (signature :: signature :: rest) else none
  | .drop, signature :: rest => if signature.1.drop then some rest else none
  | .swap, left :: right :: rest => some (right :: left :: rest)
  | .dip, held :: program :: rest =>
      if signatureStep.signProgram program then some (held :: rest) else none
  | .quote, signature :: rest =>
      if signature.1.capturable then
        some ((Permission.unrestricted, Kind.program) :: rest)
      else none
  | .compose, top :: under :: rest =>
      if signatureStep.signProgram top then
        if signatureStep.signProgram under then
          some ((Permission.unrestricted, Kind.program) :: rest)
        else none
      else none
  | .run, program :: rest => if signatureStep.signProgram program then some rest else none
  | _, _ => none
where
  /-- A signature names a program position. -/
  signProgram : Permission × Kind → Bool
    | (_, kind) => decide (kind = Kind.program)

private theorem isSome_map (option : Option α) (map : α → β) :
    (Option.map map option).isSome = option.isSome := by
  cases option <;> rfl

private theorem signatureStep_dup (stack : List Item) :
    Option.map signatureStack (step .dup stack) = signatureStep .dup (signatureStack stack) := by
  cases stack with
  | nil => rfl
  | cons item rest =>
      cases bit : item.permission.dup <;>
        simp [step, signatureStep, bit, Item.signature]

private theorem signatureStep_drop (stack : List Item) :
    Option.map signatureStack (step .drop stack) = signatureStep .drop (signatureStack stack) := by
  cases stack with
  | nil => rfl
  | cons item rest =>
      cases bit : item.permission.drop <;>
        simp [step, signatureStep, bit, Item.signature]

private theorem signatureStep_swap (stack : List Item) :
    Option.map signatureStack (step .swap stack) = signatureStep .swap (signatureStack stack) := by
  cases stack with
  | nil => rfl
  | cons item rest =>
      cases rest with
      | nil => rfl
      | cons second tail => simp [step, signatureStep, Item.signature]

private theorem signatureStep_dip (stack : List Item) :
    Option.map signatureStack (step .dip stack) = signatureStep .dip (signatureStack stack) := by
  cases stack with
  | nil => rfl
  | cons item rest =>
      cases rest with
      | nil => rfl
      | cons program tail =>
          by_cases isProgram : program.kind = Kind.program
          · have holds : program.isProgram = true := by
              unfold Item.isProgram
              exact decide_eq_true_iff.mpr isProgram
            simp [step, signatureStep, signatureStep.signProgram, isProgram, Item.signature,
              holds]
          · have notProgram : program.isProgram = false := by
              unfold Item.isProgram
              exact decide_eq_false_iff_not.mpr isProgram
            simp [step, signatureStep, signatureStep.signProgram, isProgram, Item.signature,
              notProgram]

private theorem signatureStep_quote (stack : List Item) :
    Option.map signatureStack (step .quote stack) = signatureStep .quote (signatureStack stack) := by
  cases stack with
  | nil => rfl
  | cons item rest =>
      cases bit : item.permission.capturable <;>
        simp [step, signatureStep, bit, Item.signature, Item.program, Permission.unrestricted]

private theorem signatureStep_compose (stack : List Item) :
    Option.map signatureStack (step .compose stack) =
      signatureStep .compose (signatureStack stack) := by
  cases stack with
  | nil => rfl
  | cons top rest =>
      cases rest with
      | nil => rfl
      | cons under tail =>
          by_cases topProgram : top.kind = Kind.program
          · by_cases underProgram : under.kind = Kind.program
            · simp [step, signatureStep, signatureStep.signProgram, topProgram, underProgram,
                Item.isProgram, Item.signature, Item.program, Permission.unrestricted]
            · simp [step, signatureStep, signatureStep.signProgram, topProgram, underProgram,
                Item.isProgram, Item.signature]
          · simp [step, signatureStep, signatureStep.signProgram, topProgram, Item.isProgram,
              Item.signature]

private theorem signatureStep_run (stack : List Item) :
    Option.map signatureStack (step .run stack) = signatureStep .run (signatureStack stack) := by
  cases stack with
  | nil => rfl
  | cons program rest =>
      by_cases isProgram : program.kind = Kind.program
      · simp [step, signatureStep, signatureStep.signProgram, isProgram, Item.isProgram,
          Item.signature]
      · simp [step, signatureStep, signatureStep.signProgram, isProgram, Item.isProgram,
          Item.signature]

/-- A step depends only on the signature stack: the same table with the tags erased. -/
theorem step_signature (word : Word) (stack : List Item) :
    Option.map signatureStack (step word stack) =
      signatureStep word (signatureStack stack) := by
  cases word with
  | dup => exact signatureStep_dup stack
  | drop => exact signatureStep_drop stack
  | swap => exact signatureStep_swap stack
  | dip => exact signatureStep_dip stack
  | quote => exact signatureStep_quote stack
  | compose => exact signatureStep_compose stack
  | run => exact signatureStep_run stack

/-- The step's admission decision depends only on the signature stack. -/
theorem step_admission_agrees (word : Word) (left right : List Item)
    (agree : signatureStack left = signatureStack right) :
    (step word left).isSome = (step word right).isSome := by
  have leftSide : (step word left).isSome =
      (signatureStep word (signatureStack left)).isSome := by
    rw [← step_signature word left, isSome_map]
  have rightSide : (step word right).isSome =
      (signatureStep word (signatureStack right)).isSome := by
    rw [← step_signature word right, isSome_map]
  rw [leftSide, rightSide, agree]

/-- The run on signatures only. -/
def signatureRun : List Word → List (Permission × Kind) → Option (List (Permission × Kind))
  | [], signatures => some signatures
  | word :: rest, signatures => signatureStep word signatures >>= signatureRun rest

private theorem signatureRun_eq (words : List Word) (stack : List Item) :
    Option.map signatureStack (run words stack) = signatureRun words (signatureStack stack) := by
  induction words generalizing stack with
  | nil => rfl
  | cons word rest hypothesis =>
      simp only [run, signatureRun]
      cases stepped : step word stack with
      | none =>
          have erased := step_signature word stack
          rw [stepped] at erased
          simp only [Option.map_none] at erased
          have empty : signatureStep word (signatureStack stack) = none := erased.symm
          rw [empty]
          rfl
      | some next =>
          have erased := step_signature word stack
          rw [stepped] at erased
          simp only [Option.map_some] at erased
          rw [← erased]
          show Option.map signatureStack (run rest next) =
            signatureRun rest (signatureStack next)
          exact hypothesis next

/-- Admission of a straight-line sequence depends only on the signature stack. -/
theorem run_admission_agrees (words : List Word) (left right : List Item)
    (agree : signatureStack left = signatureStack right) :
    (run words left).isSome = (run words right).isSome := by
  have leftSide : (run words left).isSome =
      (signatureRun words (signatureStack left)).isSome := by
    rw [← signatureRun_eq words left, isSome_map]
  have rightSide : (run words right).isSome =
      (signatureRun words (signatureStack right)).isSome := by
    rw [← signatureRun_eq words right, isSome_map]
  rw [leftSide, rightSide, agree]

/-- Accepted runs of two agreeing stacks leave agreeing results. -/
theorem run_results_agree (words : List Word) (left right leftOut rightOut : List Item)
    (agree : signatureStack left = signatureStack right)
    (leftRun : run words left = some leftOut) (rightRun : run words right = some rightOut) :
    signatureStack leftOut = signatureStack rightOut := by
  have leftSide := signatureRun_eq words left
  have rightSide := signatureRun_eq words right
  rw [leftRun] at leftSide
  rw [rightRun] at rightSide
  simp only [Option.map_some] at leftSide rightSide
  have same : signatureRun words (signatureStack left) = signatureRun words (signatureStack right) := by
    rw [agree]
  rw [same] at leftSide
  rw [← rightSide] at leftSide
  exact Option.some.inj leftSide

/-- One joined position: the same position in both arms, or two conflicting positions. -/
inductive Merged where
  | same (item : Item)
  | conflicting (left right : Item)
deriving DecidableEq, Repr, BEq

/-- Merge one position. Identical positions collapse to `same`; anything else, including two
different tags with the same permission, is recorded as conflicting. -/
def mergePosition (left right : Item) : Merged :=
  if left = right then .same left else .conflicting left right

/-- The abstract stack two accepted arms share, at equal length. The length premise is the join
obligation: a position one arm lacks is not a shared obligation and is never padded. -/
structure BranchJoin where
  leftArm : List Item
  rightArm : List Item
  sameLength : leftArm.length = rightArm.length

/-- The position-wise merge of both arms. -/
def BranchJoin.merged (join : BranchJoin) : List Merged :=
  List.zipWith (fun left right => mergePosition left right) join.leftArm join.rightArm

/-- **Representation**: a merge never loses either arm's position. -/
theorem merge_represents_each_arm (left right : Item) :
    ((∃ item, mergePosition left right = .same item ∧ item = left ∧ item = right) ∨
      mergePosition left right = .conflicting left right) := by
  unfold mergePosition
  by_cases same : left = right
  · rw [ite_eq_left same]
    exact Or.inl ⟨left, rfl, rfl, same⟩
  · rw [ite_eq_right same]
    exact Or.inr rfl

/-- Distinct positions are never merged, so a join cannot pick one arm's obligation and drop the
other's. -/
theorem distinct_positions_are_not_merged (left right : Item) (different : left ≠ right) :
    mergePosition left right = .conflicting left right := by
  unfold mergePosition
  rw [ite_eq_right different]

/-- The seeded one-armed join — keeping only the first arm at every position — is refuted wherever
the arms differ. -/
theorem one_armed_join_is_refuted (left right : Item) (different : left ≠ right) :
    (Merged.same left) ≠ mergePosition left right := by
  rw [distinct_positions_are_not_merged left right different]
  simp only [ne_eq, reduceCtorEq, not_false_eq_true]

/-- Equal counts with different owners are conflicting: identity is not a count. -/
theorem same_count_different_identity_is_conflicting :
    mergePosition (Item.mustUse 1) (Item.mustUse 2) =
      .conflicting (Item.mustUse 1) (Item.mustUse 2) := by
  decide

/-- Two positions with the same permission but different tags are still conflicting, so a permission
join cannot stand in for an owner join. -/
theorem same_permission_different_tag_is_conflicting :
    mergePosition (Item.atLeastOnce 1) (Item.atLeastOnce 2) =
      .conflicting (Item.atLeastOnce 1) (Item.atLeastOnce 2) := by
  decide

/-- A join of arms of different lengths is not a `BranchJoin` at all: the premise is not optional
and cannot be discharged by agreeing counts. -/
theorem unequal_arms_cannot_be_joined :
    ([Item.data 1] : List Item).length ≠ ([Item.data 1, Item.data 2] : List Item).length := by
  decide

/-- A duplicated obligation in one arm cannot be merged: the extra position has no counterpart. -/
theorem duplicated_obligation_cannot_be_joined :
    branch [.dup] [] [Item.atLeastOnce 1] = none := by
  decide

/-! ### Decided obligations -/

theorem dup_requires_dup_bit (item : Item) (rest : List Item) (holds : item.permission.dup = true) :
    step .dup (item :: rest) = some (item :: item :: rest) := by
  simp [step, holds]

theorem dup_refused_without_dup_bit (item : Item) (rest : List Item)
    (missing : item.permission.dup = false) : step .dup (item :: rest) = none := by
  simp [step, missing]

theorem drop_requires_drop_bit (item : Item) (rest : List Item)
    (holds : item.permission.drop = true) : step .drop (item :: rest) = some rest := by
  simp [step, holds]

theorem drop_refused_without_drop_bit (item : Item) (rest : List Item)
    (missing : item.permission.drop = false) : step .drop (item :: rest) = none := by
  simp [step, missing]

/-- `swap` moves either permission class without an eligibility check. -/
theorem swap_permutes_two (left right : Item) (rest : List Item) :
    step .swap (left :: right :: rest) = some (right :: left :: rest) := rfl

/-- `dip` restores the held position and consumes the program position, constraining no
permission. -/
theorem dip_restores_and_consumes_program (held : Item) (tag : Nat) (rest : List Item) :
    step .dip (held :: Item.program tag :: rest) = some (held :: rest) := by
  first | rfl | simp [step, Item.isProgram, Item.program]

/-- `dip` refuses a non-program position in the program slot. -/
theorem dip_requires_a_program (held other : Item) (rest : List Item)
    (notProgram : other.isProgram = false) : step .dip (held :: other :: rest) = none := by
  simp [step, notProgram]

/-- `quote` accepts only a capturable position and produces an unrestricted program position: the
captured permission is replaced, never carried into the program value. -/
theorem quote_replaces_the_permission (item : Item) (rest : List Item)
    (capturable : item.permission.capturable = true) :
    step .quote (item :: rest) = some (Item.program item.tag :: rest) := by
  simp [step, capturable]

theorem quote_refused_without_capture (item : Item) (rest : List Item)
    (refused : item.permission.capturable = false) : step .quote (item :: rest) = none := by
  simp [step, refused]

/-- `compose` produces one unrestricted program position and constrains no permission. -/
theorem compose_produces_a_program (tag other : Nat) (rest : List Item) :
    step .compose (Item.program tag :: Item.program other :: rest) =
      some (Item.program tag :: rest) := by
  first | rfl | simp [step, Item.isProgram, Item.program]

/-- `run` consumes a program position, constraining no permission. -/
theorem run_consumes_the_program (tag : Nat) (rest : List Item) :
    step .run (Item.program tag :: rest) = some rest := by
  first | rfl | simp [step, Item.isProgram, Item.program]

/-- A program position is always unrestricted, so duplicating it is accepted. -/
theorem program_dup_accepted (tag : Nat) :
    step .dup [Item.program tag] = some [Item.program tag, Item.program tag] := by
  first | rfl | simp [step, Item.isProgram, Item.program, Permission.unrestricted]

/-! ### The ordinary error return -/

theorem fail_requires_discardable (stack : List Item) (out : List Item)
    (accepted : fail stack = some out) :
    ∀ item ∈ stack, item.permission.drop = true := by
  unfold fail at accepted
  by_cases all : stack.all (fun item => item.permission.drop) = true
  · intro item member
    exact List.all_eq_true.mp all item member
  · simp [all] at accepted

/-- The error return is refused while a must-use obligation is outstanding. -/
theorem fail_refuses_must_use (tag : Nat) : fail [Item.mustUse tag] = none := by
  first | rfl | simp [fail, Item.mustUse, Permission.mustUse]

/-- The error return is refused while a must-handle obligation is outstanding. -/
theorem fail_refuses_must_handle (tag : Nat) : fail [Item.atLeastOnce tag] = none := by
  first | rfl | simp [fail, Item.atLeastOnce, Permission.atLeastOnce]

/-- The error return discharges an affine (at-most-once) position, which is exactly what its drop
bit grants. -/
theorem fail_discharges_at_most_once (tag : Nat) : fail [Item.atMostOnce tag] = some [] := by
  first | rfl | simp [fail, Item.atMostOnce, Permission.atMostOnce]

/-- The error return discards plain data without an obligation, unlike `Ownership.fail`, which is
deliberately stricter. -/
theorem fail_accepts_data_stack (first second : Nat) :
    fail [Item.data first, Item.data second] = some [] := by
  first | rfl | simp [fail, Item.data, Permission.unrestricted]

/-- The difference from `Ownership.fail` as a theorem: the ownership fragment refuses to abandon a
duplicable data operand on the error return, while the permission model permits it. -/
theorem ownership_fail_refuses_data_stack :
    NobleFoundation.Ownership.run .fail [NobleFoundation.Ownership.Payload.data 1] = none := by
  decide

/-- **Protection of Program semantics**: an accepted step never produces a program position with a
constrained permission, provided the input stack already satisfies that invariant. -/
theorem step_program_positions_are_unrestricted (word : Word) (stack out : List Item)
    (accepted : step word stack = some out)
    (wellFormed : ∀ item ∈ stack, item.kind = Kind.program →
      item.permission = Permission.unrestricted) :
    ∀ item ∈ out, item.kind = Kind.program → item.permission = Permission.unrestricted := by
  cases word with
  | dup =>
      cases stack with
      | nil => simp [step] at accepted
      | cons head rest =>
          simp only [step] at accepted
          split at accepted
          · simp only [Option.some.injEq] at accepted
            have outEq : out = head :: head :: rest := accepted.symm
            intro candidate member isProgram
            rw [outEq] at member
            rcases List.mem_cons.mp member with same | later
            · rw [same] at isProgram ⊢
              exact wellFormed head (by simp) isProgram
            · rcases List.mem_cons.mp later with same | later2
              · rw [same] at isProgram ⊢
                exact wellFormed head (by simp) isProgram
              · exact wellFormed candidate (by simp [later2]) isProgram
          · simp at accepted
  | drop =>
      cases stack with
      | nil => simp [step] at accepted
      | cons head rest =>
          simp only [step] at accepted
          split at accepted
          · simp only [Option.some.injEq] at accepted
            have outEq : out = rest := accepted.symm
            intro candidate member
            rw [outEq] at member
            exact wellFormed candidate (List.mem_of_mem_tail member)
          · simp at accepted
  | swap =>
      cases stack with
      | nil => simp [step] at accepted
      | cons first rest =>
          cases rest with
          | nil => simp [step] at accepted
          | cons second tail =>
              simp only [step, Option.some.injEq] at accepted
              have outEq : out = second :: first :: tail := accepted.symm
              intro candidate member isProgram
              rw [outEq] at member
              rcases List.mem_cons.mp member with same | later
              · rw [same] at isProgram ⊢
                exact wellFormed second (by simp) isProgram
              · rcases List.mem_cons.mp later with same | later2
                · rw [same] at isProgram ⊢
                  exact wellFormed first (by simp) isProgram
                · exact wellFormed candidate (by simp [later2]) isProgram
  | dip =>
      cases stack with
      | nil => simp [step] at accepted
      | cons held rest =>
          cases rest with
          | nil => simp [step] at accepted
          | cons program tail =>
              simp only [step] at accepted
              split at accepted
              · simp only [Option.some.injEq] at accepted
                have outEq : out = held :: tail := accepted.symm
                intro candidate member isProgram
                rw [outEq] at member
                rcases List.mem_cons.mp member with same | later
                · rw [same] at isProgram ⊢
                  exact wellFormed held (by simp) isProgram
                · exact wellFormed candidate (by simp [later]) isProgram
              · simp at accepted
  | quote =>
      cases stack with
      | nil => simp [step] at accepted
      | cons head rest =>
          simp only [step] at accepted
          split at accepted
          · simp only [Option.some.injEq] at accepted
            have outEq : out = Item.program head.tag :: rest := accepted.symm
            intro candidate member isProgram
            rw [outEq] at member
            rcases List.mem_cons.mp member with same | later
            · rw [same] at isProgram ⊢
              rfl
            · exact wellFormed candidate (by simp [later]) isProgram
          · simp at accepted
  | compose =>
      cases stack with
      | nil => simp [step] at accepted
      | cons top rest =>
          cases rest with
          | nil => simp [step] at accepted
          | cons under tail =>
              simp only [step] at accepted
              split at accepted
              · split at accepted
                · simp only [Option.some.injEq] at accepted
                  have outEq : out = Item.program top.tag :: tail := accepted.symm
                  intro candidate member isProgram
                  rw [outEq] at member
                  rcases List.mem_cons.mp member with same | later
                  · rw [same] at isProgram ⊢
                    rfl
                  · exact wellFormed candidate (by simp [later]) isProgram
                · simp at accepted
              · simp at accepted
  | run =>
      cases stack with
      | nil => simp [step] at accepted
      | cons program rest =>
          simp only [step] at accepted
          split at accepted
          · simp only [Option.some.injEq] at accepted
            have outEq : out = rest := accepted.symm
            intro candidate member
            rw [outEq] at member
            exact wellFormed candidate (List.mem_of_mem_tail member)
          · simp at accepted

/-- A program value's environment is capturable, so duplicating or dropping the program never
multiplies or discards a constrained payload. -/
theorem captured_environment_is_capturable (environment : List Permission)
    (captured : ∀ permission ∈ environment, permission.capturable = true) :
    ∀ permission ∈ environment, permission = Permission.unrestricted := by
  intro permission member
  exact (capturable_iff_unrestricted permission).mp (captured permission member)

/-! ### The control matrix

Each row is one retained decision: a word, error return or branch applied to a concrete stack, with
the decision the model claims. `matrix_holds` is kernel-checked by `decide`, so the table is not
prose. -/

inductive Action where
  | word (word : Word)
  | fail
  | branch (thenArm elseArm : List Word)
deriving DecidableEq, Repr

def Action.apply : Action → List Item → Option (List Item)
  | .word operation, stack => step operation stack
  | .fail, stack => NobleFoundation.UsePermissions.fail stack
  | .branch thenArm elseArm, stack =>
      NobleFoundation.UsePermissions.branch thenArm elseArm stack

structure Case where
  name : String
  action : Action
  stack : List Item
  expected : Option (List Item)

/-- A retained row: the model's decision is the row's expected decision, compared as data. -/
def Case.holds (case : Case) : Bool := case.action.apply case.stack == case.expected

def matrix : List Case := [
  ⟨"dup-data", .word .dup, [Item.data 1], some [Item.data 1, Item.data 1]⟩,
  ⟨"dup-must-use", .word .dup, [Item.mustUse 1], none⟩,
  ⟨"dup-must-handle", .word .dup, [Item.atLeastOnce 1],
    some [Item.atLeastOnce 1, Item.atLeastOnce 1]⟩,
  ⟨"dup-affine", .word .dup, [Item.atMostOnce 1], none⟩,
  ⟨"nested-wrapper-data-captures", .word .quote,
    [Item.mk Kind.value 10
      (listOf (pairOf Permission.unrestricted
        (sumOf (variantWrapper Permission.unrestricted Permission.unrestricted)
          Permission.unrestricted)))],
    some [Item.program 10]⟩,
  ⟨"nested-wrapper-must-use-refuses-dup", .word .dup,
    [Item.mk Kind.value 11
      (listOf (pairOf Permission.unrestricted
        (sumOf (variantWrapper Permission.mustUse Permission.unrestricted)
          Permission.unrestricted)))], none⟩,
  ⟨"nested-wrapper-affine-allows-drop", .word .drop,
    [Item.mk Kind.value 12
      (listOf (pairOf Permission.unrestricted
        (sumOf (variantWrapper Permission.atMostOnce Permission.unrestricted)
          Permission.unrestricted)))], some []⟩,
  ⟨"nested-wrapper-must-handle-refuses-drop", .word .drop,
    [Item.mk Kind.value 13
      (listOf (pairOf Permission.unrestricted
        (sumOf (variantWrapper Permission.atLeastOnce Permission.unrestricted)
          Permission.unrestricted)))], none⟩,
  ⟨"drop-data", .word .drop, [Item.data 1], some []⟩,
  ⟨"drop-must-use", .word .drop, [Item.mustUse 1], none⟩,
  ⟨"drop-affine", .word .drop, [Item.atMostOnce 1], some []⟩,
  ⟨"drop-must-handle", .word .drop, [Item.atLeastOnce 1], none⟩,
  ⟨"swap-two-permissions", .word .swap, [Item.mustUse 1, Item.data 2],
    some [Item.data 2, Item.mustUse 1]⟩,
  ⟨"swap-needs-two", .word .swap, [Item.mustUse 1], none⟩,
  ⟨"quote-data", .word .quote, [Item.data 3], some [Item.program 3]⟩,
  ⟨"quote-must-use", .word .quote, [Item.mustUse 3], none⟩,
  ⟨"quote-must-handle", .word .quote, [Item.atLeastOnce 3], none⟩,
  ⟨"quote-affine", .word .quote, [Item.atMostOnce 3], none⟩,
  ⟨"quote-program", .word .quote, [Item.program 4], some [Item.program 4]⟩,
  ⟨"dip-restores", .word .dip, [Item.mustUse 5, Item.program 6], some [Item.mustUse 5]⟩,
  ⟨"dip-needs-a-program", .word .dip, [Item.data 5, Item.data 6], none⟩,
  ⟨"compose-programs", .word .compose, [Item.program 7, Item.program 8], some [Item.program 7]⟩,
  ⟨"compose-needs-programs", .word .compose, [Item.mustUse 7, Item.program 8], none⟩,
  ⟨"run-consumes-a-program", .word .run, [Item.program 9, Item.data 1], some [Item.data 1]⟩,
  ⟨"run-needs-a-program", .word .run, [Item.mustUse 9], none⟩,
  ⟨"fail-empty", .fail, [], some []⟩,
  ⟨"fail-data", .fail, [Item.data 1, Item.data 2], some []⟩,
  ⟨"fail-affine", .fail, [Item.atMostOnce 1], some []⟩,
  ⟨"fail-must-use", .fail, [Item.mustUse 1], none⟩,
  ⟨"fail-must-handle", .fail, [Item.atLeastOnce 1], none⟩,
  ⟨"branch-dup-then-drop", .branch [.dup, .drop] [.dup, .drop], [Item.data 1],
    some [Item.data 1]⟩,
  ⟨"branch-equal-arms", .branch [.swap] [.swap], [Item.data 1, Item.data 2],
    some [Item.data 2, Item.data 1]⟩,
  ⟨"branch-unequal-arms", .branch [.swap] [], [Item.data 1, Item.data 2], none⟩,
  ⟨"branch-duplicates-must-handle", .branch [.dup] [], [Item.atLeastOnce 1], none⟩,
  ⟨"branch-discards-obligation", .branch [.swap] [.drop], [Item.mustUse 1, Item.data 2], none⟩,
  ⟨"branch-drops-then-uses", .branch [.drop] [.swap], [Item.mustUse 1, Item.data 2], none⟩,
  ⟨"branch-must-handle-duplicate", .branch [.dup, .drop] [.swap],
    [Item.atLeastOnce 1, Item.data 2], none⟩,
  ⟨"branch-programs", .branch [.compose] [.compose], [Item.program 1, Item.program 2],
    some [Item.program 1]⟩
]

/-- **The retained control matrix is kernel-checked**: every row's decision is the model's. -/
theorem matrix_holds : matrix.all Case.holds = true := by decide

/-! ### Seeded wrong rules

Each rule changes one decision of the reference table. Every theorem names a retained control that
refutes it, so a rule cannot be weakened silently: the control is the case the reference refuses and
the seeded rule accepts, or the reverse. -/

/-- A parameterised rule table: the reference is `Rules.correct`. -/
structure Rules where
  dupAllows : Permission → Bool
  dropAllows : Permission → Bool
  quoteAllows : Permission → Bool
  programPermission : List Permission → Permission
  failAllows : List Item → Bool
  keepFirstArm : Bool

def Rules.correct : Rules where
  dupAllows := fun permission => permission.dup
  dropAllows := fun permission => permission.drop
  quoteAllows := fun permission => permission.capturable
  programPermission := fun _ => Permission.unrestricted
  failAllows := fun stack => stack.all (fun item => item.permission.drop)
  keepFirstArm := false

def stepWith (rules : Rules) : Word → List Item → Option (List Item)
  | .dup, item :: rest =>
      if rules.dupAllows item.permission then some (item :: item :: rest) else none
  | .drop, item :: rest => if rules.dropAllows item.permission then some rest else none
  | .swap, left :: right :: rest => some (right :: left :: rest)
  | .dip, held :: program :: rest => if program.isProgram then some (held :: rest) else none
  | .quote, item :: rest =>
      if rules.quoteAllows item.permission then
        some (Item.mk Kind.program item.tag (rules.programPermission [item.permission]) :: rest)
      else none
  | .compose, top :: under :: rest =>
      if top.isProgram then
        if under.isProgram then
          some (Item.mk Kind.program top.tag (rules.programPermission []) :: rest)
        else none
      else none
  | .run, program :: rest => if program.isProgram then some rest else none
  | _, _ => none

def failWith (rules : Rules) (stack : List Item) : Option (List Item) :=
  if rules.failAllows stack then some [] else none

def runWith (rules : Rules) : List Word → List Item → Option (List Item)
  | [], stack => some stack
  | word :: rest, stack => stepWith rules word stack >>= runWith rules rest

def branchWith (rules : Rules) (thenArm elseArm : List Word) (start : List Item) :
    Option (List Item) :=
  match runWith rules thenArm start, runWith rules elseArm start with
  | some left, some right =>
      if rules.keepFirstArm then some left else if left = right then some left else none
  | _, _ => none

/-- Seeded rule 1: discard ignores its bit. Refuted by a must-use position. -/
def seededDropAlways : Rules := { Rules.correct with dropAllows := fun _ => true }

/-- Seeded rule 2: duplication ignores its bit. Refuted by a must-use position. -/
def seededDupAlways : Rules := { Rules.correct with dupAllows := fun _ => true }

/-- Seeded rule 3: capture requires only the duplicate bit. Refuted by a must-handle position. -/
def seededQuoteDupOnly : Rules :=
  { Rules.correct with quoteAllows := fun permission => permission.dup }

/-- Seeded rule 4: capture requires only the discard bit. Refuted by an affine position. -/
def seededQuoteDropOnly : Rules :=
  { Rules.correct with quoteAllows := fun permission => permission.drop }

/-- Seeded rule 5: capture ignores permissions entirely. Refuted by a must-use position. -/
def seededQuoteAlways : Rules := { Rules.correct with quoteAllows := fun _ => true }

/-- Seeded rule 6: the error return ignores outstanding obligations. -/
def seededFailAlways : Rules := { Rules.correct with failAllows := fun _ => true }

/-- Seeded rule 7: the join keeps the first arm instead of comparing both. -/
def seededKeepFirstArm : Rules := { Rules.correct with keepFirstArm := true }

/-- Seeded rule 8: a program inherits its environment's combined permission instead of being
unrestricted. -/
def seededProgramInherits : Rules :=
  { Rules.correct with
    programPermission :=
      fun environment => environment.foldl Permission.combine Permission.unrestricted }

theorem seeded_drop_always_refuted_by_must_use :
    (stepWith seededDropAlways .drop [Item.mustUse 1]).isSome = true ∧
      (step .drop [Item.mustUse 1]).isSome = false := by
  decide

theorem seeded_dup_always_refuted_by_must_use :
    (stepWith seededDupAlways .dup [Item.mustUse 1]).isSome = true ∧
      (step .dup [Item.mustUse 1]).isSome = false := by
  decide

theorem seeded_quote_dup_only_refuted_by_must_handle :
    (stepWith seededQuoteDupOnly .quote [Item.atLeastOnce 7]).isSome = true ∧
      (step .quote [Item.atLeastOnce 7]).isSome = false := by
  decide

theorem seeded_quote_drop_only_refuted_by_at_most_once :
    (stepWith seededQuoteDropOnly .quote [Item.atMostOnce 7]).isSome = true ∧
      (step .quote [Item.atMostOnce 7]).isSome = false := by
  decide

theorem seeded_quote_always_refuted_by_must_use :
    (stepWith seededQuoteAlways .quote [Item.mustUse 7]).isSome = true ∧
      (step .quote [Item.mustUse 7]).isSome = false := by
  decide

theorem seeded_fail_always_refuted_by_must_use :
    (failWith seededFailAlways [Item.mustUse 1]).isSome = true ∧
      (fail [Item.mustUse 1]).isSome = false := by
  decide

theorem seeded_join_keeping_one_arm_refuted :
    (branchWith seededKeepFirstArm [.swap] [] [Item.data 1, Item.data 2]).isSome = true ∧
      (branch [.swap] [] [Item.data 1, Item.data 2]).isSome = false := by
  decide

/-- Seeded rule 8 is refuted by the selected profile's own invariant: a valid program is data, so
dropping a program position must be allowed, while inheriting a must-handle environment makes the
program non-discardable and would silently discard that obligation. -/
theorem seeded_program_inherits_refuted :
    (stepWith seededProgramInherits .drop
        [Item.mk Kind.program 7
          (seededProgramInherits.programPermission [Permission.atLeastOnce])]).isSome = false ∧
      (step .drop [Item.program 7]).isSome = true := by
  decide

/-- **Forbidden capture followed by Program duplication.** Under the duplicate-only capture rule, a
must-handle payload is quoted into a program position, the program is duplicated, and both copies
are discarded: the obligation disappears without being handled. The reference refuses at the
quotation, so the chain never starts. -/
theorem forbidden_capture_then_program_duplication :
    (stepWith seededQuoteDupOnly .quote [Item.atLeastOnce 7] >>=
        stepWith seededQuoteDupOnly .dup >>= stepWith seededQuoteDupOnly .drop >>=
        stepWith seededQuoteDupOnly .drop).isSome = true ∧
      (step .quote [Item.atLeastOnce 7]).isSome = false := by
  decide

/-- The reference capture rule, as the seed refutation states it: capture requires both bits, so a
must-handle position is never quoted. -/
theorem capture_requires_both_bits (permission : Permission)
    (capturable : permission.capturable = true) :
    permission.dup = true ∧ permission.drop = true :=
  (capturable_iff_bits permission).mp capturable

end NobleFoundation.UsePermissions
