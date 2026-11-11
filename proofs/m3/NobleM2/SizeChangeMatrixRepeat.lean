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

end NobleM2.SizeChange
