/-
Model of the token codec of the replayable rewrite certificate (issue #42, codec half).

`Certificate.lean` models the *format* of `ngraph-cert/0` derivations but says nothing about
transport. This file models a token codec for the same derivation and proves what the codec
does and does not buy:

* `encode` and `decode` are mutual inverses on derivations whose rule identities decode at
  all (`decode_encode_replays`), so acceptance is invariant under a transport round trip
  (`acceptance_preserved_by_codec`);
* every step occupies exactly seven tokens (`encodeStep_length`), so a short tail is refused
  rather than silently shortened, and the envelope carries the declared step count, so a
  chunk removed in transit refuses (`dropped_chunk_refused`) instead of shortening the
  derivation;
* a rule token outside the modelled set refuses decoding, so the codec cannot smuggle a rule
  the checker does not know past the envelope;
* the encoded revision and model tokens are exactly what the decoder re-checks, so a stale
  format or semantic-model token is refused at transport rather than at acceptance only.

What this is not: tokens are modelled as `String`s and no byte-level envelope is modelled at
all, so nothing here binds the `serde`/binary bytes the Rust encoder writes to these
structures; no statement covers envelope digests, replay-budget accounting or the identity
rules.
-/
import NobleFoundation.Certificate

namespace NobleFoundation.CertificateCodec

open NobleFoundation.Certificate

/-! ### Token codec -/

/-- Render one 64-bit operand as its decimal token. -/
def bitToken (value : BitVec 64) : String := Nat.repr value.toNat

/-- Render one numbered field as its decimal token. -/
def natToken (value : Nat) : String := Nat.repr value

/-- The value of a decimal token: at least one character and every character an ASCII digit.
A sign, a prefix, an underscore, a non-digit or an empty token has no value. Unlike the
standard library's `String.toNat?`, this parser is a plain structural recursion, so the
kernel can evaluate it on literal tokens. -/
def decimal (token : String) : Option Nat :=
  if token.toList ≠ [] ∧ token.toList.all Char.isDigit then
    some (Nat.ofDigitChars 10 token.toList 0)
  else none

/-- A rendered `Nat` is a decimal token of exactly its value, so the encoder cannot produce a
token the parser reads differently. -/
theorem decimal_repr (value : Nat) : decimal (Nat.repr value) = some value := by
  unfold decimal
  rw [Nat.repr_eq_ofList_toDigits, String.toList_ofList]
  have hne : (Nat.toDigits 10 value) ≠ [] := Nat.toDigits_ne_nil
  have hdigits : (Nat.toDigits 10 value).all Char.isDigit = true :=
    List.all_eq_true.mpr fun character member =>
      Nat.isDigit_of_mem_toDigits (by decide) (by decide) member
  rw [ite_eq_left ⟨hne, hdigits⟩, Nat.ofDigitChars_ten_toDigits]

/-- The tokens of one step, in wire order: rule, index, first, second, first bits, second
bits, claimed. Exactly seven tokens, never fewer. -/
def encodeStep (step : Step) : List String :=
  [natToken step.rule, natToken step.index, natToken step.first, natToken step.second,
    bitToken step.firstBits, bitToken step.secondBits, bitToken step.claimed]

/-- The step chunks of a whole derivation: one seven-token chunk per step, in wire order. -/
def encodeSteps (derivation : Derivation) : List (List String) :=
  derivation.steps.map encodeStep

/-- The modelled rule identities: an unknown or out-of-range rule token has no decoded claim. -/
def decodeRule (value : Nat) : Option Nat :=
  if value ≤ 2 then some value else none

/-- Decode one step from exactly seven tokens. A rule token outside the modelled set, or a
bit token outside the 64-bit range, refuses the whole step. Node and operand tokens are
modelled as `Nat`s: the Rust decoder additionally refuses an index that is not a `u32`,
which no graph of this model can name, and this file claims nothing about that bound. -/
def decodeStep (tokens : List String) : Option Step :=
  match tokens with
  | [rule, index, first, second, firstBits, secondBits, claimed] =>
      match (decimal rule).bind decodeRule, decimal index, decimal first, decimal second,
          decimal firstBits, decimal secondBits, decimal claimed with
      | some ruleValue, some indexValue, some firstValue, some secondValue,
          some firstBitsValue, some secondBitsValue, some claimedValue =>
          if h : firstBitsValue < 2 ^ 64 ∧ secondBitsValue < 2 ^ 64 ∧
              claimedValue < 2 ^ 64 then
            some ⟨ruleValue, indexValue, firstValue, secondValue,
              BitVec.ofNatLT firstBitsValue h.1,
              BitVec.ofNatLT secondBitsValue h.2.1,
              BitVec.ofNatLT claimedValue h.2.2⟩
          else none
      | _, _, _, _, _, _, _ => none
  | _ => none

/-- Decode one seven-token chunk per step, refusing the whole envelope as soon as any chunk
refuses. A short chunk list therefore refuses rather than shortening the derivation. -/
def decodeSteps : List (List String) → Option (List Step)
  | [] => some []
  | chunk :: rest =>
      match decodeStep chunk, decodeSteps rest with
      | some step, some tail => some (step :: tail)
      | _, _ => none

/-- Decode a whole envelope, re-checking the revision token, the model token and the declared
step count before any step is decoded. A chunk list whose length differs from the declared
count is refused, so dropping a chunk cannot shorten the derivation. -/
def decode (revisionToken modelToken : String) (declaredSteps : Nat)
    (chunks : List (List String)) : Option Derivation :=
  if revisionToken = revision then
    if modelToken = modelRevision then
      if chunks.length = declaredSteps then
        (decodeSteps chunks).map (fun steps => ⟨revisionToken, modelToken, steps⟩)
      else none
    else none
  else none

/-! ### The step round trip -/

/-- Decoding an encoded step whose rule identity is in the modelled set returns the same step:
the encoder cannot renumber a node, retag a rule or move a constant. -/
theorem decode_encode_step_replays (step : Step) (hrule : step.rule ≤ 2) :
    decodeStep (encodeStep step) = some step := by
  have hbounds : step.firstBits.toNat < 2 ^ 64 ∧ step.secondBits.toNat < 2 ^ 64 ∧
      step.claimed.toNat < 2 ^ 64 :=
    ⟨step.firstBits.isLt, step.secondBits.isLt, step.claimed.isLt⟩
  unfold decodeStep encodeStep bitToken natToken
  simp only [decimal_repr]
  have hdecoded : (some step.rule).bind decodeRule = some step.rule := by
    simp [decodeRule, hrule]
  rw [hdecoded]
  simp [hbounds]

/-- Decoding an encoded derivation returns the derivation itself. -/
theorem decode_encode_replays (derivation : Derivation) (hrev : derivation.revision = revision)
    (hmodel : derivation.model = modelRevision)
    (hall : ∀ step ∈ derivation.steps, step.rule ≤ 2) :
    decode derivation.revision derivation.model derivation.steps.length
        (encodeSteps derivation) = some derivation := by
  have hsteps : ∀ (steps : List Step), (∀ member ∈ steps, member.rule ≤ 2) →
      decodeSteps (steps.map encodeStep) = some steps := by
    intro steps hallSteps
    induction steps with
    | nil => rfl
    | cons step tail ih =>
        have hstep : step.rule ≤ 2 := hallSteps step List.mem_cons_self
        show decodeSteps (encodeStep step :: List.map encodeStep tail) = some (step :: tail)
        unfold decodeSteps
        rw [decode_encode_step_replays step hstep]
        rw [ih (fun member hmem => hallSteps member (List.mem_cons_of_mem step hmem))]
  have hcount : (encodeSteps derivation).length = derivation.steps.length := by
    simp [encodeSteps]
  unfold decode
  rw [ite_eq_left hrev, ite_eq_left hmodel, ite_eq_left hcount]
  rw [show encodeSteps derivation = derivation.steps.map encodeStep from rfl]
  rw [hsteps derivation.steps hall]
  rfl

/-! ### Refusals -/

/-- A step is exactly seven tokens. -/
theorem encodeStep_length (step : Step) : (encodeStep step).length = 7 := rfl

/-- A truncated step envelope is refused: six tokens are not a step. -/
theorem truncated_step_refused (rule index first second firstBits secondBits : String) :
    decodeStep [rule, index, first, second, firstBits, secondBits] = none := by
  simp [decodeStep]

/-- A rule token outside the modelled set refuses the step, so an unknown rule cannot travel
through the codec at all. -/
theorem unknown_rule_token_refused (rule index first second firstBits secondBits claimed : String)
    (ruleValue : Nat) (hdecode : decimal rule = some ruleValue) (hbound : ¬ ruleValue ≤ 2) :
    decodeStep [rule, index, first, second, firstBits, secondBits, claimed] = none := by
  have hrule : decodeRule ruleValue = none := by simp [decodeRule, hbound]
  simp [decodeStep, hdecode, hrule]

/-- A stale revision token refuses decoding before any step is inspected. -/
theorem stale_revision_token_refused (tokens : List (List String)) (declaredSteps : Nat)
    (token modelToken : String) (hstale : token ≠ revision) :
    decode token modelToken declaredSteps tokens = none := by
  simp [decode, hstale]

/-- A stale model token refuses decoding before any step is inspected: a certificate produced
for another semantic model does not silently travel as this one. -/
theorem stale_model_token_refused (tokens : List (List String)) (declaredSteps : Nat)
    (token : String) (hstale : token ≠ modelRevision) :
    decode revision token declaredSteps tokens = none := by
  simp [decode, hstale]

/-- A chunk list whose length differs from the declared step count refuses, whatever its
chunks contain. -/
theorem chunk_count_refused (token modelToken : String) (declaredSteps : Nat)
    (chunks : List (List String)) (hcount : chunks.length ≠ declaredSteps) :
    decode token modelToken declaredSteps chunks = none := by
  by_cases hrev : token = revision
  · rw [decode, ite_eq_left hrev]
    by_cases hmodel : modelToken = modelRevision
    · rw [ite_eq_left hmodel]
      simp [hcount]
    · rw [ite_eq_right hmodel]
  · rw [decode, ite_eq_right hrev]

/-- Dropping a chunk refuses rather than shortening the derivation: the encoded count is
the derivation's own step count, and a shorter chunk list disagrees with it. -/
theorem dropped_chunk_refused (derivation : Derivation) (chunks : List (List String))
    (hchunks : chunks.length < derivation.steps.length) :
    decode revision modelRevision derivation.steps.length chunks = none := by
  apply chunk_count_refused
  exact Nat.ne_of_lt hchunks

/-! ### Acceptance is invariant under transport -/

/-- A transport round trip preserves acceptance: the checker's verdict is about the
derivation, not about the envelope that carried it. -/
theorem acceptance_preserved_by_codec (graph : NobleFoundation.PackedSchedule.Graph)
    (derivation : Derivation) (hrev : derivation.revision = revision)
    (hmodel : derivation.model = modelRevision)
    (hall : ∀ step ∈ derivation.steps, step.rule ≤ 2) :
    (decode derivation.revision derivation.model derivation.steps.length
        (encodeSteps derivation)).map (accepted graph) =
      some (accepted graph derivation) := by
  rw [decode_encode_replays derivation hrev hmodel hall]
  rfl

end NobleFoundation.CertificateCodec
