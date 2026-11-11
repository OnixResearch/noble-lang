import NobleM2.SizeChangeCriterion

/-!
# Soundness of a uniform closure measure (issue #101)

The closure criterion asks that every closure member carry *some* strict edge. That is not enough for
termination by itself - the strict edges could sit at different measures on different members, which
is exactly what the cycle argument has to rule out.

What is enough, and is proved here: a **fixed** measure decreases at every closure member. Then
whichever site a run chooses, that step's graph is a closure member, so the measure drops, and no
infinite run exists. This condition sits strictly between the shared-measure criterion (which asks
it of the sites only) and the general closure criterion:

- `closure_decreases_at` lifts "this measure decreases at every site" to every closure member;
- `closure_uniform_terminates` is the soundness theorem for the uniform premise;
- `splitSites_no_uniform` shows the separated family has no uniform measure, so uniformity is not a
  consequence of the general criterion - the gap the cycle argument must close.
-/

namespace NobleM2.SizeChange

/-- A measure decreasing at every site decreases at every closure member. -/
theorem closure_decreases_at {k n : Nat} {sites : Sites k n} {i : Fin n}
    (h : ∀ s, sites s i = .decrease) : ∀ g, Closure sites g → g i = .decrease := by
  intro g member
  induction member with
  | site s => exact h s
  | compose _ _ left _ => exact composeGraph_decrease_left left

/-- **Soundness for a uniform closure measure**: no run respects it. -/
theorem closure_uniform_terminates {k n : Nat} {sites : Sites k n} {i : Fin n}
    (h : ∀ g, Closure sites g → g i = .decrease) : ∀ _run : Run k n sites, False := by
  intro run
  apply no_decreasing_chain (fun t => run.state t i)
  intro t
  exact run.respects t i (h (sites (run.choose t)) (.site (run.choose t)))

/-- The uniform premise follows from a site family whose shared measure decreases everywhere. -/
theorem uniform_of_sites_uniform {k n : Nat} {sites : Sites k n} {i : Fin n}
    (h : ∀ s, sites s i = .decrease) : ∀ g, Closure sites g → g i = .decrease :=
  closure_decreases_at h

/-! ## The separated family has no uniform measure -/

/-- No measure decreases at every member of `splitSites`'s closure. -/
theorem splitSites_no_uniform :
    ¬ ∀ g, Closure splitSites g → g (1 : Fin 2) = .decrease := by
  intro h
  have mem : Closure splitSites (splitSites 0) := .site 0
  have := h (splitSites 0) mem
  simp [splitSites] at this

end NobleM2.SizeChange
