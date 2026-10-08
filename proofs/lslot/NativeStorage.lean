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

/-!
Pure arithmetic and source-binding projection for the selected two-quote,
three-add recipe. This is a composition of *two independent* successful
`directScalarQuote` results, not a model of their sequential execution in a
shared guest stack. The first add consumes the two quoted payloads, the second
adds `y`, and the third adds `x`, each modulo 2^64. No theorem below relates
this projection to guest callbacks, dispatch, heap/refcount changes, a
snapshot, a C implementation, or a fresh guest invocation. In particular,
equality of root values is not equality of source root bindings.
-/

def i64Modulus : Nat := 2 ^ 64

def wrappingAdd64 (left right : Nat) : Nat :=
  (left + right) % i64Modulus

def selectedFourInput (x y first second : Nat) : Nat :=
  wrappingAdd64 x (wrappingAdd64 y (wrappingAdd64 first second))

/-- The three ordered, wrapping additions compute the four-input sum modulo
    2^64, including when one or more additions overflow. -/
theorem selected_four_input_modulo (x y first second : Nat) :
    selectedFourInput x y first second =
      (x + (y + (first + second))) % i64Modulus := by
  simp [selectedFourInput, wrappingAdd64, Nat.add_mod]

/-- Only the *payloads* from the existing narrow directScalarQuote relation
    feed this pure composition. This Option has no guest-state transition. -/
def selectedQuotedFourInput (x y : Nat)
    (left right : ScalarQuoteInput) : Option Nat := do
  let first ← directScalarQuote left
  let second ← directScalarQuote right
  return selectedFourInput x y first.payloadBits second.payloadBits

theorem selected_quoted_composition (x y : Nat)
    (left right : ScalarQuoteInput) (first second : ScalarQuoteResult)
    (leftOk : directScalarQuote left = some first)
    (rightOk : directScalarQuote right = some second) :
    selectedQuotedFourInput x y left right =
      some ((x + (y + (first.payloadBits + second.payloadBits))) % i64Modulus) := by
  simp [selectedQuotedFourInput, leftOk, rightOk, selected_four_input_modulo]

/-- Two positions in the source root-input list. This is source selection,
    not a claim about physical handle equality or Wasm aliasing. -/
def selectedRootFourInput (x y : Nat) (roots : Fin 2 → Nat)
    (left right : Fin 2) : Nat :=
  selectedFourInput x y (roots left) (roots right)

theorem selected_quotes_match_roots (x y : Nat)
    (left right : ScalarQuoteInput) (first second : ScalarQuoteResult)
    (roots : Fin 2 → Nat) (leftRoot rightRoot : Fin 2)
    (leftOk : directScalarQuote left = some first)
    (rightOk : directScalarQuote right = some second)
    (firstCapture : first.payloadBits = roots leftRoot)
    (secondCapture : second.payloadBits = roots rightRoot) :
    selectedQuotedFourInput x y left right =
      some (selectedRootFourInput x y roots leftRoot rightRoot) := by
  simp [selectedQuotedFourInput, leftOk, rightOk, selectedRootFourInput,
    firstCapture, secondCapture]

/-- These are *mathematical projections* of this one fixed recipe, not Noble
    digest values or a claim that a host's DefinitionId uses this encoding.
    Renaming a root preserves `shared`/`distinct`; duplicating one binding
    is not the same source graph as capturing two independent bindings. -/
inductive PureCaptureTopology where
  | shared
  | distinct
  deriving DecidableEq, Repr

structure PureDefinitionId where
  topology : PureCaptureTopology
  deriving DecidableEq, Repr

/-- Program-value data retains the definition shape *and* the captured I64
    bits modulo 2^64. Shared roots have one logical capture, distinct roots
    have two, even when their numerical values agree. -/
structure PureProgramValueId where
  definitionId : PureDefinitionId
  captures : List Nat
  deriving DecidableEq, Repr

def selectedDefinitionId (left right : Fin 2) : PureDefinitionId :=
  ⟨if left = right then .shared else .distinct⟩

def selectedCaptureValues (roots : Fin 2 → Nat)
    (left right : Fin 2) : List Nat :=
  if left = right then [roots left % i64Modulus]
  else [roots left % i64Modulus, roots right % i64Modulus]

def selectedProgramValueId (roots : Fin 2 → Nat)
    (left right : Fin 2) : PureProgramValueId :=
  ⟨selectedDefinitionId left right, selectedCaptureValues roots left right⟩

/-- Either physical root position may represent the same *logical* alias
    class; this equality does not depend on the captured bits. -/
theorem alias_definition_ignores_root_position (left right : Fin 2) :
    selectedDefinitionId left left = selectedDefinitionId right right := by
  simp [selectedDefinitionId]

/-- Under equal captures, moving a shared binding to another source position
    also preserves this pure program-value projection. -/
theorem alias_value_ignores_equal_root_renaming
    (roots : Fin 2 → Nat) (left right : Fin 2)
    (sameCapture : roots left = roots right) :
    selectedProgramValueId roots left left =
      selectedProgramValueId roots right right := by
  simp [selectedProgramValueId, selectedDefinitionId,
    selectedCaptureValues, sameCapture]

/-- Equal numeric outputs need not mean equal source alias graphs or IDs:
    `[v,v]` from one source binding has one logical capture; `[v,v]` from
    independent bindings has two. -/
theorem equal_values_do_not_erase_alias_class (x y value : Nat) :
    selectedRootFourInput x y (fun _ => value) 0 0 =
      selectedRootFourInput x y (fun _ => value) 1 0 ∧
    selectedDefinitionId 0 0 ≠ selectedDefinitionId 1 0 ∧
    selectedProgramValueId (fun _ => value) 0 0 ≠
      selectedProgramValueId (fun _ => value) 1 0 := by
  refine ⟨rfl, by decide, ?_⟩
  intro sameValueId
  exact (by decide : selectedDefinitionId 0 0 ≠ selectedDefinitionId 1 0)
    (congrArg PureProgramValueId.definitionId sameValueId)

/-- Conversely, changing the captured value preserves the definition shape
    but changes the program-value data, even when the shared source root has
    been renamed. No cryptographic hash collision property is asserted. -/
theorem changed_capture_preserves_definition_not_value (first second : Nat)
    (different : first % i64Modulus ≠ second % i64Modulus) :
    selectedDefinitionId 0 0 = selectedDefinitionId 1 1 ∧
    selectedProgramValueId (fun _ => first) 0 0 ≠
      selectedProgramValueId (fun _ => second) 1 1 := by
  refine ⟨rfl, ?_⟩
  intro sameValueId
  have sameCaptures :
      [first % i64Modulus] = [second % i64Modulus] := by
    simpa [selectedProgramValueId, selectedCaptureValues] using
      (congrArg PureProgramValueId.captures sameValueId)
  exact different (by simpa using sameCaptures)

end NobleLiveStorage
