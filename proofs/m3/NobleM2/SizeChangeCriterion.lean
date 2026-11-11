import NobleM2.SizeChangeIdempotent

/-!
# The closure criterion's premise (issue #101)

The tight criterion asks that every member of the closure carry a strict edge, i.e. decrease at some
measure. This module states that premise and proves what can be proved about it without the
graph-theoretic cycle argument:

- `HasStrict g` is `∃ i, g i = .decrease`;
- `closure_criterion_of_sites`: if every *site* decreases somewhere, then every closure member does.
  The sites need not agree on the measure;
- `closure_criterion_of_certified`: the shared-measure criterion implies it;
- `closure_criterion_refused_of_site`: a site with no decrease refutes it;
- `splitSites`: a family where one site decreases measure 0 and the other measure 1. It has no
  shared measure, so the shared-measure criterion refuses it, yet every site decreases somewhere,
  so the closure criterion accepts.

That last point is the substance: the closure criterion's premise is strictly weaker, which is why
it accepts families this project's decision procedure refuses. Whether *acceptance* by it implies
termination is exactly the open soundness theorem - for the shared-measure criterion that direction
is `certifiedAll_terminates`.
-/

namespace NobleM2.SizeChange

/-- A graph with a strict edge somewhere. -/
def HasStrict {n : Nat} (g : Graph n) : Prop := ∃ i, g i = .decrease

/-- A certified graph has a strict edge. -/
theorem certified_hasStrict {n : Nat} {g : Graph n} (h : certified g = true) : HasStrict g := by
  obtain ⟨i, hi⟩ := by simpa [certified] using h
  exact ⟨i, hi⟩

/-- Every site decreasing somewhere makes every closure member decrease somewhere. -/
theorem closure_criterion_of_sites {k n : Nat} {sites : Sites k n}
    (h : ∀ s, HasStrict (sites s)) : ∀ g, Closure sites g → HasStrict g := by
  intro g member
  induction member with
  | site s => exact h s
  | compose _ _ left _ =>
      obtain ⟨i, hi⟩ := left
      exact ⟨i, composeGraph_decrease_left hi⟩

/-- The shared-measure criterion implies the closure criterion. -/
theorem closure_criterion_of_certified {k n : Nat} {sites : Sites k n}
    (h : certifiedAll sites = true) : ∀ g, Closure sites g → HasStrict g :=
  closure_criterion_of_sites (fun s => certified_hasStrict (certifiedAll_implies_certified h s))

/-- A site with no strict edge refutes the closure criterion. -/
theorem closure_criterion_refused_of_site {k n : Nat} {sites : Sites k n} (s : Fin k)
    (h : ¬ HasStrict (sites s)) : ¬ ∀ g, Closure sites g → HasStrict g :=
  fun every => h (every _ (.site s))

/-! ## A family the two criteria separate -/

/-- Site `s` decreases `s` (measure 0 for site 0, measure 1 for site 1) and passes the other. -/
def splitSites : Sites 2 2 := fun s i =>
  if s = 0 ∧ i = 0 then .decrease else if s = 1 ∧ i = 1 then .decrease else .unknown

theorem splitSites_each : ∀ s, HasStrict (splitSites s) := by
  intro s
  by_cases h0 : s = 0
  · exact ⟨0, by simp [splitSites, h0]⟩
  · have hval : s.val ≠ 0 := fun h => h0 (Fin.ext (by simpa using h))
    have h1 : s = 1 := by
      apply Fin.ext
      have := s.isLt
      omega
    exact ⟨1, by simp [splitSites, h1]⟩

/-- With no shared decreasing measure the shared-measure criterion refuses the family... -/
theorem splitSites_no_shared : certifiedAll splitSites = false := by
  decide

/-- ...yet the closure criterion accepts it. -/
theorem splitSites_closure_criterion : ∀ g, Closure splitSites g → HasStrict g :=
  closure_criterion_of_sites splitSites_each

end NobleM2.SizeChange
