import Std

/-!
# Concrete `[0, 8192)` Linear-Memory Model for Bounded Wasm Runtime Helpers

Mathlib-free byte-level linear-memory model mirroring `crates/noble-wasm/runtime/`:
- `[0, 4096)` (`stack.wat:1-45`): operand slots of 16 bytes (`kind : i32` at `+0..+4`,
  4-byte padding at `+4..+8` left untouched by `$push_tag`, `payload : i64` at `+8..+16`).
- `[4096, 8192)` (`stack.wat:61-88`): continuation slots of 8 bytes (`index : i32` at
  `+0..+4`, `environment : i32` at `+4..+8`).
- `[8192, ...)` : disjoint higher linear-memory regions (traversal stacks, reflection
  buffers, heap cells, literal text segments).

All concrete helpers (`$fail`, `$quota`, `$need`, `$slot_kind`, `$slot_value`, `$push_tag`,
`$push_i64`, `$take`, `$pop_kind`, `$enqueue`, `$dequeue`, `$runtime_tick`) preserve the
exact guard order, 32-bit unsigned wrapping arithmetic, dead-slot zeroing, and untouched
padding of `stack.wat` and `accounting.wat`.
-/

namespace NobleContracts.WasmLinearMemory

/-! ## 1. Byte-Addressed Memory, Frame Equivalence, and Little-Endian Load/Store -/

def opRegionEnd : Nat := 4096
def contRegionStart : Nat := 4096
def contRegionEnd : Nat := 8192

/-- Total byte-addressed WebAssembly linear memory (`addr ↦ byte`). -/
abbrev LinearMem := Nat → BitVec 8

/-- All-zero initial linear memory. -/
def zeroMem : LinearMem := fun _ => 0

/-- Agreement of two linear memories on the half-open byte range `[lo, hi)`. -/
def MemEqOn (lo hi : Nat) (m1 m2 : LinearMem) : Prop :=
  ∀ a, lo ≤ a → a < hi → m1 a = m2 a

theorem MemEqOn.refl (lo hi : Nat) (m : LinearMem) : MemEqOn lo hi m m :=
  fun _ _ _ => rfl

theorem MemEqOn.symm {lo hi : Nat} {m1 m2 : LinearMem} (h : MemEqOn lo hi m1 m2) :
    MemEqOn lo hi m2 m1 :=
  fun a hlo hhi => (h a hlo hhi).symm

theorem MemEqOn.trans {lo hi : Nat} {m1 m2 m3 : LinearMem}
    (h12 : MemEqOn lo hi m1 m2) (h23 : MemEqOn lo hi m2 m3) :
    MemEqOn lo hi m1 m3 :=
  fun a hlo hhi => (h12 a hlo hhi).trans (h23 a hlo hhi)

theorem MemEqOn.mono {lo' hi' lo hi : Nat} {m1 m2 : LinearMem}
    (h : MemEqOn lo hi m1 m2) (hlo : lo ≤ lo') (hhi : hi' ≤ hi) :
    MemEqOn lo' hi' m1 m2 :=
  fun a ha1 ha2 => h a (Nat.le_trans hlo ha1) (Nat.lt_of_lt_of_le ha2 hhi)

/-- Functional single-byte memory update. -/
def writeByte (m : LinearMem) (addr : Nat) (b : BitVec 8) : LinearMem :=
  fun a => if a = addr then b else m a

@[simp] theorem writeByte_same (m : LinearMem) (addr : Nat) (b : BitVec 8) :
    writeByte m addr b addr = b := by
  simp [writeByte]

@[simp] theorem writeByte_ne {m : LinearMem} {addr a : Nat} (b : BitVec 8) (hne : a ≠ addr) :
    writeByte m addr b a = m a := by
  simp [writeByte, hne]

theorem writeByte_preserves_outside {m : LinearMem} {addr lo hi : Nat} (b : BitVec 8)
    (h : addr < lo ∨ hi ≤ addr) : MemEqOn lo hi m (writeByte m addr b) := by
  intro a hlo hhi
  have hne : a ≠ addr := by omega
  simp [writeByte, hne]

/-- Byte `i` (little-endian) of a 32-bit word. -/
def getByteLE32 (v : BitVec 32) (i : Nat) : BitVec 8 :=
  BitVec.ofNat 8 (v.toNat >>> (8 * i))

/-- Byte `i` (little-endian) of a 64-bit word. -/
def getByteLE64 (v : BitVec 64) (i : Nat) : BitVec 8 :=
  BitVec.ofNat 8 (v.toNat >>> (8 * i))

/-- Little-endian `i32.store` at byte address `addr`. -/
def storeI32 (m : LinearMem) (addr : Nat) (v : BitVec 32) : LinearMem :=
  let m0 := writeByte m addr (getByteLE32 v 0)
  let m1 := writeByte m0 (addr + 1) (getByteLE32 v 1)
  let m2 := writeByte m1 (addr + 2) (getByteLE32 v 2)
  writeByte m2 (addr + 3) (getByteLE32 v 3)

/-- Little-endian `i32.load` at byte address `addr`. -/
def loadI32 (m : LinearMem) (addr : Nat) : BitVec 32 :=
  BitVec.ofNat 32 (
    (m addr).toNat +
    (m (addr + 1)).toNat * 2 ^ 8 +
    (m (addr + 2)).toNat * 2 ^ 16 +
    (m (addr + 3)).toNat * 2 ^ 24)

/-- Little-endian `i64.store` at byte address `addr`. -/
def storeI64 (m : LinearMem) (addr : Nat) (v : BitVec 64) : LinearMem :=
  let m0 := writeByte m addr (getByteLE64 v 0)
  let m1 := writeByte m0 (addr + 1) (getByteLE64 v 1)
  let m2 := writeByte m1 (addr + 2) (getByteLE64 v 2)
  let m3 := writeByte m2 (addr + 3) (getByteLE64 v 3)
  let m4 := writeByte m3 (addr + 4) (getByteLE64 v 4)
  let m5 := writeByte m4 (addr + 5) (getByteLE64 v 5)
  let m6 := writeByte m5 (addr + 6) (getByteLE64 v 6)
  writeByte m6 (addr + 7) (getByteLE64 v 7)

/-- Little-endian `i64.load` at byte address `addr`. -/
def loadI64 (m : LinearMem) (addr : Nat) : BitVec 64 :=
  BitVec.ofNat 64 (
    (m addr).toNat +
    (m (addr + 1)).toNat * 2 ^ 8 +
    (m (addr + 2)).toNat * 2 ^ 16 +
    (m (addr + 3)).toNat * 2 ^ 24 +
    (m (addr + 4)).toNat * 2 ^ 32 +
    (m (addr + 5)).toNat * 2 ^ 40 +
    (m (addr + 6)).toNat * 2 ^ 48 +
    (m (addr + 7)).toNat * 2 ^ 56)

/-- `i32.load` after `i32.store` at the same address recovers the stored 32-bit word. -/
theorem loadI32_storeI32 (m : LinearMem) (addr : Nat) (v : BitVec 32) :
    loadI32 (storeI32 m addr v) addr = v := by
  apply BitVec.eq_of_toNat_eq
  have hv : v.toNat < 2 ^ 32 := v.isLt
  simp [loadI32, storeI32, writeByte, getByteLE32, BitVec.toNat_ofNat, Nat.shiftRight_eq_div_pow]
  omega

/-- `i64.load` after `i64.store` at the same address recovers the stored 64-bit word. -/
theorem loadI64_storeI64 (m : LinearMem) (addr : Nat) (v : BitVec 64) :
    loadI64 (storeI64 m addr v) addr = v := by
  apply BitVec.eq_of_toNat_eq
  have hv : v.toNat < 2 ^ 64 := v.isLt
  simp [loadI64, storeI64, writeByte, getByteLE64, BitVec.toNat_ofNat, Nat.shiftRight_eq_div_pow]
  omega

/-- `storeI32` at `addr` preserves every byte outside `[addr, addr + 4)`. -/
theorem storeI32_preserves_outside {m : LinearMem} {addr lo hi : Nat} (v : BitVec 32)
    (h : addr + 4 ≤ lo ∨ hi ≤ addr) : MemEqOn lo hi m (storeI32 m addr v) := by
  intro a hlo hhi
  have h0 : a ≠ addr := by omega
  have h1 : a ≠ addr + 1 := by omega
  have h2 : a ≠ addr + 2 := by omega
  have h3 : a ≠ addr + 3 := by omega
  simp [storeI32, writeByte, h0, h1, h2, h3]

/-- `storeI64` at `addr` preserves every byte outside `[addr, addr + 8)`. -/
theorem storeI64_preserves_outside {m : LinearMem} {addr lo hi : Nat} (v : BitVec 64)
    (h : addr + 8 ≤ lo ∨ hi ≤ addr) : MemEqOn lo hi m (storeI64 m addr v) := by
  intro a hlo hhi
  have h0 : a ≠ addr := by omega
  have h1 : a ≠ addr + 1 := by omega
  have h2 : a ≠ addr + 2 := by omega
  have h3 : a ≠ addr + 3 := by omega
  have h4 : a ≠ addr + 4 := by omega
  have h5 : a ≠ addr + 5 := by omega
  have h6 : a ≠ addr + 6 := by omega
  have h7 : a ≠ addr + 7 := by omega
  simp [storeI64, writeByte, h0, h1, h2, h3, h4, h5, h6, h7]

/-- `storeI64` of `0` zeroes every byte in `[addr, addr + 8)`. -/
theorem storeI64_zero_clears (m : LinearMem) (addr a : Nat)
    (hlo : addr ≤ a) (hhi : a < addr + 8) :
    storeI64 m addr 0 a = 0 := by
  have hcases :
      a = addr ∨ a = addr + 1 ∨ a = addr + 2 ∨ a = addr + 3 ∨
      a = addr + 4 ∨ a = addr + 5 ∨ a = addr + 6 ∨ a = addr + 7 := by omega
  rcases hcases with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;>
    simp [storeI64, writeByte, getByteLE64]

/-- `loadI32` depends only on the 4 bytes `[addr, addr + 4)`. -/
theorem loadI32_memEqOn {m1 m2 : LinearMem} {lo hi addr : Nat}
    (h : MemEqOn lo hi m1 m2) (hlo : lo ≤ addr) (hhi : addr + 4 ≤ hi) :
    loadI32 m1 addr = loadI32 m2 addr := by
  have b0 : m1 addr = m2 addr := h addr hlo (by omega)
  have b1 : m1 (addr + 1) = m2 (addr + 1) := h (addr + 1) (by omega) (by omega)
  have b2 : m1 (addr + 2) = m2 (addr + 2) := h (addr + 2) (by omega) (by omega)
  have b3 : m1 (addr + 3) = m2 (addr + 3) := h (addr + 3) (by omega) (by omega)
  simp [loadI32, b0, b1, b2, b3]

/-- `loadI64` depends only on the 8 bytes `[addr, addr + 8)`. -/
theorem loadI64_memEqOn {m1 m2 : LinearMem} {lo hi addr : Nat}
    (h : MemEqOn lo hi m1 m2) (hlo : lo ≤ addr) (hhi : addr + 8 ≤ hi) :
    loadI64 m1 addr = loadI64 m2 addr := by
  have b0 : m1 addr = m2 addr := h addr hlo (by omega)
  have b1 : m1 (addr + 1) = m2 (addr + 1) := h (addr + 1) (by omega) (by omega)
  have b2 : m1 (addr + 2) = m2 (addr + 2) := h (addr + 2) (by omega) (by omega)
  have b3 : m1 (addr + 3) = m2 (addr + 3) := h (addr + 3) (by omega) (by omega)
  have b4 : m1 (addr + 4) = m2 (addr + 4) := h (addr + 4) (by omega) (by omega)
  have b5 : m1 (addr + 5) = m2 (addr + 5) := h (addr + 5) (by omega) (by omega)
  have b6 : m1 (addr + 6) = m2 (addr + 6) := h (addr + 6) (by omega) (by omega)
  have b7 : m1 (addr + 7) = m2 (addr + 7) := h (addr + 7) (by omega) (by omega)
  simp [loadI64, b0, b1, b2, b3, b4, b5, b6, b7]

/-! ## 2. Concrete Linear-Memory Runtime State and WAT Helpers -/

/-- Concrete WebAssembly runtime state mirroring the globals and linear memory of
    `crates/noble-wasm/runtime/{stack,accounting,common}.wat`. -/
structure LinearRuntimeState where
  mem : LinearMem
  sp : BitVec 32
  cp : BitVec 32
  failure : BitVec 32
  quotaReason : BitVec 32
  phase : BitVec 32
  steps : BitVec 32
  operandLimit : BitVec 32
  continuationLimit : BitVec 32
  stepLimit : BitVec 32
  operandPeak : BitVec 32
  continuationPeak : BitVec 32

/-- Concrete `$fail (code : i32)` (`accounting.wat:2-3`):
    sets `$failure := code` iff `$failure = 0`; leaves `$quota_reason` untouched. -/
def concreteFail (cst : LinearRuntimeState) (code : BitVec 32) : LinearRuntimeState :=
  if cst.failure = 0 then { cst with failure := code } else cst

/-- Concrete `$quota (reason : i32)` (`accounting.wat:4-7`):
    sets `$quota_reason := reason` and `$failure := 1 + $phase` iff `$failure = 0`. -/
def concreteQuota (cst : LinearRuntimeState) (reason : BitVec 32) : LinearRuntimeState :=
  if cst.failure = 0 then
    { cst with quotaReason := reason, failure := 1 + cst.phase }
  else
    cst

/-- Concrete `$need (count : i32) -> i32` (`stack.wat:2-6`). -/
def concreteNeed (cst : LinearRuntimeState) (count : BitVec 32) : BitVec 32 × LinearRuntimeState :=
  if cst.failure ≠ 0 then (0, cst)
  else if cst.sp < count then (0, concreteFail cst 4)
  else (1, cst)

/-- Concrete `$slot_kind (index : i32) -> i32` (`stack.wat:7-10`).
    Preserves the exact WAT guard order: NO top `$failure` check; checks `index ≥_u $sp`. -/
def concreteSlotKind (cst : LinearRuntimeState) (index : BitVec 32) : BitVec 32 × LinearRuntimeState :=
  if cst.sp ≤ index then (0, concreteFail cst 4)
  else (loadI32 cst.mem (index * 16).toNat, cst)

/-- Concrete `$slot_value (index : i32) -> i64` (`stack.wat:11-14`).
    Preserves the exact WAT guard order: NO top `$failure` check; checks `index ≥_u $sp`. -/
def concreteSlotValue (cst : LinearRuntimeState) (index : BitVec 32) : BitVec 64 × LinearRuntimeState :=
  if cst.sp ≤ index then (0, concreteFail cst 4)
  else (loadI64 cst.mem ((index * 16).toNat + 8), cst)

/-- Concrete `$push_tag (kind : i32) (value : i64)` (`stack.wat:15-25`).
    Stores `kind` at `address + 0..+4` and `value` at `address + 8..+16`, leaving
    `address + 4..+8` padding untouched. -/
def concretePushTag (cst : LinearRuntimeState) (kind : BitVec 32) (value : BitVec 64) :
    LinearRuntimeState :=
  if cst.failure ≠ 0 then cst
  else
    let bytes := (cst.sp + 1) * 16
    if cst.operandLimit < bytes then concreteQuota cst 4
    else
      let address := cst.sp * 16
      let mem1 := storeI32 cst.mem address.toNat kind
      let mem2 := storeI64 mem1 (address.toNat + 8) value
      let peak' := if cst.operandPeak < bytes then bytes else cst.operandPeak
      { cst with mem := mem2, sp := cst.sp + 1, operandPeak := peak' }

/-- Concrete `$push_i64 (value : i64)` (`stack.wat:26`). -/
def concretePushI64 (cst : LinearRuntimeState) (value : BitVec 64) : LinearRuntimeState :=
  concretePushTag cst 1 value

/-- Concrete `$take () -> i64` (`stack.wat:33-40`).
    Zeros all 16 bytes `[sp' * 16, sp' * 16 + 16)` on pop via two 8-byte `i64.store 0`. -/
def concreteTake (cst : LinearRuntimeState) : BitVec 64 × LinearRuntimeState :=
  let (ok, cst1) := concreteNeed cst 1
  if ok = 0 then (0, cst1)
  else
    let sp' := cst1.sp - 1
    let address := sp' * 16
    let val := loadI64 cst1.mem (address.toNat + 8)
    let mem1 := storeI64 cst1.mem address.toNat 0
    let mem2 := storeI64 mem1 (address.toNat + 8) 0
    (val, { cst1 with mem := mem2, sp := sp' })

/-- Concrete `$pop_kind (expected : i32) -> i64` (`stack.wat:41-45`).
    Leaves `$sp` and memory unchanged on failure, underflow, or tag mismatch. -/
def concretePopKind (cst : LinearRuntimeState) (expected : BitVec 32) : BitVec 64 × LinearRuntimeState :=
  let (ok, cst1) := concreteNeed cst 1
  if ok = 0 then (0, cst1)
  else
    let (k, cst2) := concreteSlotKind cst1 (cst1.sp - 1)
    if k ≠ expected then (0, concreteFail cst2 4)
    else concreteTake cst2

/-- Concrete `$enqueue (index : i32) (environment : i32)` (`stack.wat:62-72`). -/
def concreteEnqueue (cst : LinearRuntimeState) (fnIdx env : BitVec 32) : LinearRuntimeState :=
  if cst.failure ≠ 0 then cst
  else
    let bytes := (cst.cp + 1) * 8
    if cst.continuationLimit < bytes then concreteQuota cst 5
    else
      let address := (4096 : BitVec 32) + cst.cp * 8
      let mem1 := storeI32 cst.mem address.toNat fnIdx
      let mem2 := storeI32 mem1 (address.toNat + 4) env
      let peak' := if cst.continuationPeak < bytes then bytes else cst.continuationPeak
      { cst with mem := mem2, cp := cst.cp + 1, continuationPeak := peak' }

/-- Concrete top-continuation dequeue matching `$dispatch` (`stack.wat:80-86`):
    decrements `$cp`, loads `(index, environment)` from `4096 + cp' * 8`, and zeros
    all 8 bytes `[4096 + cp' * 8, 4096 + cp' * 8 + 8)`. -/
def concreteDequeue (cst : LinearRuntimeState) : (BitVec 32 × BitVec 32) × LinearRuntimeState :=
  if cst.failure ≠ 0 ∨ cst.cp = 0 then ((0, 0), cst)
  else
    let cp' := cst.cp - 1
    let address := (4096 : BitVec 32) + cp' * 8
    let fnIdx := loadI32 cst.mem address.toNat
    let env := loadI32 cst.mem (address.toNat + 4)
    let mem' := storeI64 cst.mem address.toNat 0
    ((fnIdx, env), { cst with mem := mem', cp := cp' })

/-- Concrete `$runtime_tick () -> i32` (`accounting.wat:10-15`). -/
def concreteRuntimeTick (cst : LinearRuntimeState) : BitVec 32 × LinearRuntimeState :=
  if cst.failure ≠ 0 then (0, cst)
  else if cst.stepLimit ≤ cst.steps then (0, concreteQuota cst 6)
  else (1, { cst with steps := cst.steps + 1 })

/-! ## 3. Case Characterization Lemmas -/

theorem concreteFail_ok (cst : LinearRuntimeState) (code : BitVec 32) (hf : cst.failure = 0) :
    concreteFail cst code = { cst with failure := code } := by
  unfold concreteFail; split <;> first | rfl | contradiction

theorem concreteFail_failed (cst : LinearRuntimeState) (code : BitVec 32) (hf : cst.failure ≠ 0) :
    concreteFail cst code = cst := by
  unfold concreteFail; split <;> first | contradiction | rfl

theorem concreteQuota_ok (cst : LinearRuntimeState) (reason : BitVec 32) (hf : cst.failure = 0) :
    concreteQuota cst reason = { cst with quotaReason := reason, failure := 1 + cst.phase } := by
  unfold concreteQuota; split <;> first | rfl | contradiction

theorem concreteQuota_failed (cst : LinearRuntimeState) (reason : BitVec 32) (hf : cst.failure ≠ 0) :
    concreteQuota cst reason = cst := by
  unfold concreteQuota; split <;> first | contradiction | rfl

theorem concreteNeed_failed (cst : LinearRuntimeState) (count : BitVec 32) (hf : cst.failure ≠ 0) :
    concreteNeed cst count = (0, cst) := by
  unfold concreteNeed; split <;> first | rfl | contradiction

theorem concreteNeed_underflow (cst : LinearRuntimeState) (count : BitVec 32)
    (hf : cst.failure = 0) (hlt : cst.sp < count) :
    concreteNeed cst count = (0, { cst with failure := 4 }) := by
  unfold concreteNeed
  split
  · contradiction
  · rw [concreteFail_ok cst 4 hf]

theorem concreteNeed_ok (cst : LinearRuntimeState) (count : BitVec 32)
    (hf : cst.failure = 0) (hle : count ≤ cst.sp) :
    concreteNeed cst count = (1, cst) := by
  have hnotlt : ¬ cst.sp < count := by
    rw [BitVec.lt_def]; rw [BitVec.le_def] at hle; omega
  unfold concreteNeed
  split
  · contradiction
  · rfl

theorem concretePushTag_failed (cst : LinearRuntimeState) (k : BitVec 32) (v : BitVec 64)
    (hf : cst.failure ≠ 0) :
    concretePushTag cst k v = cst := by
  unfold concretePushTag; split <;> first | rfl | contradiction

theorem concretePushTag_quota (cst : LinearRuntimeState) (k : BitVec 32) (v : BitVec 64)
    (hf : cst.failure = 0) (hgt : cst.operandLimit < (cst.sp + 1) * 16) :
    concretePushTag cst k v = { cst with quotaReason := 4, failure := 1 + cst.phase } := by
  unfold concretePushTag
  split
  · contradiction
  · dsimp only
    split
    · exact concreteQuota_ok cst 4 hf
    · contradiction

theorem concretePushTag_ok (cst : LinearRuntimeState) (k : BitVec 32) (v : BitVec 64)
    (hf : cst.failure = 0) (hle : (cst.sp + 1) * 16 ≤ cst.operandLimit) :
    let bytes := (cst.sp + 1) * 16
    let address := cst.sp * 16
    let mem1 := storeI32 cst.mem address.toNat k
    let mem2 := storeI64 mem1 (address.toNat + 8) v
    let peak' := if cst.operandPeak < bytes then bytes else cst.operandPeak
    concretePushTag cst k v = { cst with mem := mem2, sp := cst.sp + 1, operandPeak := peak' } := by
  have hnotgt : ¬ cst.operandLimit < (cst.sp + 1) * 16 := by
    rw [BitVec.lt_def]; rw [BitVec.le_def] at hle; omega
  unfold concretePushTag
  split
  · contradiction
  · dsimp only
    split
    · contradiction
    · rfl

theorem concreteTake_failed (cst : LinearRuntimeState) (hf : cst.failure ≠ 0) :
    concreteTake cst = (0, cst) := by
  unfold concreteTake
  rw [concreteNeed_failed cst 1 hf]
  dsimp only
  split <;> first | rfl | contradiction

theorem concreteTake_underflow (cst : LinearRuntimeState)
    (hf : cst.failure = 0) (hlt : cst.sp < 1) :
    concreteTake cst = (0, { cst with failure := 4 }) := by
  unfold concreteTake
  rw [concreteNeed_underflow cst 1 hf hlt]
  dsimp only
  split <;> first | rfl | contradiction

theorem concreteTake_ok (cst : LinearRuntimeState)
    (hf : cst.failure = 0) (hle : (1 : BitVec 32) ≤ cst.sp) :
    let sp' := cst.sp - 1
    let address := sp' * 16
    let val := loadI64 cst.mem (address.toNat + 8)
    let mem1 := storeI64 cst.mem address.toNat 0
    let mem2 := storeI64 mem1 (address.toNat + 8) 0
    concreteTake cst = (val, { cst with mem := mem2, sp := sp' }) := by
  unfold concreteTake
  rw [concreteNeed_ok cst 1 hf hle]
  dsimp only
  split
  · next h => exact absurd h (by decide : (1 : BitVec 32) ≠ 0)
  · rfl

theorem concretePopKind_failed (cst : LinearRuntimeState) (expected : BitVec 32)
    (hf : cst.failure ≠ 0) :
    concretePopKind cst expected = (0, cst) := by
  unfold concretePopKind
  rw [concreteNeed_failed cst 1 hf]
  dsimp only
  split <;> first | rfl | contradiction

theorem concretePopKind_underflow (cst : LinearRuntimeState) (expected : BitVec 32)
    (hf : cst.failure = 0) (hlt : cst.sp < 1) :
    concretePopKind cst expected = (0, { cst with failure := 4 }) := by
  unfold concretePopKind
  rw [concreteNeed_underflow cst 1 hf hlt]
  dsimp only
  split <;> first | rfl | contradiction

theorem concretePopKind_mismatch (cst : LinearRuntimeState) (expected : BitVec 32)
    (hf : cst.failure = 0) (hle : (1 : BitVec 32) ≤ cst.sp)
    (hne : loadI32 cst.mem ((cst.sp - 1) * 16).toNat ≠ expected) :
    concretePopKind cst expected = (0, { cst with failure := 4 }) := by
  have hnotle : ¬ cst.sp ≤ cst.sp - 1 := by
    rw [BitVec.le_def, BitVec.toNat_sub_of_le hle]
    have h1 : (1 : BitVec 32).toNat = 1 := rfl
    rw [BitVec.le_def, h1] at hle
    omega
  unfold concretePopKind
  rw [concreteNeed_ok cst 1 hf hle]
  dsimp only
  split
  · next h => exact absurd h (by decide : (1 : BitVec 32) ≠ 0)
  · have hsk : concreteSlotKind cst (cst.sp - 1) = (loadI32 cst.mem ((cst.sp - 1) * 16).toNat, cst) := by
      unfold concreteSlotKind; split <;> first | contradiction | rfl
    rw [hsk]
    dsimp only
    split
    · rw [concreteFail_ok cst 4 hf]
    · contradiction

theorem concretePopKind_ok (cst : LinearRuntimeState) (expected : BitVec 32)
    (hf : cst.failure = 0) (hle : (1 : BitVec 32) ≤ cst.sp)
    (heq : loadI32 cst.mem ((cst.sp - 1) * 16).toNat = expected) :
    concretePopKind cst expected = concreteTake cst := by
  have hnotle : ¬ cst.sp ≤ cst.sp - 1 := by
    rw [BitVec.le_def, BitVec.toNat_sub_of_le hle]
    have h1 : (1 : BitVec 32).toNat = 1 := rfl
    rw [BitVec.le_def, h1] at hle
    omega
  unfold concretePopKind
  rw [concreteNeed_ok cst 1 hf hle]
  dsimp only
  split
  · next h => exact absurd h (by decide : (1 : BitVec 32) ≠ 0)
  · have hsk : concreteSlotKind cst (cst.sp - 1) = (expected, cst) := by
      unfold concreteSlotKind; split <;> first | contradiction | exact congrArg (·, cst) heq
    rw [hsk]
    dsimp only
    split
    · contradiction
    · rfl

theorem concreteEnqueue_failed (cst : LinearRuntimeState) (fnIdx env : BitVec 32)
    (hf : cst.failure ≠ 0) :
    concreteEnqueue cst fnIdx env = cst := by
  unfold concreteEnqueue; split <;> first | rfl | contradiction

theorem concreteEnqueue_quota (cst : LinearRuntimeState) (fnIdx env : BitVec 32)
    (hf : cst.failure = 0) (hgt : cst.continuationLimit < (cst.cp + 1) * 8) :
    concreteEnqueue cst fnIdx env = { cst with quotaReason := 5, failure := 1 + cst.phase } := by
  unfold concreteEnqueue
  split
  · contradiction
  · dsimp only
    split
    · exact concreteQuota_ok cst 5 hf
    · contradiction

theorem concreteEnqueue_ok (cst : LinearRuntimeState) (fnIdx env : BitVec 32)
    (hf : cst.failure = 0) (hle : (cst.cp + 1) * 8 ≤ cst.continuationLimit) :
    let bytes := (cst.cp + 1) * 8
    let address := (4096 : BitVec 32) + cst.cp * 8
    let mem1 := storeI32 cst.mem address.toNat fnIdx
    let mem2 := storeI32 mem1 (address.toNat + 4) env
    let peak' := if cst.continuationPeak < bytes then bytes else cst.continuationPeak
    concreteEnqueue cst fnIdx env = { cst with mem := mem2, cp := cst.cp + 1, continuationPeak := peak' } := by
  have hnotgt : ¬ cst.continuationLimit < (cst.cp + 1) * 8 := by
    rw [BitVec.lt_def]; rw [BitVec.le_def] at hle; omega
  unfold concreteEnqueue
  split
  · contradiction
  · dsimp only
    split
    · contradiction
    · rfl

theorem concreteDequeue_skip (cst : LinearRuntimeState)
    (hskip : cst.failure ≠ 0 ∨ cst.cp = 0) :
    concreteDequeue cst = ((0, 0), cst) := by
  unfold concreteDequeue; split <;> first | rfl | contradiction

theorem concreteDequeue_ok (cst : LinearRuntimeState)
    (hf : cst.failure = 0) (hcp : cst.cp ≠ 0) :
    let cp' := cst.cp - 1
    let address := (4096 : BitVec 32) + cp' * 8
    let fnIdx := loadI32 cst.mem address.toNat
    let env := loadI32 cst.mem (address.toNat + 4)
    let mem' := storeI64 cst.mem address.toNat 0
    concreteDequeue cst = ((fnIdx, env), { cst with mem := mem', cp := cp' }) := by
  unfold concreteDequeue
  split
  · next h => rcases h with h1 | h2; exact absurd hf h1; exact absurd h2 hcp
  · rfl

theorem concreteRuntimeTick_failed (cst : LinearRuntimeState) (hf : cst.failure ≠ 0) :
    concreteRuntimeTick cst = (0, cst) := by
  unfold concreteRuntimeTick; split <;> first | rfl | contradiction

theorem concreteRuntimeTick_quota (cst : LinearRuntimeState)
    (hf : cst.failure = 0) (hge : cst.stepLimit ≤ cst.steps) :
    concreteRuntimeTick cst = (0, { cst with quotaReason := 6, failure := 1 + cst.phase }) := by
  unfold concreteRuntimeTick
  split
  · contradiction
  · rw [concreteQuota_ok cst 6 hf]

theorem concreteRuntimeTick_ok (cst : LinearRuntimeState)
    (hf : cst.failure = 0) (hlt : cst.steps < cst.stepLimit) :
    concreteRuntimeTick cst = (1, { cst with steps := cst.steps + 1 }) := by
  have hnotle : ¬ cst.stepLimit ≤ cst.steps := by
    rw [BitVec.le_def]; rw [BitVec.lt_def] at hlt; omega
  unfold concreteRuntimeTick
  split
  · contradiction
  · rfl

/-! ## 4. Non-Wrapping Address Arithmetic and Region Non-Interference Theorems -/

private theorem bv32_one_toNat : (1 : BitVec 32).toNat = 1 := rfl
private theorem bv32_8_toNat : (8 : BitVec 32).toNat = 8 := rfl
private theorem bv32_16_toNat : (16 : BitVec 32).toNat = 16 := rfl
private theorem bv32_4096_toNat : (4096 : BitVec 32).toNat = 4096 := rfl

theorem bv32_sp_bytes_toNat (sp : BitVec 32) (hsp : sp.toNat ≤ 256) :
    ((sp + 1) * 16).toNat = (sp.toNat + 1) * 16 := by
  have h1 : (sp + 1).toNat = sp.toNat + 1 := by
    rw [BitVec.toNat_add, bv32_one_toNat]
    omega
  rw [BitVec.toNat_mul, h1, bv32_16_toNat]
  omega

theorem bv32_sp_addr_toNat (sp : BitVec 32) (hsp : sp.toNat ≤ 256) :
    (sp * 16).toNat = sp.toNat * 16 := by
  rw [BitVec.toNat_mul, bv32_16_toNat]
  omega

theorem bv32_cp_bytes_toNat (cp : BitVec 32) (hcp : cp.toNat ≤ 512) :
    ((cp + 1) * 8).toNat = (cp.toNat + 1) * 8 := by
  have h1 : (cp + 1).toNat = cp.toNat + 1 := by
    rw [BitVec.toNat_add, bv32_one_toNat]
    omega
  rw [BitVec.toNat_mul, h1, bv32_8_toNat]
  omega

theorem bv32_cp_addr_toNat (cp : BitVec 32) (hcp : cp.toNat ≤ 512) :
    ((4096 : BitVec 32) + cp * 8).toNat = 4096 + cp.toNat * 8 := by
  have hmul : (cp * 8).toNat = cp.toNat * 8 := by
    rw [BitVec.toNat_mul, bv32_8_toNat]
    omega
  rw [BitVec.toNat_add, bv32_4096_toNat, hmul]
  omega

/-- `concretePushTag` preserves all bytes at or above `4096` (including `[4096, 8192)` and `[8192, ...)`). -/
theorem concretePushTag_preserves_above (cst : LinearRuntimeState) (k : BitVec 32) (v : BitVec 64)
    (hsp : cst.sp.toNat ≤ 256) (hlim : cst.operandLimit.toNat ≤ 2048)
    {lo hi : Nat} (hlo : 4096 ≤ lo) :
    MemEqOn lo hi cst.mem (concretePushTag cst k v).mem := by
  by_cases hf : cst.failure = 0
  · by_cases hgt : cst.operandLimit < (cst.sp + 1) * 16
    · rw [concretePushTag_quota cst k v hf hgt]
      exact MemEqOn.refl lo hi cst.mem
    · have hle : (cst.sp + 1) * 16 ≤ cst.operandLimit := by
        rw [BitVec.le_def]; rw [BitVec.lt_def] at hgt; omega
      rw [concretePushTag_ok cst k v hf hle]
      have hbytes : ((cst.sp + 1) * 16).toNat = (cst.sp.toNat + 1) * 16 :=
        bv32_sp_bytes_toNat cst.sp hsp
      have hleNat : (cst.sp.toNat + 1) * 16 ≤ 2048 := by
        rw [BitVec.le_def, hbytes] at hle; omega
      have haddr : (cst.sp * 16).toNat = cst.sp.toNat * 16 := bv32_sp_addr_toNat cst.sp hsp
      have hm1 : MemEqOn lo hi cst.mem (storeI32 cst.mem (cst.sp * 16).toNat k) :=
        storeI32_preserves_outside k (Or.inl (by omega))
      have hm2 : MemEqOn lo hi (storeI32 cst.mem (cst.sp * 16).toNat k)
          (storeI64 (storeI32 cst.mem (cst.sp * 16).toNat k) ((cst.sp * 16).toNat + 8) v) :=
        storeI64_preserves_outside v (Or.inl (by omega))
      exact hm1.trans hm2
  · rw [concretePushTag_failed cst k v hf]
    exact MemEqOn.refl lo hi cst.mem

/-- `concretePushTag` preserves the continuation region `[4096, 8192)` and continuation globals. -/
theorem concretePushTag_preserves_contRegion (cst : LinearRuntimeState) (k : BitVec 32) (v : BitVec 64)
    (hsp : cst.sp.toNat ≤ 256) (hlim : cst.operandLimit.toNat ≤ 2048) :
    MemEqOn 4096 8192 cst.mem (concretePushTag cst k v).mem ∧
    (concretePushTag cst k v).cp = cst.cp ∧
    (concretePushTag cst k v).continuationPeak = cst.continuationPeak := by
  refine ⟨concretePushTag_preserves_above cst k v hsp hlim (Nat.le_refl 4096), ?_, ?_⟩
  all_goals
    by_cases hf : cst.failure = 0
    · by_cases hgt : cst.operandLimit < (cst.sp + 1) * 16
      · rw [concretePushTag_quota cst k v hf hgt]
      · have hle : (cst.sp + 1) * 16 ≤ cst.operandLimit := by
          rw [BitVec.le_def]; rw [BitVec.lt_def] at hgt; omega
        rw [concretePushTag_ok cst k v hf hle]
    · rw [concretePushTag_failed cst k v hf]

/-- `concretePushTag` preserves all bytes strictly below `cst.sp.toNat * 16`. -/
theorem concretePushTag_preserves_below (cst : LinearRuntimeState) (k : BitVec 32) (v : BitVec 64)
    (hsp : cst.sp.toNat ≤ 256) :
    MemEqOn 0 (cst.sp.toNat * 16) cst.mem (concretePushTag cst k v).mem := by
  by_cases hf : cst.failure = 0
  · by_cases hgt : cst.operandLimit < (cst.sp + 1) * 16
    · rw [concretePushTag_quota cst k v hf hgt]
      exact MemEqOn.refl _ _ _
    · have hle : (cst.sp + 1) * 16 ≤ cst.operandLimit := by
        rw [BitVec.le_def]; rw [BitVec.lt_def] at hgt; omega
      rw [concretePushTag_ok cst k v hf hle]
      have haddr : (cst.sp * 16).toNat = cst.sp.toNat * 16 := bv32_sp_addr_toNat cst.sp hsp
      have hm1 : MemEqOn 0 (cst.sp.toNat * 16) cst.mem (storeI32 cst.mem (cst.sp * 16).toNat k) :=
        storeI32_preserves_outside k (Or.inr (by omega))
      have hm2 : MemEqOn 0 (cst.sp.toNat * 16) (storeI32 cst.mem (cst.sp * 16).toNat k)
          (storeI64 (storeI32 cst.mem (cst.sp * 16).toNat k) ((cst.sp * 16).toNat + 8) v) :=
        storeI64_preserves_outside v (Or.inr (by omega))
      exact hm1.trans hm2
  · rw [concretePushTag_failed cst k v hf]
    exact MemEqOn.refl _ _ _

/-- `concretePushTag` preserves the `sp ≤ 256` region bound under `operandLimit ≤ 2048`. -/
theorem concretePushTag_sp_le (cst : LinearRuntimeState) (k : BitVec 32) (v : BitVec 64)
    (hsp : cst.sp.toNat ≤ 256) (hlim : cst.operandLimit.toNat ≤ 2048) :
    (concretePushTag cst k v).sp.toNat ≤ 256 := by
  by_cases hf : cst.failure = 0
  · by_cases hgt : cst.operandLimit < (cst.sp + 1) * 16
    · rw [concretePushTag_quota cst k v hf hgt]; exact hsp
    · have hle : (cst.sp + 1) * 16 ≤ cst.operandLimit := by
        rw [BitVec.le_def]; rw [BitVec.lt_def] at hgt; omega
      rw [concretePushTag_ok cst k v hf hle]
      have hbytes : ((cst.sp + 1) * 16).toNat = (cst.sp.toNat + 1) * 16 :=
        bv32_sp_bytes_toNat cst.sp hsp
      have hleNat : (cst.sp.toNat + 1) * 16 ≤ 2048 := by
        rw [BitVec.le_def, hbytes] at hle; omega
      have hadd : (cst.sp + 1).toNat = cst.sp.toNat + 1 := by
        rw [BitVec.toNat_add, bv32_one_toNat]; omega
      dsimp only
      rw [hadd]
      omega
  · rw [concretePushTag_failed cst k v hf]; exact hsp

/-- `concreteTake` preserves all bytes at or above `4096`. -/
theorem concreteTake_preserves_above (cst : LinearRuntimeState) (hsp : cst.sp.toNat ≤ 256)
    {lo hi : Nat} (hlo : 4096 ≤ lo) :
    MemEqOn lo hi cst.mem (concreteTake cst).2.mem := by
  by_cases hf : cst.failure = 0
  · by_cases hlt : cst.sp < 1
    · rw [concreteTake_underflow cst hf hlt]
      exact MemEqOn.refl _ _ _
    · have hle : (1 : BitVec 32) ≤ cst.sp := by
        rw [BitVec.le_def]; rw [BitVec.lt_def] at hlt; omega
      rw [concreteTake_ok cst hf hle]
      have hsp' : (cst.sp - 1).toNat = cst.sp.toNat - 1 := BitVec.toNat_sub_of_le hle
      have haddr : ((cst.sp - 1) * 16).toNat = (cst.sp.toNat - 1) * 16 := by
        rw [bv32_sp_addr_toNat (cst.sp - 1) (by omega), hsp']
      have hm1 : MemEqOn lo hi cst.mem (storeI64 cst.mem ((cst.sp - 1) * 16).toNat 0) :=
        storeI64_preserves_outside 0 (Or.inl (by omega))
      have hm2 : MemEqOn lo hi (storeI64 cst.mem ((cst.sp - 1) * 16).toNat 0)
          (storeI64 (storeI64 cst.mem ((cst.sp - 1) * 16).toNat 0) (((cst.sp - 1) * 16).toNat + 8) 0) :=
        storeI64_preserves_outside 0 (Or.inl (by omega))
      exact hm1.trans hm2
  · rw [concreteTake_failed cst hf]
    exact MemEqOn.refl _ _ _

/-- `concreteTake` preserves the continuation region `[4096, 8192)` and continuation globals. -/
theorem concreteTake_preserves_contRegion (cst : LinearRuntimeState) (hsp : cst.sp.toNat ≤ 256) :
    MemEqOn 4096 8192 cst.mem (concreteTake cst).2.mem ∧
    (concreteTake cst).2.cp = cst.cp ∧
    (concreteTake cst).2.continuationPeak = cst.continuationPeak := by
  refine ⟨concreteTake_preserves_above cst hsp (Nat.le_refl 4096), ?_, ?_⟩
  all_goals
    by_cases hf : cst.failure = 0
    · by_cases hlt : cst.sp < 1
      · rw [concreteTake_underflow cst hf hlt]
      · have hle : (1 : BitVec 32) ≤ cst.sp := by
          rw [BitVec.le_def]; rw [BitVec.lt_def] at hlt; omega
        rw [concreteTake_ok cst hf hle]
    · rw [concreteTake_failed cst hf]

/-- `concreteTake` preserves all bytes strictly below `(cst.sp.toNat - 1) * 16`. -/
theorem concreteTake_preserves_below (cst : LinearRuntimeState) (hsp : cst.sp.toNat ≤ 256) :
    MemEqOn 0 ((cst.sp.toNat - 1) * 16) cst.mem (concreteTake cst).2.mem := by
  by_cases hf : cst.failure = 0
  · by_cases hlt : cst.sp < 1
    · rw [concreteTake_underflow cst hf hlt]
      exact MemEqOn.refl _ _ _
    · have hle : (1 : BitVec 32) ≤ cst.sp := by
        rw [BitVec.le_def]; rw [BitVec.lt_def] at hlt; omega
      rw [concreteTake_ok cst hf hle]
      have hsp' : (cst.sp - 1).toNat = cst.sp.toNat - 1 := BitVec.toNat_sub_of_le hle
      have haddr : ((cst.sp - 1) * 16).toNat = (cst.sp.toNat - 1) * 16 := by
        rw [bv32_sp_addr_toNat (cst.sp - 1) (by omega), hsp']
      have hm1 : MemEqOn 0 ((cst.sp.toNat - 1) * 16) cst.mem
          (storeI64 cst.mem ((cst.sp - 1) * 16).toNat 0) :=
        storeI64_preserves_outside 0 (Or.inr (by omega))
      have hm2 : MemEqOn 0 ((cst.sp.toNat - 1) * 16)
          (storeI64 cst.mem ((cst.sp - 1) * 16).toNat 0)
          (storeI64 (storeI64 cst.mem ((cst.sp - 1) * 16).toNat 0) (((cst.sp - 1) * 16).toNat + 8) 0) :=
        storeI64_preserves_outside 0 (Or.inr (by omega))
      exact hm1.trans hm2
  · rw [concreteTake_failed cst hf]
    exact MemEqOn.refl _ _ _

/-- When `concreteTake` succeeds, all 16 bytes of the popped operand slot are zeroed. -/
theorem concreteTake_zeros_popped (cst : LinearRuntimeState) (hsp : cst.sp.toNat ≤ 256)
    (hf : cst.failure = 0) (hne : 0 < cst.sp.toNat) :
    ∀ a, (cst.sp.toNat - 1) * 16 ≤ a → a < cst.sp.toNat * 16 →
      (concreteTake cst).2.mem a = 0 := by
  intro a hlo hhi
  have hle : (1 : BitVec 32) ≤ cst.sp := by
    rw [BitVec.le_def, bv32_one_toNat]; omega
  have hsp' : (cst.sp - 1).toNat = cst.sp.toNat - 1 := BitVec.toNat_sub_of_le hle
  have haddr : ((cst.sp - 1) * 16).toNat = (cst.sp.toNat - 1) * 16 := by
    rw [bv32_sp_addr_toNat (cst.sp - 1) (by omega), hsp']
  rw [concreteTake_ok cst hf hle]
  dsimp only
  by_cases hhalf : a < (cst.sp.toNat - 1) * 16 + 8
  · have hpres : MemEqOn ((cst.sp.toNat - 1) * 16) ((cst.sp.toNat - 1) * 16 + 8)
        (storeI64 cst.mem ((cst.sp - 1) * 16).toNat 0)
        (storeI64 (storeI64 cst.mem ((cst.sp - 1) * 16).toNat 0) (((cst.sp - 1) * 16).toNat + 8) 0) :=
      storeI64_preserves_outside 0 (Or.inr (by omega))
    rw [← hpres a hlo hhalf]
    exact storeI64_zero_clears cst.mem ((cst.sp - 1) * 16).toNat a (by omega) (by omega)
  · exact storeI64_zero_clears (storeI64 cst.mem ((cst.sp - 1) * 16).toNat 0)
      (((cst.sp - 1) * 16).toNat + 8) a (by omega) (by omega)

/-- `concretePopKind` preserves all bytes at or above `4096`. -/
theorem concretePopKind_preserves_above (cst : LinearRuntimeState) (expected : BitVec 32)
    (hsp : cst.sp.toNat ≤ 256) {lo hi : Nat} (hlo : 4096 ≤ lo) :
    MemEqOn lo hi cst.mem (concretePopKind cst expected).2.mem := by
  by_cases hf : cst.failure = 0
  · by_cases hlt : cst.sp < 1
    · rw [concretePopKind_underflow cst expected hf hlt]
      exact MemEqOn.refl _ _ _
    · have hle : (1 : BitVec 32) ≤ cst.sp := by
        rw [BitVec.le_def]; rw [BitVec.lt_def] at hlt; omega
      by_cases heq : loadI32 cst.mem ((cst.sp - 1) * 16).toNat = expected
      · rw [concretePopKind_ok cst expected hf hle heq]
        exact concreteTake_preserves_above cst hsp hlo
      · rw [concretePopKind_mismatch cst expected hf hle heq]
        exact MemEqOn.refl _ _ _
  · rw [concretePopKind_failed cst expected hf]
    exact MemEqOn.refl _ _ _

/-- `concretePopKind` preserves the continuation region `[4096, 8192)` and continuation globals. -/
theorem concretePopKind_preserves_contRegion (cst : LinearRuntimeState) (expected : BitVec 32)
    (hsp : cst.sp.toNat ≤ 256) :
    MemEqOn 4096 8192 cst.mem (concretePopKind cst expected).2.mem ∧
    (concretePopKind cst expected).2.cp = cst.cp ∧
    (concretePopKind cst expected).2.continuationPeak = cst.continuationPeak := by
  refine ⟨concretePopKind_preserves_above cst expected hsp (Nat.le_refl 4096), ?_, ?_⟩
  all_goals
    by_cases hf : cst.failure = 0
    · by_cases hlt : cst.sp < 1
      · rw [concretePopKind_underflow cst expected hf hlt]
      · have hle : (1 : BitVec 32) ≤ cst.sp := by
          rw [BitVec.le_def]; rw [BitVec.lt_def] at hlt; omega
        by_cases heq : loadI32 cst.mem ((cst.sp - 1) * 16).toNat = expected
        · rw [concretePopKind_ok cst expected hf hle heq]
          have ht := concreteTake_preserves_contRegion cst hsp
          first | exact ht.2.1 | exact ht.2.2
        · rw [concretePopKind_mismatch cst expected hf hle heq]
    · rw [concretePopKind_failed cst expected hf]

/-- `concretePopKind` preserves all bytes strictly below `(cst.sp.toNat - 1) * 16`. -/
theorem concretePopKind_preserves_below (cst : LinearRuntimeState) (expected : BitVec 32)
    (hsp : cst.sp.toNat ≤ 256) :
    MemEqOn 0 ((cst.sp.toNat - 1) * 16) cst.mem (concretePopKind cst expected).2.mem := by
  by_cases hf : cst.failure = 0
  · by_cases hlt : cst.sp < 1
    · rw [concretePopKind_underflow cst expected hf hlt]
      exact MemEqOn.refl _ _ _
    · have hle : (1 : BitVec 32) ≤ cst.sp := by
        rw [BitVec.le_def]; rw [BitVec.lt_def] at hlt; omega
      by_cases heq : loadI32 cst.mem ((cst.sp - 1) * 16).toNat = expected
      · rw [concretePopKind_ok cst expected hf hle heq]
        exact concreteTake_preserves_below cst hsp
      · rw [concretePopKind_mismatch cst expected hf hle heq]
        exact MemEqOn.refl _ _ _
  · rw [concretePopKind_failed cst expected hf]
    exact MemEqOn.refl _ _ _

/-- `concreteEnqueue` preserves the operand region `[0, 4096)` and operand globals. -/
theorem concreteEnqueue_preserves_opRegion (cst : LinearRuntimeState) (fnIdx env : BitVec 32)
    (hcp : cst.cp.toNat ≤ 512) :
    MemEqOn 0 4096 cst.mem (concreteEnqueue cst fnIdx env).mem ∧
    (concreteEnqueue cst fnIdx env).sp = cst.sp ∧
    (concreteEnqueue cst fnIdx env).operandPeak = cst.operandPeak := by
  refine ⟨?_, ?_, ?_⟩
  · by_cases hf : cst.failure = 0
    · by_cases hgt : cst.continuationLimit < (cst.cp + 1) * 8
      · rw [concreteEnqueue_quota cst fnIdx env hf hgt]
        exact MemEqOn.refl _ _ _
      · have hle : (cst.cp + 1) * 8 ≤ cst.continuationLimit := by
          rw [BitVec.le_def]; rw [BitVec.lt_def] at hgt; omega
        rw [concreteEnqueue_ok cst fnIdx env hf hle]
        have haddr : ((4096 : BitVec 32) + cst.cp * 8).toNat = 4096 + cst.cp.toNat * 8 :=
          bv32_cp_addr_toNat cst.cp hcp
        have hm1 : MemEqOn 0 4096 cst.mem
            (storeI32 cst.mem ((4096 : BitVec 32) + cst.cp * 8).toNat fnIdx) :=
          storeI32_preserves_outside fnIdx (Or.inr (by omega))
        have hm2 : MemEqOn 0 4096
            (storeI32 cst.mem ((4096 : BitVec 32) + cst.cp * 8).toNat fnIdx)
            (storeI32 (storeI32 cst.mem ((4096 : BitVec 32) + cst.cp * 8).toNat fnIdx)
              (((4096 : BitVec 32) + cst.cp * 8).toNat + 4) env) :=
          storeI32_preserves_outside env (Or.inr (by omega))
        exact hm1.trans hm2
    · rw [concreteEnqueue_failed cst fnIdx env hf]
      exact MemEqOn.refl _ _ _
  all_goals
    by_cases hf : cst.failure = 0
    · by_cases hgt : cst.continuationLimit < (cst.cp + 1) * 8
      · rw [concreteEnqueue_quota cst fnIdx env hf hgt]
      · have hle : (cst.cp + 1) * 8 ≤ cst.continuationLimit := by
          rw [BitVec.le_def]; rw [BitVec.lt_def] at hgt; omega
        rw [concreteEnqueue_ok cst fnIdx env hf hle]
    · rw [concreteEnqueue_failed cst fnIdx env hf]

/-- `concreteEnqueue` preserves all bytes at or above `8192` when `continuationLimit ≤ 4096`. -/
theorem concreteEnqueue_preserves_above (cst : LinearRuntimeState) (fnIdx env : BitVec 32)
    (hcp : cst.cp.toNat ≤ 512) (hlim : cst.continuationLimit.toNat ≤ 4096)
    {lo hi : Nat} (hlo : 8192 ≤ lo) :
    MemEqOn lo hi cst.mem (concreteEnqueue cst fnIdx env).mem := by
  by_cases hf : cst.failure = 0
  · by_cases hgt : cst.continuationLimit < (cst.cp + 1) * 8
    · rw [concreteEnqueue_quota cst fnIdx env hf hgt]
      exact MemEqOn.refl _ _ _
    · have hle : (cst.cp + 1) * 8 ≤ cst.continuationLimit := by
        rw [BitVec.le_def]; rw [BitVec.lt_def] at hgt; omega
      rw [concreteEnqueue_ok cst fnIdx env hf hle]
      have hbytes : ((cst.cp + 1) * 8).toNat = (cst.cp.toNat + 1) * 8 :=
        bv32_cp_bytes_toNat cst.cp hcp
      have hleNat : (cst.cp.toNat + 1) * 8 ≤ 4096 := by
        rw [BitVec.le_def, hbytes] at hle; omega
      have haddr : ((4096 : BitVec 32) + cst.cp * 8).toNat = 4096 + cst.cp.toNat * 8 :=
        bv32_cp_addr_toNat cst.cp hcp
      have hm1 : MemEqOn lo hi cst.mem
          (storeI32 cst.mem ((4096 : BitVec 32) + cst.cp * 8).toNat fnIdx) :=
        storeI32_preserves_outside fnIdx (Or.inl (by omega))
      have hm2 : MemEqOn lo hi
          (storeI32 cst.mem ((4096 : BitVec 32) + cst.cp * 8).toNat fnIdx)
          (storeI32 (storeI32 cst.mem ((4096 : BitVec 32) + cst.cp * 8).toNat fnIdx)
            (((4096 : BitVec 32) + cst.cp * 8).toNat + 4) env) :=
        storeI32_preserves_outside env (Or.inl (by omega))
      exact hm1.trans hm2
  · rw [concreteEnqueue_failed cst fnIdx env hf]
    exact MemEqOn.refl _ _ _

/-- `concreteEnqueue` preserves all existing continuation bytes in `[4096, 4096 + cp * 8)`. -/
theorem concreteEnqueue_preserves_below (cst : LinearRuntimeState) (fnIdx env : BitVec 32)
    (hcp : cst.cp.toNat ≤ 512) :
    MemEqOn 4096 (4096 + cst.cp.toNat * 8) cst.mem (concreteEnqueue cst fnIdx env).mem := by
  by_cases hf : cst.failure = 0
  · by_cases hgt : cst.continuationLimit < (cst.cp + 1) * 8
    · rw [concreteEnqueue_quota cst fnIdx env hf hgt]
      exact MemEqOn.refl _ _ _
    · have hle : (cst.cp + 1) * 8 ≤ cst.continuationLimit := by
        rw [BitVec.le_def]; rw [BitVec.lt_def] at hgt; omega
      rw [concreteEnqueue_ok cst fnIdx env hf hle]
      have haddr : ((4096 : BitVec 32) + cst.cp * 8).toNat = 4096 + cst.cp.toNat * 8 :=
        bv32_cp_addr_toNat cst.cp hcp
      have hm1 : MemEqOn 4096 (4096 + cst.cp.toNat * 8) cst.mem
          (storeI32 cst.mem ((4096 : BitVec 32) + cst.cp * 8).toNat fnIdx) :=
        storeI32_preserves_outside fnIdx (Or.inr (by omega))
      have hm2 : MemEqOn 4096 (4096 + cst.cp.toNat * 8)
          (storeI32 cst.mem ((4096 : BitVec 32) + cst.cp * 8).toNat fnIdx)
          (storeI32 (storeI32 cst.mem ((4096 : BitVec 32) + cst.cp * 8).toNat fnIdx)
            (((4096 : BitVec 32) + cst.cp * 8).toNat + 4) env) :=
        storeI32_preserves_outside env (Or.inr (by omega))
      exact hm1.trans hm2
  · rw [concreteEnqueue_failed cst fnIdx env hf]
    exact MemEqOn.refl _ _ _

/-- `concreteEnqueue` preserves the `cp ≤ 512` bound under `continuationLimit ≤ 4096`. -/
theorem concreteEnqueue_cp_le (cst : LinearRuntimeState) (fnIdx env : BitVec 32)
    (hcp : cst.cp.toNat ≤ 512) (hlim : cst.continuationLimit.toNat ≤ 4096) :
    (concreteEnqueue cst fnIdx env).cp.toNat ≤ 512 := by
  by_cases hf : cst.failure = 0
  · by_cases hgt : cst.continuationLimit < (cst.cp + 1) * 8
    · rw [concreteEnqueue_quota cst fnIdx env hf hgt]; exact hcp
    · have hle : (cst.cp + 1) * 8 ≤ cst.continuationLimit := by
        rw [BitVec.le_def]; rw [BitVec.lt_def] at hgt; omega
      rw [concreteEnqueue_ok cst fnIdx env hf hle]
      have hbytes : ((cst.cp + 1) * 8).toNat = (cst.cp.toNat + 1) * 8 :=
        bv32_cp_bytes_toNat cst.cp hcp
      have hleNat : (cst.cp.toNat + 1) * 8 ≤ 4096 := by
        rw [BitVec.le_def, hbytes] at hle; omega
      have hadd : (cst.cp + 1).toNat = cst.cp.toNat + 1 := by
        rw [BitVec.toNat_add, bv32_one_toNat]; omega
      dsimp only
      rw [hadd]
      omega
  · rw [concreteEnqueue_failed cst fnIdx env hf]; exact hcp

/-- `concreteDequeue` preserves the operand region `[0, 4096)` and operand globals. -/
theorem concreteDequeue_preserves_opRegion (cst : LinearRuntimeState) (hcp : cst.cp.toNat ≤ 512) :
    MemEqOn 0 4096 cst.mem (concreteDequeue cst).2.mem ∧
    (concreteDequeue cst).2.sp = cst.sp ∧
    (concreteDequeue cst).2.operandPeak = cst.operandPeak := by
  refine ⟨?_, ?_, ?_⟩
  · by_cases hskip : cst.failure ≠ 0 ∨ cst.cp = 0
    · rw [concreteDequeue_skip cst hskip]
      exact MemEqOn.refl _ _ _
    · have hf : cst.failure = 0 := Classical.not_not.mp (fun h => hskip (Or.inl h))
      have hne0 : cst.cp ≠ 0 := fun h => hskip (Or.inr h)
      rw [concreteDequeue_ok cst hf hne0]
      have hge1 : (1 : BitVec 32) ≤ cst.cp := by
        rw [BitVec.le_def, bv32_one_toNat]
        have hneNat : cst.cp.toNat ≠ 0 := fun hz =>
          hne0 (BitVec.eq_of_toNat_eq (by simpa using hz))
        omega
      have hcp' : (cst.cp - 1).toNat = cst.cp.toNat - 1 := BitVec.toNat_sub_of_le hge1
      have haddr : ((4096 : BitVec 32) + (cst.cp - 1) * 8).toNat = 4096 + (cst.cp.toNat - 1) * 8 := by
        rw [bv32_cp_addr_toNat (cst.cp - 1) (by omega), hcp']
      exact storeI64_preserves_outside 0 (Or.inr (by omega))
  all_goals
    by_cases hskip : cst.failure ≠ 0 ∨ cst.cp = 0
    · rw [concreteDequeue_skip cst hskip]
    · have hf : cst.failure = 0 := Classical.not_not.mp (fun h => hskip (Or.inl h))
      have hne0 : cst.cp ≠ 0 := fun h => hskip (Or.inr h)
      rw [concreteDequeue_ok cst hf hne0]

/-- `concreteDequeue` preserves all bytes at or above `8192`. -/
theorem concreteDequeue_preserves_above (cst : LinearRuntimeState) (hcp : cst.cp.toNat ≤ 512)
    {lo hi : Nat} (hlo : 8192 ≤ lo) :
    MemEqOn lo hi cst.mem (concreteDequeue cst).2.mem := by
  by_cases hskip : cst.failure ≠ 0 ∨ cst.cp = 0
  · rw [concreteDequeue_skip cst hskip]
    exact MemEqOn.refl _ _ _
  · have hf : cst.failure = 0 := Classical.not_not.mp (fun h => hskip (Or.inl h))
    have hne0 : cst.cp ≠ 0 := fun h => hskip (Or.inr h)
    rw [concreteDequeue_ok cst hf hne0]
    have hge1 : (1 : BitVec 32) ≤ cst.cp := by
      rw [BitVec.le_def, bv32_one_toNat]
      have hneNat : cst.cp.toNat ≠ 0 := fun hz =>
        hne0 (BitVec.eq_of_toNat_eq (by simpa using hz))
      omega
    have hcp' : (cst.cp - 1).toNat = cst.cp.toNat - 1 := BitVec.toNat_sub_of_le hge1
    have haddr : ((4096 : BitVec 32) + (cst.cp - 1) * 8).toNat = 4096 + (cst.cp.toNat - 1) * 8 := by
      rw [bv32_cp_addr_toNat (cst.cp - 1) (by omega), hcp']
    exact storeI64_preserves_outside 0 (Or.inl (by omega))

/-- `concreteDequeue` preserves all remaining continuation bytes `[4096, 4096 + (cp - 1) * 8)`. -/
theorem concreteDequeue_preserves_below (cst : LinearRuntimeState) (hcp : cst.cp.toNat ≤ 512) :
    MemEqOn 4096 (4096 + (cst.cp.toNat - 1) * 8) cst.mem (concreteDequeue cst).2.mem := by
  by_cases hskip : cst.failure ≠ 0 ∨ cst.cp = 0
  · rw [concreteDequeue_skip cst hskip]
    exact MemEqOn.refl _ _ _
  · have hf : cst.failure = 0 := Classical.not_not.mp (fun h => hskip (Or.inl h))
    have hne0 : cst.cp ≠ 0 := fun h => hskip (Or.inr h)
    rw [concreteDequeue_ok cst hf hne0]
    have hge1 : (1 : BitVec 32) ≤ cst.cp := by
      rw [BitVec.le_def, bv32_one_toNat]
      have hneNat : cst.cp.toNat ≠ 0 := fun hz =>
        hne0 (BitVec.eq_of_toNat_eq (by simpa using hz))
      omega
    have hcp' : (cst.cp - 1).toNat = cst.cp.toNat - 1 := BitVec.toNat_sub_of_le hge1
    have haddr : ((4096 : BitVec 32) + (cst.cp - 1) * 8).toNat = 4096 + (cst.cp.toNat - 1) * 8 := by
      rw [bv32_cp_addr_toNat (cst.cp - 1) (by omega), hcp']
    exact storeI64_preserves_outside 0 (Or.inr (by omega))

/-- When `concreteDequeue` succeeds, all 8 bytes of the popped continuation entry are zeroed. -/
theorem concreteDequeue_zeros_popped (cst : LinearRuntimeState) (hcp : cst.cp.toNat ≤ 512)
    (hf : cst.failure = 0) (hne : 0 < cst.cp.toNat) :
    ∀ a, 4096 + (cst.cp.toNat - 1) * 8 ≤ a → a < 4096 + cst.cp.toNat * 8 →
      (concreteDequeue cst).2.mem a = 0 := by
  intro a hlo hhi
  have hne0 : cst.cp ≠ 0 := fun h => by simp [h] at hne
  have hge1 : (1 : BitVec 32) ≤ cst.cp := by
    rw [BitVec.le_def, bv32_one_toNat]; omega
  have hcp' : (cst.cp - 1).toNat = cst.cp.toNat - 1 := BitVec.toNat_sub_of_le hge1
  have haddr : ((4096 : BitVec 32) + (cst.cp - 1) * 8).toNat = 4096 + (cst.cp.toNat - 1) * 8 := by
    rw [bv32_cp_addr_toNat (cst.cp - 1) (by omega), hcp']
  rw [concreteDequeue_ok cst hf hne0]
  exact storeI64_zero_clears cst.mem ((4096 : BitVec 32) + (cst.cp - 1) * 8).toNat a
    (by omega) (by omega)

/-- `concreteRuntimeTick` leaves linear memory and all stack/continuation pointers and peaks unchanged. -/
theorem concreteRuntimeTick_preserves_mem (cst : LinearRuntimeState) :
    (concreteRuntimeTick cst).2.mem = cst.mem ∧
    (concreteRuntimeTick cst).2.sp = cst.sp ∧
    (concreteRuntimeTick cst).2.cp = cst.cp ∧
    (concreteRuntimeTick cst).2.operandPeak = cst.operandPeak ∧
    (concreteRuntimeTick cst).2.continuationPeak = cst.continuationPeak := by
  by_cases hf : cst.failure = 0
  · by_cases hge : cst.stepLimit ≤ cst.steps
    · rw [concreteRuntimeTick_quota cst hf hge]
      exact ⟨rfl, rfl, rfl, rfl, rfl⟩
    · have hlt : cst.steps < cst.stepLimit := by
        rw [BitVec.lt_def]; rw [BitVec.le_def] at hge; omega
      rw [concreteRuntimeTick_ok cst hf hlt]
      exact ⟨rfl, rfl, rfl, rfl, rfl⟩
  · rw [concreteRuntimeTick_failed cst hf]
    exact ⟨rfl, rfl, rfl, rfl, rfl⟩

end NobleContracts.WasmLinearMemory
