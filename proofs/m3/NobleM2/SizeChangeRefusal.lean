import NobleM2.SizeChangeFixtures
import NobleM2.SizeChangeGuarded

/-!
# The refusal's witness: naming the offending site (issue #101)

`SizeChangeSites` decides a site family and `SizeChangeFixtures` proves the four conformance
verdicts, but a refusal there states only that **no** measure decreases at every site. The issue
asks for a checker that reports "the offending call cycle", so a refusal has to name something.

For the bounded shared-measure criterion, the witness is a site per measure: for every measure, at
least one self-call site at which that measure does not decrease. `certifiedAll_refusal_site` states
exactly that, and `certifiedAll_refusal_iff` makes the report exact — a family is refused **iff**
every measure has such a site, so the report is not a sample of failures.

The argument-layer corollary `checked_family_refusal_site` names the failing argument instead of the
relation: for every measure there is a site whose argument is not the predecessor. That is the shape
a front end reports, and it is why the fixtures below are stated twice — once relationally, once at
the argument layer.

Refusal still means **not shown terminating**, never "diverges": the witness is a property of this
criterion, not a claim that the program loops.
-/

namespace NobleM2.SizeChange

/-- A relation whose `decrease` test is not true is not a decrease. -/
theorem ne_decrease_of_beq_not_true {rel : Rel} (h : ¬ ((rel == .decrease) = true)) :
    rel ≠ .decrease := by
  intro same
  exact h (by simp [same])

/-- **A constructive `all`-refutation**: a failed `all` has a witness in the list. The core
`List.all_eq_false` states the same fact through a classical case split, which is why this local
induction is used instead of it: this lane keeps every theorem to `propext` and `Quot.sound`. -/
theorem exists_of_all_eq_false : ∀ (l : List α) (p : α → Bool), l.all p = false →
    ∃ a, a ∈ l ∧ ¬ p a = true
  | [], _, h => by simp at h
  | a :: t, p, h => by
    rw [List.all_cons] at h
    cases held : p a with
    | false => exact ⟨a, List.mem_cons_self, by simp [held]⟩
    | true =>
      rw [held] at h
      simp only [Bool.true_and] at h
      obtain ⟨witness, member, fails⟩ := exists_of_all_eq_false t p h
      exact ⟨witness, List.mem_cons_of_mem a member, fails⟩

/-- **A constructive `any`-refutation**: when no element satisfies the predicate, every element
refutes it. Stated without the classical `List.any_eq_false`. -/
theorem false_of_any_eq_false : ∀ (l : List α) (p : α → Bool), l.any p = false →
    ∀ a, a ∈ l → p a = false
  | [], _, _, _, member => by simp at member
  | a :: t, p, h, b, member => by
    rw [List.any_cons] at h
    cases held : p a with
    | true =>
      rw [held] at h
      exact Bool.noConfusion h
    | false =>
      rw [held] at h
      simp only [Bool.false_or] at h
      rcases List.mem_cons.mp member with same | later
      · subst same
        exact held
      · exact false_of_any_eq_false t p h b later

/-- **The refusal names a site for every measure.** For each declared measure there is a self-call
site where that measure does not decrease; the report therefore identifies the cycle member the
criterion fails on, measure by measure.

With no declared measures (`n = 0`) the statement is vacuous, which is correct: `certifiedAll` is
`false` there and no measure can be named. -/
theorem certifiedAll_refusal_site {k n : Nat} {sites : Sites k n}
    (h : certifiedAll sites = false) : ∀ i : Fin n, ∃ s : Fin k, sites s i ≠ .decrease := by
  intro i
  have row : (List.finRange k).all (fun s => sites s i == .decrease) = false :=
    false_of_any_eq_false (List.finRange n)
      (fun i => (List.finRange k).all fun s => sites s i == .decrease) h i (List.mem_finRange i)
  obtain ⟨s, _member, not_true⟩ := exists_of_all_eq_false (List.finRange k)
    (fun s => sites s i == .decrease) row
  exact ⟨s, ne_decrease_of_beq_not_true not_true⟩

/-- **The refusal report is exact**: a family is refused exactly when every measure has a site that
does not decrease it. Nothing the criterion refuses is missing from the report. -/
theorem certifiedAll_refusal_iff {k n : Nat} {sites : Sites k n} :
    certifiedAll sites = false ↔ ∀ i : Fin n, ∃ s : Fin k, sites s i ≠ .decrease := by
  constructor
  · exact certifiedAll_refusal_site
  · intro every
    cases accepted : certifiedAll sites with
    | false => rfl
    | true =>
      obtain ⟨i, hi⟩ := certifiedAll_sound accepted
      obtain ⟨s, fails⟩ := every i
      exact absurd (hi s) fails

/-- At the argument layer the refusal names, for every measure, a site whose argument is not the
predecessor. This is the witness a checker reports for a real definition. -/
theorem checked_family_refusal_site {k n : Nat} {argsOf : Fin k → Args n}
    (h : certifiedAll (fun s => graphOf (argsOf s)) = false) (i : Fin n) :
    ∃ s : Fin k, (argsOf s) i ≠ .predOfSame := by
  obtain ⟨s, fails⟩ := certifiedAll_refusal_site h i
  exact ⟨s, arg_ne_pred_of_graph_ne_decrease fails⟩

/-! ## The four fixtures, with their witnesses -/

/-- **Stringent refusal, fixture 3**: the non-decreasing self-call is refused, and its only measure
has its only site named. -/
theorem nonDecreasingFixture_cycle :
    ∀ i : Fin 1, ∃ s : Fin 1, nonDecreasingFixture s i ≠ .decrease :=
  certifiedAll_refusal_site nonDecreasingFixture_refused

/-- What the report says about that site: it is site `0`, and it passes the measure through
(`equal`), so it is not a decrease. -/
theorem nonDecreasingFixture_names_site :
    nonDecreasingFixture 0 0 = .equal ∧ nonDecreasingFixture 0 0 ≠ .decrease := by
  constructor
  · rfl
  · intro same
    cases same

/-- **Stringent refusal, fixture 4**: the wrapping-`I64` measure is refused, and its only measure
has its only site named. -/
theorem wrappingFixture_cycle :
    ∀ i : Fin 1, ∃ s : Fin 1, graphOf (wrappingArgs s) i ≠ .decrease :=
  certifiedAll_refusal_site wrappingFixture_refused

/-- At the argument layer the named argument is `opaque`: the checker cannot classify the wrapping
arithmetic, so it refuses rather than assuming a decrease. -/
theorem wrappingFixture_names_argument :
    wrappingArgs 0 0 = .opaque ∧ wrappingArgs 0 0 ≠ .predOfSame := by
  constructor
  · rfl
  · intro same
    cases same

/-- The pass-through family from `SizeChangeGuarded` reports the same witness at the argument
layer. -/
theorem staticArgs_names_argument :
    ∀ i : Fin 1, ∃ _s : Fin 1, staticArgs i ≠ Arg.predOfSame := by
  intro i
  exact ⟨0, by intro same; cases same⟩

end NobleM2.SizeChange
