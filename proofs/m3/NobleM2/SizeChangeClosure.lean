import NobleM2.SizeChangeSites

/-!
# The size-change closure over composed call graphs (issue #101)

`SizeChangeSites` decides a family of self-call sites under one shared decreasing measure. The
general size-change principle composes the sites' graphs along paths and asks that every graph in
the resulting closure carries a strict decrease. This module builds that closure language and
proves the two facts the bounded fragment can prove about it without a semantic model of paths:

- **monotonicity.** A decrease survives composition on either side *when the other side is known*,
  which is what makes the closure of a certified family decrease its shared measure everywhere;
- **no free decrease.** A composition of `equal` graphs is `equal`, so a cycle made only of
  pass-through sites cannot become decreasing by composing it, and a family containing such a site
  is refused by the closure criterion.

The side condition is essential: `unknown` permits an increase, so `decrease` composed with
`unknown` is `unknown`, not `decrease`. Composing a decrease at measure 0 with a decrease at
measure 1 therefore yields no decrease at all rather than a decrease at each, which is why a family
must agree on one measure. `Rel.allowed_compose` in `SizeChange` proves the table describes real
two-step paths.

The general SCT soundness theorem — a closure in which *every* idempotent graph has a strict edge on
a cycle, for a run that satisfies every site's relations — needs a semantic account of the path the
run takes and stays open. Refusal still means **not shown terminating**.
-/

namespace NobleM2.SizeChange

/-- Composition of two call graphs, measure by measure. -/
def composeGraph {n : Nat} (g h : Graph n) : Graph n := fun i => (g i).compose (h i)

/-- A decrease in the left factor survives a composed step, provided the right factor is known at
that measure. -/
theorem composeGraph_decrease_left {n : Nat} {g h : Graph n} {i : Fin n}
    (hi : g i = .decrease) (hk : h i ≠ .unknown) : composeGraph g h i = .decrease := by
  simp [composeGraph, hi, Rel.compose_decrease_left (h i) hk]

/-- A decrease in the right factor survives a composed step, provided the left factor is known at
that measure. -/
theorem composeGraph_decrease_right {n : Nat} {g h : Graph n} {i : Fin n}
    (hi : h i = .decrease) (hk : g i ≠ .unknown) : composeGraph g h i = .decrease := by
  simp [composeGraph, hi, Rel.compose_decrease_right (g i) hk]

/-- Every graph reachable from a site family by composition. -/
inductive Closure {k n : Nat} (sites : Sites k n) : Graph n → Prop where
  | site (s : Fin k) : Closure sites (sites s)
  | compose {g h : Graph n} : Closure sites g → Closure sites h →
      Closure sites (composeGraph g h)

/-- A measure that decreases at every site decreases at every closure member. The composed case
needs both factors known at that measure, which the induction hypothesis supplies: both are already
`.decrease` there. -/
theorem closure_decreases_at {k n : Nat} {sites : Sites k n} {i : Fin n}
    (h : ∀ s, sites s i = .decrease) : ∀ g, Closure sites g → g i = .decrease := by
  intro g member
  induction member with
  | site s => exact h s
  | compose _ _ left right =>
      exact composeGraph_decrease_left left (Rel.known_of_decrease right)

/-- A certified family's shared measure survives the whole closure: the witness is outside the
`∀`, which is what "shared" means. -/
theorem closure_decreases {k n : Nat} {sites : Sites k n} (h : certifiedAll sites = true) :
    ∃ i, ∀ g, Closure sites g → g i = .decrease := by
  obtain ⟨i, hi⟩ := certifiedAll_sound h
  exact ⟨i, closure_decreases_at hi⟩

/-- A graph that passes every measure through. -/
def AllEqual {n : Nat} (g : Graph n) : Prop := ∀ i, g i = .equal

theorem allEqual_no_decrease {n : Nat} {g : Graph n} (h : AllEqual g) :
    ¬ ∃ i, g i = .decrease := by
  rintro ⟨i, hi⟩
  have := h i
  rw [this] at hi
  exact Rel.noConfusion hi

theorem composeGraph_allEqual {n : Nat} {g h : Graph n}
    (hg : AllEqual g) (hh : AllEqual h) : AllEqual (composeGraph g h) := by
  intro i
  simp [composeGraph, hg i, hh i, Rel.compose]

/-- Composition never invents a decrease out of pass-through sites. -/
theorem composeGraph_allEqual_no_decrease {n : Nat} {g h : Graph n}
    (hg : AllEqual g) (hh : AllEqual h) : ¬ ∃ i, composeGraph g h i = .decrease :=
  allEqual_no_decrease (composeGraph_allEqual hg hh)

/-- A family with one pass-through site is refused by the closure criterion. -/
theorem closure_refuses_allEqual_site {k n : Nat} {sites : Sites k n} (s : Fin k)
    (h : AllEqual (sites s)) :
    ¬ ∀ g, Closure sites g → ∃ i, g i = .decrease := by
  intro every
  exact allEqual_no_decrease h (every (sites s) (.site s))

/-! ## Fixtures -/

/-- Two sites, both decreasing measure `0`: the closure keeps the decrease. -/
def pairDecreasing : Sites 2 2
  | _, 0 => .decrease
  | _, _ => .unknown

theorem pairDecreasing_closure :
    ∃ i, ∀ g, Closure pairDecreasing g → g i = .decrease := by
  refine ⟨0, closure_decreases_at ?_⟩
  intro _
  rfl

/-- A family where site `1` passes measure `0` through is refused by the closure criterion. -/
def pairMixed : Sites 2 2
  | 1, _ => .equal
  | _, 0 => .decrease
  | _, _ => .unknown

theorem pairMixed_refused_by_closure :
    ¬ ∀ g, Closure pairMixed g → ∃ i, g i = .decrease :=
  closure_refuses_allEqual_site 1 (fun _ => rfl)

end NobleM2.SizeChange
