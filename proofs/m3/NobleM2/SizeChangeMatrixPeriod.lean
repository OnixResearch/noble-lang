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

/-- **Iterating the recurrence**: every exponent congruent to `m` modulo `d` has the same power. -/
theorem power_iterate {n : Nat} {g : MGraph n} {a d : Nat}
    (h : powerMatrix a g = composeMatrix (powerMatrix a g) (powerMatrix d g)) :
    ∀ (k m : Nat), a ≤ m → powerMatrix (m + k * d) g = powerMatrix m g := by
  intro k
  induction k with
  | zero => intro m _; simp
  | succ k ih =>
      intro m hm
      have harith : m + (k + 1) * d = (m + k * d) + d := by
        rw [Nat.succ_mul]
        omega
      rw [harith]
      rw [← power_recurrence h (m + k * d) (by omega)]
      exact ih m hm

/-- **An idempotent power exists**: `power (a * d)` is its own square. -/
theorem power_idempotent {n : Nat} {g : MGraph n} {a d : Nat} (hd : 0 < d)
    (h : powerMatrix a g = composeMatrix (powerMatrix a g) (powerMatrix d g)) :
    composeMatrix (powerMatrix (a * d) g) (powerMatrix (a * d) g) = powerMatrix (a * d) g := by
  rw [← powerMatrix_add]
  have hge : a ≤ a * d := by
    have hd1 : 1 ≤ d := hd
    calc a = a * 1 := (Nat.mul_one a).symm
      _ ≤ a * d := Nat.mul_le_mul_left a hd1
  exact power_iterate h a (a * d) hge

/-- **An idempotent matrix represents all its positive powers.** -/
theorem idempotent_power_stable {n : Nat} {x : MGraph n} (h : composeMatrix x x = x) :
    ∀ k, 1 ≤ k → powerMatrix k x = x := by
  intro k _
  have hp1 : powerMatrix 1 x = x := composeMatrix_id_right x
  have hrec : powerMatrix 1 x = composeMatrix (powerMatrix 1 x) (powerMatrix 1 x) := by
    rw [hp1]
    exact h.symm
  have hiter := power_iterate hrec (k - 1) 1 (Nat.le_refl 1)
  rw [Nat.mul_one] at hiter
  have hk : 1 + (k - 1) = k := by omega
  rw [hk] at hiter
  rw [hiter, hp1]

/-- **One representative per idempotent**: a nodup list of positive powers of an idempotent matrix
has at most one element. -/
theorem idempotent_power_nodup_length {n : Nat} {x : MGraph n} (h : composeMatrix x x = x) :
    ∀ (l : List Nat), (∀ k ∈ l, 1 ≤ k) →
      (l.map (fun k => powerMatrix k x)).Nodup → l.length ≤ 1
  | [], _, _ => by simp
  | [_], _, _ => by simp
  | a :: b :: rest, hpos, hnd => by
      exfalso
      have ha : powerMatrix a x = x := idempotent_power_stable h a (hpos a (by simp))
      have hb : powerMatrix b x = x := idempotent_power_stable h b (hpos b (by simp))
      rw [List.map_cons, List.nodup_cons] at hnd
      have hhead : powerMatrix a x ∉ (b :: rest).map (fun k => powerMatrix k x) := hnd.1
      apply hhead
      rw [ha]
      have hbmem : powerMatrix b x ∈ (b :: rest).map (fun k => powerMatrix k x) :=
        List.mem_map.mpr ⟨b, by simp, rfl⟩
      simp [hb]

end NobleM2.SizeChange
