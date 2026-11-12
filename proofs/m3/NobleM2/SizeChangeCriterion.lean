import NobleM2.SizeChangeIdempotent

/-!
# The closure criterion's premise (issue #101)

The tight criterion asks that every member of the closure carry a strict edge, i.e. decrease at some
measure. This module states that premise and proves what is true of it:

- `HasStrict g` is `∃ i, g i = .decrease`;
- `closure_hasStrict_of_certified`: if one measure decreases at every site, then every closure member
  decreases at it (`closure_decreases_at` in `SizeChangeClosure`), hence has a strict edge;
- `closure_criterion_refused_of_site`: a site with no decrease refutes it;
- `splitSites`: a family where one site decreases measure 0 and the other measure 1. It has no shared
  measure, so the shared-measure criterion refuses it, and its two sites compose to `unknown` at every
  measure, so the closure premise is not a consequence of per-site strictness.

## Why per-site strict edges are not enough

Composition is sound (`Rel.allowed_compose`): a strict edge survives a composed step only when the
other factor is known at that measure. `splitSites_product_unknown` is the concrete case — composing
a decrease at measure 0 with a decrease at measure 1 yields `unknown` at both, because each site is
`unknown` where the other decreases. So the closure premise is strictly stronger than "each site
decreases somewhere".

## Why the closure premise is still not sound here

`alternateSites` is the counterexample. Its two sites drop either measure and hold the other
(`equal`), so every closure member is known everywhere and has a strict edge; but `Run` constrains
only `.decrease` steps, so a run may raise a measure on an `equal` step. `alternateRun` is exactly
such a run, and `alternate_criterion_unsound` states it. Making the premise sound needs a run model
that also honours `equal`/non-increasing steps — the semantic content the classical cycle argument
consumes — and that is not attempted in this project.

Refusal still means **not shown terminating**.
-/

namespace NobleM2.SizeChange

/-- A graph with a strict edge somewhere. -/
def HasStrict {n : Nat} (g : Graph n) : Prop := ∃ i, g i = .decrease

/-- A certified graph has a strict edge. -/
theorem certified_hasStrict {n : Nat} {g : Graph n} (h : certified g = true) : HasStrict g := by
  obtain ⟨i, hi⟩ := by simpa [certified] using h
  exact ⟨i, hi⟩

/-- A certified family's closure members all have a strict edge: the shared measure. -/
theorem closure_hasStrict_of_certified {k n : Nat} {sites : Sites k n}
    (h : certifiedAll sites = true) : ∀ g, Closure sites g → HasStrict g := by
  obtain ⟨i, hi⟩ := certifiedAll_sound h
  intro g member
  exact ⟨i, closure_decreases_at hi g member⟩

/-- A site with no strict edge refutes the closure premise. -/
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

/-- ...and its sites do not compose to a strict edge: site 0 is `unknown` at measure 1 and site 1 is
`unknown` at measure 0, so the product is `unknown` everywhere. -/
theorem splitSites_product_unknown :
    composeGraph (splitSites 0) (splitSites 1) = fun _ => .unknown := by
  funext i
  cases i using Fin.cases with
  | zero => rfl
  | succ j =>
      cases j using Fin.cases with
      | zero => rfl
      | succ k => exact Fin.elim0 k

/-- Composing the two sites is `unknown` at measure 0: the composition claims no decrease at all,
because each site is `unknown` where the other decreases. -/
theorem splitSites_compose_zero :
    composeGraph (splitSites 0) (splitSites 1) (0 : Fin 2) = .unknown := by
  rw [splitSites_product_unknown]

/-- The same statement in refutation form: the old table's conclusion is now false. -/
theorem splitSites_compose_zero_ne_decrease :
    composeGraph (splitSites 0) (splitSites 1) (0 : Fin 2) ≠ .decrease := by
  intro h
  rw [splitSites_compose_zero] at h
  exact Rel.noConfusion h

/-- The product of the two sites has no strict edge. -/
theorem splitSites_product_no_strict : ¬ HasStrict (composeGraph (splitSites 0) (splitSites 1)) := by
  rintro ⟨i, hi⟩
  rw [splitSites_product_unknown] at hi
  exact Rel.noConfusion hi

/-! ## The separated family still has a run

The shared-measure criterion refuses `splitSites`, yet the family does run forever: each step drops
its own measure and the other measure is unconstrained. Refusal therefore means "not shown", exactly
as the module documentation claims. -/

/-- Site 0 decreases exactly measure 0. -/
theorem splitSites_zero_iff (i : Fin 2) : splitSites 0 i = .decrease ↔ i = 0 := by
  cases i using Fin.cases with
  | zero => simp [splitSites]
  | succ j => simp [splitSites]

/-- Site 1 decreases exactly measure 1. -/
theorem splitSites_one_iff (i : Fin 2) : splitSites 1 i = .decrease ↔ i = 1 := by
  cases i using Fin.cases with
  | zero => simp [splitSites]
  | succ j => simp [splitSites]

/-- A definitional parity predicate, so that `evenFor (t + 1)` reduces without arithmetic. -/
def evenFor : Nat → Bool
  | 0 => true
  | t + 1 => !evenFor t

/-- Even steps choose site 0, odd steps site 1. -/
def cycleChoose (t : Nat) : Fin 2 := if evenFor t then 0 else 1

/-- Even steps show measure 0 at 2 and drop it to 0; odd steps show measure 1 at 2 and drop it to
0. The other measure is unconstrained at each step, which is exactly what `unknown` permits. -/
def cycleState (t : Nat) (i : Fin 2) : Nat :=
  if evenFor t then (if i = 0 then 2 else 0) else (if i = 0 then 0 else 2)

/-- The alternating run respects every relation `splitSites` declares. -/
theorem cycle_respects (t : Nat) (i : Fin 2) :
    splitSites (cycleChoose t) i = .decrease → cycleState (t + 1) i < cycleState t i := by
  intro h
  cases hp : evenFor t
  · have hc : cycleChoose t = 1 := by simp [cycleChoose, hp]
    rw [hc] at h
    have hi : i = 1 := splitSites_one_iff i |>.mp h
    subst hi
    simp [cycleState, hp, evenFor]
  · have hc : cycleChoose t = 0 := by simp [cycleChoose, hp]
    rw [hc] at h
    have hi : i = 0 := splitSites_zero_iff i |>.mp h
    subst hi
    simp [cycleState, hp, evenFor]

/-- **The run exists**: an infinite execution of `splitSites`. -/
def cycleRun : Run 2 2 splitSites where
  choose := cycleChoose
  state := cycleState
  respects := cycle_respects

/-- The real two-step path does not decrease at measure 0: the run is back where it started. This is
the fact the old composition table contradicted. -/
theorem cycleRun_state_returns :
    cycleRun.state 0 (0 : Fin 2) = 2 ∧ cycleRun.state 2 (0 : Fin 2) = 2 := by
  constructor <;> decide

/-- Refusal is not a divergence claim: the refused family has a run. -/
theorem splitSites_refused_but_runs :
    certifiedAll splitSites = false ∧ Nonempty (Run 2 2 splitSites) :=
  ⟨splitSites_no_shared, ⟨cycleRun⟩⟩

/-! ## Refutation: a strict edge in every member is not enough -/

/-- Two sites over two measures: site 0 drops measure 0 and holds measure 1, site 1 the other way
round. No entry of either site is `unknown`. -/
def alternateSites : Sites 2 2
  | 0, 0 => .decrease
  | 0, 1 => .equal
  | 1, 0 => .equal
  | 1, 1 => .decrease

/-- The two elements of `Fin 2`, explicitly. -/
theorem fin_two_cases (i : Fin 2) : i = 0 ∨ i = 1 := by
  have hv : i.val = 0 ∨ i.val = 1 := by
    have := i.isLt
    omega
  rcases hv with h | h
  · exact Or.inl (Fin.ext (by simpa using h))
  · exact Or.inr (Fin.ext (by simpa using h))

/-- Site 0 decreases exactly measure 0. -/
theorem alternate_zero_iff (i : Fin 2) : alternateSites 0 i = .decrease ↔ i = 0 := by
  cases i using Fin.cases with
  | zero => simp [alternateSites]
  | succ j =>
      cases j using Fin.cases with
      | zero => simp [alternateSites]
      | succ k => exact Fin.elim0 k

/-- Site 1 decreases exactly measure 1. -/
theorem alternate_one_iff (i : Fin 2) : alternateSites 1 i = .decrease ↔ i = 1 := by
  cases i using Fin.cases with
  | zero => simp [alternateSites]
  | succ j =>
      cases j using Fin.cases with
      | zero => simp [alternateSites]
      | succ k => exact Fin.elim0 k

/-- Every closure member of `alternateSites` is known at every measure and has a strict edge: both
properties survive composition, since neither site has an `unknown` entry. -/
theorem alternate_closed {g : Graph 2} (member : Closure alternateSites g) :
    (∀ i, g i ≠ .unknown) ∧ HasStrict g := by
  induction member with
  | site s =>
      have hknown : ∀ i : Fin 2, alternateSites s i ≠ .unknown := by
        rcases fin_two_cases s with rfl | rfl <;>
          · intro i
            rcases fin_two_cases i with rfl | rfl <;> decide
      have hstrict : HasStrict (alternateSites s) := by
        rcases fin_two_cases s with rfl | rfl
        · exact ⟨0, rfl⟩
        · exact ⟨1, rfl⟩
      exact ⟨hknown, hstrict⟩
  | compose _ _ left right =>
      refine ⟨?_, ?_⟩
      · intro i
        exact Rel.known_compose (left.1 i) (right.1 i)
      · obtain ⟨i, hi⟩ := left.2
        exact ⟨i, composeGraph_decrease_left hi (right.1 i)⟩

/-- Every closure member of `alternateSites` has a strict edge. -/
theorem alternate_closure_strict : ∀ g, Closure alternateSites g → HasStrict g :=
  fun _ member => (alternate_closed member).2

/-- The alternating run respects every relation `alternateSites` declares. -/
theorem alternate_respects (t : Nat) (i : Fin 2) :
    alternateSites (cycleChoose t) i = .decrease → cycleState (t + 1) i < cycleState t i := by
  intro h
  cases hp : evenFor t
  · have hc : cycleChoose t = 1 := by simp [cycleChoose, hp]
    rw [hc] at h
    have hi : i = 1 := alternate_one_iff i |>.mp h
    subst hi
    simp [cycleState, hp, evenFor]
  · have hc : cycleChoose t = 0 := by simp [cycleChoose, hp]
    rw [hc] at h
    have hi : i = 0 := alternate_zero_iff i |>.mp h
    subst hi
    simp [cycleState, hp, evenFor]

/-- An infinite execution of `alternateSites`: it alternates the two sites and the two measures. -/
def alternateRun : Run 2 2 alternateSites where
  choose := cycleChoose
  state := cycleState
  respects := alternate_respects

/-- **The per-member premise is unsound in this model**: it accepts `alternateSites`, which has a
run. The reason is the model, not the composition: `Run` says nothing about `equal` steps, so a
measure may rise again on one. -/
theorem alternate_criterion_unsound :
    (∀ g, Closure alternateSites g → HasStrict g) ∧ Nonempty (Run 2 2 alternateSites) :=
  ⟨alternate_closure_strict, ⟨alternateRun⟩⟩

end NobleM2.SizeChange
