/-
  Generated bounded size-change closure certificate (issue #101).

  The finite matrix set and each call-site path are checked in the Lean model. Accepted
  definitions carry one strict diagonal witness for every idempotent matrix; refusals name a
  real idempotent closure entry without such an edge. Front-end/source correspondence is separate.
-/
import NobleM2.SizeChangeSCT

namespace NobleSizeChangeCertificate

open NobleM2.SizeChange

/-- Bounded size-change evidence for definition 0: 1 self-call site(s) over 2 declared measure(s). The matrices and paths below are checked by the model. -/
def definition_00 : MGraph 2 := fun oldIndex newIndex =>
  match oldIndex.val, newIndex.val with
  | 0, 1 => .same
  | 1, 0 => .strict
  | _, _ => .unknown

def definition_01 : MGraph 2 := fun oldIndex newIndex =>
  match oldIndex.val, newIndex.val with
  | 0, 0 => .strict
  | 1, 1 => .strict
  | _, _ => .unknown

def definition_02 : MGraph 2 := fun oldIndex newIndex =>
  match oldIndex.val, newIndex.val with
  | 0, 1 => .strict
  | 1, 0 => .strict
  | _, _ => .unknown

def definition_0Sites : Fin 1 → MGraph 2 := fun site =>
  match site.val with
  | 0 => definition_00
  | _ => (fun _ _ => .unknown)

def definition_0Entries : List (MatrixClosureEntry 1 2) :=
  [
    ⟨definition_00, [0]⟩,
    ⟨definition_01, [0, 0]⟩,
    ⟨definition_02, [0, 0, 0]⟩
  ]

def definition_0Certificate : MatrixClosureCertificate 1 2 definition_0Sites :=
  ⟨definition_0Entries, [⟨definition_01, 0⟩], by decide, by decide⟩

theorem definition_0Criterion : ∀ {graph : MGraph 2}, MatrixClosure definition_0Sites graph → composeMatrix graph graph = graph → HasStrictDiagonal graph := by
  intro graph member idempotent
  exact certificate_closure_criterion definition_0Certificate member idempotent

end NobleSizeChangeCertificate