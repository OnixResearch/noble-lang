import NobleM2.SizeChangeMatrixDecEq

/-!
# A bounded search for equal powers (issue #101)

Step one of the tight bound is that some power of a matrix is idempotent, which starts from two
positions with equal powers. A checker finds them by scanning the first `(4 ^ n) ^ n + 1` positions.

- `repeatSearch n g`: the scan, over pairs of positions with the first smaller;
- `repeatSearch_sound`: a true result names such a pair;
- `repeatSearch_complete`: a known pair certifies the scan.

What is *not* proved is the direction a checker needs in practice — that the scan is always true,
because the powers cannot all be distinct (`matrix_sequence_not_injective`). That needs the converse
of `List.any_eq_false` composed with the trichotomy of `Nat` on indices, and is the next lemma.
-/

namespace NobleM2.SizeChange

/-- Scan the first `(4 ^ n) ^ n + 1` powers of `g` for two equal values. -/
def repeatSearch (n : Nat) (g : MGraph n) : Bool :=
  (List.finRange ((matrixAux n).length + 1)).any fun i =>
    (List.finRange ((matrixAux n).length + 1)).any fun j =>
      (i.val < j.val) && decide (powerMatrix i.val g = powerMatrix j.val g)

/-- A true scan result names a pair of positions with equal powers. -/
theorem repeatSearch_sound {n : Nat} {g : MGraph n} (h : repeatSearch n g = true) :
    ∃ i j : Fin ((matrixAux n).length + 1),
      i.val < j.val ∧ powerMatrix i.val g = powerMatrix j.val g := by
  obtain ⟨i, _, hi⟩ := List.any_eq_true.mp h
  obtain ⟨j, _, hj⟩ := List.any_eq_true.mp hi
  simp only [Bool.and_eq_true, decide_eq_true_eq] at hj
  exact ⟨i, j, hj.1, hj.2⟩

/-- A known pair of equal powers certifies the scan. -/
theorem repeatSearch_complete {n : Nat} {g : MGraph n} {i j : Fin ((matrixAux n).length + 1)}
    (hlt : i.val < j.val) (heq : powerMatrix i.val g = powerMatrix j.val g) :
    repeatSearch n g = true := by
  refine List.any_eq_true.mpr ⟨i, List.mem_finRange i, ?_⟩
  refine List.any_eq_true.mpr ⟨j, List.mem_finRange j, ?_⟩
  simp only [Bool.and_eq_true, decide_eq_true_eq]
  exact ⟨hlt, heq⟩

/-- Membership in a false `any` means the predicate fails everywhere. -/
theorem any_eq_false_of_eq_false {α : Type} {p : α → Bool} {l : List α}
    (h : l.any p = false) : ∀ x ∈ l, p x = false := by
  induction l with
  | nil => intro x hx; simp at hx
  | cons a rest ih =>
      intro x hx
      rw [List.any_cons] at h
      have hboth := Bool.or_eq_false_iff.mp h
      rcases List.mem_cons.mp hx with rfl | hrest
      · exact hboth.1
      · exact ih hboth.2 x hrest

/-- A false scan means the powers are injective over the scanned positions. -/
theorem repeatSearch_false_injective {n : Nat} {g : MGraph n}
    (h : repeatSearch n g = false) :
    ∀ i j : Fin ((matrixAux n).length + 1),
      powerMatrix i.val g = powerMatrix j.val g → i = j := by
  intro i j heq
  apply Fin.ext
  by_cases hlt : i.val < j.val
  · exfalso
    have hrow := any_eq_false_of_eq_false h i (List.mem_finRange i)
    have hall := any_eq_false_of_eq_false hrow j (List.mem_finRange j)
    have htrue : ((i.val < j.val) && decide (powerMatrix i.val g = powerMatrix j.val g)) = true := by
      simp [hlt, heq]
    rw [htrue] at hall
    exact absurd hall (by simp)
  · by_cases heqval : i.val = j.val
    · exact heqval
    · exfalso
      have hgt : j.val < i.val := by omega
      have hrow := any_eq_false_of_eq_false h j (List.mem_finRange j)
      have hall := any_eq_false_of_eq_false hrow i (List.mem_finRange i)
      have htrue : ((j.val < i.val) && decide (powerMatrix j.val g = powerMatrix i.val g)) = true := by
        simp [hgt, heq.symm]
      rw [htrue] at hall
      exact absurd hall (by simp)

/-- **The scan always succeeds**: the powers cannot all be distinct. -/
theorem repeatSearch_true {n : Nat} (g : MGraph n) : repeatSearch n g = true := by
  cases h : repeatSearch n g with
  | false =>
      have hinj := repeatSearch_false_injective h
      exact absurd hinj (matrix_sequence_not_injective (fun i => powerMatrix i.val g))
  | true => rfl

/-- Consequently two positions with equal powers exist, explicitly. -/
theorem repeatSearch_witness {n : Nat} (g : MGraph n) :
    ∃ i j : Fin ((matrixAux n).length + 1),
      i.val < j.val ∧ powerMatrix i.val g = powerMatrix j.val g :=
  repeatSearch_sound (repeatSearch_true g)

end NobleM2.SizeChange
