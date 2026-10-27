import NobleContracts.NamedSubject
import NobleContracts.NamedV2

/-! Revision-3 reusable-callee obligations over the NamedV1 semantics
(`PREUSE-CALL-01` / `PREUSE-LEMMA-01`). A separate module on purpose: revisions
1 and 2 (`NamedSubject`, `NamedV2`) are imported byte-for-byte unchanged and
nothing here changes their behavior. This is a semantic model, not source
acceptance, correspondence, or guest authority. -/
namespace NobleContracts.NamedV3
open NobleContracts

/-- T1: concatenation factors through the actual intermediate stack, for the
*named* semantics. This is the NamedV1 analog of `Rules.Run.append_iff` and
`Rules.exec_append_iff`, proved independently here rather than transported,
because the NamedV1 `Exec` reverses its stacks at the boundary. -/
theorem run_append_iff {env : NamedV1.Environment} {a b : List NamedV1.Op}
    {s t : NamedV1.Stack} :
    NamedV1.Run env (a ++ b) s t ↔ ∃ m, NamedV1.Run env a s m ∧ NamedV1.Run env b m t := by
  induction a generalizing s with
  | nil =>
      simp only [List.nil_append]
      exact ⟨fun h => ⟨s, .nil, h⟩, fun ⟨m, hm, hb⟩ => by cases hm; exact hb⟩
  | cons op ops ih =>
      constructor
      · intro h
        cases h with
        | cons hs hr =>
            obtain ⟨m, ha, hb⟩ := ih.mp hr
            exact ⟨m, .cons hs ha, hb⟩
      · rintro ⟨m, ha, hb⟩
        cases ha with
        | cons hs hr => exact .cons hs (ih.mpr ⟨m, hr, hb⟩)

@[simp] theorem exec_append_iff {env : NamedV1.Environment} {a b : List NamedV1.Op}
    {s t : NamedV1.Stack} :
    NamedV1.Exec env (a ++ b) s t ↔ ∃ m, NamedV1.Exec env a s m ∧ NamedV1.Exec env b m t := by
  constructor
  · intro h
    obtain ⟨m, ha, hb⟩ := run_append_iff.mp h
    exact ⟨m.reverse, by simpa [NamedV1.Exec] using ha, by simpa [NamedV1.Exec] using hb⟩
  · rintro ⟨m, ha, hb⟩
    exact run_append_iff.mpr ⟨m.reverse, by simpa [NamedV1.Exec] using ha,
      by simpa [NamedV1.Exec] using hb⟩

/-- T6: the `Holds₂` evaluation lemmas for the revision-3 vocabulary. `true`
always holds, and an ordering comparison holds exactly when the named
evaluation produced `true`, so no assertion is invented for undefined or
non-boolean operands. -/
@[simp] theorem holds_true₂ (env : NamedV1.Environment)
    (before after params : NamedV1.Stack) : NamedV2.Holds₂ env (.bool true) before after params := rfl

@[simp] theorem holds_eq₂_iff (env : NamedV1.Environment) (a b : Term)
    (before after params : NamedV1.Stack) :
    NamedV2.Holds₂ env (.eq a b) before after params ↔
      ∃ x y, NamedV2.evaluate₂ env a before after params = some x ∧
        NamedV2.evaluate₂ env b before after params = some y ∧ x = y := by
  simp only [NamedV2.Holds₂, NamedV2.evaluate₂]
  cases hx : NamedV2.evaluate₂ env a before after params with
  | none => simp
  | some x =>
      cases hy : NamedV2.evaluate₂ env b before after params with
      | none => simp
      | some y => simp

end NobleContracts.NamedV3
