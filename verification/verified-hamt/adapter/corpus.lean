module

public import Environment

public section

/-!
# Independent corpus and controls for the VerifiedHAMT adapter (issue #60)

Every expected value here is written **literally from the reference rule** — `entries.find?` with
`entry.slot == slot` — not produced by the adapter's conversion or by the map's hashing. The adapter
must agree with these values, so the corpus is an oracle rather than a second copy of the adapter.

The corpus covers empty, singleton, absent-key, repeated identical slot, repeated slot with different
provenance and body, sparse slots, collision-heavy slots and repeated lookups.
-/

namespace NobleVerifiedHamtAdapter

/-- Build one entry. A named constructor keeps every field's position explicit, so a substituted
provenance or interface list is a different entry rather than a reordered one. -/
def entry (slot provenance rank : Nat) (interfaces body dependencies : List Nat) : Entry :=
  ⟨slot, provenance, interfaces, body, dependencies, rank⟩

/-- The reference environment used throughout: sparse slots with distinct fields. -/
def sparse : Environment :=
  [
    entry 7 1 4 [1, 2] [100] [],
    entry 3 2 5 [3] [200] [7],
    entry 512 3 6 [1, 2, 3] [300] [7, 3],
    entry 65 4 7 [9] [400] [],
  ]

/-- A duplicate slot with a **different** provenance and body: the first occurrence is what Noble's
first-match lookup returns. -/
def repeatedDifferent : Environment :=
  [entry 7 11 4 [1, 2] [100] [], entry 3 12 5 [3] [200] [7], entry 7 13 8 [1, 2, 9] [999] [7]]

/-- A duplicate slot with the very same entry twice: first-wins and last-wins agree here, so the
corpus separates "duplicates" from "different duplicates". -/
def repeatedIdentical : Environment :=
  [entry 7 1 4 [1, 2] [100] [], entry 7 1 4 [1, 2] [100] []]

/-- Collision-heavy: many slots built from the same small residue family. -/
def collisionHeavy : Environment :=
  (List.range 64).map (fun index => entry (index * 32 + 1) index index [index] [index] [])

/-- The last-write-wins conversion the bundled constructor performs, kept **only** to show that the
duplicate fixture detects accidentally choosing it. -/
def lastWins (environment : Environment) : Option Entry :=
  environment.foldl (fun found entry => if entry.slot == 7 then some entry else found) none

/-! ## Expected values, written literally from the reference rule -/

/-- Empty: nothing is there, and that is an answer rather than a default. -/
example : lookupEntry [] 7 = none := by decide

/-- Singleton hit and miss. -/
example : lookupEntry [entry 7 1 4 [1, 2] [100] []] 7 = some (entry 7 1 4 [1, 2] [100] []) := by
  decide

example : lookupEntry [entry 7 1 4 [1, 2] [100] []] 8 = none := by decide

/-- Absent key inside a populated environment. -/
example : lookupEntry sparse 99 = none := by decide

/-- Sparse and collision-heavy slots: hits return the complete entry. -/
example : lookupEntry sparse 512 = some (entry 512 3 6 [1, 2, 3] [300] [7, 3]) := by decide

example : lookupEntry collisionHeavy 1953 = some (entry 1953 61 61 [61] [61] []) := by decide

/-! ## The adapter agrees with those literal values -/

example : (toMap sparse).find? 512 = some (entry 512 3 6 [1, 2, 3] [300] [7, 3]) := by
  rw [lookup_first_match]
  decide

example : (toMap sparse).find? 99 = none := by
  rw [lookup_first_match]
  decide

example : (toMap collisionHeavy).find? 1953 = some (entry 1953 61 61 [61] [61] []) := by
  rw [lookup_first_match]
  decide

example : (toMap ([] : Environment)).find? 7 = none := by
  rw [lookup_first_match]
  decide

/-! ## Duplicate-slot control: the first entry wins, the last one does not -/

example : lookupEntry repeatedDifferent 7 = some (entry 7 11 4 [1, 2] [100] []) := by decide

example : (toMap repeatedDifferent).find? 7 = some (entry 7 11 4 [1, 2] [100] []) := by
  rw [lookup_first_match]
  decide

example : lastWins repeatedDifferent = some (entry 7 13 8 [1, 2, 9] [999] [7]) := by decide

/-- The control bites: the first-wins answer and the last-wins answer are different entries, so a
conversion that silently chose the last occurrence would fail the corpus above rather than pass on
the slot alone. -/
example : lookupEntry repeatedDifferent 7 ≠ lastWins repeatedDifferent := by decide

/-- Identical duplicates: both policies agree, which is why the different-duplicate fixture is the one
that carries the control. -/
example : lookupEntry repeatedIdentical 7 = some (entry 7 1 4 [1, 2] [100] []) := by decide

example : lastWins repeatedIdentical = some (entry 7 1 4 [1, 2] [100] []) := by decide

/-! ## Substituted fields fail the relation -/

/-- Same slot, substituted provenance and interfaces: the complete-entry comparison refuses it, so
membership in the index alone never authorizes a call. -/
example : lookupEntry sparse 7 ≠ some (entry 7 99 4 [1, 2] [100] []) := by decide

example : lookupEntry sparse 7 ≠ some (entry 7 1 4 [4, 2] [100] []) := by decide

/-- A dropped entry is the same as an absent key: the index reports absence, not a stale entry. -/
example : lookupEntry (sparse.drop 1) 7 = none := by decide

/-! ## Repeated lookups are stable -/

example : (toMap repeatedDifferent).find? 7 =
    some (entry 7 11 4 [1, 2] [100] []) := by
  rw [lookup_first_match]
  decide

end NobleVerifiedHamtAdapter
