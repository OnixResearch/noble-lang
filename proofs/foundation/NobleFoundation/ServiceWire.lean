/-
A kernel-checked reference for the closed ServiceRef wire format (issue #48).

The subject is the one admitted Preserves text record of the dataspace codec,
`<service "NAME" #t>` or `<service "NAME" #f>`, as specified by
`crates/noble-kernel/src/dataspace/wire.rs` at the boundary this model was
written against:

- the wire byte budget is 64 bytes (`MAX_WIRE_BYTES`);
- the configured depth is admitted only for `1 ..= 4` (`MAX_WIRE_DEPTH`);
- a name is 1..=32 bytes (`MAX_NAME_BYTES`) drawn from ASCII alphanumerics,
  `_` and `-`;
- the record admits no escaping, embedded values, extension tags or trailing
  bytes, so encoding is a function of the value and the decoder accepts one
  byte sequence per value.

Bytes are modelled as `List Nat` values below 256. The parser matches the record
shape by literal byte patterns and requires the closing suffix to consume the
rest of the input, so full-input consumption and canonicality are structural
rather than checked postconditions.

Decision precedence mirrors the Rust decoder: byte budget, then depth
configuration, then depth scan, then the closed-schema match (which subsumes the
UTF-8 and name-alphabet checks, because every admitted byte is ASCII). The model
does not claim the Rust code is a refinement of it; that edge stays open.
-/
namespace NobleFoundation.ServiceWire

set_option maxRecDepth 100000

/-- The largest admitted wire length in bytes. -/
def maxWireBytes : Nat := 64

/-- The largest admitted configured depth. -/
def maxWireDepth : Nat := 4

/-- The largest admitted name length in bytes. -/
def maxNameBytes : Nat := 32

/-- Every admitted name byte: an ASCII alphanumeric, `_` or `-`. -/
def nameByte (byte : Nat) : Bool :=
  (decide (48 ≤ byte) && decide (byte ≤ 57)) || (decide (65 ≤ byte) && decide (byte ≤ 90))
    || (decide (97 ≤ byte) && decide (byte ≤ 122)) || byte == 95 || byte == 45

/-- A name is admitted exactly when it is 1..=32 bytes of `nameByte`. -/
def validName (name : List Nat) : Bool :=
  decide (1 ≤ name.length) && decide (name.length ≤ maxNameBytes) && name.all nameByte

/-- No admitted name byte is the closing quote or an angle bracket. -/
theorem nameByte_excludes_structure (byte : Nat) (h : nameByte byte = true) :
    byte ≠ 34 ∧ byte ≠ 60 ∧ byte ≠ 62 := by
  unfold nameByte at h
  simp only [Bool.or_eq_true, Bool.and_eq_true, decide_eq_true_iff, beq_iff_eq] at h
  omega

/-- The ten-byte record prefix. -/
def recordPrefix : List Nat := [60, 115, 101, 114, 118, 105, 99, 101, 32, 34]

/-- The five-byte record suffix, which carries the readiness tag. -/
def recordSuffix (ready : Bool) : List Nat := [34, 32, 35, if ready then 116 else 102, 62]

/-- The canonical encoding of one admitted value. -/
def encode (name : List Nat) (ready : Bool) : List Nat := recordPrefix ++ name ++ recordSuffix ready

/-- One decoding decision. The error classes are semantic classes, not wording. -/
inductive Decision where
  | byteLimit
  | depthLimit
  | schema
  | accepted (name : List Nat) (ready : Bool)
deriving DecidableEq, Repr

/-- Collect bytes up to the closing quote; `none` when the quote never appears. -/
def takeName : List Nat → Option (List Nat × List Nat)
  | [] => none
  | byte :: rest =>
      if byte = 34 then some ([], rest) else
        match takeName rest with
        | none => none
        | some (name, tail) => some (byte :: name, tail)

/-- The number of opening brackets in a byte list. -/
def openBrackets (bytes : List Nat) : Nat := (bytes.filter (· = 60)).length

/-- Whether the input opens more brackets than the configured depth admits. -/
def depthExceeds (maxDepth : Nat) (bytes : List Nat) : Bool :=
  decide (maxDepth < openBrackets bytes)

/-- The closed-schema match. A trailing byte, an altered readiness tag, an
embedded value or an escape all fail here, because the suffix patterns consume
the whole remainder. -/
def parseService : List Nat → Option (List Nat × Bool)
  | 60 :: 115 :: 101 :: 114 :: 118 :: 105 :: 99 :: 101 :: 32 :: 34 :: rest =>
      match takeName rest with
      | none => none
      | some (name, tail) =>
          match tail with
          | [32, 35, 116, 62] => some (name, true)
          | [32, 35, 102, 62] => some (name, false)
          | _ => none
  | _ => none

/-- The decoder, in the precedence order the Rust decoder enforces. -/
def decode (bytes : List Nat) (configuredDepth : Nat) : Decision :=
  if bytes.length > maxWireBytes then .byteLimit
  else if ¬ (1 ≤ configuredDepth ∧ configuredDepth ≤ maxWireDepth) then .depthLimit
  else if depthExceeds configuredDepth bytes then .depthLimit
  else
    match parseService bytes with
    | none => .schema
    | some (name, ready) =>
        if encode name ready = bytes then
          if validName name then .accepted name ready else .schema
        else .schema

/-- The record prefix is nine bytes. -/
theorem recordPrefix_length : recordPrefix.length = 10 := rfl

/-- The record suffix is five bytes whatever the readiness tag is. -/
theorem recordSuffix_length (ready : Bool) : (recordSuffix ready).length = 5 := by
  unfold recordSuffix
  cases ready <;> rfl

/-- The exact wire length of a canonical encoding. -/
theorem encode_length (name : List Nat) (ready : Bool) :
    (encode name ready).length = 15 + name.length := by
  unfold encode
  rw [List.length_append, List.length_append, recordPrefix_length, recordSuffix_length]
  omega

/-- A list draws less than two opening brackets when its head is not one and its
tail has none. -/
theorem openBrackets_cons (head : Nat) (tail : List Nat) (hHead : head ≠ 60)
    (hTail : openBrackets tail = 0) : openBrackets (head :: tail) = 0 := by
  unfold openBrackets at hTail ⊢
  rw [List.filter_cons]
  simp only [decide_eq_true_eq]
  rw [if_neg hHead]
  exact hTail

/-- A name draw from the admitted alphabet opens no bracket. -/
theorem openBrackets_of_name (name : List Nat)
    (hAll : ∀ byte ∈ name, nameByte byte = true) : openBrackets name = 0 := by
  induction name with
  | nil => rfl
  | cons head tail ih =>
      have hHead : head ≠ 60 := (nameByte_excludes_structure head (hAll head (by simp))).2.1
      have hTail : ∀ byte ∈ tail, nameByte byte = true := by
        intro byte hmem
        exact hAll byte (List.mem_cons_of_mem head hmem)
      exact openBrackets_cons head tail hHead (ih hTail)

/-- A name carries no structural byte, so the canonical encoding opens exactly
one bracket: the record prefix's. -/
theorem depthExceeds_encode (name : List Nat) (ready : Bool) (configuredDepth : Nat)
    (hDepth : 1 ≤ configuredDepth)
    (hAll : ∀ byte ∈ name, nameByte byte = true) :
    depthExceeds configuredDepth (encode name ready) = false := by
  unfold depthExceeds
  rw [decide_eq_false_iff_not]
  intro hCount
  have hPrefix : openBrackets recordPrefix = 1 := rfl
  have hNoOpen : ∀ byte ∈ name, byte ≠ 60 := by
    intro byte hmem
    exact (nameByte_excludes_structure byte (hAll byte hmem)).2.1
  have hSuffix : openBrackets (recordSuffix ready) = 0 := by
    unfold recordSuffix openBrackets
    cases ready <;> rfl
  have hName : openBrackets name = 0 := openBrackets_of_name name hAll
  have hAllCount : openBrackets (encode name ready) = 1 := by
    unfold encode
    unfold openBrackets at hPrefix hName hSuffix ⊢
    rw [List.filter_append, List.filter_append]
    rw [List.length_append, List.length_append, hPrefix, hName, hSuffix]
  omega

/-- `takeName` succeeds on a name drawn from the admitted alphabet, and leaves
the suffix untouched. -/
theorem takeName_encode (name : List Nat) (tag : Nat)
    (hAll : ∀ byte ∈ name, nameByte byte = true) :
    takeName (name ++ [34, 32, 35, tag, 62]) = some (name, [32, 35, tag, 62]) := by
  induction name with
  | nil =>
      rw [List.nil_append]
      simp only [takeName]
      exact rfl
  | cons head tail ih =>
      have hHead := nameByte_excludes_structure head (hAll head (by simp))
      have hTail : ∀ byte ∈ tail, nameByte byte = true := by
        intro byte hmem
        exact hAll byte (List.mem_cons_of_mem head hmem)
      rw [List.cons_append, takeName, if_neg hHead.1, ih hTail]

/-- The parser recognizes one canonical encoding. -/
theorem parseService_encode (name : List Nat) (ready : Bool)
    (hAll : ∀ byte ∈ name, nameByte byte = true) :
    parseService (encode name ready) = some (name, ready) := by
  cases ready
  · unfold encode recordPrefix recordSuffix
    simp only [Bool.false_eq_true, List.cons_append, List.nil_append, reduceIte, parseService]
    rw [takeName_encode name 102 hAll]
    rfl
  · unfold encode recordPrefix recordSuffix
    simp only [List.cons_append, List.nil_append, reduceIte, parseService]
    rw [takeName_encode name 116 hAll]
    rfl

/-- Round trip: every admitted value decodes from its canonical encoding at an
admitted configured depth. -/
theorem decode_encode (name : List Nat) (ready : Bool) (configuredDepth : Nat)
    (hValid : validName name = true)
    (hDepthAdmitted : 1 ≤ configuredDepth ∧ configuredDepth ≤ maxWireDepth) :
    decode (encode name ready) configuredDepth = .accepted name ready := by
  unfold validName at hValid
  rw [Bool.and_eq_true, Bool.and_eq_true] at hValid
  obtain ⟨⟨hNonEmpty, hBound32⟩, hAll⟩ := hValid
  have hAll' : ∀ byte ∈ name, nameByte byte = true := by
    intro byte hmem
    exact List.all_eq_true.mp hAll byte hmem
  have hLengthOk : ¬ (encode name ready).length > maxWireBytes := by
    rw [encode_length]
    have hBound : name.length ≤ maxNameBytes := decide_eq_true_iff.mp hBound32
    unfold maxNameBytes at hBound
    unfold maxWireBytes
    omega
  have hScan := depthExceeds_encode name ready configuredDepth hDepthAdmitted.1 hAll'
  unfold decode
  rw [if_neg hLengthOk]
  rw [if_neg (by intro h; exact h hDepthAdmitted)]
  rw [hScan]
  rw [if_neg (by simp)]
  rw [parseService_encode name ready hAll']
  simp only [validName, hNonEmpty, hBound32, hAll, Bool.and_self, reduceIte]

/-- Canonicality: an accepted byte sequence is exactly the canonical encoding of
the value it decodes to, because the accept branch checks the canonical bytes and
the parser consumes the whole input. One value therefore has one accepted
encoding, and the trailer, suffix and name cases below show that leftover or
altered bytes are rejected rather than ignored. -/
theorem encode_of_accepted (bytes : List Nat) (configuredDepth : Nat)
    (name : List Nat) (ready : Bool)
    (hDecode : decode bytes configuredDepth = .accepted name ready) :
    encode name ready = bytes := by
  unfold decode at hDecode
  have hLen : ¬ bytes.length > maxWireBytes := by
    intro h
    rw [if_pos h] at hDecode
    exact absurd hDecode (by simp)
  have hDepth : ¬ ¬ (1 ≤ configuredDepth ∧ configuredDepth ≤ maxWireDepth) := by
    intro h
    rw [if_neg hLen, if_pos h] at hDecode
    exact absurd hDecode (by simp)
  have hScan : ¬ depthExceeds configuredDepth bytes := by
    intro h
    rw [if_neg hLen, if_neg hDepth, if_pos h] at hDecode
    exact absurd hDecode (by simp)
  rw [if_neg hLen, if_neg hDepth, if_neg hScan] at hDecode
  cases hParse : parseService bytes with
  | none =>
      rw [hParse] at hDecode
      exact absurd hDecode (by simp)
  | some parsed =>
      obtain ⟨parsedName, parsedReady⟩ := parsed
      rw [hParse] at hDecode
      dsimp only at hDecode
      by_cases hBytes : encode parsedName parsedReady = bytes
      · rw [if_pos hBytes] at hDecode
        by_cases hValid : validName parsedName = true
        · rw [if_pos hValid] at hDecode
          injection hDecode with hName hReady
          subst hName
          subst hReady
          exact hBytes
        · rw [if_neg hValid] at hDecode
          exact absurd hDecode (by simp)
      · rw [if_neg hBytes] at hDecode
        exact absurd hDecode (by simp)

/-- The longest admitted name still fits the wire budget. -/
theorem longest_name_fits_wire_budget (name : List Nat) (ready : Bool)
    (hValid : validName name = true) :
    (encode name ready).length ≤ maxWireBytes := by
  rw [encode_length]
  unfold validName at hValid
  rw [Bool.and_eq_true, Bool.and_eq_true] at hValid
  have hBound : name.length ≤ maxNameBytes := decide_eq_true_iff.mp hValid.1.2
  unfold maxNameBytes at hBound
  unfold maxWireBytes
  omega

/-- The byte budget is decided before the depth scan, so an oversized input is
reported as a byte-limit failure whatever its brackets say. -/
theorem byte_limit_precedes_depth_scan :
    decode (List.replicate 65 60) 4 = .byteLimit := by
  decide

/-- The configured depth is decided before the schema match, so an unadmitted
configuration rejects a well-formed record as a depth failure. -/
theorem depth_configuration_precedes_schema :
    decode (encode [99, 108, 111, 99, 107] true) 0 = .depthLimit
      ∧ decode (encode [99, 108, 111, 99, 107] true) 5 = .depthLimit := by
  decide

/-- The depth scan is decided before the schema match: five opening brackets
are a depth failure, not a schema failure. -/
theorem depth_scan_precedes_schema :
    decode [60, 60, 60, 60, 60, 115, 101, 114, 118, 105, 99, 101, 34, 120, 34, 32, 35, 116, 62,
        62, 62, 62, 62] 4
      = .depthLimit := by
  decide

/-- A readiness tag other than `#t`/`#f` is a schema failure. -/
theorem altered_readiness_is_schema :
    decode [60, 115, 101, 114, 118, 105, 99, 101, 34, 99, 108, 111, 99, 107, 34, 32, 35, 120, 62] 4
      = .schema := by
  decide

/-- Trailing bytes are rejected, so the decoder is a document decoder and not a
prefix decoder. -/
theorem trailing_bytes_are_schema :
    decode (encode [99, 108, 111, 99, 107] true ++ [98]) 4 = .schema := by
  decide

/-- An empty name is rejected although the rest of the record is well formed. -/
theorem empty_name_is_schema :
    decode [60, 115, 101, 114, 118, 105, 99, 101, 34, 34, 32, 35, 116, 62] 4 = .schema := by
  decide

/-- A name byte outside the admitted alphabet is rejected, which is the model's
form of both the non-ASCII and the wrong-alphabet rejections. -/
theorem foreign_name_byte_is_schema :
    decode [60, 115, 101, 114, 118, 105, 99, 101, 34, 99, 255, 34, 32, 35, 116, 62] 4 = .schema := by
  decide

/-- A 33-byte name exceeds the name budget. -/
theorem overlong_name_is_rejected :
    validName (List.replicate 33 97) = false := by
  decide

/-- The longest admitted name is admitted and round-trips, which is the name
bound's exact accepting edge. -/
theorem longest_name_round_trips :
    decode (encode (List.replicate 32 97) false) 4
      = .accepted (List.replicate 32 97) false := by
  apply decode_encode
  · decide
  · unfold maxWireDepth
    omega

/-- The shortest admitted name round-trips. -/
theorem shortest_name_round_trips :
    decode (encode [97] true) 4 = .accepted [97] true := by
  apply decode_encode
  · decide
  · unfold maxWireDepth
    omega

end NobleFoundation.ServiceWire
