import NobleM2.SizeChangeMatrixSplice
import NobleM2.SizeChangeMatrixRepeat

/-!
# Finding the repeated prefix product (issue #101)

The replacement lemma's hypothesis is guaranteed by the pigeonhole; this module produces it.

- `hasRepeatPrefix l`: a decision procedure scanning prefix pairs for equal products;
- `hasRepeatPrefix_sound`: a true verdict names the pair;
- `hasRepeatPrefix_true`: for a path longer than the enumeration the scan always succeeds, proved
  through `injective_bound` rather than through `Nodup` - a false verdict would make the prefix
  products injective over one more position than there are matrices;
- `matrixProduct_shorten`: the replacement lemma with the hypothesis discharged.

The counting conclusion (`n * n`) remains open.
-/

namespace NobleM2.SizeChange

/-- Scan prefix pairs for two products that agree. -/
def hasRepeatPrefix {n : Nat} (l : List (MGraph n)) : Bool :=
  (List.range (l.length + 1)).any fun j =>
    (List.range j).any fun i => decide (matrixProduct (l.take i) = matrixProduct (l.take j))

/-- A true verdict names the pair. -/
theorem hasRepeatPrefix_sound {n : Nat} {l : List (MGraph n)}
    (h : hasRepeatPrefix l = true) :
    ∃ i j, i < j ∧ j ≤ l.length ∧
      matrixProduct (l.take i) = matrixProduct (l.take j) := by
  obtain ⟨j, hjmem, hinner⟩ := List.any_eq_true.mp h
  obtain ⟨i, himem, heq⟩ := List.any_eq_true.mp hinner
  rw [decide_eq_true_eq] at heq
  have hjlt : j < l.length + 1 := List.mem_range.mp hjmem
  have hij : i < j := List.mem_range.mp himem
  exact ⟨i, j, hij, by omega, heq⟩

/-- An element whose predicate holds makes the scan true. -/
theorem any_of_mem_eq_true {α : Type} {p : α → Bool} :
    ∀ {l : List α} {x : α}, x ∈ l → p x = true → l.any p = true
  | [], x, hx, _ => by simp at hx
  | a :: rest, x, hx, hp => by
      rw [List.any_cons]
      rcases List.mem_cons.mp hx with rfl | hmem
      · rw [hp]
        rfl
      · rw [any_of_mem_eq_true hmem hp]
        simp

/-- **The scan always succeeds on a long path.** -/
theorem hasRepeatPrefix_true {n : Nat} (l : List (MGraph n))
    (hlong : (matrixAux n).length + 1 ≤ l.length) : hasRepeatPrefix l = true := by
  cases h : hasRepeatPrefix l with
  | true => rfl
  | false =>
      exfalso
      have hinner : ∀ j, j < l.length + 1 →
          ((List.range j).any fun i => decide (matrixProduct (l.take i) = matrixProduct (l.take j)))
            = false :=
        fun j hj => any_eq_false_of_eq_false h j (List.mem_range.mpr hj)
      have hinj : ∀ k₁ k₂ : Fin (l.length + 1),
          matrixProduct (l.take k₁.val) = matrixProduct (l.take k₂.val) → k₁ = k₂ := by
        intro k₁ k₂ heq
        by_cases hlt : k₁.val < k₂.val
        · exfalso
          have hfalse := hinner k₂.val k₂.isLt
          have htrue : decide (matrixProduct (l.take k₁.val) = matrixProduct (l.take k₂.val))
              = true := by rw [decide_eq_true_eq]; exact heq
          have := any_of_mem_eq_true
            (p := fun i => decide (matrixProduct (l.take i) = matrixProduct (l.take k₂.val)))
            (x := k₁.val) (List.mem_range.mpr hlt) htrue
          rw [hfalse] at this
          exact Bool.noConfusion this
        · by_cases hgt : k₂.val < k₁.val
          · exfalso
            have hfalse := hinner k₁.val k₁.isLt
            have htrue : decide (matrixProduct (l.take k₂.val) = matrixProduct (l.take k₁.val))
                = true := by rw [decide_eq_true_eq]; exact heq.symm
            have := any_of_mem_eq_true
              (p := fun i => decide (matrixProduct (l.take i) = matrixProduct (l.take k₁.val)))
              (x := k₂.val) (List.mem_range.mpr hgt) htrue
            rw [hfalse] at this
            exact Bool.noConfusion this
          · exact Fin.ext (by omega)
      have hbound := injective_bound (matrixAux n) (fun k : Fin (l.length + 1) =>
        matrixProduct (l.take k.val)) (fun k => matrixAux_complete _) hinj
      omega

/-- **The replacement lemma, discharged**: a path longer than the matrix enumeration has a strictly
shorter path with the same product. -/
theorem matrixProduct_shorten {n : Nat} (l : List (MGraph n))
    (hlong : (matrixAux n).length + 1 ≤ l.length) :
    ∃ l' : List (MGraph n), l'.length < l.length ∧ matrixProduct l' = matrixProduct l := by
  obtain ⟨i, j, hij, hj, heq⟩ := hasRepeatPrefix_sound (hasRepeatPrefix_true l hlong)
  exact matrixProduct_shorten_of_repeat l hij hj heq

end NobleM2.SizeChange
