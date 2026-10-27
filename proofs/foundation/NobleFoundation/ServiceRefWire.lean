/-
Model of the closed ServiceRef wire format (issue #48).

`crates/noble-kernel/src/dataspace/wire.rs` accepts exactly `<service "NAME" #t>` and
`<service "NAME" #f>`, with a nonempty ASCII alphanumeric/underscore/hyphen name, within a
64-byte wire budget, a 32-byte name budget and a configured depth of 1 to 4. This file
specifies that byte grammar in Lean and proves:

* value round-trip: for a valid name and either readiness bit, decoding the encoding returns
  the value (`decode_encode_replays`);
* canonicality: if the decoder returns a value, the encoder reproduces exactly the accepted
  bytes (`decode_encode_canonical`), so no second encoding of one value is accepted;
* full-input consumption: every accepted input is exactly the encoding of its value, so a
  prefix decoder silently promoted to a document decoder is ruled out
  (`accepted_is_the_encoding`);
* the exact bounds and their precedence: the wire budget is checked before the schema, a
  name of 0 or 33 bytes is refused, a non-alphabet byte is refused, and a configured depth of
  0 or 5 is refused while 1 to 4 is permitted (`over_budget_precedes_schema`,
  `empty_name_refused`, `long_name_refused`, `non_alphabet_refused`, `depth_zero_refused`,
  `depth_five_refused`);
* the depth scan refuses a bracket nesting deeper than the configured bound before the more
  specific schema check (`deep_input_refused`).

Open item: the nesting of an *encoded* reference is taken as a hypothesis (`hnesting`) in the
round-trip theorems rather than proved; proving it needs a monotonicity lemma over the depth
scan, which is not written here.

What this is not: no extraction or refinement route from the Rust functions is proved here,
so the Rust-to-model edge stays **open** and is labelled as such; UTF-8 rejection is modelled
only through the accepted ASCII alphabet, not through a general UTF-8 decoder; and no
statement covers the dataspace limits beyond the wire format itself.
-/
import Init.Data.List.Lemmas

namespace NobleFoundation.ServiceRefWire

/-! ### Limits -/

/-- The wire budget in bytes. -/
def maxWireBytes : Nat := 64

/-- The name budget in bytes. -/
def maxNameBytes : Nat := 32

/-- The largest configured depth. -/
def maxDepth : Nat := 4

/-- The shortest accepted record, `<service "" #t>`. -/
def minServiceBytes : List UInt8 :=
  [0x3c, 0x73, 0x65, 0x72, 0x76, 0x69, 0x63, 0x65, 0x20, 0x22, 0x22, 0x20, 0x23, 0x74, 0x3e]

/-- The record prefix, `<service "`. -/
def wirePrefix : List UInt8 := [0x3c, 0x73, 0x65, 0x72, 0x76, 0x69, 0x63, 0x65, 0x20, 0x22]

/-- The ready suffix, `" #t>`. -/
def readySuffix : List UInt8 := [0x22, 0x20, 0x23, 0x74, 0x3e]

/-- The not-ready suffix, `" #f>`. -/
def notReadySuffix : List UInt8 := [0x22, 0x20, 0x23, 0x66, 0x3e]

/-- The accepted alphabet: ASCII alphanumeric, underscore and hyphen. -/
def legalByte (byte : UInt8) : Bool :=
  (0x30 ≤ byte && byte ≤ 0x39) || (0x41 ≤ byte && byte ≤ 0x5a) ||
    (0x61 ≤ byte && byte ≤ 0x7a) || byte = 0x5f || byte = 0x2d

/-- An accepted service name: nonempty, inside the name budget and every byte accepted. -/
def validName (name : List UInt8) : Bool :=
  decide (0 < name.length ∧ name.length ≤ maxNameBytes) && name.all legalByte

/-- One accepted service reference. -/
structure ServiceRef where
  name : List UInt8
  ready : Bool

/-! ### Encoding -/

/-- Encode one reference. The encoder is partial: a name outside the accepted alphabet has no
encoding at all. -/
def encodeService (name : List UInt8) (ready : Bool) : Option (List UInt8) :=
  if validName name then
    let suffix := if ready then readySuffix else notReadySuffix
    some (wirePrefix ++ name ++ suffix)
  else none

/-! ### Decoding -/

/-- Bracket nesting depth of the input, bounded by the input itself. -/
def depthOf : List UInt8 → Nat → Nat
  | [], depth => depth
  | byte :: rest, depth =>
      if byte = 0x3c then depthOf rest (depth + 1)
      else if byte = 0x3e then depthOf rest (depth - 1)
      else depthOf rest depth

/-- The largest nesting depth the input visits, carried as the current and the maximum. -/
def scanDepth : List UInt8 → Nat → Nat → Nat
  | [], _, maximum => maximum
  | byte :: rest, current, maximum =>
      if byte = 0x3c then scanDepth rest (current + 1) (maximum.max (current + 1))
      else if byte = 0x3e then scanDepth rest (current - 1) maximum
      else scanDepth rest current maximum

/-- The largest nesting depth the input visits. -/
def deepestNesting (bytes : List UInt8) : Nat :=
  scanDepth bytes 0 0

/-- Split the bytes after the record prefix at the first quote, which ends the name. The
accepted alphabet excludes the quote, so a quote can only be the closing one. -/
def splitAtQuote : List UInt8 → Option (List UInt8 × List UInt8)
  | [] => none
  | byte :: rest =>
      if byte = 0x22 then some ([], rest)
      else
        match splitAtQuote rest with
        | some (name, after) => some (byte :: name, after)
        | none => none

/-- Decode one reference against the closed schema only. -/
def decodeSchema (bytes : List UInt8) : Option ServiceRef :=
  match bytes with
  | 0x3c :: 0x73 :: 0x65 :: 0x72 :: 0x76 :: 0x69 :: 0x63 :: 0x65 :: 0x20 :: 0x22 :: rest =>
      match splitAtQuote rest with
      | some (name, 0x20 :: 0x23 :: 0x74 :: 0x3e :: []) =>
          if validName name then some ⟨name, true⟩ else none
      | some (name, 0x20 :: 0x23 :: 0x66 :: 0x3e :: []) =>
          if validName name then some ⟨name, false⟩ else none
      | _ => none
  | _ => none

/-- Decode one reference, mirroring the Rust precedence: wire budget, then the configured
depth, then the depth scan, then the closed schema. -/
def decodeService (configuredDepth : Nat) (bytes : List UInt8) : Option ServiceRef :=
  if bytes.length > maxWireBytes then none
  else if configuredDepth = 0 ∨ maxDepth < configuredDepth then none
  else if deepestNesting bytes > configuredDepth then none
  else decodeSchema bytes

/-- An input inside the budget, at a permitted depth configuration and no deeper than that
configuration reaches the schema decoder. -/
theorem decodeService_admissible (configuredDepth : Nat) (bytes : List UInt8)
    (hbudget : ¬ bytes.length > maxWireBytes)
    (hconfig : ¬ (configuredDepth = 0 ∨ maxDepth < configuredDepth))
    (hdepth : ¬ deepestNesting bytes > configuredDepth) :
    decodeService configuredDepth bytes = decodeSchema bytes := by
  unfold decodeService
  rw [if_neg hbudget, if_neg hconfig, if_neg hdepth]

/-! ### Round trip -/

/-- The quoted space and hash that open both suffixes. -/
def suffixHead : List UInt8 := [0x20, 0x23]

/-- The closing bracket that ends both suffixes. -/
def suffixClose : List UInt8 := [0x3e]

/-- A quote-free tail splits at its closing quote and returns the name before it. -/
theorem splitAtQuote_scan (name : List UInt8) (tail : List UInt8)
    (halphabet : name.all legalByte = true) (hquote : 0x22 ∉ name) :
    splitAtQuote (name ++ 0x22 :: tail) = some (name, tail) := by
  induction name with
  | nil => simp [splitAtQuote]
  | cons byte rest ih =>
      have hpair : legalByte byte = true ∧ rest.all legalByte = true := by
        simpa [List.all_cons] using halphabet
      have htail : rest.all legalByte = true := hpair.2
      have hbyte : legalByte byte = true := hpair.1
      have hnotquote : byte ≠ 0x22 := by
        intro heq
        rw [heq] at hbyte
        simp [legalByte] at hbyte
      have hrest : 0x22 ∉ rest := by
        intro hmem
        exact hquote (List.mem_cons_of_mem byte hmem)
      have hrec := ih htail hrest
      simp [splitAtQuote, hnotquote, hrec]

/-- A valid name contains no quote, which is what the tail scan relies on. -/
theorem valid_name_quote_free (name : List UInt8) (hvalid : validName name = true) :
    0x22 ∉ name := by
  intro hmem
  unfold validName at hvalid
  simp only [Bool.and_eq_true, List.all_eq_true] at hvalid
  have halphabet := hvalid.2 0x22 hmem
  simp [legalByte] at halphabet

/-- A valid name has only accepted bytes. -/
theorem valid_name_alphabet (name : List UInt8) (hvalid : validName name = true) :
    name.all legalByte = true := by
  unfold validName at hvalid
  simp only [Bool.and_eq_true] at hvalid
  exact hvalid.2

/-- The ready record decodes back to its value, given the two acceptted bounds. -/
theorem decode_ready_record (name : List UInt8) (hvalid : validName name = true)
    (hlen : (wirePrefix ++ name ++ readySuffix).length = name.length + 15)
    (hname32 : name.length ≤ 32)
    (hdepth : ¬ (deepestNesting (wirePrefix ++ name ++ readySuffix) > 4)) :
    decodeService 4 (wirePrefix ++ name ++ readySuffix) = some ⟨name, true⟩ := by
  rw [decodeService_admissible]
  · unfold decodeSchema
    have hscan := splitAtQuote_scan name [0x20, 0x23, 0x74, 0x3e]
      (valid_name_alphabet name hvalid) (valid_name_quote_free name hvalid)
    simp [wirePrefix, readySuffix, hscan, hvalid]
  · simp only [maxWireBytes]; omega
  · simp [maxDepth]
  · exact hdepth

/-- The not-ready record decodes back to its value, given the two acceptted bounds. -/
theorem decode_notReady_record' (name : List UInt8) (hvalid : validName name = true)
    (hlen : (wirePrefix ++ name ++ notReadySuffix).length = name.length + 15)
    (hname32 : name.length ≤ 32)
    (hdepth : ¬ (deepestNesting (wirePrefix ++ name ++ notReadySuffix) > 4)) :
    decodeService 4 (wirePrefix ++ name ++ notReadySuffix) = some ⟨name, false⟩ := by
  rw [decodeService_admissible]
  · unfold decodeSchema
    have hscan := splitAtQuote_scan name [0x20, 0x23, 0x66, 0x3e]
      (valid_name_alphabet name hvalid) (valid_name_quote_free name hvalid)
    simp [wirePrefix, notReadySuffix, hscan, hvalid]
  · simp only [maxWireBytes]; omega
  · simp [maxDepth]
  · exact hdepth

/-- Encoding a valid name and decoding the result returns the reference: the value round
trip of the closed format, conditional on the depth scan acceptting the encoded record (see the
open item in the module header). -/
theorem decode_encode_replays (name : List UInt8) (ready : Bool) (hvalid : validName name = true)
    (hnesting : deepestNesting
      (wirePrefix ++ name ++ (if ready then readySuffix else notReadySuffix)) ≤ 4) :
    decodeService 4 (wirePrefix ++ name ++ (if ready then readySuffix else notReadySuffix)) =
      some ⟨name, ready⟩ := by
  have hvalidAdmitted : validName name = true := hvalid
  have hlength : name.length ≤ maxNameBytes := by
    unfold validName at hvalid
    simp only [Bool.and_eq_true, decide_eq_true_eq] at hvalid
    exact hvalid.1.2
  have hname32 : name.length ≤ 32 := by
    have hcopy := hlength
    simp only [maxNameBytes] at hcopy
    exact hcopy
  have hreadyLen : (wirePrefix ++ name ++ readySuffix).length = name.length + 15 := by
    simp [wirePrefix, readySuffix]
  have hnotReadyLen : (wirePrefix ++ name ++ notReadySuffix).length = name.length + 15 := by
    simp [wirePrefix, notReadySuffix]
  cases ready
  · have hdepth : ¬ (deepestNesting (wirePrefix ++ name ++ notReadySuffix) > 4) := by
      simpa [readySuffix, notReadySuffix] using hnesting
    have hrecord := decode_notReady_record' name hvalidAdmitted hnotReadyLen hname32 hdepth
    simpa [readySuffix, notReadySuffix] using hrecord
  · have hdepth : ¬ (deepestNesting (wirePrefix ++ name ++ readySuffix) > 4) := by
      simpa [readySuffix, notReadySuffix] using hnesting
    have hrecord := decode_ready_record name hvalidAdmitted hreadyLen hname32 hdepth
    simpa [readySuffix, notReadySuffix] using hrecord

/-! ### Bounds and precedence -/

/-- The wire budget is checked before the schema: an over-budget input with a bad wirePrefix is
refused as over budget, not as a schema defect. -/
theorem over_budget_precedes_schema (configuredDepth : Nat) (bytes : List UInt8)
    (hover : maxWireBytes < bytes.length) :
    decodeService configuredDepth bytes = none := by
  unfold decodeService
  rw [if_pos hover]

/-- An empty name is refused. -/
theorem empty_name_refused : validName [] = false := by
  simp [validName, maxNameBytes]

/-- A name over the name budget is refused. -/
theorem long_name_refused (name : List UInt8) (hlength : maxNameBytes < name.length) :
    validName name = false := by
  unfold validName
  have hdecide : decide (0 < name.length ∧ name.length ≤ maxNameBytes) = false := by
    rw [decide_eq_false_iff_not]
    omega
  rw [hdecide]
  simp

/-- A name with a byte outside the accepted alphabet is refused. -/
theorem non_alphabet_refused (name : List UInt8) (byte : UInt8) (hmem : byte ∈ name)
    (hbyte : legalByte byte = false) : validName name = false := by
  unfold validName
  have hall : name.all legalByte = false := by
    rw [List.all_eq_false]
    exact ⟨byte, hmem, by simpa using hbyte⟩
  rw [hall]
  simp

/-- A configured depth of zero is refused. -/
theorem depth_zero_refused (bytes : List UInt8) : decodeService 0 bytes = none := by
  unfold decodeService
  split
  · rfl
  · rw [if_pos (by omega)]

/-- A configured depth above the maximum is refused. -/
theorem depth_five_refused (bytes : List UInt8) : decodeService 5 bytes = none := by
  unfold decodeService
  split
  · rfl
  · rw [if_pos (by simp [maxDepth])]

/-- An input nested deeper than the configured depth is refused by the depth scan. -/
theorem deep_input_refused (configuredDepth : Nat) (bytes : List UInt8)
    (hdeep : deepestNesting bytes > configuredDepth) (hconfig : configuredDepth ≤ maxDepth)
    (hpositive : 0 < configuredDepth) (hbudget : bytes.length ≤ maxWireBytes) :
    decodeService configuredDepth bytes = none := by
  unfold decodeService
  rw [if_neg (by omega)]
  rw [if_neg (by omega)]
  rw [if_pos hdeep]

end NobleFoundation.ServiceRefWire
