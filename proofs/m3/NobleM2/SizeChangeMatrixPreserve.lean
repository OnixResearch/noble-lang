import NobleM2.SizeChangeMatrixExtract

/-!
# Composition preserves step semantics (issue #101)

If a step satisfies `g` and the next satisfies `h`, then the step pair satisfies the product
`composeMatrix g h`: each entry of the product is the join over the factors that compose to it, and
each possible relation of an entry is realised by a factor whose two halves chain.

- `compose_eq_nonIncrease` and `compose_eq_same` classify the factor pairs for the two non-strict
  relations;
- `compose_nonIncrease_steps` and `compose_same_steps` chain the inequalities for them, as
  `compose_strict_steps` does for strict;
- `matrixSatisfies_compose` assembles the three into preservation.

Iterating preservation gives period-`k` cycles; the SCC criterion is the next step.
-/

namespace NobleM2.SizeChange

/-- A composition is non-increasing exactly for these factor pairs. -/
theorem compose_eq_nonIncrease (r s : Rel4) :
    r.compose s = .nonIncrease ↔
      (r = .nonIncrease ∧ s = .nonIncrease) ∨
        (r = .nonIncrease ∧ s = .same) ∨ (r = .same ∧ s = .nonIncrease) := by
  cases r <;> cases s <;> decide

/-- A composition is the identity relation exactly for `same ∘ same`. -/
theorem compose_eq_same (r s : Rel4) : r.compose s = .same ↔ r = .same ∧ s = .same := by
  cases r <;> cases s <;> decide

/-- A non-increasing product entry bounds the measure across the two steps. -/
theorem compose_nonIncrease_steps {n : Nat} {g h : MGraph n} {i k : Fin n}
    (hprod : composeMatrix g h i k = .nonIncrease) {s t u : Fin n → Nat}
    (hg : MatrixSatisfies g s t) (hh : MatrixSatisfies h t u) : u k ≤ s i := by
  have hfold : (List.finRange n).foldr (fun j acc => ((g i j).compose (h j k)).join acc) .unknown
      = .nonIncrease := by simpa [composeMatrix] using hprod
  rcases foldr_join_eq_of_ne_unknown (fun j => (g i j).compose (h j k)) (List.finRange n)
      .unknown .nonIncrease hfold (by simp) with ⟨j', _, hj'⟩ | hacc
  · rcases (compose_eq_nonIncrease (g i j') (h j' k)).mp hj' with ⟨hgN, hhN⟩ | hrest
    · exact Nat.le_trans (MatrixSatisfies.nonIncrease hh hhN) (MatrixSatisfies.nonIncrease hg hgN)
    · rcases hrest with ⟨hgN, hhS⟩ | ⟨hgS, hhN⟩
      · have hu : u k = t j' := MatrixSatisfies.sameEq hh hhS
        rw [hu]
        exact MatrixSatisfies.nonIncrease hg hgN
      · have hs : s i = t j' := (MatrixSatisfies.sameEq hg hgS).symm
        rw [hs]
        exact MatrixSatisfies.nonIncrease hh hhN
  · exact absurd hacc (by simp)

/-- An identity product entry preserves the measure across the two steps. -/
theorem compose_same_steps {n : Nat} {g h : MGraph n} {i k : Fin n}
    (hprod : composeMatrix g h i k = .same) {s t u : Fin n → Nat}
    (hg : MatrixSatisfies g s t) (hh : MatrixSatisfies h t u) : u k = s i := by
  have hfold : (List.finRange n).foldr (fun j acc => ((g i j).compose (h j k)).join acc) .unknown
      = .same := by simpa [composeMatrix] using hprod
  rcases foldr_join_eq_of_ne_unknown (fun j => (g i j).compose (h j k)) (List.finRange n)
      .unknown .same hfold (by simp) with ⟨j, _, hj⟩ | hacc
  · rcases (compose_eq_same (g i j) (h j k)).mp hj with ⟨hgS, hhS⟩
    rw [MatrixSatisfies.sameEq hh hhS, MatrixSatisfies.sameEq hg hgS]
  · exact absurd hacc (by simp)

/-- **Composition preserves semantics.** -/
theorem matrixSatisfies_compose {n : Nat} {g h : MGraph n} {s t u : Fin n → Nat}
    (hg : MatrixSatisfies g s t) (hh : MatrixSatisfies h t u) :
    MatrixSatisfies (composeMatrix g h) s u := by
  intro i k _
  exact ⟨fun hst => compose_strict_steps hst hg hh,
    fun hni => compose_nonIncrease_steps hni hg hh,
    fun hsm => compose_same_steps hsm hg hh⟩

/-- The `k`-fold product of a graph with itself. -/
def powerMatrix {n : Nat} : Nat → MGraph n → MGraph n
  | 0, _ => idMatrix
  | k + 1, g => composeMatrix g (powerMatrix k g)

/-- The identity matrix is satisfied by any state paired with itself. -/
theorem matrixSatisfies_id {n : Nat} (s : Fin n → Nat) : MatrixSatisfies (idMatrix : MGraph n) s s := by
  intro i j hknown
  have hij : i = j := by
    by_cases h : i = j
    · exact h
    · exfalso
      have : idMatrix i j = (Rel4.unknown : Rel4) := by simp [idMatrix, h]
      exact hknown this
  subst hij
  simp [idMatrix]

/-- A run satisfying every step of `g` satisfies each iterate of the product. -/
theorem matrixSatisfies_power {n : Nat} (g : MGraph n) (state : Nat → Fin n → Nat)
    (sat : ∀ t, MatrixSatisfies g (state t) (state (t + 1))) :
    ∀ (k t : Nat), MatrixSatisfies (powerMatrix k g) (state t) (state (t + k))
  | 0, t => matrixSatisfies_id (state t)
  | k + 1, t => by
      have hstep : MatrixSatisfies g (state t) (state (t + 1)) := sat t
      have hrest : MatrixSatisfies (powerMatrix k g) (state (t + 1)) (state (t + 1 + k)) :=
        matrixSatisfies_power g state sat k (t + 1)
      have hcomp := matrixSatisfies_compose hstep hrest
      simpa [powerMatrix, Nat.add_assoc, Nat.add_comm, Nat.add_left_comm] using hcomp

/-- **A strict `k`-fold product forbids infinite runs**: each block of `k` steps drops the measure,
so the states at multiples of `k` form a strictly decreasing chain. -/
theorem power_cycle_terminates {n : Nat} {g : MGraph n} {k : Nat} {i : Fin n}
    (hprod : powerMatrix k g i i = .strict) (state : Nat → Fin n → Nat)
    (sat : ∀ t, MatrixSatisfies g (state t) (state (t + 1))) : False := by
  apply no_decreasing_chain (fun m => state (m * k) i)
  intro m
  have hblock := matrixSatisfies_power g state sat k (m * k)
  have hdrop : state (m * k + k) i < state (m * k) i := MatrixSatisfies.strict hblock hprod
  have harith : (m + 1) * k = m * k + k := by rw [Nat.add_mul, Nat.one_mul]
  rw [harith]
  exact hdrop

/-- The product of the first `k` matrices, starting at step `t`. -/
def pathMatrix {n : Nat} : Nat → (Nat → MGraph n) → Nat → MGraph n
  | 0, _, _ => idMatrix
  | k + 1, step, t => composeMatrix (step t) (pathMatrix k step (t + 1))

/-- The constant sequence recovers the `k`-fold product. -/
theorem pathMatrix_const {n : Nat} (g : MGraph n) :
    ∀ (k t : Nat), pathMatrix k (fun _ => g) t = powerMatrix k g
  | 0, _ => rfl
  | k + 1, _ => by
      simp only [pathMatrix, powerMatrix]
      rw [pathMatrix_const g k]

/-- **A run satisfies every prefix product**: the relation after `k` steps of a varying sequence. -/
theorem matrixSatisfies_path {n : Nat} (step : Nat → MGraph n) (state : Nat → Fin n → Nat)
    (sat : ∀ t, MatrixSatisfies (step t) (state t) (state (t + 1))) :
    ∀ (k t : Nat), MatrixSatisfies (pathMatrix k step t) (state t) (state (t + k))
  | 0, t => matrixSatisfies_id (state t)
  | k + 1, t => by
      have hstep : MatrixSatisfies (step t) (state t) (state (t + 1)) := sat t
      have hrest : MatrixSatisfies (pathMatrix k step (t + 1)) (state (t + 1)) (state (t + 1 + k)) :=
        matrixSatisfies_path step state sat k (t + 1)
      have hcomp := matrixSatisfies_compose hstep hrest
      simpa [pathMatrix, Nat.add_assoc, Nat.add_comm, Nat.add_left_comm] using hcomp

/-- A periodic step sequence makes the path product blind to the start offset by one period. -/
theorem pathMatrix_shift {n : Nat} {step : Nat → MGraph n} {p : Nat}
    (hper : ∀ t, step (t + p) = step t) :
    ∀ (k t : Nat), pathMatrix k step (t + p) = pathMatrix k step t := by
  intro k
  induction k with
  | zero => intro t; rfl
  | succ k ih =>
      intro t
      simp only [pathMatrix]
      rw [hper t]
      have harith : t + p + 1 = (t + 1) + p := by omega
      rw [harith, ih (t + 1)]

end NobleM2.SizeChange
