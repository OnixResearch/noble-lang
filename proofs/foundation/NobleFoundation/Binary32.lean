/-
A kernel-reducible binary32 model for numeric graphs (issue #38).

The model is deliberately small and explicit: raw 32-bit encodings, an exact
scaled-integer interpretation of the finite encodings, round-to-nearest /
ties-to-even rounding with gradual underflow, and the three arithmetic
operations that Numeric-Graph-v1 selects (add, subtract, multiply). It is a
*model*: it says nothing about the Rust evaluator, `wasm-opt`, or any host FPU.

Encoding layout: `s eeeeeeee mmmmmmmmmmmmmmmmmmmmmmm`, sign, 8-bit biased
exponent (bias 127) and 23-bit fraction.

- exponent 255 and fraction 0 is an infinity;
- exponent 255 with any other fraction is a NaN, and every NaN is one
  observation class (`obsEq`);
- exponent 0 with fraction 0 is a zero, and its sign is retained;
- exponent 0 with a nonzero fraction is subnormal;
- any other encoding is normal.

Every finite encoding has an exact value `mantissa * 2^exponent` with an
integer mantissa, so no rational arithmetic is needed to define the reference.
All definitions are total and structural, so the kernel can check the retained
expectations by `decide`.
-/
namespace NobleFoundation.Binary32

set_option exponentiation.threshold 4096

abbrev Bits := BitVec 32

/-- Observation classes of one encoding. Signed zeros stay distinct. -/
inductive Class where
  | negZero
  | posZero
  | subnormal
  | normal
  | infinity
  | notANumber
deriving DecidableEq, Repr

def exponent (b : Bits) : Nat := (b >>> 23).toNat &&& 0xFF

def fraction (b : Bits) : Nat := b.toNat &&& 0x7FFFFF

def isNegative (b : Bits) : Bool := b.toNat >>> 31 == 1

def classify (b : Bits) : Class :=
  match exponent b, fraction b, isNegative b with
  | 255, 0, _ => .infinity
  | 255, _, _ => .notANumber
  | 0, 0, true => .negZero
  | 0, 0, false => .posZero
  | 0, _, _ => .subnormal
  | _, _, _ => .normal

/-- `obsEq` is the observation equality of the graph: every NaN encoding is the
same observation and every other class keeps its exact bytes. -/
def obsEq (x y : Bits) : Bool :=
  if classify x == .notANumber then classify y == .notANumber
  else if classify y == .notANumber then false
  else x == y

/-- Canonical quiet NaN used by every arithmetic result. -/
def canonicalNaN : Bits := 0x7FC00000

def infinityOf (isNeg : Bool) : Bits := if isNeg then 0xFF800000 else 0x7F800000

def zeroOf (isNeg : Bool) : Bits := if isNeg then 0x80000000 else 0x00000000

/-- `2^k` as a natural number. -/
def pow2 (k : Nat) : Nat := 2 ^ k

/-- Fuel-bounded floor binary logarithm. The fuel `n + 1` always suffices and
the recursion is structural on the fuel, so the kernel can reduce it. -/
def log2 (n : Nat) : Nat :=
  let rec go (n : Nat) (acc : Nat) (fuel : Nat) : Nat :=
    match fuel with
    | 0 => acc
    | fuel + 1 => if n < 2 then acc else go (n / 2) (acc + 1) fuel
  go n 0 (n + 1)

/-- The exact value of a finite encoding as `(mantissa, exponent)` with
`value = mantissa * 2^exponent`. The sign is reported separately. -/
def scaledMagnitude (b : Bits) : Nat × Int :=
  let e := exponent b
  let f := fraction b
  if e == 0 then (f, -149) else (2 ^ 23 + f, (e : Int) - 127 - 23)

/-- The signed exact value of a finite encoding as a scaled integer pair. -/
def scaledValue (b : Bits) : Int × Int :=
  let (m, s) := scaledMagnitude b
  ((if isNegative b then -(m : Int) else (m : Int)), s)

/-- The floor of the binary logarithm of `a / b` for positive `a` and `b`,
returned as an integer. For `b ≤ a` it is `log2 (a / b)`; otherwise the
quotient `b / a` has floor logarithm `m` with `a * 2^m ≤ b < a * 2^(m+1)`, so
the answer is `-(m + 1)`. -/
def floorLog2Ratio (a b : Nat) : Int :=
  if b ≤ a then ((log2 (a / b) : Nat) : Int) else -(((log2 (b / a) : Nat) : Int) + 1)

/-- `2^k` for a nonnegative `Int` exponent, as a natural number. -/
def pow2Int (k : Int) : Nat := pow2 k.toNat

/-- Round the magnitude `a / b` of a nonzero rational to nearest, ties to even,
and return the magnitude encoding of the result with a positive sign. -/
def roundPositive (a b : Nat) : Nat :=
  let e := floorLog2Ratio a b
  let scaled : Nat × Nat :=
    if (0 : Int) ≤ 23 - e then (a * pow2Int (23 - e), b)
    else (a, b * pow2Int (e - 23))
  let q := scaled.1 / scaled.2
  let r := scaled.1 % scaled.2
  let rounded :=
    if 2 * r > scaled.2 then q + 1
    else if 2 * r = scaled.2 then q + q % 2
    else q
  let carried := if rounded = pow2 24 then (pow2 23, e + 1) else (rounded, e)
  let m := carried.1
  let e' := carried.2
  if e' > 127 then
    255 * pow2 23
  else if e' < -126 then
    let n := (a * pow2 149) / b
    let r' := (a * pow2 149) % b
    let sub :=
      if 2 * r' > b then n + 1
      else if 2 * r' = b then n + n % 2
      else n
    if sub ≥ pow2 23 then pow2 23 else sub
  else
    ((e' + 127).toNat) * pow2 23 + (m - pow2 23)

/-- Round the signed scaled integer `m * 2^s` (with `m ≠ 0`) to binary32,
given the sign the IEEE rules assign to the result. -/
def roundScaled (m : Int) (s : Int) (isNeg : Bool) : Bits :=
  let a := m.natAbs
  let b := 1
  let numerator := if (0 : Int) ≤ s then a * pow2Int s else a
  let denominator := if (0 : Int) ≤ s then b else b * pow2Int (-s)
  let magnitude := roundPositive numerator denominator
  BitVec.ofNat 32 (if isNeg then magnitude ||| 0x80000000 else magnitude)

/-- Addition with IEEE 754 binary32 rounding: NaN propagates as the canonical
NaN, infinities of opposite sign give NaN, and a zero result keeps a negative
sign only when both operands are negative zeros. Otherwise the exact sum is
formed in scaled integers and rounded exactly once. -/
def add (x y : Bits) : Bits :=
  match classify x, classify y with
  | .notANumber, _ => canonicalNaN
  | _, .notANumber => canonicalNaN
  | .infinity, .infinity =>
      if isNegative x == isNegative y then infinityOf (isNegative x) else canonicalNaN
  | .infinity, _ => infinityOf (isNegative x)
  | _, .infinity => infinityOf (isNegative y)
  | _, _ =>
      let (mx, sx) := scaledValue x
      let (my, sy) := scaledValue y
      let s := if sx ≤ sy then sx else sy
      let ax := mx * (pow2Int (sx - s) : Int)
      let ay := my * (pow2Int (sy - s) : Int)
      let sum := ax + ay
      if sum = 0 then zeroOf (isNegative x && isNegative y)
      else roundScaled sum s (sum < 0)

/-- Subtraction is addition of the sign-flipped right operand. -/
def sub (x y : Bits) : Bits := add x (y ^^^ 0x80000000)

/-- Multiplication with IEEE 754 binary32 rounding: the result sign is the
exclusive-or of the operand signs even for zero results, and zero times
infinity is NaN. Otherwise the exact product is formed in scaled integers and
rounded exactly once. -/
def mul (x y : Bits) : Bits :=
  match classify x, classify y with
  | .notANumber, _ => canonicalNaN
  | _, .notANumber => canonicalNaN
  | .infinity, .infinity => infinityOf (isNegative x != isNegative y)
  | .infinity, .posZero => canonicalNaN
  | .infinity, .negZero => canonicalNaN
  | .posZero, .infinity => canonicalNaN
  | .negZero, .infinity => canonicalNaN
  | .infinity, _ => infinityOf (isNegative x != isNegative y)
  | _, .infinity => infinityOf (isNegative x != isNegative y)
  | .posZero, _ | .negZero, _ | _, .posZero | _, .negZero =>
      zeroOf (isNegative x != isNegative y)
  | _, _ =>
      let (mx, sx) := scaledValue x
      let (my, sy) := scaledValue y
      let product := mx * my
      if product = 0 then zeroOf (isNegative x != isNegative y)
      else roundScaled product (sx + sy) (isNegative x != isNegative y)

/-! ### Ordered affine fold (the `sem.rs` layer, classified in the #38 change)

`affine` in `crates/noble-ngraph/src/sem.rs` is the documented *ordered* fold
`v₀ = bias; vᵢ = rn(vᵢ₋₁ + rn(cᵢ × xᵢ))`, i.e. one multiply rounding and one
add rounding per term, in stored term order. `fold` is that recursion over the
same reference operations, so the fold's order is a model statement rather than
a comment: every term rounds its product before the accumulator sees it, and the
accumulator is never re-associated.
-/

/-- The ordered affine fold over a finite term list, one multiply rounding and one add
rounding per term. -/
def fold (accumulator : Bits) : List (Bits × Bits) → Bits
  | [] => accumulator
  | (coefficient, input) :: terms => fold (add accumulator (mul coefficient input)) terms

/-- An empty term list returns the bias unchanged, so the fold is the documented recursion
rather than an identity shortcut. -/
theorem fold_empty (bias : Bits) : fold bias [] = bias := rfl

/-- Splitting the terms splits the fold, so partial progress is the fold over the prefix. -/
theorem fold_append (bias : Bits) (first second : List (Bits × Bits)) :
    fold bias (first ++ second) = fold (fold bias first) second := by
  induction first generalizing bias with
  | nil => rfl
  | cons term rest ih =>
      simp only [List.cons_append, fold]
      exact ih (add bias (mul term.1 term.2))

/-- `1.0` as an encoding. -/
def oneBits : Bits := 0x3F800000

/-- `-1.0` as an encoding. -/
def negOneBits : Bits := 0xBF800000

/-- `2^24`, whose unit in the last place is `2`, so `1.0` is not representable beside it. -/
def twoPow24Bits : Bits := 0x4B800000

/-- The signed zero of the bias is retained: a negative-zero bias with a negative-zero
product stays negative. -/
theorem fold_negative_zero_bias_stays_negative :
    fold (zeroOf true) [(oneBits, zeroOf true)] = zeroOf true := by
  decide

/-- A positive-zero bias with the same product stays positive, so the two fixtures separate the
signs rather than sharing one expectation. -/
theorem fold_positive_zero_bias_stays_positive :
    fold (zeroOf false) [(oneBits, zeroOf true)] = zeroOf false := by
  decide

/-- **The fold's order is load-bearing**: at `bias = 1.0` the terms `1.0 · 2^24` then
`-1.0 · 2^24` fold to zero, because the `1.0` is rounded away before the cancellation, while
re-associating the same three values gives `1.0`. Reassociation therefore changes this documented
result, which is exactly what the `sem.rs` contract forbids. This theorem does not exercise FMA
contraction; `Expectations.implicit_contraction_is_observable` and
`Mutations.implicit_contraction_is_caught` cover it on a separate three-operand case. -/
theorem fold_order_is_load_bearing :
    fold oneBits [(oneBits, twoPow24Bits), (negOneBits, twoPow24Bits)] = zeroOf false
      ∧ add oneBits (add (mul oneBits twoPow24Bits) (mul negOneBits twoPow24Bits))
          = oneBits := by
  constructor <;> decide

end NobleFoundation.Binary32
