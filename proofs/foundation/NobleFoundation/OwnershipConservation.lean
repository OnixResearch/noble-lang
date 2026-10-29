/-
  Whole-program ownership accounting for the ordered stack fragment (issue #59).

  `Ownership` has no word that acquires, releases or consumes a must-use
  resource.  Consequently the conservation theorem below is exact, but its
  resource-moving content is partly vacuous: an admitted word can only act on
  duplicable data, while a resource can remain untouched elsewhere in the
  stack.  The concrete `resourceCarry` fixture keeps a resource in that
  untouched position while accepted data words run, so the resource case is
  represented rather than omitted.
-/
import NobleFoundation.Ownership

namespace NobleFoundation.OwnershipConservation

open NobleFoundation.Ownership

/-- One accepted word preserves the ordered multiset of must-use resources.

    An admitted `dup`, `drop` or `quote` step inspects only a duplicable
    payload, and an admitted `swap` permutes two such payloads.  Thus every
    resource in the input remains in the output, while data may be duplicated,
    dropped or retained.
-/
theorem step_conserves_obligations (word : Word) (operands out : List Payload)
    (accepted : step word operands = some out) :
    (out.filter containsResource).Perm (operands.filter containsResource) := by
  cases word with
  | dup =>
      cases operands with
      | nil => simp [step] at accepted
      | cons payload rest =>
          by_cases eligible : duplicable payload = true
          · have noResource : containsResource payload = false :=
              (duplicable_iff payload).mp eligible
            simp [step, eligible] at accepted
            subst out
            simp [noResource]
          · simp [step, eligible] at accepted
  | drop =>
      cases operands with
      | nil => simp [step] at accepted
      | cons payload rest =>
          by_cases eligible : duplicable payload = true
          · have noResource : containsResource payload = false :=
              (duplicable_iff payload).mp eligible
            simp [step, eligible] at accepted
            subst out
            simp [noResource]
          · simp [step, eligible] at accepted
  | quote =>
      cases operands with
      | nil => simp [step] at accepted
      | cons payload rest =>
          by_cases eligible : duplicable payload = true
          · have noResource : containsResource payload = false :=
              (duplicable_iff payload).mp eligible
            simp [step, eligible] at accepted
            subst out
            simp [noResource]
          · simp [step, eligible] at accepted
  | swap =>
      cases operands with
      | nil => simp [step] at accepted
      | cons left rest =>
          cases rest with
          | nil => simp [step] at accepted
          | cons right rest =>
              by_cases leftEligible : duplicable left = true
              · by_cases rightEligible : duplicable right = true
                · have leftNoResource : containsResource left = false :=
                    (duplicable_iff left).mp leftEligible
                  have rightNoResource : containsResource right = false :=
                    (duplicable_iff right).mp rightEligible
                  simp [step, leftEligible, rightEligible] at accepted
                  subst out
                  simp [leftNoResource, rightNoResource]
                · simp [step, leftEligible, rightEligible] at accepted
              · simp [step, leftEligible] at accepted

/-- Every declaratively accounted program preserves the resource obligations. -/
theorem accounts_conserves_obligations {program : Program} {operands out : List Payload}
    (accounts : Accounts program operands out) :
    (out.filter containsResource).Perm (operands.filter containsResource) := by
  induction accounts with
  | word accepted => exact step_conserves_obligations _ _ _ accepted
  | compose first second firstIH secondIH =>
      exact List.Perm.trans secondIH firstIH
  | branch thenAccounts elseAccounts thenIH elseIH => exact thenIH
  | fail => exact List.Perm.refl []

/-- Checker acceptance preserves the resource obligations by `run_sound`. -/
theorem run_conserves_obligations (program : Program) (operands out : List Payload)
    (accepted : run program operands = some out) :
    (out.filter containsResource).Perm (operands.filter containsResource) := by
  exact accounts_conserves_obligations (run_sound program operands out accepted)

/-- `runConfiguration` returns the segment it was given, not a changed segment. -/
theorem runConfiguration_segment_preserved (segment : List Payload) (program : Program)
    (operands segment' out : List Payload)
    (accepted : runConfiguration segment program operands = some (segment', out)) :
    segment' = segment := by
  unfold runConfiguration at accepted
  cases programRun : run program operands with
  | none => simp [programRun] at accepted
  | some actual =>
      simp [programRun] at accepted
      obtain ⟨segmentEq, outEq⟩ := accepted
      exact segmentEq.symm

/-- A concrete accepted data-word run carrying an untouched resource tail. -/
def resourceCarryProgram : Program :=
  .compose (.word .dup) (.word .drop)

def resourceCarryInput : List Payload :=
  [.data 1, .resource 7]

def resourceCarryOutput : List Payload :=
  [.data 1, .resource 7]

/-- The executable checker accepts the concrete data-word/resource fixture. -/
theorem resource_carry_run :
    run resourceCarryProgram resourceCarryInput = some resourceCarryOutput := by
  decide

/-- The same concrete fixture has a declarative `Accounts` derivation. -/
theorem resource_carry_accounts :
    Accounts resourceCarryProgram resourceCarryInput resourceCarryOutput := by
  exact run_sound _ _ _ resource_carry_run

end NobleFoundation.OwnershipConservation
