import Mathlib.Data.ZMod.Basic
import MathlibI64
import NobleContracts.Model
import Mathlib.Tactic.Ring

/-!
# Signed I64 representation bridge (issue #15, Phase 1)

Three independent descriptions of a Noble `I64` meet here:

* `MathlibI64.signed`/`MathlibI64.wrap` — the Mathlib `ZMod (2^64)` oracle that
  `verification/mathlib-i64/compare.mjs` compiles and evaluates (imported, not
  copied, from `proofs/m3/MathlibI64.lean`);
* `BitVec 64` with `BitVec.toInt` — the carrier of `NobleContracts.Value.i64`
  in the MC1 model (`proofs/mc1/NobleContracts/Model.lean`);
* the K-NUM-01 formula `((z + 2^63) mod 2^64) - 2^63` on mathematical integers.

Everything in this file is a statement about these *models*.  None of it says
anything about the Rust frontend, the Wasm lowering, the runtime helpers or
the engine; see `proofs/foundation/README.md` for the boundary table.
-/

namespace NobleFoundation

/-- The I64 modulus `2^64`. -/
abbrev modulus : ℕ := 2 ^ 64

/-- Residues modulo `2^64`, the oracle's carrier. -/
abbrev Z64 := ZMod modulus

/-- The accepted signed I64 range `[-2^63, 2^63 - 1]`.  This is the set of
mathematical integers an I64 value denotes, and (see `Literal.lean`) the set of
source literals the frontend is specified to accept. -/
def I64Range (n : ℤ) : Prop := -2 ^ 63 ≤ n ∧ n < 2 ^ 63

instance (n : ℤ) : Decidable (I64Range n) := by unfold I64Range; infer_instance

/-- The representation map from the MC1 carrier to the oracle carrier. -/
def toZ (x : BitVec 64) : Z64 := (x.toNat : Z64)

/-- The inverse representation map. -/
def ofZ (r : Z64) : BitVec 64 := BitVec.ofNat 64 r.val

/-! ## `ZMod (2^64)` ≃ `BitVec 64` -/

theorem toZ_ofZ (r : Z64) : toZ (ofZ r) = r := by
  unfold toZ ofZ
  rw [BitVec.toNat_ofNat, Nat.mod_eq_of_lt (ZMod.val_lt r)]
  exact ZMod.natCast_zmod_val r

theorem ofZ_toZ (x : BitVec 64) : ofZ (toZ x) = x := by
  unfold toZ ofZ
  apply BitVec.eq_of_toNat_eq
  rw [BitVec.toNat_ofNat, ZMod.val_natCast, Nat.mod_mod, Nat.mod_eq_of_lt x.isLt]

theorem toZ_injective : Function.Injective toZ := fun a b h => by
  rw [← ofZ_toZ a, ← ofZ_toZ b, h]

theorem ofZ_injective : Function.Injective ofZ := fun a b h => by
  rw [← toZ_ofZ a, ← toZ_ofZ b, h]

/-- `BitVec.ofInt` is exactly integer reduction into the oracle carrier. -/
theorem toZ_ofInt (n : ℤ) : toZ (BitVec.ofInt 64 n) = (n : Z64) := by
  unfold toZ
  rw [BitVec.toNat_ofInt]
  have hnonneg : 0 ≤ n % ((2 ^ 64 : ℕ) : ℤ) := Int.emod_nonneg _ (by decide)
  have hcast : (((n % ((2 ^ 64 : ℕ) : ℤ)).toNat : ℕ) : ℤ) = n % ((2 ^ 64 : ℕ) : ℤ) :=
    Int.toNat_of_nonneg hnonneg
  rw [← Int.cast_natCast, hcast]
  exact (ZMod.intCast_mod n modulus)

theorem ofZ_intCast (n : ℤ) : ofZ (n : Z64) = BitVec.ofInt 64 n := by
  rw [← toZ_ofInt, ofZ_toZ]

theorem toZ_add (a b : BitVec 64) : toZ (a + b) = toZ a + toZ b := by
  unfold toZ
  rw [BitVec.toNat_add, ZMod.natCast_mod, Nat.cast_add]

theorem toZ_mul (a b : BitVec 64) : toZ (a * b) = toZ a * toZ b := by
  unfold toZ
  rw [BitVec.toNat_mul, ZMod.natCast_mod, Nat.cast_mul]

theorem toZ_sub (a b : BitVec 64) : toZ (a - b) = toZ a - toZ b := by
  unfold toZ
  rw [BitVec.toNat_sub, ZMod.natCast_mod, Nat.cast_add,
    Nat.cast_sub (Nat.le_of_lt b.isLt), ZMod.natCast_self]
  ring

theorem toZ_neg (a : BitVec 64) : toZ (-a) = -toZ a := by
  rw [show -a = 0 - a by simp, toZ_sub]
  simp [toZ]

/-! ## Signed decoding agrees across the three descriptions -/

/-- The oracle's decoding is `Int.bmod`, the balanced residue used by core
`BitVec.toInt`. -/
theorem wrap_eq_bmod (n : ℤ) : MathlibI64.wrap n = n.bmod (2 ^ 64) := by
  unfold MathlibI64.wrap MathlibI64.signed
  have hv : (((n : ZMod (2 ^ 64)).val : ℕ) : ℤ) = n % ((2 ^ 64 : ℕ) : ℤ) :=
    ZMod.val_intCast n
  simp only [Int.bmod]
  have hlt : (n : ZMod (2 ^ 64)).val < 2 ^ 63 ↔
      n % ((2 ^ 64 : ℕ) : ℤ) < (((2 ^ 64 : ℕ) : ℤ) + 1) / 2 := by
    rw [← hv]; constructor <;> intro h <;> omega
  change (if (n : ZMod (2 ^ 64)).val < 2 ^ 63 then (((n : ZMod (2 ^ 64)).val : ℕ) : ℤ)
      else (((n : ZMod (2 ^ 64)).val : ℕ) : ℤ) - ((2 ^ 64 : ℕ) : ℤ)) = _
  rw [hv]
  split_ifs with h1 h2 h2 <;> first | rfl | (exfalso; have := hlt.mp h1; omega) | (exfalso; have := hlt.mpr h2; omega)

/-- The oracle's `wrap` is exactly the MC1 literal helper followed by
`BitVec.toInt`. -/
theorem wrap_eq_toInt_ofInt (n : ℤ) : MathlibI64.wrap n = (BitVec.ofInt 64 n).toInt := by
  rw [wrap_eq_bmod, BitVec.toInt_ofInt]

/-- K-NUM-01's formula `((z + 2^63) mod 2^64) - 2^63` is the oracle's `wrap`. -/
theorem wrap_eq_knum01 (z : ℤ) :
    MathlibI64.wrap z = (z + 2 ^ 63) % 2 ^ 64 - 2 ^ 63 := by
  rw [wrap_eq_bmod]
  simp only [Int.bmod]
  push_cast
  split_ifs <;> omega

/-- Signed decoding of a residue agrees with `BitVec.toInt` of its bit vector. -/
theorem signed_eq_toInt (r : Z64) : MathlibI64.signed r = (ofZ r).toInt := by
  have hv := r.val_lt
  change (if r.val < 2 ^ 63 then ((r.val : ℕ) : ℤ) else ((r.val : ℕ) : ℤ) - ((2 ^ 64 : ℕ) : ℤ)) = _
  rw [BitVec.toInt_eq_toNat_cond]
  unfold ofZ
  rw [BitVec.toNat_ofNat, Nat.mod_eq_of_lt hv]
  split_ifs <;> first | rfl | omega

/-- MC1 `BitVec.toInt` decoding agrees with the oracle's signed residue. -/
theorem signed_toZ (x : BitVec 64) : MathlibI64.signed (toZ x) = x.toInt := by
  rw [signed_eq_toInt, ofZ_toZ]

/-- Every decoded I64 lies in the signed range. -/
theorem toInt_range (x : BitVec 64) : I64Range x.toInt :=
  ⟨by have := BitVec.le_toInt x; simpa using this,
   by have := @BitVec.toInt_lt 64 x; simpa using this⟩

theorem signed_range (r : Z64) : I64Range (MathlibI64.signed r) := by
  rw [signed_eq_toInt]; exact toInt_range _

/-- Integer-cast of the decoded value recovers the representation. -/
theorem intCast_toInt (x : BitVec 64) : ((x.toInt : ℤ) : Z64) = toZ x := by
  conv_rhs => rw [← BitVec.ofInt_toInt (x := x)]
  rw [toZ_ofInt]

/-- Round trip residue → signed integer → residue. -/
theorem intCast_signed (r : Z64) : ((MathlibI64.signed r : ℤ) : Z64) = r := by
  rw [signed_eq_toInt, intCast_toInt, toZ_ofZ]

/-- Round trip integer → bit vector → integer is the identity exactly on the
signed range. -/
theorem toInt_ofInt_of_range {n : ℤ} (h : I64Range n) : (BitVec.ofInt 64 n).toInt = n := by
  rw [BitVec.toInt_ofInt]
  exact Int.bmod_eq_of_le (by have := h.1; push_cast; omega) (by have := h.2; push_cast; omega)

theorem toInt_ofInt_eq_iff (n : ℤ) : (BitVec.ofInt 64 n).toInt = n ↔ I64Range n := by
  constructor
  · intro h; rw [← h]; exact toInt_range _
  · exact toInt_ofInt_of_range

/-- Round trip integer → residue → signed integer, also exactly on the range. -/
theorem signed_intCast_eq_iff (n : ℤ) : MathlibI64.signed (n : Z64) = n ↔ I64Range n := by
  have : MathlibI64.signed (n : Z64) = MathlibI64.wrap n := rfl
  rw [this, wrap_eq_toInt_ofInt, toInt_ofInt_eq_iff]

/-- Round trip bit vector → integer → bit vector (core `BitVec.ofInt_toInt`). -/
theorem ofInt_toInt (x : BitVec 64) : BitVec.ofInt 64 x.toInt = x := BitVec.ofInt_toInt

theorem ofInt_wrap (n : ℤ) : BitVec.ofInt 64 (MathlibI64.wrap n) = BitVec.ofInt 64 n := by
  rw [wrap_eq_toInt_ofInt, BitVec.ofInt_toInt]

theorem ofInt_sub (x y : ℤ) :
    BitVec.ofInt 64 (x - y) = BitVec.ofInt 64 x - BitVec.ofInt 64 y := by
  apply toZ_injective
  rw [toZ_sub, toZ_ofInt, toZ_ofInt, toZ_ofInt, Int.cast_sub]

/-! ## MC1 arithmetic decodes to the oracle's wrapped integer arithmetic -/

theorem toInt_add (a b : BitVec 64) : (a + b).toInt = MathlibI64.wrap (a.toInt + b.toInt) := by
  rw [wrap_eq_toInt_ofInt, BitVec.ofInt_add, BitVec.ofInt_toInt, BitVec.ofInt_toInt]

theorem toInt_sub (a b : BitVec 64) : (a - b).toInt = MathlibI64.wrap (a.toInt - b.toInt) := by
  rw [wrap_eq_toInt_ofInt, ofInt_sub, BitVec.ofInt_toInt, BitVec.ofInt_toInt]

theorem toInt_mul (a b : BitVec 64) : (a * b).toInt = MathlibI64.wrap (a.toInt * b.toInt) := by
  rw [wrap_eq_toInt_ofInt, BitVec.ofInt_mul, BitVec.ofInt_toInt, BitVec.ofInt_toInt]

/-- The oracle's residue-level statements (`MathlibI64.add_residue` …) and the
MC1 operations are the same ring operation transported along `toZ`. -/
theorem oracle_add (x y : ℤ) :
    MathlibI64.wrap (x + y) = (BitVec.ofInt 64 x + BitVec.ofInt 64 y).toInt := by
  rw [wrap_eq_toInt_ofInt, BitVec.ofInt_add]

theorem oracle_sub (x y : ℤ) :
    MathlibI64.wrap (x - y) = (BitVec.ofInt 64 x - BitVec.ofInt 64 y).toInt := by
  rw [wrap_eq_toInt_ofInt, ofInt_sub]

theorem oracle_mul (x y : ℤ) :
    MathlibI64.wrap (x * y) = (BitVec.ofInt 64 x * BitVec.ofInt 64 y).toInt := by
  rw [wrap_eq_toInt_ofInt, BitVec.ofInt_mul]

end NobleFoundation
