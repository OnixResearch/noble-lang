/-
  A total roundoff certificate checker for the binary32 add/multiply fragment (issue #44).

  The design (`.cairn/changes/numerical-error-certificate`) separates an untrusted analyzer from a
  small total checker: the analyzer proposes an enclosure of every node's real value and a bound on
  the absolute error of its machine value, and the checker accepts or refuses. The proved statement
  is "accepted certificate ⇒ per-node bound for every input meeting the checked premises".

  - The fragment is a finite acyclic graph of inputs, constant bit patterns, `add` and `mul` in
    stored order. The real graph keeps that order and reads every constant exactly. `sub`, the
    affine fold and the piecewise operations are refused as unsupported.
  - Inputs are explicit: a declared domain for the real input and a declared bound on the machine
    input's distance from it. A bound of zero says the real input is the exact value of the bits.
  - Subnormals are handled by the proved mixed rule `|rn(x) - x| ≤ u · |x| + η`
    (`SequenceFinite.add_mixed_error`, `SequenceFinite.mul_mixed_error`), never flushed. Overflow is
    refused: the checker requires every machine result the enclosures allow to stay below `2^127`,
    and `noOverflow_of_lt` turns that into the premise of the mixed rule.
  - `check_sound`: an accepted certificate encloses every node, the machine value is finite and
    within the node's error bound of the real value. `check_output_bound` states it per output.
  - The outcomes keep `unknown` (a check failed), `unsupported` and `exhausted` (the graph or a
    certificate rational exceeds the budget) apart; the checker never reports a disproved bound.

  This is a statement about the exact reference model only: no analyzer, Rust, Wasm or host
  floating-point correspondence is claimed, and no encoded `noble-ngraph` graph is bound.
-/
import NobleFoundation.Binary32SequenceFinite

namespace NobleFoundation.Binary32.Checker

open NobleFoundation.Binary32
open NobleFoundation.Binary32.Error
open NobleFoundation.Binary32.Scale
open NobleFoundation.Binary32.Subnormal
open NobleFoundation.Binary32.Sequence
open NobleFoundation.Binary32.SequenceFinite

set_option exponentiation.threshold 4096

/-! ## From a magnitude to the finite premises -/

/-- The overflow threshold the checker uses. It is sufficient, not tight: magnitudes between it and
the largest finite value are refused although they would round to a finite value. -/
def overflowLimit : ℚ := (2 : ℚ) ^ (127 : ℤ)

/-- The carried exponent is at most one above the floor logarithm of the ratio. -/
theorem carried_le_succ (a b : Nat) : (carriedPair a b).2 ≤ floorLog2Ratio a b + 1 := by
  unfold carriedPair
  dsimp only
  split <;> dsimp only <;> omega

/-- **A ratio below `2^127` does not overflow**: the lower bracket keeps its floor logarithm below
`127`, so the carried exponent is at most `127`. -/
theorem noOverflow_of_lt (a b : Nat) (ha : 0 < a) (hb : 0 < b)
    (small : (a : ℚ) < overflowLimit * b) : NoOverflow (a, b) := by
  have lower := floorLog2Ratio_lower a b ha hb
  have hbq : (0 : ℚ) < b := by exact_mod_cast hb
  have powers : (2 : ℚ) ^ floorLog2Ratio a b < (2 : ℚ) ^ (127 : ℤ) :=
    lt_of_mul_lt_mul_right (lt_of_le_of_lt lower small) (le_of_lt hbq)
  have exponentLt : floorLog2Ratio a b < 127 :=
    (zpow_lt_zpow_iff_right₀ (by norm_num : (1 : ℚ) < 2)).1 powers
  have carried := carried_le_succ a b
  unfold NoOverflow
  dsimp only
  omega

/-- The scaled form: a nonzero `m · 2^s` of magnitude below `2^127` does not overflow. -/
theorem noOverflow_scaled (m s : Int) (nonzero : m ≠ 0)
    (small : |(m : ℚ) * (2 : ℚ) ^ s| < overflowLimit) : NoOverflow (scaledArgs m s) := by
  obtain ⟨numeratorPos, denominatorPos⟩ := scaledArgs_pos m s nonzero
  apply noOverflow_of_lt _ _ numeratorPos denominatorPos
  have hbq : (0 : ℚ) < (scaledArgs m s).2 := by exact_mod_cast denominatorPos
  have magnitude : |(m : ℚ) * (2 : ℚ) ^ s| = (m.natAbs : ℚ) * (2 : ℚ) ^ s := by
    rw [abs_mul, abs_of_pos (zpow_pos (by norm_num : (0 : ℚ) < 2) s), Nat.cast_natAbs, Int.cast_abs]
  rw [magnitude, ← scaledArgs_ratio, div_lt_iff₀ hbq] at small
  exact small

/-- An encoding whose exponent field is not all ones is finite. -/
theorem finite_of_exponent (b : Bits) (field : exponent b ≠ 255) : Finite b := by
  unfold Finite classify
  constructor <;> split <;> simp_all

/-- **A magnitude below the infinity encoding is finite with either sign.** -/
theorem finite_of_magnitude (magnitude : Nat) (small : magnitude < 255 * 2 ^ 23) (negative : Bool) :
    Finite (BitVec.ofNat 32 (if negative then magnitude ||| 0x80000000 else magnitude)) := by
  apply finite_of_exponent
  cases negative
  · simp only [Bool.false_eq_true, if_false]
    rw [exponent_of_toNat, toNat_ofNat_of_lt magnitude (by omega)]
    omega
  · simp only [if_true]
    have orAdd : magnitude ||| 0x80000000 = magnitude + 2 ^ 31 := by
      rw [Nat.or_comm, show (0x80000000 : Nat) = 2 ^ 31 * 1 from rfl,
        ← Nat.two_pow_add_eq_or_of_lt (by omega : magnitude < 2 ^ 31) 1]
      omega
    rw [exponent_of_toNat, orAdd, toNat_ofNat_of_lt _ (by omega)]
    omega

/-- A positive rounding that does not overflow stays below the infinity encoding. -/
theorem roundPositive_below_infinity (a b : Nat) (ha : 0 < a) (hb : 0 < b)
    (finite : NoOverflow (a, b)) : roundPositive a b < 255 * 2 ^ 23 := by
  unfold NoOverflow at finite
  dsimp only at finite
  by_cases normal : -126 ≤ (carriedPair a b).2
  · rw [roundPositive_normal_nat a b ⟨normal, finite⟩]
    have significand := carried_significand_lt a b ha hb
    have exponentBound : ((carriedPair a b).2 + 127).toNat ≤ 254 := by omega
    simp only [pow2]
    have product : ((carriedPair a b).2 + 127).toNat * 2 ^ 23 ≤ 254 * 2 ^ 23 :=
      Nat.mul_le_mul_right _ exponentBound
    omega
  · rw [roundPositive_eq_significand a b ha hb (by omega)]
    have bound := subnormal_significand_bound a b ha hb (by omega)
    omega

/-- **A rounding that does not overflow is finite**, with either sign. -/
theorem roundScaled_finite (m s : Int) (nonzero : m ≠ 0) (finite : NoOverflow (scaledArgs m s))
    (negative : Bool) : Finite (roundScaled m s negative) := by
  obtain ⟨numeratorPos, denominatorPos⟩ := scaledArgs_pos m s nonzero
  have small := roundPositive_below_infinity _ _ numeratorPos denominatorPos finite
  have roundedIs : roundScaled m s negative =
      BitVec.ofNat 32 (if negative then roundPositive (scaledArgs m s).1 (scaledArgs m s).2 |||
        0x80000000 else roundPositive (scaledArgs m s).1 (scaledArgs m s).2) := rfl
  rw [roundedIs]
  exact finite_of_magnitude _ small negative

theorem zeroOf_finite (negative : Bool) : Finite (zeroOf negative) := by
  unfold Finite
  cases negative <;> decide

/-- **A finite addition has a finite result.** -/
theorem add_finite (x y : Bits) (ok : AddFinite x y) : Finite (add x y) := by
  obtain ⟨finiteX, finiteY, range⟩ := ok
  rw [add_eq x y finiteX finiteY]
  split
  · exact zeroOf_finite _
  · rename_i nonzero
    exact roundScaled_finite _ _ nonzero (range nonzero) _

/-- **A finite multiplication has a finite result.** -/
theorem mul_finite (x y : Bits) (ok : MulFinite x y) : Finite (mul x y) := by
  obtain ⟨⟨xNaN, xInfinite⟩, ⟨yNaN, yInfinite⟩, range⟩ := ok
  by_cases zeroX : classify x = .posZero ∨ classify x = .negZero
  · have result : mul x y = zeroOf (isNegative x != isNegative y) := by
      unfold mul
      rcases zeroX with classX | classX <;>
      · rw [classX]
        cases classY : classify y <;> simp_all
    rw [result]
    exact zeroOf_finite _
  by_cases zeroY : classify y = .posZero ∨ classify y = .negZero
  · have result : mul x y = zeroOf (isNegative x != isNegative y) := by
      unfold mul
      rcases zeroY with classY | classY <;>
      · rw [classY]
        cases classX : classify x <;> simp_all
    rw [result]
    exact zeroOf_finite _
  have nonzeroX : classify x = .subnormal ∨ classify x = .normal := by
    cases classX : classify x <;> simp_all
  have nonzeroY : classify y = .subnormal ∨ classify y = .normal := by
    cases classY : classify y <;> simp_all
  obtain ⟨nonzero, _⟩ := product_sign x y (nonzero_class_magnitude x nonzeroX)
    (nonzero_class_magnitude y nonzeroY)
  rw [mul_scaled_eq x y nonzeroX nonzeroY]
  exact roundScaled_finite _ _ nonzero (range nonzero) _

/-- **The checker's overflow test establishes the finite premises of an addition.** -/
theorem addFinite_of_small (x y : Bits) (finiteX : Finite x) (finiteY : Finite y)
    (small : |exactValue x + exactValue y| < overflowLimit) : AddFinite x y :=
  ⟨finiteX, finiteY, fun nonzero => noOverflow_scaled _ _ nonzero (by rw [← exact_sum]; exact small)⟩

/-- **The checker's overflow test establishes the finite premises of a multiplication.** -/
theorem mulFinite_of_small (x y : Bits) (finiteX : Finite x) (finiteY : Finite y)
    (small : |exactValue x * exactValue y| < overflowLimit) : MulFinite x y :=
  ⟨finiteX, finiteY,
    fun nonzero => noOverflow_scaled _ _ nonzero (by rw [← exact_product]; exact small)⟩

instance (b : Bits) : Decidable (Finite b) :=
  inferInstanceAs (Decidable (classify b ≠ .notANumber ∧ classify b ≠ .infinity))

/-! ## The fragment, its two semantics, and the certificate -/

/-- One node of a graph in stored order. Operands name earlier nodes by position. -/
inductive Node where
  /-- The graph input at a position. -/
  | input (index : Nat)
  /-- A constant bit pattern, read exactly. -/
  | const (bits : Bits)
  /-- `F32Add` of two earlier nodes. -/
  | add (left right : Nat)
  /-- `F32Mul` of two earlier nodes. -/
  | mul (left right : Nat)
  /-- Any operation outside the fragment: `F32Sub`, `F32Affine` or a piecewise operation. -/
  | outside
deriving DecidableEq, Repr

/-- A graph: nodes in stored order and the positions of its outputs. -/
structure Graph where
  nodes : List Node
  outputs : List Nat

/-- One node's machine value, given the machine values of the nodes before it. -/
def machineNode (inputs : List Bits) (done : List Bits) : Node → Bits
  | .input index => (inputs[index]?).getD canonicalNaN
  | .const bits => bits
  | .add left right => add ((done[left]?).getD canonicalNaN) ((done[right]?).getD canonicalNaN)
  | .mul left right => mul ((done[left]?).getD canonicalNaN) ((done[right]?).getD canonicalNaN)
  | .outside => canonicalNaN

/-- The machine values of the remaining nodes, appended to those already computed. -/
def machineFrom (inputs : List Bits) : List Bits → List Node → List Bits
  | done, [] => done
  | done, node :: rest => machineFrom inputs (done ++ [machineNode inputs done node]) rest

/-- One node's real value. The real graph keeps the stored operation order and reads every
constant bit pattern exactly. -/
def realNode (inputs : List ℚ) (done : List ℚ) : Node → ℚ
  | .input index => (inputs[index]?).getD 0
  | .const bits => exactValue bits
  | .add left right => (done[left]?).getD 0 + (done[right]?).getD 0
  | .mul left right => (done[left]?).getD 0 * (done[right]?).getD 0
  | .outside => 0

/-- The real values of the remaining nodes, appended to those already computed. -/
def realFrom (inputs : List ℚ) : List ℚ → List Node → List ℚ
  | done, [] => done
  | done, node :: rest => realFrom inputs (done ++ [realNode inputs done node]) rest

/-- A declared input: the domain of the real input and the bound on the machine input's distance
from it. A bound of zero says the real input is the exact value of the supplied bits. -/
structure InputDomain where
  lo : ℚ
  hi : ℚ
  inputError : ℚ

/-- The analyzer's proposal for one node: an enclosure of its real value and a bound on the absolute
error of its machine value. -/
structure NodeBound where
  lo : ℚ
  hi : ℚ
  error : ℚ

/-- A certificate: the declared input domains and one proposed bound per node, in stored order. -/
structure Certificate where
  domains : List InputDomain
  bounds : List NodeBound

/-- The checking budget: the largest graph, and the bit size of every certificate rational. -/
structure Budget where
  nodes : Nat
  bits : Nat

/-! ## The checker -/

/-- The magnitude of every value in `[lo, hi]` is at most this. -/
def magnitudeOf (lo hi : ℚ) : ℚ := max |lo| |hi|

/-- The reach of a sum: the largest magnitude of a machine sum the operands' bounds allow. -/
def sumReach (a b : NodeBound) : ℚ :=
  magnitudeOf (a.lo - a.error + (b.lo - b.error)) (a.hi + a.error + (b.hi + b.error))

/-- The error a sum inherits from its operands plus one mixed rounding at its reach. -/
def sumError (a b : NodeBound) : ℚ := a.error + b.error + unit * sumReach a b + eta

/-- The interval product's lower end. -/
def productLow (a b : NodeBound) : ℚ :=
  min (min (a.lo * b.lo) (a.lo * b.hi)) (min (a.hi * b.lo) (a.hi * b.hi))

/-- The interval product's upper end. -/
def productHigh (a b : NodeBound) : ℚ :=
  max (max (a.lo * b.lo) (a.lo * b.hi)) (max (a.hi * b.lo) (a.hi * b.hi))

/-- The reach of a product: the largest magnitude of a machine product the operands' bounds allow. -/
def productReach (a b : NodeBound) : ℚ :=
  (magnitudeOf a.lo a.hi + a.error) * (magnitudeOf b.lo b.hi + b.error)

/-- The error a product inherits from its operands plus one mixed rounding at its reach. -/
def productError (a b : NodeBound) : ℚ :=
  a.error * (magnitudeOf b.lo b.hi + b.error) + magnitudeOf a.lo a.hi * b.error +
    unit * productReach a b + eta

/-- The verdict on one node. -/
inductive Verdict where
  | ok
  | unknown
  | unsupported
deriving DecidableEq, Repr

/-- The checks for one node, given the bounds already accepted for the nodes before it. An operand
that does not name an earlier node is unsupported, as is every operation outside the fragment. -/
def checkNode (domains : List InputDomain) (done : List NodeBound) (bound : NodeBound) :
    Node → Verdict
  | .input index =>
      match domains[index]? with
      | some domain =>
          if bound.lo ≤ domain.lo ∧ domain.hi ≤ bound.hi ∧ domain.inputError ≤ bound.error then .ok
          else .unknown
      | none => .unsupported
  | .const bits =>
      if Finite bits then
        if bound.lo ≤ exactValue bits ∧ exactValue bits ≤ bound.hi ∧ 0 ≤ bound.error then .ok
        else .unknown
      else .unsupported
  | .add left right =>
      match done[left]?, done[right]? with
      | some a, some b =>
          if bound.lo ≤ a.lo + b.lo ∧ a.hi + b.hi ≤ bound.hi ∧ sumReach a b < overflowLimit ∧
              sumError a b ≤ bound.error then .ok
          else .unknown
      | _, _ => .unsupported
  | .mul left right =>
      match done[left]?, done[right]? with
      | some a, some b =>
          if bound.lo ≤ productLow a b ∧ productHigh a b ≤ bound.hi ∧
              productReach a b < overflowLimit ∧ productError a b ≤ bound.error then .ok
          else .unknown
      | _, _ => .unsupported
  | .outside => .unsupported

/-- The checker's answer. `unknown` means a check failed, so the certificate does not establish its
bound; it is not a disproof. -/
inductive Outcome where
  | certified
  | unknown (position : Nat)
  | unsupported (position : Nat)
  | exhausted
deriving DecidableEq, Repr

/-- Check the remaining nodes against the remaining bounds, one node at a time. -/
def checkFrom (domains : List InputDomain) : List NodeBound → List Node → List NodeBound → Outcome
  | _, [], [] => .certified
  | done, [], _ :: _ => .unknown done.length
  | done, _ :: _, [] => .unknown done.length
  | done, node :: nodes, bound :: bounds =>
      match checkNode domains done bound node with
      | .ok => checkFrom domains (done ++ [bound]) nodes bounds
      | .unknown => .unknown done.length
      | .unsupported => .unsupported done.length

/-- A rational fits the budget when its numerator and denominator are below `2^bits`. -/
def fits (bits : Nat) (q : ℚ) : Bool := decide (q.num.natAbs < 2 ^ bits) && decide (q.den < 2 ^ bits)

/-- The graph and the certificate are within the budget. -/
def withinBudget (budget : Budget) (graph : Graph) (certificate : Certificate) : Bool :=
  decide (graph.nodes.length ≤ budget.nodes) && decide (certificate.domains.length ≤ budget.nodes) &&
    certificate.domains.all (fun domain =>
      fits budget.bits domain.lo && fits budget.bits domain.hi && fits budget.bits domain.inputError) &&
    certificate.bounds.all (fun bound =>
      fits budget.bits bound.lo && fits budget.bits bound.hi && fits budget.bits bound.error)

/-- **The total checker.** -/
def check (budget : Budget) (graph : Graph) (certificate : Certificate) : Outcome :=
  if withinBudget budget graph certificate then
    match graph.outputs.find? (fun output => graph.nodes.length ≤ output) with
    | some output => .unsupported output
    | none => checkFrom certificate.domains [] graph.nodes certificate.bounds
  else .exhausted

/-! ## Soundness -/

/-- The consumer's obligation for actual inputs: each machine input is finite, its real input lies
in the declared domain, and the two are within the declared input error. -/
def Applies (domains : List InputDomain) (machine : List Bits) (real : List ℚ) : Prop :=
  ∀ (index : Nat) (domain : InputDomain), domains[index]? = some domain →
    ∃ (x : Bits) (r : ℚ), machine[index]? = some x ∧
    real[index]? = some r ∧ Finite x ∧ domain.lo ≤ r ∧ r ≤ domain.hi ∧
      |exactValue x - r| ≤ domain.inputError

/-- What an accepted bound says about one node. -/
def Encloses (bound : NodeBound) (machine : Bits) (real : ℚ) : Prop :=
  Finite machine ∧ bound.lo ≤ real ∧ real ≤ bound.hi ∧ |exactValue machine - real| ≤ bound.error

/-- Bounds, machine values and real values align, and every bound encloses its node. -/
def AllEnclose (bounds : List NodeBound) (machine : List Bits) (real : List ℚ) : Prop :=
  machine.length = bounds.length ∧ real.length = bounds.length ∧
    ∀ (position : Nat) (bound : NodeBound), bounds[position]? = some bound →
      ∃ (m : Bits) (r : ℚ), machine[position]? = some m ∧
      real[position]? = some r ∧ Encloses bound m r

theorem abs_le_magnitudeOf {lo hi y : ℚ} (low : lo ≤ y) (high : y ≤ hi) : |y| ≤ magnitudeOf lo hi := by
  unfold magnitudeOf
  rw [abs_le]
  constructor
  · have := neg_abs_le lo
    have := le_max_left |lo| |hi|
    linarith
  · have := le_abs_self hi
    have := le_max_right |lo| |hi|
    linarith

/-- **The interval product encloses every product** of values in the two intervals. -/
theorem mul_mem_bounds {a b : NodeBound} {x y : ℚ} (xLow : a.lo ≤ x) (xHigh : x ≤ a.hi)
    (yLow : b.lo ≤ y) (yHigh : y ≤ b.hi) : productLow a b ≤ x * y ∧ x * y ≤ productHigh a b := by
  unfold productLow productHigh
  simp only [min_le_iff, le_max_iff]
  constructor
  · rcases le_total 0 y with yNonneg | yNonpos
    · have first : a.lo * y ≤ x * y := mul_le_mul_of_nonneg_right xLow yNonneg
      rcases le_total 0 a.lo with loNonneg | loNonpos
      · have second : a.lo * b.lo ≤ a.lo * y := mul_le_mul_of_nonneg_left yLow loNonneg
        exact Or.inl (Or.inl (by linarith))
      · have second : a.lo * b.hi ≤ a.lo * y := mul_le_mul_of_nonpos_left yHigh loNonpos
        exact Or.inl (Or.inr (by linarith))
    · have first : a.hi * y ≤ x * y := mul_le_mul_of_nonpos_right xHigh yNonpos
      rcases le_total 0 a.hi with hiNonneg | hiNonpos
      · have second : a.hi * b.lo ≤ a.hi * y := mul_le_mul_of_nonneg_left yLow hiNonneg
        exact Or.inr (Or.inl (by linarith))
      · have second : a.hi * b.hi ≤ a.hi * y := mul_le_mul_of_nonpos_left yHigh hiNonpos
        exact Or.inr (Or.inr (by linarith))
  · rcases le_total 0 y with yNonneg | yNonpos
    · have first : x * y ≤ a.hi * y := mul_le_mul_of_nonneg_right xHigh yNonneg
      rcases le_total 0 a.hi with hiNonneg | hiNonpos
      · have second : a.hi * y ≤ a.hi * b.hi := mul_le_mul_of_nonneg_left yHigh hiNonneg
        exact Or.inr (Or.inr (by linarith))
      · have second : a.hi * y ≤ a.hi * b.lo := mul_le_mul_of_nonpos_left yLow hiNonpos
        exact Or.inr (Or.inl (by linarith))
    · have first : x * y ≤ a.lo * y := mul_le_mul_of_nonpos_right xLow yNonpos
      rcases le_total 0 a.lo with loNonneg | loNonpos
      · have second : a.lo * y ≤ a.lo * b.hi := mul_le_mul_of_nonneg_left yHigh loNonneg
        exact Or.inl (Or.inr (by linarith))
      · have second : a.lo * y ≤ a.lo * b.lo := mul_le_mul_of_nonpos_left yLow loNonpos
        exact Or.inl (Or.inl (by linarith))

/-- **An accepted sum node encloses its value**: one mixed rounding at the sum's reach plus the
operands' errors. -/
theorem add_encloses {a b bound : NodeBound} {ma mb : Bits} {ra rb : ℚ}
    (enclosesA : Encloses a ma ra) (enclosesB : Encloses b mb rb) (low : bound.lo ≤ a.lo + b.lo)
    (high : a.hi + b.hi ≤ bound.hi) (small : sumReach a b < overflowLimit)
    (enough : sumError a b ≤ bound.error) : Encloses bound (add ma mb) (ra + rb) := by
  obtain ⟨finiteA, aLow, aHigh, errorA⟩ := enclosesA
  obtain ⟨finiteB, bLow, bHigh, errorB⟩ := enclosesB
  rw [abs_le] at errorA errorB
  have reach : |exactValue ma + exactValue mb| ≤ sumReach a b := by
    apply abs_le_magnitudeOf
    · linarith [errorA.1, errorB.1]
    · linarith [errorA.2, errorB.2]
  have finite := addFinite_of_small ma mb finiteA finiteB (lt_of_le_of_lt reach small)
  have rounding := add_mixed_error ma mb finite
  have scaled := mul_le_mul_of_nonneg_left reach unit_nonneg
  refine ⟨add_finite ma mb finite, by linarith, by linarith, ?_⟩
  rw [abs_le] at rounding ⊢
  unfold sumError at enough
  constructor
  · linarith [rounding.1, errorA.1, errorB.1]
  · linarith [rounding.2, errorA.2, errorB.2]

/-- **An accepted product node encloses its value**: one mixed rounding at the product's reach plus
the propagated operand errors `Ea · |b| + |a| · Eb`. -/
theorem mul_encloses {a b bound : NodeBound} {ma mb : Bits} {ra rb : ℚ}
    (enclosesA : Encloses a ma ra) (enclosesB : Encloses b mb rb) (low : bound.lo ≤ productLow a b)
    (high : productHigh a b ≤ bound.hi) (small : productReach a b < overflowLimit)
    (enough : productError a b ≤ bound.error) : Encloses bound (mul ma mb) (ra * rb) := by
  obtain ⟨finiteA, aLow, aHigh, errorA⟩ := enclosesA
  obtain ⟨finiteB, bLow, bHigh, errorB⟩ := enclosesB
  have boundA : |ra| ≤ magnitudeOf a.lo a.hi := abs_le_magnitudeOf aLow aHigh
  have boundB : |rb| ≤ magnitudeOf b.lo b.hi := abs_le_magnitudeOf bLow bHigh
  have errorANonneg : 0 ≤ a.error := le_trans (abs_nonneg _) errorA
  have machineA : |exactValue ma| ≤ magnitudeOf a.lo a.hi + a.error := by
    have := abs_sub_abs_le_abs_sub (exactValue ma) ra
    linarith
  have machineB : |exactValue mb| ≤ magnitudeOf b.lo b.hi + b.error := by
    have := abs_sub_abs_le_abs_sub (exactValue mb) rb
    linarith
  have reach : |exactValue ma * exactValue mb| ≤ productReach a b := by
    rw [abs_mul]
    exact mul_le_mul machineA machineB (abs_nonneg _) (le_trans (abs_nonneg _) machineA)
  have finite := mulFinite_of_small ma mb finiteA finiteB (lt_of_le_of_lt reach small)
  have rounding := mul_mixed_error ma mb finite
  have scaled := mul_le_mul_of_nonneg_left reach unit_nonneg
  have propagated : |exactValue ma * exactValue mb - ra * rb| ≤
      a.error * (magnitudeOf b.lo b.hi + b.error) + magnitudeOf a.lo a.hi * b.error := by
    have split : exactValue ma * exactValue mb - ra * rb =
        (exactValue ma - ra) * exactValue mb + ra * (exactValue mb - rb) := by ring
    rw [split]
    calc |(exactValue ma - ra) * exactValue mb + ra * (exactValue mb - rb)|
        ≤ |(exactValue ma - ra) * exactValue mb| + |ra * (exactValue mb - rb)| := abs_add_le _ _
      _ = |exactValue ma - ra| * |exactValue mb| + |ra| * |exactValue mb - rb| := by
          rw [abs_mul, abs_mul]
      _ ≤ a.error * (magnitudeOf b.lo b.hi + b.error) + magnitudeOf a.lo a.hi * b.error :=
          add_le_add (mul_le_mul errorA machineB (abs_nonneg _) errorANonneg)
            (mul_le_mul boundA errorB (abs_nonneg _) (le_trans (abs_nonneg _) boundA))
  obtain ⟨productLowLe, productHighGe⟩ := mul_mem_bounds aLow aHigh bLow bHigh
  refine ⟨mul_finite ma mb finite, le_trans low productLowLe, le_trans productHighGe high, ?_⟩
  have total := abs_sub_le (exactValue (mul ma mb)) (exactValue ma * exactValue mb) (ra * rb)
  unfold productError at enough
  linarith

/-- **One accepted node encloses its machine and real values**, given that every earlier node does
and the inputs meet their declared domains. -/
theorem checkNode_sound {domains : List InputDomain} {inputsMachine : List Bits} {inputsReal : List ℚ}
    (applies : Applies domains inputsMachine inputsReal) {done : List NodeBound} {ms : List Bits}
    {rs : List ℚ} (earlier : AllEnclose done ms rs) (bound : NodeBound) (node : Node)
    (ok : checkNode domains done bound node = .ok) :
    Encloses bound (machineNode inputsMachine ms node) (realNode inputsReal rs node) := by
  cases node with
  | input index =>
      cases found : domains[index]? with
      | none => simp [checkNode, found] at ok
      | some domain =>
          simp only [checkNode, found] at ok
          split at ok
          · rename_i checks
            obtain ⟨low, high, inputError⟩ := checks
            obtain ⟨x, r, machineIs, realIs, finiteX, rLow, rHigh, distance⟩ :=
              applies index domain found
            simp only [machineNode, realNode, machineIs, realIs, Option.getD_some]
            exact ⟨finiteX, by linarith, by linarith, by linarith⟩
          · simp at ok
  | const bits =>
      simp only [checkNode] at ok
      split at ok
      · rename_i finite
        split at ok
        · rename_i checks
          obtain ⟨low, high, nonneg⟩ := checks
          refine ⟨finite, low, high, ?_⟩
          simpa [machineNode, realNode] using nonneg
        · simp at ok
      · simp at ok
  | add left right =>
      cases foundLeft : done[left]? with
      | none => simp [checkNode, foundLeft] at ok
      | some a =>
          cases foundRight : done[right]? with
          | none => simp [checkNode, foundLeft, foundRight] at ok
          | some b =>
              simp only [checkNode, foundLeft, foundRight] at ok
              split at ok
              · rename_i checks
                obtain ⟨low, high, small, enough⟩ := checks
                obtain ⟨ma, ra, machineA, realA, enclosesA⟩ := earlier.2.2 left a foundLeft
                obtain ⟨mb, rb, machineB, realB, enclosesB⟩ := earlier.2.2 right b foundRight
                simp only [machineNode, realNode, machineA, machineB, realA, realB, Option.getD_some]
                exact add_encloses enclosesA enclosesB low high small enough
              · simp at ok
  | mul left right =>
      cases foundLeft : done[left]? with
      | none => simp [checkNode, foundLeft] at ok
      | some a =>
          cases foundRight : done[right]? with
          | none => simp [checkNode, foundLeft, foundRight] at ok
          | some b =>
              simp only [checkNode, foundLeft, foundRight] at ok
              split at ok
              · rename_i checks
                obtain ⟨low, high, small, enough⟩ := checks
                obtain ⟨ma, ra, machineA, realA, enclosesA⟩ := earlier.2.2 left a foundLeft
                obtain ⟨mb, rb, machineB, realB, enclosesB⟩ := earlier.2.2 right b foundRight
                simp only [machineNode, realNode, machineA, machineB, realA, realB, Option.getD_some]
                exact mul_encloses enclosesA enclosesB low high small enough
              · simp at ok
  | outside => simp [checkNode] at ok

/-- Appending an enclosed node keeps every node enclosed. -/
theorem allEnclose_append {done : List NodeBound} {ms : List Bits} {rs : List ℚ}
    (earlier : AllEnclose done ms rs) {bound : NodeBound} {m : Bits} {r : ℚ}
    (last : Encloses bound m r) : AllEnclose (done ++ [bound]) (ms ++ [m]) (rs ++ [r]) := by
  obtain ⟨machineLength, realLength, each⟩ := earlier
  refine ⟨by simp [machineLength], by simp [realLength], ?_⟩
  intro position found foundAt
  rcases Nat.lt_or_ge position done.length with before | after
  · rw [List.getElem?_append_left before] at foundAt
    obtain ⟨m', r', machineAt, realAt, encloses⟩ := each position found foundAt
    refine ⟨m', r', ?_, ?_, encloses⟩
    · rw [List.getElem?_append_left (by omega)]
      exact machineAt
    · rw [List.getElem?_append_left (by omega)]
      exact realAt
  · rw [List.getElem?_append_right after] at foundAt
    obtain ⟨offset, offsetIs⟩ : ∃ offset, position - done.length = offset := ⟨_, rfl⟩
    cases offset with
    | succ offset =>
        rw [offsetIs] at foundAt
        simp at foundAt
    | zero =>
        rw [offsetIs] at foundAt
        simp only [List.getElem?_cons_zero, Option.some.injEq] at foundAt
        subst foundAt
        have atMachine : position = ms.length := by omega
        have atReal : position = rs.length := by omega
        refine ⟨m, r, ?_, ?_, last⟩
        · rw [atMachine]
          simp
        · rw [atReal]
          simp

/-- **Every accepted run of the checker encloses every node it checked.** -/
theorem checkFrom_sound {domains : List InputDomain} {inputsMachine : List Bits}
    {inputsReal : List ℚ} (applies : Applies domains inputsMachine inputsReal) :
    ∀ (nodes : List Node) (done bounds : List NodeBound) (ms : List Bits) (rs : List ℚ),
      AllEnclose done ms rs → checkFrom domains done nodes bounds = .certified →
        AllEnclose (done ++ bounds) (machineFrom inputsMachine ms nodes)
          (realFrom inputsReal rs nodes) := by
  intro nodes
  induction nodes with
  | nil =>
      intro done bounds ms rs earlier certified
      cases bounds with
      | nil => simpa [machineFrom, realFrom] using earlier
      | cons _ _ => simp [checkFrom] at certified
  | cons node rest restSound =>
      intro done bounds ms rs earlier certified
      cases bounds with
      | nil => simp [checkFrom] at certified
      | cons bound more =>
          simp only [checkFrom] at certified
          split at certified
          · rename_i verdict
            have extended := allEnclose_append earlier
              (checkNode_sound applies earlier bound node verdict)
            have later := restSound (done ++ [bound]) more _ _ extended certified
            simpa [machineFrom, realFrom, List.append_assoc] using later
          · simp at certified
          · simp at certified

/-- An accepted run checks exactly one bound per node. -/
theorem checkFrom_length (domains : List InputDomain) :
    ∀ (nodes : List Node) (done bounds : List NodeBound),
      checkFrom domains done nodes bounds = .certified → bounds.length = nodes.length := by
  intro nodes
  induction nodes with
  | nil =>
      intro done bounds certified
      cases bounds with
      | nil => rfl
      | cons _ _ => simp [checkFrom] at certified
  | cons node rest restLength =>
      intro done bounds certified
      cases bounds with
      | nil => simp [checkFrom] at certified
      | cons bound more =>
          simp only [checkFrom] at certified
          split at certified
          · simpa using restLength (done ++ [bound]) more certified
          · simp at certified
          · simp at certified

/-- The machine values of a graph's nodes from its inputs. -/
def machineValues (inputs : List Bits) (graph : Graph) : List Bits := machineFrom inputs [] graph.nodes

/-- The real values of a graph's nodes from its real inputs. -/
def realValues (inputs : List ℚ) (graph : Graph) : List ℚ := realFrom inputs [] graph.nodes

/-- **Checker soundness**: an accepted certificate encloses every node of the graph. For every
input meeting the declared domains, every machine value is finite, every real value lies in its
proposed enclosure, and the two are within the proposed error bound. -/
theorem check_sound (budget : Budget) (graph : Graph) (certificate : Certificate)
    (certified : check budget graph certificate = .certified) (machine : List Bits) (real : List ℚ)
    (applies : Applies certificate.domains machine real) :
    AllEnclose certificate.bounds (machineValues machine graph) (realValues real graph) := by
  unfold check at certified
  split at certified
  · split at certified
    · simp at certified
    · have start : AllEnclose [] [] [] := ⟨rfl, rfl, fun _ _ found => by simp at found⟩
      simpa [machineValues, realValues] using
        checkFrom_sound applies graph.nodes [] certificate.bounds [] [] start certified
  · simp at certified

/-- **The per-output bound**: for an accepted certificate, every output has a proposed bound, a
finite machine value and a real value within that bound. -/
theorem check_output_bound (budget : Budget) (graph : Graph) (certificate : Certificate)
    (certified : check budget graph certificate = .certified) (machine : List Bits) (real : List ℚ)
    (applies : Applies certificate.domains machine real) (output : Nat)
    (isOutput : output ∈ graph.outputs) :
    ∃ bound m r, certificate.bounds[output]? = some bound ∧
      (machineValues machine graph)[output]? = some m ∧ (realValues real graph)[output]? = some r ∧
        Finite m ∧ |exactValue m - r| ≤ bound.error := by
  have sound := check_sound budget graph certificate certified machine real applies
  unfold check at certified
  split at certified
  · split at certified
    · simp at certified
    · rename_i none
      have inGraph : output < graph.nodes.length := by
        have := List.find?_eq_none.1 none output isOutput
        simpa using this
      have length := checkFrom_length certificate.domains graph.nodes [] certificate.bounds certified
      have inBounds : output < certificate.bounds.length := by omega
      obtain ⟨m, r, machineAt, realAt, finite, -, -, error⟩ :=
        sound.2.2 output (certificate.bounds[output]'inBounds) (List.getElem?_eq_getElem inBounds)
      exact ⟨_, m, r, List.getElem?_eq_getElem inBounds, machineAt, realAt, finite, error⟩
  · simp at certified

/-! ## Fixtures and controls

The cancellation graph is `x · x + (-1)` over the exact input `x ∈ [1, 2]`: near `x = 1` the result
cancels to near zero, where no relative bound holds, while the certificate's absolute bound does. -/

def minusOneBits : Bits := 0xBF800000

def cancellationGraph : Graph :=
  { nodes := [.input 0, .mul 0 0, .const minusOneBits, .add 1 2], outputs := [3] }

def squareError : ℚ := 4 * unit + eta

def cancellationCertificate : Certificate :=
  { domains := [{ lo := 1, hi := 2, inputError := 0 }],
    bounds := [{ lo := 1, hi := 2, error := 0 }, { lo := 1, hi := 4, error := squareError },
      { lo := -1, hi := -1, error := 0 },
      { lo := 0, hi := 3, error := squareError + unit * (3 + squareError) + eta }] }

def fixtureBudget : Budget := { nodes := 8, bits := 256 }

set_option maxRecDepth 100000 in
/-- **The cancellation certificate is accepted.** -/
theorem cancellation_certified : check fixtureBudget cancellationGraph cancellationCertificate = .certified := by
  decide +kernel

/-- The cancellation certificate applies to the exact input `1`, so its premises are satisfiable. -/
theorem cancellation_applies : Applies cancellationCertificate.domains [oneBits] [1] := by
  intro index domain found
  cases index with
  | zero =>
      simp only [cancellationCertificate, List.getElem?_cons_zero, Option.some.injEq] at found
      subst found
      refine ⟨oneBits, 1, rfl, rfl, ?_, by norm_num, by norm_num, ?_⟩
      · unfold Finite
        decide
      · have one : exactValue oneBits = 1 := by decide +kernel
        simp [one]
  | succ index => simp [cancellationCertificate] at found

set_option maxRecDepth 100000 in
/-- **An underestimated error is refused**: dropping the absolute term from the sum's bound. -/
theorem underestimated_error_is_refused :
    check fixtureBudget cancellationGraph
      { cancellationCertificate with
        bounds := [{ lo := 1, hi := 2, error := 0 }, { lo := 1, hi := 4, error := squareError },
          { lo := -1, hi := -1, error := 0 },
          { lo := 0, hi := 3, error := squareError + unit * (3 + squareError) }] } = .unknown 3 := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **An underestimated range is refused**: the square's enclosure stops at `3`. -/
theorem underestimated_range_is_refused :
    check fixtureBudget cancellationGraph
      { cancellationCertificate with
        bounds := [{ lo := 1, hi := 2, error := 0 }, { lo := 1, hi := 3, error := squareError },
          { lo := -1, hi := -1, error := 0 },
          { lo := 0, hi := 3, error := squareError + unit * (3 + squareError) + eta }] } =
      .unknown 1 := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **A missing input error is refused**: the domain declares an input error the input's bound
omits. -/
theorem missing_input_error_is_refused :
    check fixtureBudget cancellationGraph
      { cancellationCertificate with domains := [{ lo := 1, hi := 2, inputError := 1 / 2 ^ 25 }] } =
      .unknown 0 := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **An operation outside the fragment is unsupported.** -/
theorem outside_operation_is_unsupported :
    check fixtureBudget { cancellationGraph with nodes := [.input 0, .mul 0 0, .const minusOneBits, .outside] }
      cancellationCertificate = .unsupported 3 := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **An operand that is not an earlier node is unsupported.** -/
theorem later_operand_is_unsupported :
    check fixtureBudget { cancellationGraph with nodes := [.input 0, .mul 0 2, .const minusOneBits, .add 1 2] }
      cancellationCertificate = .unsupported 1 := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **A missing output is unsupported.** -/
theorem missing_output_is_unsupported :
    check fixtureBudget { cancellationGraph with outputs := [4] } cancellationCertificate =
      .unsupported 4 := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **Oversized certificates and graphs are exhausted, not refused**: `2^-150` does not fit in 64
bits, and four nodes do not fit a three-node budget. -/
theorem oversized_is_exhausted :
    check { nodes := 8, bits := 64 } cancellationGraph cancellationCertificate = .exhausted ∧
      check { nodes := 3, bits := 256 } cancellationGraph cancellationCertificate = .exhausted := by
  decide +kernel

/-! The near-overflow graph squares its input. Over `[1, 2^63]` the square reaches `2^126` and is
certified; over `[1, 2^64]` it reaches `2^128`, which overflows, and the certificate is refused. -/

def squareGraph : Graph := { nodes := [.input 0, .mul 0 0], outputs := [1] }

def squareCertificate (top : ℚ) : Certificate :=
  { domains := [{ lo := 1, hi := top, inputError := 0 }],
    bounds := [{ lo := 1, hi := top, error := 0 },
      { lo := 1, hi := top * top, error := unit * (top * top) + eta }] }

set_option maxRecDepth 100000 in
/-- **Near overflow is certified below the limit and refused above it.** -/
theorem near_overflow_square :
    check fixtureBudget squareGraph (squareCertificate (2 ^ 63)) = .certified ∧
      check fixtureBudget squareGraph (squareCertificate (2 ^ 64)) = .unknown 1 := by
  decide +kernel

/-! The subnormal graph multiplies `1.5` by the smallest subnormal `2^-149`. The exact product
`3 · 2^-150` rounds to `2^-148`, an error of `η`: the absolute term is load-bearing. -/

def subnormalGraph : Graph :=
  { nodes := [.const threeHalvesBits, .const smallestSubnormalBits, .mul 0 1], outputs := [2] }

def subnormalCertificate (absolute : ℚ) : Certificate :=
  { domains := [],
    bounds := [{ lo := 3 / 2, hi := 3 / 2, error := 0 },
      { lo := 1 / 2 ^ 149, hi := 1 / 2 ^ 149, error := 0 },
      { lo := 3 / 2 ^ 150, hi := 3 / 2 ^ 150, error := unit * (3 / 2 ^ 150) + absolute }] }

set_option maxRecDepth 100000 in
/-- **The absolute term is load-bearing**: the certificate with `η` is accepted, the one without it
is refused, and the actual rounding error `η` exceeds the bound without it. -/
theorem subnormal_needs_eta :
    check fixtureBudget subnormalGraph (subnormalCertificate eta) = .certified ∧
      check fixtureBudget subnormalGraph (subnormalCertificate 0) = .unknown 2 ∧
      |exactValue (mul threeHalvesBits smallestSubnormalBits) - 3 / 2 ^ 150| = eta ∧
      unit * (3 / 2 ^ 150) < eta := by
  decide +kernel

end NobleFoundation.Binary32.Checker
