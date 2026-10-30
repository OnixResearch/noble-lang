import NobleFoundation.Repr

namespace NobleFoundation.CheckedI64

inductive ArithError where
  | overflow
  | divisionByZero
  deriving DecidableEq

abbrev minInt : ℤ := -(2 ^ 63 : ℤ)
abbrev upper : ℤ := 2 ^ 63

def inRange (v : ℤ) : Except ArithError ℤ :=
  if I64Range v then .ok v else .error .overflow

def checkedAdd (a b : ℤ) : Except ArithError ℤ := inRange (a + b)
def checkedSub (a b : ℤ) : Except ArithError ℤ := inRange (a - b)
def checkedMul (a b : ℤ) : Except ArithError ℤ := inRange (a * b)
def checkedNeg (a : ℤ) : Except ArithError ℤ := inRange (-a)
def checkedAbs (a : ℤ) : Except ArithError ℤ := inRange |a|

def sgn (a : ℤ) : ℤ := a.sign

def tquot (a b : ℤ) : ℤ :=
  sgn a * sgn b * ((a.natAbs / b.natAbs : ℕ) : ℤ)
def trem (a b : ℤ) : ℤ :=
  sgn a * ((a.natAbs % b.natAbs : ℕ) : ℤ)

def checkedDivRem (a b : ℤ) : Except ArithError (ℤ × ℤ) :=
  if b = 0 then .error .divisionByZero
  else if I64Range (tquot a b) then .ok (tquot a b, trem a b)
  else .error .overflow

def signedLt (x y : BitVec 64) : Prop := x.toInt < y.toInt

def inclusiveBoundary (v : ℤ) : Prop := -2 ^ 63 ≤ v ∧ v ≤ 2 ^ 63

private theorem inRange_ok_iff (x v : ℤ) :
    inRange x = .ok v ↔ I64Range x ∧ v = x := by
  unfold inRange
  by_cases hx : I64Range x <;> simp [hx, eq_comm]

private theorem inRange_overflow_iff (x : ℤ) :
    inRange x = .error .overflow ↔ ¬ I64Range x := by
  unfold inRange
  by_cases hx : I64Range x <;> simp [hx]

theorem checkedAdd_ok_iff (a b v : ℤ) :
    checkedAdd a b = .ok v ↔ I64Range (a + b) ∧ v = a + b := by
  simpa [checkedAdd] using inRange_ok_iff (a + b) v

theorem checkedSub_ok_iff (a b v : ℤ) :
    checkedSub a b = .ok v ↔ I64Range (a - b) ∧ v = a - b := by
  simpa [checkedSub] using inRange_ok_iff (a - b) v

theorem checkedMul_ok_iff (a b v : ℤ) :
    checkedMul a b = .ok v ↔ I64Range (a * b) ∧ v = a * b := by
  simpa [checkedMul] using inRange_ok_iff (a * b) v

theorem checkedNeg_ok_iff (a v : ℤ) :
    checkedNeg a = .ok v ↔ I64Range (-a) ∧ v = -a := by
  simpa [checkedNeg] using inRange_ok_iff (-a) v

theorem checkedAbs_ok_iff (a v : ℤ) :
    checkedAbs a = .ok v ↔ I64Range |a| ∧ v = |a| := by
  simpa [checkedAbs] using inRange_ok_iff |a| v

theorem checkedAdd_overflow_iff (a b : ℤ) :
    checkedAdd a b = .error .overflow ↔ ¬ I64Range (a + b) := by
  simpa [checkedAdd] using inRange_overflow_iff (a + b)

theorem checkedSub_overflow_iff (a b : ℤ) :
    checkedSub a b = .error .overflow ↔ ¬ I64Range (a - b) := by
  simpa [checkedSub] using inRange_overflow_iff (a - b)

theorem checkedMul_overflow_iff (a b : ℤ) :
    checkedMul a b = .error .overflow ↔ ¬ I64Range (a * b) := by
  simpa [checkedMul] using inRange_overflow_iff (a * b)

theorem checkedNeg_overflow_iff (a : ℤ) :
    checkedNeg a = .error .overflow ↔ ¬ I64Range (-a) := by
  simpa [checkedNeg] using inRange_overflow_iff (-a)

theorem checkedAbs_overflow_iff (a : ℤ) :
    checkedAbs a = .error .overflow ↔ ¬ I64Range |a| := by
  simpa [checkedAbs] using inRange_overflow_iff |a|

theorem neg_min_overflows :
    checkedNeg minInt = .error .overflow := by decide

theorem abs_min_overflows :
    checkedAbs minInt = .error .overflow := by decide

private theorem sign_sign (z : ℤ) : z.sign.sign = z.sign := by
  by_cases hz : z = 0
  · simp [hz]
  · rcases lt_or_gt_of_ne hz with hzneg | hzpos
    · rw [Int.sign_eq_neg_one_iff_neg.mpr hzneg]
      norm_num
    · rw [Int.sign_eq_one_iff_pos.mpr hzpos]
      norm_num

private theorem sign_sq_of_ne_zero {z : ℤ} (h : z ≠ 0) :
    sgn z * sgn z = 1 := by
  rcases lt_or_gt_of_ne h with hz | hz
  · rw [show sgn z = -1 by exact Int.sign_eq_neg_one_iff_neg.mpr hz]
    norm_num
  · rw [show sgn z = 1 by exact Int.sign_eq_one_iff_pos.mpr hz]
    norm_num

private theorem sign_natAbs_zero (z : ℤ) (h : z = 0) :
    z.sign.natAbs = 0 := by simp [h]

private theorem sign_natAbs_nonzero (z : ℤ) (h : z ≠ 0) :
    z.sign.natAbs = 1 := by
  rcases lt_or_gt_of_ne h with hz | hz
  · rw [Int.sign_eq_neg_one_iff_neg.mpr hz]
    norm_num
  · rw [Int.sign_eq_one_iff_pos.mpr hz]
    norm_num

private theorem tquot_natAbs (a b : ℤ) :
    (tquot a b).natAbs = a.natAbs / b.natAbs := by
  unfold tquot sgn
  rw [Int.natAbs_mul, Int.natAbs_mul, Int.natAbs_natCast]
  by_cases ha : a = 0
  · simp [ha]
  by_cases hb : b = 0
  · simp [hb]
  · rw [sign_natAbs_nonzero a ha, sign_natAbs_nonzero b hb]
    simp

private theorem trem_natAbs (a b : ℤ) :
    (trem a b).natAbs = a.sign.natAbs * (a.natAbs % b.natAbs) := by
  unfold trem sgn
  rw [Int.natAbs_mul, Int.natAbs_natCast]

theorem div_rem_identity (a b : ℤ) (hb : b ≠ 0) :
    a = b * tquot a b + trem a b := by
  have hcast : (b.natAbs : ℤ) *
      ((a.natAbs / b.natAbs : ℕ) : ℤ) +
      ((a.natAbs % b.natAbs : ℕ) : ℤ) = (a.natAbs : ℤ) := by
    exact_mod_cast (Nat.div_add_mod a.natAbs b.natAbs)
  have hsq : sgn b * sgn b = 1 := sign_sq_of_ne_zero hb
  calc
    a = sgn a * (a.natAbs : ℤ) := by
      exact (Int.sign_mul_natAbs a).symm
    _ = sgn a * ((b.natAbs : ℤ) *
        ((a.natAbs / b.natAbs : ℕ) : ℤ) +
        ((a.natAbs % b.natAbs : ℕ) : ℤ)) := by rw [hcast]
    _ = (sgn b * (b.natAbs : ℤ)) *
        (sgn a * sgn b * ((a.natAbs / b.natAbs : ℕ) : ℤ)) +
        sgn a * ((a.natAbs % b.natAbs : ℕ) : ℤ) := by
      simp only [sgn] at hsq ⊢
      have hfirst : b.sign * (b.natAbs : ℤ) *
          (a.sign * b.sign * ((a.natAbs / b.natAbs : ℕ) : ℤ)) =
          a.sign * ((b.natAbs : ℤ) * ((a.natAbs / b.natAbs : ℕ) : ℤ)) := by
        calc
          b.sign * (b.natAbs : ℤ) *
              (a.sign * b.sign * ((a.natAbs / b.natAbs : ℕ) : ℤ)) =
              a.sign * (b.sign * b.sign) *
                ((b.natAbs : ℤ) * ((a.natAbs / b.natAbs : ℕ) : ℤ)) := by ring
          _ = a.sign * ((b.natAbs : ℤ) *
                ((a.natAbs / b.natAbs : ℕ) : ℤ)) := by rw [hsq]; ring
      rw [hfirst]
      ring
    _ = b * tquot a b + trem a b := by
      simp only [tquot, trem, sgn]
      rw [Int.sign_mul_natAbs]

theorem trem_natAbs_lt (a b : ℤ) (hb : b ≠ 0) :
    (trem a b).natAbs < b.natAbs := by
  rw [trem_natAbs]
  have hr := Nat.mod_lt a.natAbs (Int.natAbs_pos.mpr hb)
  by_cases ha : a = 0
  · rw [sign_natAbs_zero a ha]
    simp only [Nat.zero_mul]
    exact Int.natAbs_pos.mpr hb
  · rw [sign_natAbs_nonzero a ha]
    simp
    exact hr

theorem trem_sign (a b : ℤ) (_hb : b ≠ 0) (hr : trem a b ≠ 0) :
    (trem a b).sign = sgn a := by
  have hrem : a.natAbs % b.natAbs ≠ 0 := by
    intro hzero
    apply hr
    unfold trem sgn
    rw [hzero]
    simp
  have hpos : 0 < a.natAbs % b.natAbs := Nat.pos_of_ne_zero hrem
  have hposZ : 0 < ((a.natAbs % b.natAbs : ℕ) : ℤ) := by exact_mod_cast hpos
  calc
    (trem a b).sign = (sgn a * ((a.natAbs % b.natAbs : ℕ) : ℤ)).sign := by rfl
    _ = (sgn a).sign * ((a.natAbs % b.natAbs : ℕ) : ℤ).sign := Int.sign_mul _ _
    _ = sgn a := by
      rw [Int.sign_eq_one_iff_pos.mpr hposZ]
      change (a.sign).sign * 1 = a.sign
      rw [sign_sign]
      simp

private theorem range_natAbs_le {a : ℤ} (ha : I64Range a) :
    a.natAbs ≤ 2 ^ 63 := by
  cases a with
  | ofNat n =>
      simp only [I64Range] at ha ⊢
      omega
  | negSucc n =>
      simp only [I64Range, Int.negSucc_eq] at ha ⊢
      omega

private theorem tquot_natAbs_le {a b : ℤ} (ha : I64Range a) (_hb : b ≠ 0) :
    (tquot a b).natAbs ≤ 2 ^ 63 := by
  rw [tquot_natAbs]
  exact le_trans (Nat.div_le_self _ _) (range_natAbs_le ha)

private theorem tquot_range_of_ne_upper {a b : ℤ} (ha : I64Range a)
    (_hb : I64Range b) (hb0 : b ≠ 0) (hq : tquot a b ≠ upper) :
    I64Range (tquot a b) := by
  have hbound := tquot_natAbs_le ha hb0
  have hboundZ : ((tquot a b).natAbs : ℤ) ≤ (2 ^ 63 : ℤ) := by
    exact_mod_cast hbound
  by_cases hnonneg : 0 ≤ tquot a b
  · have hn := Int.natAbs_of_nonneg hnonneg
    rw [hn] at hboundZ
    unfold upper at hq
    unfold I64Range
    constructor <;> omega
  · have hneg : tquot a b < 0 := lt_of_not_ge hnonneg
    have hnneg : 0 ≤ -tquot a b := by omega
    have hn := Int.natAbs_of_nonneg hnneg
    have hn' : ((tquot a b).natAbs : ℤ) = -tquot a b := by
      rw [← Int.natAbs_neg, hn]
    rw [hn'] at hboundZ
    unfold I64Range
    constructor <;> omega

private theorem tquot_upper_iff {a b : ℤ} (ha : I64Range a)
    (_hb : I64Range b) (hb0 : b ≠ 0) :
    tquot a b = upper ↔ a = minInt ∧ b = -1 := by
  constructor
  · intro hq
    have hbound := range_natAbs_le ha
    have hdiv : a.natAbs / b.natAbs = 2 ^ 63 := by
      rw [← tquot_natAbs]
      rw [hq]
      simp [upper]
    have hBpos : 0 < b.natAbs := Int.natAbs_pos.mpr hb0
    have hmul : (2 ^ 63 : ℕ) * b.natAbs ≤ a.natAbs := by
      apply (Nat.le_div_iff_mul_le hBpos).mp
      rw [hdiv]
    have hB : b.natAbs = 1 := by omega
    have hA : a.natAbs = 2 ^ 63 := by
      rw [hB] at hdiv
      simpa using hdiv
    have ha' : a = minInt := by
      cases a with
      | ofNat n =>
          change n = 2 ^ 63 at hA
          have hAc : (n : ℤ) = (2 ^ 63 : ℤ) := by exact_mod_cast hA
          change (n : ℤ) = minInt
          unfold minInt
          change -(2 ^ 63 : ℤ) ≤ (n : ℤ) ∧ (n : ℤ) < 2 ^ 63 at ha
          omega
      | negSucc n =>
          change n.succ = 2 ^ 63 at hA
          have hAc : (n : ℤ) + 1 = (2 ^ 63 : ℤ) := by exact_mod_cast hA
          simp only [Int.negSucc_eq]
          unfold minInt
          omega
    have hb' : b = -1 := by
      cases b with
      | ofNat n =>
          change n = 1 at hB
          have hn : n = 1 := by omega
          subst n
          exfalso
          rw [ha'] at hq
          norm_num [tquot, sgn, upper] at hq
          omega
      | negSucc n =>
          change n.succ = 1 at hB
          have hn : n = 0 := by omega
          subst n
          rfl
    exact ⟨ha', hb'⟩
  · rintro ⟨rfl, rfl⟩
    norm_num [tquot, sgn, minInt, upper, Int.negSucc_eq]

theorem checkedDivRem_overflow_iff {a b : ℤ} (ha : I64Range a)
    (hb : I64Range b) (hb0 : b ≠ 0) :
    checkedDivRem a b = .error .overflow ↔ a = minInt ∧ b = -1 := by
  unfold checkedDivRem
  rw [if_neg hb0]
  by_cases hq : I64Range (tquot a b)
  · have hnot : ¬ (a = minInt ∧ b = -1) := by
      rintro ⟨rfl, rfl⟩
      norm_num [tquot, sgn, minInt, upper, I64Range] at hq
    rw [if_pos hq]
    simp [hnot]
  · have hupper : tquot a b = upper := by
      by_contra hne
      exact hq (tquot_range_of_ne_upper ha hb hb0 hne)
    rw [if_neg hq]
    simpa [hupper] using (tquot_upper_iff ha hb hb0)

theorem checkedDivRem_divisionByZero (a : ℤ) :
    checkedDivRem a 0 = .error .divisionByZero := by
  simp [checkedDivRem]

theorem truncating_quotient_control :
    tquot (-7) 2 = -3 ∧ Int.ediv (-7) 2 = -4 ∧ Int.fdiv (-7) 2 = -4 := by
  decide

theorem truncating_remainder_control :
    trem (-7) 2 = -1 ∧ Int.emod (-7) 2 = 1 := by
  decide

theorem signed_order_control :
    signedLt (BitVec.ofInt 64 (-1)) (BitVec.ofInt 64 0) ∧
      BitVec.ult (BitVec.ofInt 64 (-1)) (BitVec.ofInt 64 0) = false := by
  unfold signedLt
  decide

theorem off_by_one_boundary_control :
    ¬ I64Range (2 ^ 63 : ℤ) ∧ inclusiveBoundary (2 ^ 63 : ℤ) := by
  unfold inclusiveBoundary I64Range
  decide

theorem swapped_sub_control :
    checkedSub 3 8 ≠ checkedSub 8 3 := by
  decide

theorem swapped_divrem_control :
    checkedDivRem 7 2 ≠ checkedDivRem 2 7 := by
  decide

theorem min_neg_one_has_no_quotient :
    checkedDivRem minInt (-1) = .error .overflow := by
  decide

end NobleFoundation.CheckedI64
