import NobleContracts.Examples

namespace NobleContracts

/-- Typing composes only at the same complete intermediate ordered stack. -/
theorem CodeTyped.append {first second : List Op} {input middle output : List Ty}
    (left : CodeTyped first input middle) (right : CodeTyped second middle output) :
    CodeTyped (first ++ second) input output := by
  induction first generalizing input with
  | nil =>
      cases left
      exact right
  | cons op ops ih =>
      cases left with
      | cons hop hrest => exact .cons hop (ih hrest)

/-- Mathematical evidence, not a guest companion or a producer-supplied status.
    Every construction requires actual typing and behavioral proof terms. -/
structure TypedPC (code : List Op) (input output : List Ty)
    (pre : Stack → Prop) (post : Stack → Stack → Prop) : Prop where
  typing : CodeTyped code input output
  behavior : PC code pre post

/-- The model/revision is shared through this fixed rule library. Exact input,
    intermediate and output interfaces are indexed in the evidence; all behavioral
    premises remain in the theorem type, including the intermediate implication.
    The resulting subject is exactly the code produced by ordinary compose. -/
theorem TypedPC.compose {first second : List Op} {input middle output : List Ty}
    {pre intermediate nextPre nextPost post}
    (left : TypedPC first input middle pre intermediate)
    (right : TypedPC second middle output nextPre nextPost)
    (bridge : ∀ initial mid, pre initial → intermediate initial mid → nextPre mid)
    (join : ∀ initial mid final,
      pre initial → intermediate initial mid → nextPost mid final → post initial final) :
    TypedPC (first ++ second) input output pre post :=
  ⟨left.typing.append right.typing,
    pc_sequence left.behavior right.behavior bridge join⟩

/-- A concrete reason that an interface match cannot discharge the implication:
    the actual increment maps -2 to -1, which does not meet input >= 0. -/
theorem increment_missing_implication :
    CodeTyped increment [.i64] [.i64] ∧
    Exec increment [.i64 (BitVec.ofInt 64 (-2))] [.i64 (BitVec.ofInt 64 (-1))] ∧
    ¬ (0 ≤ (BitVec.ofInt 64 (-1)).toInt) := by
  refine ⟨increment_typed, ?_, ?_⟩
  · exact (increment_exec_iff (BitVec.ofInt 64 (-2)) [] _).mpr rfl
  · decide

end NobleContracts
