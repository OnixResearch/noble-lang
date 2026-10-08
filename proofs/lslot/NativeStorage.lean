import Std

/-!
A deliberately small model of the installed live-linear-storage.wat storage-read
path. This formalizes little-endian, bounded memory loads and unsigned Wasm I32
arithmetic; it does not assert that a selected native binary executes this
model, nor that heap writes, refcounts, continuation dispatch, or imports
refine NobleContracts.Exec. The installed-byte interpreter separately executes
twelve original functions over a single captured 20-page memory state.
-/

namespace NobleLiveStorage

private def wordSize : Nat := 4294967296

def address (handle : Nat) : Nat := 65536 + 48 * (handle % 4096)

def load32 (bytes : Array UInt8) (at : Nat) : Option Nat := do
  let a ← bytes[at]?
  let b ← bytes[at + 1]?
  let c ← bytes[at + 2]?
  let d ← bytes[at + 3]?
  return a.toNat + 256 * b.toNat + 65536 * c.toNat + 16777216 * d.toNat

/-- The WAT's early unsigned guard precedes any load; a live cell's full
    epoch-qualified ID and low-byte kind are checked after the guarded load. -/
def storageIsLive (bytes : Array UInt8) (cursor handle : Nat) : Option Bool := do
  if handle < 4096 ∨ handle % 4096 ≥ cursor then return false
  let id ← load32 bytes (address handle + 4)
  let word ← load32 bytes (address handle)
  return decide (id = handle % wordSize ∧
    ((word % 256 + wordSize - 1) % wordSize) ≤ 16)

/-- `linear_child` preserves null but traps (represented by `none`) when a
    non-null child's epoch-qualified header is not live. This helper does not
    dereference its parent argument. -/
def linearChild (bytes : Array UInt8) (cursor child parent : Nat) : Option Nat :=
  if child == 0 then some 0
  else (storageIsLive bytes cursor child).bind fun live =>
    if live then some child else none

/-- A null edge is permitted without consulting any cell or the parent. -/
theorem null_child (bytes : Array UInt8) (cursor parent : Nat) :
    linearChild bytes cursor 0 parent = some 0 := by
  simp [linearChild]

/-- A dead non-null edge cannot silently become a usable handle. -/
theorem dead_child_traps (bytes : Array UInt8) (cursor child parent : Nat)
    (nonzero : child ≠ 0) (dead : storageIsLive bytes cursor child = some false) :
    linearChild bytes cursor child parent = none := by
  simp [linearChild, nonzero, dead]

/-- Out-of-range handles return dead without needing a readable cell. -/
theorem outside_cursor_is_dead (bytes : Array UInt8) (cursor handle : Nat)
    (out : handle < 4096 ∨ handle % 4096 ≥ cursor) :
    storageIsLive bytes cursor handle = some false := by
  simp [storageIsLive, out]

/-- A reused physical slot with a different full epoch ID is never live,
    even if its low-byte kind and slot index match. -/
theorem stale_epoch_is_dead (bytes : Array UInt8) (cursor handle id word : Nat)
    (inside : ¬ (handle < 4096 ∨ handle % 4096 ≥ cursor))
    (header : load32 bytes (address handle + 4) = some id)
    (tag : load32 bytes (address handle) = some word)
    (stale : id ≠ handle % wordSize) :
    storageIsLive bytes cursor handle = some false := by
  simp [storageIsLive, inside, header, tag, stale]

/-- The exact child read traps instead of accepting a reused slot under
    a different epoch, provided the nonzero handle denotes that slot. -/
theorem stale_child_traps (bytes : Array UInt8) (cursor handle parent id word : Nat)
    (nonzero : handle ≠ 0)
    (inside : ¬ (handle < 4096 ∨ handle % 4096 ≥ cursor))
    (header : load32 bytes (address handle + 4) = some id)
    (tag : load32 bytes (address handle) = some word)
    (stale : id ≠ handle % wordSize) :
    linearChild bytes cursor handle parent = none :=
  dead_child_traps bytes cursor handle parent nonzero
    (stale_epoch_is_dead bytes cursor handle id word inside header tag stale)

/-!
Source-only model of the *direct, captured I64 child* of one quote_entry call.
The input is the imported 20-page memory, not an interpreter's decoded result.
Only the operand slot at [0, 16) is overlaid; all other bytes are read from
the original finite array. This does not model the null-child branch, quota
failures, dispatch, imports, table calls, continuations, or a whole invocation.
In particular, none of the following theorems identifies this model with the
selected installed Wasm or with an independent C oracle. That source-bound
refinement still requires executing the selected quote_entry -> a -> unbox ->
kind/payload -> push_tag path on identical imported bytes and globals, checking
its stores, globals, traps, refcounts and negative cases against both models.
-/

def load64 (bytes : Array UInt8) (at : Nat) : Option Nat := do
  let low ← load32 bytes at
  let high ← load32 bytes (at + 4)
  return low + wordSize * high

/-- A live, epoch-qualified physical cell with the requested low-byte kind. -/
def liveWithKind (bytes : Array UInt8) (cursor handle kind : Nat) : Option Bool := do
  let live ← storageIsLive bytes cursor handle
  if !live then return false
  let word ← load32 bytes (address handle)
  return decide (word % 256 = kind)

private def quoteRequire (condition : Prop) [Decidable condition] : Option Unit :=
  if condition then some () else none

/-- Decode the root and its actual child edge, including both full epoch IDs.
    Rejected children cannot be replaced by a same-slot or same-shape cell. -/
def directQuoteChild (bytes : Array UInt8) (cursor environment : Nat) : Option Nat := do
  let rootKind ← liveWithKind bytes cursor environment 4
  let _ ← quoteRequire (rootKind = true)
  let code ← load32 bytes (address environment + 28)
  let _ ← quoteRequire (code = 1)
  let child ← load32 bytes (address environment + 16)
  let _ ← quoteRequire (child ≠ 0)
  let childKind ← liveWithKind bytes cursor child 1
  let _ ← quoteRequire (childKind = true)
  return child

structure ScalarQuoteInput where
  memory : Array UInt8
  heapCursor : Nat
  environment : Nat
  failure : Nat
  sp : Nat
  cp : Nat
  rp : Nat
  liveFrame : Nat
  operandLimit : Nat
  operandPeak : Nat
  quoteInvocations : Nat

/-- Bounded i32 globals and the successful, single-slot stack precondition. -/
def ScalarQuoteInput.valid (s : ScalarQuoteInput) : Prop :=
  s.memory.size = 20 * 65536 ∧ 0 < s.heapCursor ∧ s.heapCursor ≤ 4096 ∧
  s.environment < wordSize ∧ s.failure = 0 ∧ s.sp = 0 ∧
  s.cp = 0 ∧ s.rp = 0 ∧ s.liveFrame = 0 ∧
  load32 s.memory 57352 = some 0 ∧
  16 ≤ s.operandLimit ∧ s.operandLimit ≤ 2048 ∧
  s.operandPeak ≤ s.operandLimit ∧ s.quoteInvocations < wordSize

/-- Encode an I64's bits, not its signed numerical interpretation. -/
def quotePayloadByte (bits byte : Nat) : UInt8 :=
  UInt8.ofNat ((bits / 256 ^ byte) % 256)

/-- The tag is stored as little-endian i32; the four existing padding bytes
    remain untouched. Only the eight payload bytes are encoded anew. -/
def quoteSlot (pad4 pad5 pad6 pad7 : UInt8) (bits : Nat) : Array UInt8 :=
  #[(1 : UInt8), 0, 0, 0, pad4, pad5, pad6, pad7,
    quotePayloadByte bits 0, quotePayloadByte bits 1,
    quotePayloadByte bits 2, quotePayloadByte bits 3,
    quotePayloadByte bits 4, quotePayloadByte bits 5,
    quotePayloadByte bits 6, quotePayloadByte bits 7]

theorem quote_slot_size (p4 p5 p6 p7 : UInt8) (bits : Nat) :
    (quoteSlot p4 p5 p6 p7 bits).size = 16 := by
  simp [quoteSlot]

structure ScalarQuoteResult where
  before : Array UInt8
  slot : Array UInt8
  logicalTag : Nat
  payloadBits : Nat
  sp : Nat
  operandPeak : Nat
  quoteInvocations : Nat

/-- Finite-array overlay: only physical byte addresses 0 through 15 select
    the replacement slot. No heap cell or refcount sidecar is copied/written. -/
def ScalarQuoteResult.readByte (r : ScalarQuoteResult) (at : Nat) : Option UInt8 :=
  if at < 16 then r.slot[at]? else r.before[at]?

/-- No null-child fallback, implicit refcount operation, or partial quota
    transition: every rejected precondition returns `none`. -/
def directScalarQuote (s : ScalarQuoteInput) : Option ScalarQuoteResult := do
  let _ ← quoteRequire s.valid
  let child ← directQuoteChild s.memory s.heapCursor s.environment
  let bits ← load64 s.memory (address child + 8)
  let pad4 ← s.memory[4]?
  let pad5 ← s.memory[5]?
  let pad6 ← s.memory[6]?
  let pad7 ← s.memory[7]?
  return {
    before := s.memory
    slot := quoteSlot pad4 pad5 pad6 pad7 bits
    logicalTag := 1
    payloadBits := bits
    sp := 1
    operandPeak := max s.operandPeak 16
    quoteInvocations := (s.quoteInvocations + 1) % wordSize
  }

theorem quote_slot_tag (p4 p5 p6 p7 : UInt8) (bits : Nat) :
    (quoteSlot p4 p5 p6 p7 bits)[0]? = some (1 : UInt8) := by
  simp [quoteSlot]

theorem quote_slot_preserves_padding (p4 p5 p6 p7 : UInt8) (bits : Nat) :
    (quoteSlot p4 p5 p6 p7 bits)[4]? = some p4 ∧
    (quoteSlot p4 p5 p6 p7 bits)[5]? = some p5 ∧
    (quoteSlot p4 p5 p6 p7 bits)[6]? = some p6 ∧
    (quoteSlot p4 p5 p6 p7 bits)[7]? = some p7 := by
  simp [quoteSlot]

theorem quote_slot_encodes_payload (p4 p5 p6 p7 : UInt8) (bits : Nat) :
    (quoteSlot p4 p5 p6 p7 bits)[8]? = some (quotePayloadByte bits 0) ∧
    (quoteSlot p4 p5 p6 p7 bits)[9]? = some (quotePayloadByte bits 1) ∧
    (quoteSlot p4 p5 p6 p7 bits)[10]? = some (quotePayloadByte bits 2) ∧
    (quoteSlot p4 p5 p6 p7 bits)[11]? = some (quotePayloadByte bits 3) ∧
    (quoteSlot p4 p5 p6 p7 bits)[12]? = some (quotePayloadByte bits 4) ∧
    (quoteSlot p4 p5 p6 p7 bits)[13]? = some (quotePayloadByte bits 5) ∧
    (quoteSlot p4 p5 p6 p7 bits)[14]? = some (quotePayloadByte bits 6) ∧
    (quoteSlot p4 p5 p6 p7 bits)[15]? = some (quotePayloadByte bits 7) := by
  simp [quoteSlot]

theorem scalar_quote_outside_slot (r : ScalarQuoteResult) (at : Nat) (outside : 16 ≤ at) :
    r.readByte at = r.before[at]? := by
  have notInSlot : ¬ at < 16 := Nat.not_lt.mpr outside
  simp [ScalarQuoteResult.readByte, notInSlot]

/-- Incoming, saved, and module-root u16 sidecars all lie in this interval. -/
theorem scalar_quote_refcount_sidecar_unchanged (r : ScalarQuoteResult) (at : Nat)
    (sidecar : 1052672 ≤ at ∧ at < 1077248) :
    r.readByte at = r.before[at]? :=
  scalar_quote_outside_slot r at
    (Nat.le_trans (by decide : 16 ≤ 1052672) sidecar.1)

theorem direct_quote_wrong_code (bytes : Array UInt8) (cursor environment code : Nat)
    (root : liveWithKind bytes cursor environment 4 = some true)
    (readCode : load32 bytes (address environment + 28) = some code)
    (wrong : code ≠ 1) :
    directQuoteChild bytes cursor environment = none := by
  simp [directQuoteChild, quoteRequire, root, readCode, wrong]

theorem direct_quote_wrong_child_kind (bytes : Array UInt8)
    (cursor environment child word : Nat)
    (root : liveWithKind bytes cursor environment 4 = some true)
    (readCode : load32 bytes (address environment + 28) = some 1)
    (edge : load32 bytes (address environment + 16) = some child)
    (live : storageIsLive bytes cursor child = some true)
    (header : load32 bytes (address child) = some word)
    (wrong : word % 256 ≠ 1) :
    directQuoteChild bytes cursor environment = none := by
  have badKind : liveWithKind bytes cursor child 1 = some false := by
    simp [liveWithKind, live, header, wrong]
  simp [directQuoteChild, quoteRequire, root, readCode, edge, badKind]

theorem direct_quote_stale_child (bytes : Array UInt8)
    (cursor environment child id word : Nat)
    (root : liveWithKind bytes cursor environment 4 = some true)
    (readCode : load32 bytes (address environment + 28) = some 1)
    (edge : load32 bytes (address environment + 16) = some child)
    (inside : ¬ (child < 4096 ∨ child % 4096 ≥ cursor))
    (header : load32 bytes (address child + 4) = some id)
    (tag : load32 bytes (address child) = some word)
    (stale : id ≠ child % wordSize) :
    directQuoteChild bytes cursor environment = none := by
  have dead : storageIsLive bytes cursor child = some false :=
    stale_epoch_is_dead bytes cursor child id word inside header tag stale
  have badKind : liveWithKind bytes cursor child 1 = some false := by
    simp [liveWithKind, dead]
  simp [directQuoteChild, quoteRequire, root, readCode, edge, badKind]

/-- All three malformed child/code cases above also refuse the whole narrow
    transition, independently of the original slot's byte pattern. -/
theorem direct_scalar_quote_refuses_bad_child (s : ScalarQuoteInput)
    (bad : directQuoteChild s.memory s.heapCursor s.environment = none) :
    directScalarQuote s = none := by
  by_cases valid : s.valid
  · simp [directScalarQuote, valid, quoteRequire, bad]
  · simp [directScalarQuote, valid, quoteRequire]

end NobleLiveStorage
