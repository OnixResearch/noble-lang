import NobleM2.SizeChangeMatrixEnumeration
import NobleM2.SizeChangeMatrixPreserve

/-!
# Pigeonhole over matrix space (issue #101)

The enumeration is complete and counted, so the counting lemma from the vector lane applies directly:
no injective sequence of matrices can be longer than the enumeration.

- `matrix_sequence_not_injective`: a sequence indexed by one more position than there are matrices
  cannot be injective;
- `powerMatrix_collision`: consequently the powers of any matrix repeat within `(4 ^ n) ^ n + 1`
  positions - some two agree;
- `matrix_composites_bounded`: any nodup list of matrices is no longer than the enumeration, which is
  the form a checker uses to bound the composites it must consider.

The positive form - naming the two positions that collide - needs either classical choice or the
decidable scan a checker runs; this module states the negative form the counting lemma gives.
-/

namespace NobleM2.SizeChange

/-- **Pigeonhole over matrix space**: no injective sequence outruns the enumeration. -/
theorem matrix_sequence_not_injective {n : Nat}
    (f : Fin ((matrixAux n).length + 1) → MGraph n) :
    ¬ ∀ i j : Fin ((matrixAux n).length + 1), f i = f j → i = j := by
  intro hinj
  have hbound := injective_bound (matrixAux n) f (fun i => matrixAux_complete (f i)) hinj
  omega

/-- The powers of a matrix cannot all be distinct. -/
theorem powerMatrix_collision {n : Nat} (g : MGraph n) :
    ¬ ∀ i j : Fin ((matrixAux n).length + 1),
        powerMatrix i.val g = powerMatrix j.val g → i = j :=
  matrix_sequence_not_injective (fun i => powerMatrix i.val g)

/-- Any nodup list of matrices is no longer than the enumeration. -/
theorem matrix_composites_bounded {n : Nat} (l : List (MGraph n)) (hl : l.Nodup) :
    l.length ≤ (matrixAux n).length :=
  length_le_of_subset l (matrixAux n) hl (fun x _ => matrixAux_complete x)

end NobleM2.SizeChange
