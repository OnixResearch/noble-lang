import NobleM2.SizeChangeMatrixPrefix

/-!
# Splicing a path at equal prefix products (issue #101)

When two prefixes of a path have the same product, the segment between them can be cut out: the
product of `take i ++ drop j` equals the product of the whole path, and the spliced list is shorter
when `i < j`.

No cancellation is involved — the shared prefix is simply substituted into the suffix product, which
is why the proof is three rewrites and the append law.

- `matrixProduct_splice`: the product is preserved;
- `splice_length_lt`: the spliced list is strictly shorter when `i < j ≤ l.length`.

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

/-- **The replacement lemma**: a path whose two prefixes share a product has a strictly shorter path
with the same product. -/
theorem matrixProduct_shorten_of_repeat {n : Nat} (l : List (MGraph n)) {i j : Nat}
    (hij : i < j) (hj : j ≤ l.length)
    (h : matrixProduct (l.take i) = matrixProduct (l.take j)) :
    ∃ l' : List (MGraph n), l'.length < l.length ∧ matrixProduct l' = matrixProduct l :=
  ⟨l.take i ++ l.drop j, splice_length_lt l hij hj, matrixProduct_splice l h⟩

end NobleM2.SizeChange
