import NobleM2.SizeChangeMatrixMonotone
import NobleM2.SizeChangeMatrixPigeonhole

/-!
# Long paths repeat a prefix product (issue #101)

A path longer than the matrix enumeration has two prefixes with equal products. That is the
pigeonhole meeting the product: `l.length + 1` prefix products, at most `(matrixAux n).length`
distinct matrices, so not all of them differ.

- `prefixProducts l`: the product of each prefix of `l`;
- `prefixProducts_length`: there are `l.length + 1` of them;
- `prefixProducts_not_nodup`: for `l` longer than the enumeration they cannot be pairwise distinct.

The two consequences the bound needs are not here yet: extracting the two equal prefixes
constructively (a decidable scan, as for `repeatSearch`), and splicing the path at them - the product
of `take i ++ drop j` equals the product of `l` when `i < j` have equal prefix products.
-/

namespace NobleM2.SizeChange

/-- The product of each prefix of a path. -/
def prefixProducts {n : Nat} (l : List (MGraph n)) : List (MGraph n) :=
  (List.range (l.length + 1)).map (fun i => matrixProduct (l.take i))

/-- There is one prefix product per prefix, including the empty one. -/
theorem prefixProducts_length {n : Nat} (l : List (MGraph n)) :
    (prefixProducts l).length = l.length + 1 := by
  unfold prefixProducts
  rw [List.length_map, List.length_range]

/-- **Long paths repeat a prefix product.** -/
theorem prefixProducts_not_nodup {n : Nat} (l : List (MGraph n))
    (hlong : (matrixAux n).length + 1 ≤ l.length) : ¬ (prefixProducts l).Nodup := by
  intro hnodup
  have hle := length_le_of_subset (prefixProducts l) (matrixAux n) hnodup
    (fun x _ => matrixAux_complete x)
  rw [prefixProducts_length] at hle
  omega

end NobleM2.SizeChange
