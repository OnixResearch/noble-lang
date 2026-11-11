import NobleM2.SizeChange

/-!
# Several self-call sites under one decreasing measure (issue #101)

`NobleM2.SizeChange` decides one self-loop: some measure strictly decreases at every call. A
definition may have several self-call sites, so this module lifts the criterion to a family of
sites over the same declared measures.

The criterion here is the shared-measure one: a *single* measure strictly decreases at **every**
site. It is sound by construction — whichever site runs, that measure drops — and it is what the
bounded #99 fragment can check without a closure over composed graphs. Refusal means **not shown
terminating**: the general size-change closure (composed graphs over paths, idempotent graphs with
a strict edge on a cycle) is strictly stronger and stays open.

A `Run` is any infinite sequence of sites whose states respect the relations each site declares.
The invariant the criterion needs is exactly the `respects` field, so nothing about the runtime is
assumed beyond it.
-/

namespace NobleM2.SizeChange

/-- A family of self-call sites over the same `n` measures. -/
abbrev Sites (k n : Nat) := Fin k → Graph n

/-- The shared-measure criterion: one measure decreases strictly at every site. -/
def certifiedAll {k n : Nat} (sites : Sites k n) : Bool :=
  (List.finRange n).any fun i => (List.finRange k).all fun s => sites s i == .decrease

/-- A site family certified by the shared criterion certifies each site on its own. -/
theorem certifiedAll_implies_certified {k n : Nat} {sites : Sites k n}
    (h : certifiedAll sites = true) (s : Fin k) : certified (sites s) = true := by
  obtain ⟨i, hi⟩ := by simpa [certifiedAll] using h
  unfold certified
  exact List.any_eq_true.mpr ⟨i, List.mem_finRange i, by simp [hi s]⟩

/-- The shared measure recovered from an accepted family. -/
theorem certifiedAll_sound {k n : Nat} {sites : Sites k n} (h : certifiedAll sites = true) :
    ∃ i, ∀ s, sites s i = .decrease := by
  obtain ⟨i, hi⟩ := by simpa [certifiedAll] using h
  exact ⟨i, hi⟩

/-- A refused family has no measure that decreases at every site. -/
theorem certifiedAll_refusal {k n : Nat} {sites : Sites k n} (h : certifiedAll sites = false) :
    ¬ ∃ i, ∀ s, sites s i = .decrease := by
  rintro ⟨i, hi⟩
  have accepted : certifiedAll sites = true := by
    unfold certifiedAll
    exact List.any_eq_true.mpr ⟨i, List.mem_finRange i,
      List.all_eq_true.mpr (fun s _ => by simp [hi s])⟩
  rw [accepted] at h
  exact Bool.noConfusion h

/-- An infinite sequence of sites, with the states it visits and what it assumes of each site. -/
structure Run (k n : Nat) (sites : Sites k n) where
  /-- The site chosen at step `t`. -/
  choose : Nat → Fin k
  /-- The measure vector after `t` steps. -/
  state : Nat → Fin n → Nat
  /-- Each step satisfies the relations its site declares: a `decrease` really drops. -/
  respects : ∀ (t : Nat) (i : Fin n), sites (choose t) i = .decrease → state (t + 1) i < state t i

/-- **The shared criterion is sound**: no run respects a family whose measure decreases everywhere. -/
theorem certifiedAll_terminates {k n : Nat} {sites : Sites k n} (h : certifiedAll sites = true) :
    ∀ _run : Run k n sites, False := by
  intro run
  obtain ⟨i, hi⟩ := certifiedAll_sound h
  apply no_decreasing_chain (fun t => run.state t i)
  intro t
  exact run.respects t i (hi (run.choose t))

/-! ## Fixtures -/

/-- Two sites over two measures, both decreasing on measure `0`: accepted. -/
def twoSitesAccepted : Sites 2 2
  | _, 0 => .decrease
  | _, _ => .unknown

theorem twoSitesAccepted_ok : certifiedAll twoSitesAccepted = true := rfl

/-- One site decreases measure `0`, the other passes it through: refused. -/
def twoSitesMixed : Sites 2 2
  | 0, 0 => .decrease
  | _, _ => .equal

theorem twoSitesMixed_refused : certifiedAll twoSitesMixed = false := rfl

/-- The refusal names the cycle: no measure decreases at every site. -/
theorem twoSitesMixed_reports_cycle : ¬ ∃ i, ∀ s, twoSitesMixed s i = .decrease :=
  certifiedAll_refusal twoSitesMixed_refused

/-- Every site decreasing measure `1` certifies each site individually. -/
def twoSitesSecond : Sites 2 2
  | _, 1 => .decrease
  | _, _ => .unknown

theorem twoSitesSecond_each : certified (twoSitesSecond 0) = true ∧ certified (twoSitesSecond 1) = true :=
  ⟨certifiedAll_implies_certified (by rfl) 0, certifiedAll_implies_certified (by rfl) 1⟩

end NobleM2.SizeChange
