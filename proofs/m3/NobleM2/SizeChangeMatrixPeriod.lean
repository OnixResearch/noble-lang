import NobleM2.SizeChangeMatrixExponent

/-!
# Periodicity of matrix powers (issue #101)

From one repetition `power a = power (a + d)` powers repeat with the same shift at every later
exponent. This is the core of the idempotent-power step.

- `nat_le_induction`: induction from a lower bound, proved here because the core has no
  `Nat.le_induction` (that lemma is a library one);
- `power_shift_usable`: the repetition in the factorised form the iteration consumes;
- `power_recurrence`: `power m = power (m + d)` for every `m ≥ a`.

Inducting from the bound means no exponent is decomposed, which is what made the earlier attempts
fail in a core without `nth_rewrite` or `conv`.

The iteration and the idempotent power are the next steps and are not proved here.
-/

namespace NobleM2.SizeChange

/-- Induction from a lower bound, for a property of `Nat`. -/
theorem nat_le_induction {P : Nat → Prop} (a : Nat) (hbase : P a)
    (hstep : ∀ n, a ≤ n → P n → P (n + 1)) : ∀ n, a ≤ n → P n := by
  intro n hn
  induction n with
  | zero =>
      have haz : a = 0 := by omega
      subst haz
      exact hbase
  | succ k ih =>
      by_cases hak : a ≤ k
      · exact hstep k hak (ih hak)
      · have hak_eq : a = k + 1 := by omega
        subst hak_eq
        exact hbase

/-- One repetition, rewritten as a factorisation. -/
theorem power_shift_usable {n : Nat} {g : MGraph n} {a d : Nat}
    (h : powerMatrix a g = powerMatrix (a + d) g) :
    powerMatrix a g = composeMatrix (powerMatrix a g) (powerMatrix d g) :=
  h.trans (powerMatrix_add g a d)

/-- **Powers repeat with the same shift at every later exponent.** -/
theorem power_recurrence {n : Nat} {g : MGraph n} {a d : Nat}
    (h : powerMatrix a g = composeMatrix (powerMatrix a g) (powerMatrix d g)) :
    ∀ m, a ≤ m → powerMatrix m g = powerMatrix (m + d) g := by
  refine nat_le_induction a ?base ?step
  · exact h.trans (powerMatrix_add g a d).symm
  · intro n _ ih
    have harith : n + 1 + d = (n + d) + 1 := by omega
    rw [harith]
    simp only [powerMatrix]
    rw [ih]

end NobleM2.SizeChange
