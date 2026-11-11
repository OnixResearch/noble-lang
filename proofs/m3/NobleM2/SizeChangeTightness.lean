import NobleM2.SizeChangePowers

/-!
# Refusal is tight for stalling families (issue #101)

The criterion refuses a family whose sites all pass every measure through. A refusal is only
honest if it is not vacuous: this module exhibits a real run of exactly such a family - a run that
stalls, so no measure ever drops - and proves the criterion rejects it. Together those two facts say
the refusal corresponds to a genuine non-terminating execution rather than to a failure to find a
proof.

What this does *not* establish is the converse for every family: that whenever the criterion
refuses, some run fails to terminate. That is the general soundness and completeness question and it
stays open; this module only settles the stalling case, which is the one the refusal report names.
-/

namespace NobleM2.SizeChange

/-- A family in which every site passes every measure through is refused. -/
theorem allEqual_refused {k n : Nat} [NeZero k] {sites : Sites k n} (h : ∀ s i, sites s i = .equal) :
    certifiedAll sites = false := by
  cases hc : certifiedAll sites with
  | false => rfl
  | true =>
      obtain ⟨i, hi⟩ := certifiedAll_sound hc
      have step := hi 0
      rw [h 0 i] at step
      exact Rel.noConfusion step

/-- A stalling run: the site is fixed and no measure ever changes. -/
def stalledRun {k n : Nat} [NeZero k] {sites : Sites k n} (h : ∀ s i, sites s i = .equal) :
    Run k n sites where
  choose := fun _ => 0
  state := fun _ _ => 0
  respects := by
    intro _t i hdecr
    rw [h 0 i] at hdecr
    exact Rel.noConfusion hdecr

/-- **Refusal is tight for stalling families**: rejected by the criterion, yet a run exists. -/
theorem allEqual_refused_and_stalls {k n : Nat} [NeZero k] {sites : Sites k n}
    (h : ∀ s i, sites s i = .equal) : certifiedAll sites = false ∧ Nonempty (Run k n sites) :=
  ⟨allEqual_refused h, ⟨stalledRun h⟩⟩

/-! ## Fixtures -/

/-- Two sites that pass both measures through. -/
def stallingSites : Sites 2 2 := fun _ _ => .equal

theorem stallingSites_refused : certifiedAll stallingSites = false :=
  allEqual_refused (fun _ _ => rfl)

theorem stallingSites_stalls : Nonempty (Run 2 2 stallingSites) :=
  ⟨stalledRun (fun _ _ => rfl)⟩

end NobleM2.SizeChange
