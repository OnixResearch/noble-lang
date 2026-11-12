import NobleM2.SizeChangeCriterion

/-!
# Soundness of a uniform closure measure (issue #101)

The general closure criterion asks that every closure member carry *some* strict edge. That is not
enough for termination in this model: the strict edge may sit at a different measure on different
members, and `Run` constrains only `.decrease` steps, so a composed member that is `equal` at a
measure permits that measure to rise again. `SizeChangeCriterion` refutes the per-member premise with
such a family.

What is enough, and is proved here: a **fixed** measure decreases at every closure member. Then
whichever site a run chooses, that step's graph is a closure member, so the measure drops, and no
infinite run exists.

- `closure_uniform_terminates` is the soundness theorem for that premise;
- `closure_uniform_iff_sites` shows the premise is exactly the sites' shared-measure criterion:
  every site is a closure member, so nothing is gained by moving the quantifier to the closure. A
  stronger criterion therefore has to add a semantic condition on non-strict steps, not more
  composition;
- `splitSites_no_uniform` exhibits a family with no uniform measure, so the uniform premise is not
  a consequence of the separated-decrease shape of a family.
-/

namespace NobleM2.SizeChange

/-- **Soundness for a uniform closure measure**: no run respects it. -/
theorem closure_uniform_terminates {k n : Nat} {sites : Sites k n} {i : Fin n}
    (h : ∀ g, Closure sites g → g i = .decrease) : ∀ _run : Run k n sites, False := by
  intro run
  apply no_decreasing_chain (fun t => run.state t i)
  intro t
  exact run.respects t i (h (sites (run.choose t)) (.site (run.choose t)))

/-- **The uniform premise is the shared-measure criterion**: quantifying over closure members adds
nothing, because every site is a member and every member's decrease follows from the sites'. -/
theorem closure_uniform_iff_sites {k n : Nat} (sites : Sites k n) :
    (∃ i, ∀ g, Closure sites g → g i = .decrease) ↔ (∃ i, ∀ s, sites s i = .decrease) := by
  constructor
  · rintro ⟨i, h⟩
    exact ⟨i, fun s => h (sites s) (.site s)⟩
  · rintro ⟨i, h⟩
    exact ⟨i, closure_decreases_at h⟩

/-! ## The separated family has no uniform measure -/

/-- No measure decreases at every member of `splitSites`'s closure: site 0 is `unknown` at measure
1, so the site itself refutes measure 1, and site 1 refutes measure 0. -/
theorem splitSites_no_uniform :
    ¬ ∀ g, Closure splitSites g → g (1 : Fin 2) = .decrease := by
  intro h
  have mem : Closure splitSites (splitSites 0) := .site 0
  have := h (splitSites 0) mem
  simp [splitSites] at this

end NobleM2.SizeChange
