import NobleM2.SizeChangeMatrixPrefix

/-!
# Splicing a path at equal prefix products (issue #101)

When two prefixes of a path have the same product, the segment between them can be cut out: the
product of `take i ++ drop j` equals the product of the whole path, and the spliced list is shorter
when `i < j`. The spliced list is a **sublist** of the original (`splice_sublist`), which is the
relation a replacement argument needs: without it, a one-element list holding any product would
satisfy the statement and the pigeonhole would add nothing.

No cancellation is involved — the shared prefix is simply substituted into the suffix product, which
is why the proof is three rewrites and the append law.

- `matrixProduct_splice`: the product is preserved;
- `splice_length_lt`: the spliced list is strictly shorter when `i < j ≤ l.length`;
- `splice_sublist`: the spliced list is a sublist of the original.

Together with `prefixProducts_not_nodup` these are the replacement lemma: a path longer than the
enumeration has a strictly shorter path with the same product. The construction of the equal pair is
a decidable scan and is not here yet.
-/

namespace NobleM2.SizeChange

/-- **Splicing at equal prefix products preserves the product.** -/
theorem matrixProduct_splice {n : Nat} (l : List (MGraph n)) {i j : Nat}
    (h : matrixProduct (l.take i) = matrixProduct (l.take j)) :
    matrixProduct (l.take i ++ l.drop j) = matrixProduct l := by
  calc matrixProduct (l.take i ++ l.drop j)
      = composeMatrix (matrixProduct (l.take i)) (matrixProduct (l.drop j)) :=
        matrixProduct_append _ _
    _ = composeMatrix (matrixProduct (l.take j)) (matrixProduct (l.drop j)) := by rw [h]
    _ = matrixProduct (l.take j ++ l.drop j) := (matrixProduct_append _ _).symm
    _ = matrixProduct l := by rw [List.take_append_drop]

/-- Splicing between two distinct positions shortens the path. -/
theorem splice_length_lt {n : Nat} (l : List (MGraph n)) {i j : Nat}
    (hij : i < j) (hj : j ≤ l.length) :
    (l.take i ++ l.drop j).length < l.length := by
  rw [List.length_append, List.length_take, List.length_drop]
  omega

/-- Splicing produces a sublist of the original path: the prefix is kept and the suffix starts
later, which is exactly `take i` followed by `drop j` for `i ≤ j`. -/
theorem splice_sublist {n : Nat} (l : List (MGraph n)) {i j : Nat} (hij : i ≤ j) :
    (l.take i ++ l.drop j).Sublist l := by
  have hdrop : (l.drop j).Sublist (l.drop i) := by
    have h := List.drop_sublist (j - i) (l.drop i)
    rw [List.drop_drop, Nat.add_comm, Nat.sub_add_cancel hij] at h
    exact h
  have happ := List.Sublist.append (List.Sublist.refl (l.take i)) hdrop
  rwa [List.take_append_drop] at happ

/-- **The replacement lemma**: a path whose two prefixes share a product has a strictly shorter path
with the same product, and that path is a sublist of the original. -/
theorem matrixProduct_shorten_of_repeat {n : Nat} (l : List (MGraph n)) {i j : Nat}
    (hij : i < j) (hj : j ≤ l.length)
    (h : matrixProduct (l.take i) = matrixProduct (l.take j)) :
    ∃ l' : List (MGraph n),
      l'.Sublist l ∧ l'.length < l.length ∧ matrixProduct l' = matrixProduct l :=
  ⟨l.take i ++ l.drop j, splice_sublist l (Nat.le_of_lt hij), splice_length_lt l hij hj,
    matrixProduct_splice l h⟩

/-! ## Fixture -/

/-- A concrete splice: dropping the middle entry of a three-entry path is a sublist, and the
statement is not vacuous (a one-entry list would not be). -/
theorem splice_three_entries {n : Nat} (a b c : MGraph n) :
    ([a, c] : List (MGraph n)).Sublist [a, b, c] := by
  exact splice_sublist [a, b, c] (i := 1) (j := 2) (by omega)

end NobleM2.SizeChange
