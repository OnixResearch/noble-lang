/-
  Independent per-word rules and the straight-line join lift (issue #59).

  `Accounts.word` in `Ownership.lean` takes `step w operands = some out` as its premise, so the
  declarative judgment's word rules delegate to the executable checker. This module states one
  rule per word from the fragment's prose instead — `dup`, `drop` and `quote` need an operand
  with no resource at any depth, and `swap` reorders two operands unconditionally — and proves
  the executable `step` agrees with them exactly (`step_iff_wordRule`). `Judged` is the judgment
  built on those rules; it coincides with `Accounts` (`judged_iff_accounts`), and with the new
  completeness theorem (`run_complete`) the checker decides it exactly (`run_iff_judged`).

  `joined_word_admission_agrees` showed that two stacks agreeing on eligibility are admitted by
  one word in both arms or in neither. The lift recorded there as remaining work is proved here
  for whole straight-line word sequences: each step preserves the agreement
  (`step_preserves_agreement`), so a straight-line program is admitted on both stacks or on
  neither, and leaves stacks that agree again (`joined_sequence_admission_agrees`). Branches and
  the error return compare actual results and stay outside the lift.

  Nothing here relates the model to `crates/noble-kernel/src/acceptance`.
-/
import NobleFoundation.Ownership

namespace NobleFoundation.Ownership

/-! ## Per-word rules, independent of `step` -/

/-- One rule per word, stated from the fragment's prose. -/
inductive WordRule : Word → List Payload → List Payload → Prop where
  | dup {payload : Payload} {rest : List Payload} :
      containsResource payload = false →
      WordRule .dup (payload :: rest) (payload :: payload :: rest)
  | drop {payload : Payload} {rest : List Payload} :
      containsResource payload = false → WordRule .drop (payload :: rest) rest
  | swap {left right : Payload} {rest : List Payload} :
      WordRule .swap (left :: right :: rest) (right :: left :: rest)
  | quote {payload : Payload} {rest : List Payload} :
      containsResource payload = false → WordRule .quote (payload :: rest) (payload :: rest)

/-- **The executable step agrees with the per-word rules exactly.** -/
theorem step_iff_wordRule (word : Word) (operands out : List Payload) :
    step word operands = some out ↔ WordRule word operands out := by
  constructor
  · intro accepted
    cases word with
    | dup =>
        match operands, accepted with
        | payload :: rest, accepted =>
            simp only [step] at accepted
            split at accepted
            · rename_i eligible
              cases accepted
              exact .dup ((duplicable_iff payload).1 eligible)
            · cases accepted
    | drop =>
        match operands, accepted with
        | payload :: rest, accepted =>
            simp only [step] at accepted
            split at accepted
            · rename_i eligible
              cases accepted
              exact .drop ((duplicable_iff payload).1 eligible)
            · cases accepted
    | swap =>
        match operands, accepted with
        | left :: right :: rest, accepted =>
            simp only [step] at accepted
            cases accepted
            exact .swap
    | quote =>
        match operands, accepted with
        | payload :: rest, accepted =>
            simp only [step] at accepted
            split at accepted
            · rename_i eligible
              cases accepted
              exact .quote ((duplicable_iff payload).1 eligible)
            · cases accepted
  · intro rule
    cases rule with
    | dup noResource => simp [step, (duplicable_iff _).2 noResource]
    | drop noResource => simp [step, (duplicable_iff _).2 noResource]
    | swap => simp [step]
    | quote noResource => simp [step, (duplicable_iff _).2 noResource]

/-- The declarative judgment built on the independent per-word rules. -/
inductive Judged : Program → List Payload → List Payload → Prop where
  | word {w : Word} {operands out : List Payload} :
      WordRule w operands out → Judged (.word w) operands out
  | compose {first second : Program} {start middle out : List Payload} :
      Judged first start middle → Judged second middle out →
      Judged (.compose first second) start out
  | branch {thenArm elseArm : Program} {start out : List Payload} :
      Judged thenArm start out → Judged elseArm start out →
      Judged (.branch thenArm elseArm) start out
  | fail : Judged .fail [] []

/-- **The independent judgment is the original one.** -/
theorem judged_iff_accounts (program : Program) (start out : List Payload) :
    Judged program start out ↔ Accounts program start out := by
  constructor
  · intro judged
    induction judged with
    | word rule => exact .word ((step_iff_wordRule _ _ _).2 rule)
    | compose _ _ first second => exact .compose first second
    | branch _ _ thenArm elseArm => exact .branch thenArm elseArm
    | fail => exact .fail
  · intro accounts
    induction accounts with
    | word accepted => exact .word ((step_iff_wordRule _ _ _).1 accepted)
    | compose _ _ first second => exact .compose first second
    | branch _ _ thenArm elseArm => exact .branch thenArm elseArm
    | fail => exact .fail

/-- **The checker is complete for the judgment**: every derivation is an acceptance. -/
theorem run_complete {program : Program} {start out : List Payload}
    (accounts : Accounts program start out) : run program start = some out := by
  induction accounts with
  | word accepted => exact accepted
  | compose _ _ first second => simp [run, first, second]
  | branch _ _ thenArm elseArm => simp [run, thenArm, elseArm]
  | fail => rfl

/-- **The checker decides the independent judgment exactly.** -/
theorem run_iff_judged (program : Program) (start out : List Payload) :
    run program start = some out ↔ Judged program start out := by
  rw [judged_iff_accounts]
  exact ⟨run_sound program start out, run_complete⟩

/-! ## The join lift over straight-line word sequences -/

/-- A straight-line program: words and composition only. -/
def StraightLine : Program → Prop
  | .word _ => True
  | .compose first second => StraightLine first ∧ StraightLine second
  | .branch _ _ => False
  | .fail => False

/-- **One step preserves eligibility agreement** on the stacks it admits in both arms. -/
theorem step_preserves_agreement (word : Word) {left right left' right' : List Payload}
    (agree : EligibilityAgrees left right) (onLeft : step word left = some left')
    (onRight : step word right = some right') : EligibilityAgrees left' right' := by
  rw [step_iff_wordRule] at onLeft onRight
  cases onLeft <;> cases onRight <;>
    simp_all [EligibilityAgrees, eligibilityClass]

/-- **The join lift**: on a straight-line program, two stacks that agree on eligibility are
admitted in both arms or in neither, and the stacks they leave agree again. -/
theorem joined_sequence_admission_agrees (program : Program) (straight : StraightLine program) :
    ∀ (left right : List Payload), EligibilityAgrees left right →
      (run program left).isSome = (run program right).isSome ∧
        ∀ left' right', run program left = some left' → run program right = some right' →
          EligibilityAgrees left' right' := by
  induction program with
  | word w =>
      intro left right agree
      exact ⟨joined_word_admission_agrees w left right agree,
        fun _ _ onLeft onRight => step_preserves_agreement w agree onLeft onRight⟩
  | compose first second firstLift secondLift =>
      intro left right agree
      obtain ⟨firstStraight, secondStraight⟩ := straight
      obtain ⟨firstAccepted, firstAgrees⟩ := firstLift firstStraight left right agree
      cases onLeft : run first left with
      | none =>
          cases onRight : run first right with
          | none => exact ⟨by simp [run, onLeft, onRight], fun _ _ h => by simp [run, onLeft] at h⟩
          | some _ => simp [onLeft, onRight] at firstAccepted
      | some middleLeft =>
          cases onRight : run first right with
          | none => simp [onLeft, onRight] at firstAccepted
          | some middleRight =>
              have middleAgree := firstAgrees middleLeft middleRight onLeft onRight
              obtain ⟨secondAccepted, secondAgrees⟩ :=
                secondLift secondStraight middleLeft middleRight middleAgree
              refine ⟨by simpa [run, onLeft, onRight] using secondAccepted, ?_⟩
              intro left' right' finalLeft finalRight
              simp only [run, onLeft, onRight, Option.bind_eq_bind, Option.bind_some] at finalLeft finalRight
              exact secondAgrees left' right' finalLeft finalRight
  | branch _ _ _ _ => exact absurd straight id
  | fail => exact absurd straight id

/-- **The lift over the judgment**: an arm judged on a straight-line program carries over to any
stack that agrees with its start on eligibility, with agreeing results. -/
theorem joined_judgment_transfers (program : Program) (straight : StraightLine program)
    {left right left' : List Payload} (agree : EligibilityAgrees left right)
    (judged : Judged program left left') :
    ∃ right', Judged program right right' ∧ EligibilityAgrees left' right' := by
  obtain ⟨accepted, agrees⟩ := joined_sequence_admission_agrees program straight left right agree
  have onLeft := (run_iff_judged program left left').2 judged
  cases onRight : run program right with
  | none => simp [onLeft, onRight] at accepted
  | some right' =>
      exact ⟨right', (run_iff_judged program right right').1 onRight,
        agrees left' right' onLeft onRight⟩

/-! ## Controls -/

/-- **Equal length is not agreement**: a data reference and a resource at the same position
separate a straight-line `dup`, so the lift needs eligibility agreement, not a count. -/
theorem equal_length_does_not_lift :
    ([.data 1] : List Payload).length = ([.resource 1] : List Payload).length ∧
      (run (.word .dup) [.data 1]).isSome = true ∧
      (run (.word .dup) [.resource 1]).isSome = false := by
  decide

/-- **The lift crosses identities**: stacks with different owners but agreeing eligibility run a
three-word sequence alike, and the results agree again. -/
theorem lift_fixture :
    run (.compose (.word .swap) (.compose (.word .dup) (.word .drop)))
        [.resource 1, .data 2] = some [.data 2, .resource 1] ∧
      run (.compose (.word .swap) (.compose (.word .dup) (.word .drop)))
        [.resource 9, .wrapped (.data 3)] = some [.wrapped (.data 3), .resource 9] ∧
      EligibilityAgrees [.data 2, .resource 1] [.wrapped (.data 3), .resource 9] := by
  refine ⟨by decide, by decide, ?_⟩
  unfold EligibilityAgrees
  decide

/-! ## The lift over the whole fragment

The lift has two halves. **Result agreement**, that two admitted runs from agreeing stacks leave
agreeing stacks, holds for every program, branches and the error return included. **Admission
agreement**, that agreeing stacks are admitted alike, holds for every branch-free program, the
error return included. It fails for branches, because a branch compares its arms' actual results,
and identities that eligibility does not see can make two arms agree on one stack and not on the
other. -/

/-- A branch-free program: words, composition and the error return. -/
def BranchFree : Program → Prop
  | .word _ => True
  | .compose first second => BranchFree first ∧ BranchFree second
  | .branch _ _ => False
  | .fail => True

/-- Agreeing stacks have the same length. -/
theorem agreement_length {left right : List Payload} (agree : EligibilityAgrees left right) :
    left.length = right.length := by
  have lengths := congrArg List.length agree
  simpa [eligibilityClass] using lengths

/-- **Result agreement over the whole fragment**: whenever a program is admitted on two stacks that
agree on eligibility, the stacks it leaves agree again. -/
theorem accepted_results_agree (program : Program) :
    ∀ (left right left' right' : List Payload), EligibilityAgrees left right →
      run program left = some left' → run program right = some right' →
        EligibilityAgrees left' right' := by
  induction program with
  | word w =>
      intro left right left' right' agree onLeft onRight
      exact step_preserves_agreement w agree onLeft onRight
  | compose first second firstAgrees secondAgrees =>
      intro left right left' right' agree onLeft onRight
      simp only [run, Option.bind_eq_bind] at onLeft onRight
      cases middleLeft : run first left with
      | none => simp [middleLeft] at onLeft
      | some middle =>
          cases middleRight : run first right with
          | none => simp [middleRight] at onRight
          | some other =>
              rw [middleLeft, Option.bind_some] at onLeft
              rw [middleRight, Option.bind_some] at onRight
              exact secondAgrees middle other left' right'
                (firstAgrees left right middle other agree middleLeft middleRight) onLeft onRight
  | branch thenArm elseArm thenAgrees _ =>
      intro left right left' right' agree onLeft onRight
      simp only [run] at onLeft onRight
      split at onLeft
      · split at onRight
        · exact thenAgrees left right left' right' agree onLeft onRight
        · simp at onRight
      · simp at onLeft
  | fail =>
      intro left right left' right' agree onLeft onRight
      simp only [run] at onLeft onRight
      split at onLeft
      · split at onRight
        · simp only [Option.some.injEq] at onLeft onRight
          subst onLeft onRight
          exact agree
        · simp at onRight
      · simp at onLeft

/-- **Admission agreement over the branch-free fragment**: on words, composition and the error
return, two stacks that agree on eligibility are admitted in both runs or in neither. -/
theorem branch_free_admission_agrees (program : Program) (branchFree : BranchFree program) :
    ∀ (left right : List Payload), EligibilityAgrees left right →
      (run program left).isSome = (run program right).isSome := by
  induction program with
  | word w =>
      intro left right agree
      exact joined_word_admission_agrees w left right agree
  | compose first second firstLift secondLift =>
      intro left right agree
      obtain ⟨firstFree, secondFree⟩ := branchFree
      have firstAccepted := firstLift firstFree left right agree
      cases onLeft : run first left with
      | none =>
          cases onRight : run first right with
          | none => simp [run, onLeft, onRight]
          | some _ => simp [onLeft, onRight] at firstAccepted
      | some middleLeft =>
          cases onRight : run first right with
          | none => simp [onLeft, onRight] at firstAccepted
          | some middleRight =>
              have middleAgree := accepted_results_agree first left right middleLeft middleRight agree
                onLeft onRight
              simpa [run, onLeft, onRight] using secondLift secondFree middleLeft middleRight middleAgree
  | branch _ _ _ _ => exact absurd branchFree id
  | fail =>
      intro left right agree
      have sameLength := agreement_length agree
      simp only [run]
      by_cases empty : left = []
      · subst empty
        have : right = [] := List.eq_nil_of_length_eq_zero (by simpa using sameLength.symm)
        subst this
        rfl
      · have nonempty : right ≠ [] := by
          intro rightEmpty
          subst rightEmpty
          exact empty (List.eq_nil_of_length_eq_zero (by simpa using sameLength))
        simp [List.isEmpty_iff, empty, nonempty]

/-- **Branch admission does not lift**: `swap` and `dup; drop` agree on two equal data references
and not on two different ones, so a branch between them is admitted on one of two stacks that agree
on eligibility and refused on the other. The branch rule compares identities, which eligibility
does not see. -/
theorem branch_admission_does_not_lift :
    EligibilityAgrees [.data 1, .data 1] [.data 1, .data 2] ∧
      (run (.branch (.word .swap) (.compose (.word .dup) (.word .drop))) [.data 1, .data 1]).isSome =
        true ∧
      (run (.branch (.word .swap) (.compose (.word .dup) (.word .drop))) [.data 1, .data 2]).isSome =
        false := by
  refine ⟨?_, by decide, by decide⟩
  unfold EligibilityAgrees
  decide

/-- **The error return lifts**: it is admitted on two agreeing stacks alike, so it is not what keeps
branches outside the admission lift. -/
theorem fail_admission_lifts :
    (run .fail ([] : List Payload)).isSome = true ∧
      (run .fail [.data 1]).isSome = false ∧ (run .fail [.resource 1]).isSome = false := by
  decide

/-- **Result agreement over the judgment**: two derivations of one program from agreeing stacks end
in agreeing stacks, for every program. -/
theorem judged_results_agree (program : Program) {left right left' right' : List Payload}
    (agree : EligibilityAgrees left right) (onLeft : Judged program left left')
    (onRight : Judged program right right') : EligibilityAgrees left' right' :=
  accepted_results_agree program left right left' right' agree
    ((run_iff_judged program left left').2 onLeft) ((run_iff_judged program right right').2 onRight)

end NobleFoundation.Ownership
