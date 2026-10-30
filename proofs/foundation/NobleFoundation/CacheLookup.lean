/-
Model of a collision-safe cache lookup and the byte encoding used for subject keys.

The lookup model keeps the manifest in each entry in addition to an index key. The
index may collide, but a hit still requires exact manifest equality and verification.
The encoding model uses the fixed subject domain tag followed by length-prefixed byte
parts, so concatenation alone is not treated as a key format.
-/
import Std

namespace NobleFoundation.CacheLookup

/-! ## Model A: verified cache entries -/

/-- One stored cache entry. The key is intentionally separate from the stored manifest. -/
structure Entry (M K C : Type) where
  key : K
  storedManifest : M
  content : C
  verified : Bool

/-- A cache store is an ordered list of entries. -/
abbrev Store (M K C : Type) := List (Entry M K C)

/-- Search for the first entry whose index, manifest and verification bit all match. -/
def lookup [DecidableEq M] [DecidableEq K] (idx : M → K) :
    Store M K C → M → Option C
  | [], _ => none
  | entry :: rest, request =>
      if entry.key = idx request then
        if entry.storedManifest = request then
          if entry.verified then some entry.content else lookup idx rest request
        else lookup idx rest request
      else lookup idx rest request

/-- Removing one position from a list, without requiring equality on its elements. -/
def eraseAt (position : Nat) : List α → List α
  | [] => []
  | entry :: rest =>
      match position with
      | 0 => rest
      | position + 1 => entry :: eraseAt position rest

/-- A successful lookup has a stored entry carrying the requested manifest and content. -/
theorem lookup_some_has_valid_entry [DecidableEq M] [DecidableEq K]
    (idx : M → K) (store : Store M K C) (request : M) (content : C)
    (h : lookup idx store request = some content) :
    ∃ entry : Entry M K C, entry ∈ store ∧ entry.key = idx request ∧
      entry.storedManifest = request ∧ entry.verified = true ∧ entry.content = content := by
  induction store with
  | nil => simp [lookup] at h
  | cons entry rest ih =>
      by_cases hkey : entry.key = idx request
      · by_cases hmanifest : entry.storedManifest = request
        · by_cases hver : entry.verified
          · simp [lookup, hkey, hmanifest, hver] at h
            exact ⟨entry, by simp, hkey, hmanifest, by simpa using hver, h⟩
          · simp [lookup, hkey, hmanifest, hver] at h
            rcases ih h with ⟨found, hmem, hfoundKey, hfoundManifest, hfoundVerified, hfoundContent⟩
            exact ⟨found, by simp [hmem], hfoundKey, hfoundManifest, hfoundVerified, hfoundContent⟩
        · simp [lookup, hkey, hmanifest] at h
          rcases ih h with ⟨found, hmem, hfoundKey, hfoundManifest, hfoundVerified, hfoundContent⟩
          exact ⟨found, by simp [hmem], hfoundKey, hfoundManifest, hfoundVerified, hfoundContent⟩
      · simp [lookup, hkey] at h
        rcases ih h with ⟨found, hmem, hfoundKey, hfoundManifest, hfoundVerified, hfoundContent⟩
        exact ⟨found, by simp [hmem], hfoundKey, hfoundManifest, hfoundVerified, hfoundContent⟩

/-- Any successful lookup carries the exact requested manifest, even when indexes collide. -/
theorem lookup_some_implies_manifest_eq [DecidableEq M] [DecidableEq K]
    (idx : M → K) (store : Store M K C) (request : M) (content : C)
    (h : lookup idx store request = some content) :
    ∃ entry : Entry M K C, entry ∈ store ∧ entry.storedManifest = request := by
  rcases lookup_some_has_valid_entry idx store request content h with
    ⟨entry, hmem, _hkey, hmanifest, _hverified, _hcontent⟩
  exact ⟨entry, hmem, hmanifest⟩

/-- A hit in a tail remains a hit in the store after an entry is put in front. -/
theorem lookup_hit_cons [DecidableEq M] [DecidableEq K]
    (idx : M → K) (entry : Entry M K C) (rest : Store M K C) (request : M)
    (content : C) (h : lookup idx rest request = some content) :
    ∃ result, lookup idx (entry :: rest) request = some result := by
  by_cases hkey : entry.key = idx request
  · by_cases hmanifest : entry.storedManifest = request
    · by_cases hver : entry.verified
      · exact ⟨entry.content, by simp [lookup, hkey, hmanifest, hver]⟩
      · exact ⟨content, by simp [lookup, hkey, hmanifest, hver, h]⟩
    · exact ⟨content, by simp [lookup, hkey, hmanifest, h]⟩
  · exact ⟨content, by simp [lookup, hkey, h]⟩

/-- A hit in a nonempty store is either the head entry or a hit in its tail. -/
theorem lookup_cons_hit_cases [DecidableEq M] [DecidableEq K]
    (idx : M → K) (entry : Entry M K C) (rest : Store M K C) (request : M)
    (content : C) (h : lookup idx (entry :: rest) request = some content) :
    (entry.key = idx request ∧ entry.storedManifest = request ∧ entry.verified = true ∧
        entry.content = content) ∨ lookup idx rest request = some content := by
  by_cases hkey : entry.key = idx request
  · by_cases hmanifest : entry.storedManifest = request
    · by_cases hver : entry.verified
      · left
        simp [lookup, hkey, hmanifest, hver] at h
        exact ⟨hkey, hmanifest, by simpa using hver, h⟩
      · right
        simpa [lookup, hkey, hmanifest, hver] using h
    · right
      simpa [lookup, hkey, hmanifest] using h
  · right
    simpa [lookup, hkey] using h

/-- Erasing entries cannot create a hit that was absent before the erasure. -/
theorem lookup_after_erase_subset [DecidableEq M] [DecidableEq K]
    (idx : M → K) (store : Store M K C) (request : M) (position : Nat) (content : C)
    (h : lookup idx (eraseAt position store) request = some content) :
    ∃ result, lookup idx store request = some result := by
  induction store generalizing position with
  | nil => simp [eraseAt, lookup] at h
  | cons entry rest ih =>
      cases position with
      | zero => exact lookup_hit_cons idx entry rest request content h
      | succ position =>
          rcases lookup_cons_hit_cases idx entry (eraseAt position rest) request content h with
            hhead | htail
          · rcases hhead with ⟨hkey, hmanifest, hver, hcontent⟩
            exact ⟨content, by simp [lookup, hkey, hmanifest, hver, hcontent]⟩
          · rcases ih position htail with ⟨tailContent, tailHit⟩
            exact lookup_hit_cons idx entry rest request tailContent tailHit

/-! ## Model A fixtures -/

/-- A small manifest used to exercise equal indexes with unequal manifests. -/
structure FixtureManifest where
  version : Nat
deriving DecidableEq, Repr

/-- The fixture index intentionally maps every manifest to the same key. -/
def collidingIndex (_ : FixtureManifest) : Nat := 17

/-- First fixture manifest. -/
def fixtureManifestOne : FixtureManifest := { version := 1 }

/-- Second fixture manifest. -/
def fixtureManifestTwo : FixtureManifest := { version := 2 }

/-- The fixture manifests are distinct. -/
theorem colliding_manifests_distinct : fixtureManifestOne ≠ fixtureManifestTwo := by
  decide

/-- The fixture index maps the two distinct manifests to one key. -/
theorem colliding_index_equal :
    collidingIndex fixtureManifestOne = collidingIndex fixtureManifestTwo := by
  rfl

/-- A present entry for one colliding manifest does not satisfy a request for the other. -/
theorem colliding_index_wrong_manifest_misses :
    lookup collidingIndex
        [{ key := collidingIndex fixtureManifestOne,
           storedManifest := fixtureManifestOne, content := 41, verified := true }]
        fixtureManifestTwo = none := by
  decide

/-- A stale entry with the matching request key is refused by exact manifest comparison. -/
theorem stale_manifest_entry_refused :
    lookup collidingIndex
        [{ key := collidingIndex fixtureManifestTwo,
           storedManifest := fixtureManifestOne, content := 41, verified := true }]
        fixtureManifestTwo = none := by
  decide

/-- A matching key and manifest are still refused when verification is false. -/
theorem corrupt_entry_refused :
    lookup collidingIndex
        [{ key := collidingIndex fixtureManifestTwo,
           storedManifest := fixtureManifestTwo, content := 41, verified := false }]
        fixtureManifestTwo = none := by
  decide

/-- Erasing the only entry removes a hit and does not manufacture one. -/
theorem erase_hit_disappears :
    lookup collidingIndex
        (eraseAt 0
          [{ key := collidingIndex fixtureManifestOne,
             storedManifest := fixtureManifestOne, content := 41, verified := true }])
        fixtureManifestOne = none := by
  decide

/-! ## Model B: length-prefixed subject parts -/

/-- The domain separator used by `Subject::derive`. -/
def subjectDomainTag : List UInt8 :=
  [110, 111, 98, 108, 101, 46, 110, 103, 114, 97, 112, 104, 46, 115, 117, 98, 106, 101, 99,
    116, 46, 118, 48]

/-- One byte of a little-endian 64-bit length. -/
def lengthByte (length position : Nat) : UInt8 :=
  UInt8.ofNat ((length / 256 ^ position) % 256)

/-- The eight-byte little-endian length prefix. -/
def lengthBytes (length : Nat) : List UInt8 :=
  [lengthByte length 0, lengthByte length 1, lengthByte length 2, lengthByte length 3,
    lengthByte length 4, lengthByte length 5, lengthByte length 6, lengthByte length 7]

/-- The payload encoding, before the fixed domain separator. -/
def encodePayload : List (List UInt8) → List UInt8
  | [] => []
  | part :: rest => lengthBytes part.length ++ part ++ encodePayload rest

/-- Subject parts encoded with the same domain tag and length prefixes as `Subject::derive`. -/
def encodeParts (parts : List (List UInt8)) : List UInt8 :=
  subjectDomainTag ++ encodePayload parts

/-- Length prefixes always occupy eight bytes. -/
theorem lengthBytes_length (length : Nat) : (lengthBytes length).length = 8 := by
  rfl

/-- The first eight bytes of one payload entry are exactly its length prefix. -/
theorem take_lengthBytes (length : Nat) (rest : List UInt8) :
    (lengthBytes length ++ rest).take 8 = lengthBytes length := by
  have h := List.take_left (l₁ := lengthBytes length) (l₂ := rest)
  rw [lengthBytes_length] at h
  exact h

/-- Dropping one eight-byte prefix exposes the rest of the entry. -/
theorem drop_lengthBytes (length : Nat) (rest : List UInt8) :
    (lengthBytes length ++ rest).drop 8 = rest := by
  calc
    (lengthBytes length ++ rest).drop 8 =
        (lengthBytes length ++ rest).drop (lengthBytes length).length := by
          rw [lengthBytes_length]
    _ = rest := by rw [List.drop_append, List.drop_length]; simp

/-- Under the 64-bit bound, the eight little-endian bytes determine the length. -/
theorem lengthBytes_injective_of_lt {length₁ length₂ : Nat}
    (h₁ : length₁ < 2 ^ 64) (h₂ : length₂ < 2 ^ 64)
    (h : lengthBytes length₁ = lengthBytes length₂) : length₁ = length₂ := by
  have h0 := congrArg (fun xs => xs[0]?) h
  have h1 := congrArg (fun xs => xs[1]?) h
  have h2 := congrArg (fun xs => xs[2]?) h
  have h3 := congrArg (fun xs => xs[3]?) h
  have h4 := congrArg (fun xs => xs[4]?) h
  have h5 := congrArg (fun xs => xs[5]?) h
  have h6 := congrArg (fun xs => xs[6]?) h
  have h7 := congrArg (fun xs => xs[7]?) h
  simp [lengthBytes] at h0 h1 h2 h3 h4 h5 h6 h7
  have h0' := congrArg UInt8.toNat h0
  have h1' := congrArg UInt8.toNat h1
  have h2' := congrArg UInt8.toNat h2
  have h3' := congrArg UInt8.toNat h3
  have h4' := congrArg UInt8.toNat h4
  have h5' := congrArg UInt8.toNat h5
  have h6' := congrArg UInt8.toNat h6
  have h7' := congrArg UInt8.toNat h7
  simp [lengthByte] at h0' h1' h2' h3' h4' h5' h6' h7'
  omega

/-- Equal lists with a common prefix have equal suffixes. -/
theorem append_prefix_cancel {α : Type} (pre left right : List α)
    (h : pre ++ left = pre ++ right) : left = right := by
  induction pre with
  | nil => exact h
  | cons head tail ih =>
      exact ih (List.cons.inj h).2

/-- The fixed tag cancels from equal subject encodings. -/
theorem encodeParts_payload_eq {parts₁ parts₂ : List (List UInt8)}
    (h : encodeParts parts₁ = encodeParts parts₂) :
    encodePayload parts₁ = encodePayload parts₂ := by
  exact append_prefix_cancel subjectDomainTag (encodePayload parts₁) (encodePayload parts₂) h

/-- Equal payloads with bounded parts have equal part lists. -/
theorem encodePayload_injective_of_lt {parts₁ parts₂ : List (List UInt8)}
    (h₁ : ∀ part ∈ parts₁, part.length < 2 ^ 64)
    (h₂ : ∀ part ∈ parts₂, part.length < 2 ^ 64)
    (h : encodePayload parts₁ = encodePayload parts₂) : parts₁ = parts₂ := by
  induction parts₁ generalizing parts₂ with
  | nil =>
      cases parts₂ with
      | nil => rfl
      | cons part rest =>
          simp [encodePayload, lengthBytes] at h
  | cons part₁ rest₁ ih =>
      cases parts₂ with
      | nil =>
          simp [encodePayload, lengthBytes] at h
      | cons part₂ rest₂ =>
          simp only [encodePayload] at h
          have hhead : lengthBytes part₁.length = lengthBytes part₂.length := by
            calc
              lengthBytes part₁.length =
                  (lengthBytes part₁.length ++ part₁ ++ encodePayload rest₁).take 8 := by
                    symm
                    simpa [List.append_assoc] using
                      (take_lengthBytes part₁.length (part₁ ++ encodePayload rest₁))
              _ = (lengthBytes part₂.length ++ part₂ ++ encodePayload rest₂).take 8 :=
                congrArg (List.take 8) h
              _ = lengthBytes part₂.length := by
                simpa [List.append_assoc] using
                  (take_lengthBytes part₂.length (part₂ ++ encodePayload rest₂))
          have hlen : part₁.length = part₂.length :=
            lengthBytes_injective_of_lt (h₁ part₁ (by simp)) (h₂ part₂ (by simp)) hhead
          have htail : part₁ ++ encodePayload rest₁ = part₂ ++ encodePayload rest₂ := by
            calc
              part₁ ++ encodePayload rest₁ =
                  (lengthBytes part₁.length ++ part₁ ++ encodePayload rest₁).drop 8 := by
                    symm
                    simpa [List.append_assoc] using
                      (drop_lengthBytes part₁.length (part₁ ++ encodePayload rest₁))
              _ = (lengthBytes part₂.length ++ part₂ ++ encodePayload rest₂).drop 8 :=
                congrArg (List.drop 8) h
              _ = part₂ ++ encodePayload rest₂ := by
                simpa [List.append_assoc] using
                  (drop_lengthBytes part₂.length (part₂ ++ encodePayload rest₂))
          have hparts := List.append_inj htail hlen
          have hrest : encodePayload rest₁ = encodePayload rest₂ := hparts.2
          have hrest₁ : ∀ part ∈ rest₁, part.length < 2 ^ 64 := by
            intro part hmem
            exact h₁ part (by simp [hmem])
          have hrest₂ : ∀ part ∈ rest₂, part.length < 2 ^ 64 := by
            intro part hmem
            exact h₂ part (by simp [hmem])
          have hrec := ih hrest₁ hrest₂ hrest
          simp [hparts.1, hrec]

/-- Length-prefixed subject encoding is injective when every part is shorter than 2^64. -/
theorem encodeParts_injective_of_lt {parts₁ parts₂ : List (List UInt8)}
    (h₁ : ∀ part ∈ parts₁, part.length < 2 ^ 64)
    (h₂ : ∀ part ∈ parts₂, part.length < 2 ^ 64)
    (h : encodeParts parts₁ = encodeParts parts₂) : parts₁ = parts₂ := by
  exact encodePayload_injective_of_lt h₁ h₂ (encodeParts_payload_eq h)

/-- A changed field changes the length-prefixed encoding. -/
theorem changed_field_changes_encoding :
    encodeParts [[1], [2, 3]] ≠ encodeParts [[1, 2], [3]] := by
  decide

/-- Plain concatenation loses field boundaries on the same two inputs. -/
def naiveConcat (parts : List (List UInt8)) : List UInt8 := parts.flatten

theorem naive_concat_field_collision :
    naiveConcat [[1], [2, 3]] = naiveConcat [[1, 2], [3]] := by
  decide

end NobleFoundation.CacheLookup
