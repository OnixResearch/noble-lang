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
  | 1, 0 => .same
  | _, _ => .unknown

def definition_01 : MGraph 2 := fun oldIndex newIndex =>
  match oldIndex.val, newIndex.val with
  | 0, 0 => .same
  | 1, 1 => .same
  | _, _ => .unknown

def definition_0Sites : Fin 1 → MGraph 2 := fun site =>
  match site.val with
  | 0 => definition_00
  | _ => (fun _ _ => .unknown)

def definition_0Entries : List (MatrixClosureEntry 1 2) :=
  [
    ⟨definition_00, [0]⟩,
    ⟨definition_01, [0, 0]⟩
  ]

def definition_0ClosureSetCertificate : MatrixClosureSetCertificate 1 2 definition_0Sites := ⟨definition_0Entries, by decide⟩

def definition_0Refusal : MatrixIdempotentRefusal definition_0ClosureSetCertificate :=
  ⟨⟨definition_01, [0, 0]⟩, by decide, by decide, by decide⟩

theorem definition_0RefusalGraphIsClosure : MatrixClosure definition_0Sites definition_0Refusal.entry.graph := refusal_graph_in_closure definition_0Refusal

theorem definition_0RefusalHasNoStrictDiagonal : ¬ HasStrictDiagonal definition_0Refusal.entry.graph := refusal_no_strict_diagonal definition_0Refusal

end NobleSizeChangeCertificate