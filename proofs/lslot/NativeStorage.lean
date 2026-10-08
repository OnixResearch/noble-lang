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

end NobleLiveStorage
