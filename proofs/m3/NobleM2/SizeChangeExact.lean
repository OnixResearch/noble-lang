import NobleM2.SizeChangeCriterion

/-!
# The checker's verdict is exact (issue #101)

Criterion one asks the checker to report **either** a shared decreasing measure **or** the fact that
no measure decreases at every site - never an inconclusive third answer. (It does not report a call
cycle; nothing in the criterion constructs one.) The two halves are already proved separately
(`certifiedAll_sound`, and `certifiedAll_refusal` for the refusal direction); this packages them as
an equivalence, so a caller may read the Bool as the proposition without qualification.

```
certifiedAll sites = true  ↔  ∃ i, ∀ s, sites s i = .decrease
```

Read left to right, acceptance yields the shared measure that `certifiedAll_terminates` consumes.
Read right to left, a shared measure certifies the family - so the checker never refuses something
that has the property it is looking for.
-/

namespace NobleM2.SizeChange

/-- **Exactness**: the checker accepts a family exactly when a shared decreasing measure exists. -/
theorem certifiedAll_iff_shared {k n : Nat} [NeZero k] (sites : Sites k n) :
    certifiedAll sites = true ↔ ∃ i, ∀ s, sites s i = .decrease := by
  constructor
  · exact certifiedAll_sound
  · rintro ⟨i, hi⟩
    unfold certifiedAll
    exact List.any_eq_true.mpr ⟨i, List.mem_finRange i,
      List.all_eq_true.mpr (fun s _ => by simp [hi s])⟩

end NobleM2.SizeChange
