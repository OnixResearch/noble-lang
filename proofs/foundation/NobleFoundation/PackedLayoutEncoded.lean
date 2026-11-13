/-
Finite, encoded packed-layout validation (issue #80, unit 1).

The model uses finite lists for node, layer, register, constant and slot tables. Each natural
number uses a self-delimiting unary code; each 64-bit constant uses exactly 64 bits. This is an
explicit model encoding, not the Rust serde byte format. Validation delegates to the existing
PackedLayout obligations after total decoding.
-/
import Init.Data.List.Lemmas
import NobleFoundation.PackedLayout

namespace NobleFoundation.PackedLayoutEncoded

open NobleFoundation.PackedSchedule

/-- The finite declared layout data carried by the encoding. -/
structure Tables where
  revisionOk : Bool
  regs : Nat
  registers : List (Option Nat)
  consts : List (Nat × BitVec 64)
  slotRegs : List (Nat × Nat)
  layers : List (Nat × Nat)
  failSuffixMin : List Nat

/-- A finite graph, its checking metadata, and the declared layout tables. -/
structure Input where
  nodes : List Node
  layerCount : Nat
  failNodes : List Node
  layerOf : List Nat
  tables : Tables

/-- Self-delimiting unary natural encoding, terminated by `false`. -/
def encodeNat : Nat → List Bool
  | 0 => [false]
  | n + 1 => true :: encodeNat n

/-- Read a unary natural and preserve the unconsumed suffix. -/
def decodeNatPrefix : List Bool → Option (Nat × List Bool)
  | [] => none
  | false :: rest => some (0, rest)
  | true :: rest => (decodeNatPrefix rest).map fun (value, tail) => (value + 1, tail)
termination_by bits => bits.length

def encodeBool (value : Bool) : List Bool := [value]

def decodeBoolPrefix : List Bool → Option (Bool × List Bool)
  | [] => none
  | value :: rest => some (value, rest)

/-- Fixed-width, little-endian bits for a bounded natural. -/
def encodeFixedBits : Nat → Nat → List Bool
  | 0, _ => []
  | width + 1, value => (value % 2 == 1) :: encodeFixedBits width (value / 2)

/-- Read exactly `width` little-endian bits as a natural. -/
def decodeFixedBitsPrefix : Nat → List Bool → Option (Nat × List Bool)
  | 0, bits => some (0, bits)
  | width + 1, bit :: rest =>
      (decodeFixedBitsPrefix width rest).map fun (value, tail) =>
        ((if bit then 1 else 0) + 2 * value, tail)
  | _, [] => none
termination_by width bits => width + bits.length

def encodeWord (value : BitVec 64) : List Bool := encodeFixedBits 64 value.toNat

def decodeWordPrefix (bits : List Bool) : Option (BitVec 64 × List Bool) :=
  match decodeFixedBitsPrefix 64 bits with
  | some (value, rest) =>
      if value < 2 ^ 64 then some (BitVec.ofNat 64 value, rest) else none
  | none => none

/-- Prefix a list's elements with its unary length. -/
def encodeList (encodeElement : α → List Bool) (values : List α) : List Bool :=
  encodeNat values.length ++ values.flatMap encodeElement

def decodeListFrom (decodeElement : List Bool → Option (α × List Bool)) : Nat →
    List Bool → Option (List α × List Bool)
  | 0, bits => some ([], bits)
  | count + 1, bits => do
      let (value, rest) ← decodeElement bits
      let (values, tail) ← decodeListFrom decodeElement count rest
      some (value :: values, tail)
termination_by count _ => count

def decodeListPrefix (decodeElement : List Bool → Option (α × List Bool))
    (bits : List Bool) : Option (List α × List Bool) := do
  let (count, rest) ← decodeNatPrefix bits
  decodeListFrom decodeElement count rest

/-- Constructor tags make node alternatives disjoint. -/
def encodeNode : Node → List Bool
  | .input slot => encodeNat 0 ++ encodeNat slot
  | .constant bits => encodeNat 1 ++ encodeWord bits
  | .add first second => encodeNat 2 ++ encodeNat first ++ encodeNat second
  | .sub first second => encodeNat 3 ++ encodeNat first ++ encodeNat second
  | .mul first second => encodeNat 4 ++ encodeNat first ++ encodeNat second
  | .eq first second => encodeNat 5 ++ encodeNat first ++ encodeNat second

def decodeNodePrefix (bits : List Bool) : Option (Node × List Bool) := do
  let (tag, rest) ← decodeNatPrefix bits
  match tag with
  | 0 => do
      let (slot, tail) ← decodeNatPrefix rest
      some (.input slot, tail)
  | 1 => do
      let (value, tail) ← decodeWordPrefix rest
      some (.constant value, tail)
  | 2 => do
      let (first, rest) ← decodeNatPrefix rest
      let (second, tail) ← decodeNatPrefix rest
      some (.add first second, tail)
  | 3 => do
      let (first, rest) ← decodeNatPrefix rest
      let (second, tail) ← decodeNatPrefix rest
      some (.sub first second, tail)
  | 4 => do
      let (first, rest) ← decodeNatPrefix rest
      let (second, tail) ← decodeNatPrefix rest
      some (.mul first second, tail)
  | 5 => do
      let (first, rest) ← decodeNatPrefix rest
      let (second, tail) ← decodeNatPrefix rest
      some (.eq first second, tail)
  | _ => none

/-- Encode a register row, retaining the distinction between absent and present registers. -/
def encodeRegister : Option Nat → List Bool
  | none => [false]
  | some value => true :: encodeNat value

def decodeRegisterPrefix (bits : List Bool) : Option (Option Nat × List Bool) := do
  let (present, rest) ← decodeBoolPrefix bits
  if present then
    let (value, tail) ← decodeNatPrefix rest
    some (some value, tail)
  else
    some (none, rest)

def encodeNatPair (pair : Nat × Nat) : List Bool :=
  encodeNat pair.1 ++ encodeNat pair.2

def decodeNatPairPrefix (bits : List Bool) : Option ((Nat × Nat) × List Bool) := do
  let (first, rest) ← decodeNatPrefix bits
  let (second, tail) ← decodeNatPrefix rest
  some ((first, second), tail)

def encodeConstant (constant : Nat × BitVec 64) : List Bool :=
  encodeNat constant.1 ++ encodeWord constant.2

def decodeConstantPrefix (bits : List Bool) : Option ((Nat × BitVec 64) × List Bool) := do
  let (register, rest) ← decodeNatPrefix bits
  let (value, tail) ← decodeWordPrefix rest
  some ((register, value), tail)

def encodeTables (tables : Tables) : List Bool :=
  encodeBool tables.revisionOk ++ encodeNat tables.regs ++
    encodeList encodeRegister tables.registers ++
    encodeList encodeConstant tables.consts ++
    encodeList encodeNatPair tables.slotRegs ++
    encodeList encodeNatPair tables.layers ++
    encodeList encodeNat tables.failSuffixMin

def decodeTablesPrefix (bits : List Bool) : Option (Tables × List Bool) := do
  let (revisionOk, rest) ← decodeBoolPrefix bits
  let (regs, rest) ← decodeNatPrefix rest
  let (registers, rest) ← decodeListPrefix decodeRegisterPrefix rest
  let (consts, rest) ← decodeListPrefix decodeConstantPrefix rest
  let (slotRegs, rest) ← decodeListPrefix decodeNatPairPrefix rest
  let (layers, rest) ← decodeListPrefix decodeNatPairPrefix rest
  let (failSuffixMin, tail) ← decodeListPrefix decodeNatPrefix rest
  some ({ revisionOk, regs, registers, consts, slotRegs, layers, failSuffixMin }, tail)

/-- Encode every finite component in a fixed order. -/
def encodeInput (input : Input) : List Bool :=
  encodeList encodeNode input.nodes ++ encodeNat input.layerCount ++
    encodeList encodeNode input.failNodes ++ encodeList encodeNat input.layerOf ++
    encodeTables input.tables

def decodeInputPrefix (bits : List Bool) : Option (Input × List Bool) := do
  let (nodes, rest) ← decodeListPrefix decodeNodePrefix bits
  let (layerCount, rest) ← decodeNatPrefix rest
  let (failNodes, rest) ← decodeListPrefix decodeNodePrefix rest
  let (layerOf, rest) ← decodeListPrefix decodeNatPrefix rest
  let (tables, tail) ← decodeTablesPrefix rest
  some ({ nodes, layerCount, failNodes, layerOf, tables }, tail)

/-- Total decoder: malformed inputs and trailing bits have no decoded layout. -/
def decodeInput (bits : List Bool) : Option Input := do
  let (input, rest) ← decodeInputPrefix bits
  if rest.isEmpty then some input else none

def modelGraph (input : Input) : Graph where
  arity := input.nodes.length
  output := 0
  node := fun index => input.nodes.getD index (.constant 0)

def modelFails (input : Input) : Node → Bool := fun node => input.failNodes.contains node

/-- The finite layer assignment is extended totally outside the declared node list. -/
def modelLayerOf (input : Input) (index : Nat) : Nat := input.layerOf.getD index 0

def modelLayout (input : Input) : PackedLayout.Layout where
  revision := if input.tables.revisionOk then PackedLayout.revision else "invalid-layout-revision"
  regs := input.tables.regs
  reg := fun index => input.tables.registers.getD index none
  consts := input.tables.consts
  slotRegs := input.tables.slotRegs
  layers := input.tables.layers
  failSuffixMin := input.tables.failSuffixMin

def acceptedInput (input : Input) : Bool :=
  PackedLayout.accepted (modelGraph input) input.layerCount (modelFails input)
    (modelLayerOf input) (modelLayout input)

/-- Decode and validate; malformed bit strings are refused. -/
def validateEncoded (bits : List Bool) : Bool :=
  match decodeInput bits with
  | some input => acceptedInput input
  | none => false

/-- The declared size is the exact number of bits in the chosen encoding. -/
def encodedSize (input : Input) : Nat := (encodeInput input).length

theorem decodeNatPrefix_encodeNat_append (value : Nat) (rest : List Bool) :
    decodeNatPrefix (encodeNat value ++ rest) = some (value, rest) := by
  induction value with
  | zero => simp [encodeNat, decodeNatPrefix]
  | succ value ih =>
      simp only [encodeNat, List.cons_append, decodeNatPrefix, ih]
      rfl

attribute [simp] decodeNatPrefix_encodeNat_append

private theorem decodeBoolPrefix_encodeBool_append (value : Bool) (rest : List Bool) :
    decodeBoolPrefix (encodeBool value ++ rest) = some (value, rest) := by
  cases value <;> rfl

private theorem decodeFixedBitsPrefix_encodeFixedBits_append (width value : Nat)
    (hvalue : value < 2 ^ width) (rest : List Bool) :
    decodeFixedBitsPrefix width (encodeFixedBits width value ++ rest) = some (value, rest) := by
  induction width generalizing value rest with
  | zero =>
      have : value = 0 := by omega
      subst value
      simp [encodeFixedBits, decodeFixedBitsPrefix]
  | succ width ih =>
      have hdiv : value / 2 < 2 ^ width := by omega
      have hmod : value % 2 = 0 ∨ value % 2 = 1 := by omega
      simp only [encodeFixedBits, List.cons_append, decodeFixedBitsPrefix]
      rw [ih (value / 2) hdiv rest]
      cases hmod with
      | inl hzero => simp [hzero]; omega
      | inr hone => simp [hone]; omega

private theorem decodeWordPrefix_encodeWord_append (value : BitVec 64) (rest : List Bool) :
    decodeWordPrefix (encodeWord value ++ rest) = some (value, rest) := by
  unfold decodeWordPrefix encodeWord
  rw [decodeFixedBitsPrefix_encodeFixedBits_append 64 value.toNat value.isLt rest]
  change (if value.toNat < 2 ^ 64 then
      some (BitVec.ofNat 64 value.toNat, rest) else none) = some (value, rest)
  by_cases h : value.toNat < 2 ^ 64
  · simp only [h, ite_true, Option.some.injEq, Prod.mk.injEq]
    constructor
    · apply BitVec.eq_of_toNat_eq
      rw [BitVec.toNat_ofNat, Nat.mod_eq_of_lt value.isLt]
    · trivial
  · simp only [h, ite_false]
    exact False.elim (h value.isLt)

private theorem decodeListFrom_encodeList_append
    (decodeElement : List Bool → Option (α × List Bool))
    (encodeElement : α → List Bool)
    (elementRoundTrip : ∀ value rest,
      decodeElement (encodeElement value ++ rest) = some (value, rest))
    (values : List α) (rest : List Bool) :
    decodeListFrom decodeElement values.length (values.flatMap encodeElement ++ rest) =
      some (values, rest) := by
  induction values with
  | nil => simp [decodeListFrom]
  | cons value tail ih =>
      simp only [List.length_cons, List.flatMap_cons, decodeListFrom, List.append_assoc]
      simp [elementRoundTrip, ih]

private theorem decodeListPrefix_encodeList_append
    (decodeElement : List Bool → Option (α × List Bool))
    (encodeElement : α → List Bool)
    (elementRoundTrip : ∀ value rest,
      decodeElement (encodeElement value ++ rest) = some (value, rest))
    (values : List α) (rest : List Bool) :
    decodeListPrefix decodeElement (encodeList encodeElement values ++ rest) =
      some (values, rest) := by
  simp [decodeListPrefix, encodeList, List.append_assoc, decodeListFrom_encodeList_append,
    decodeNatPrefix_encodeNat_append, elementRoundTrip]

private theorem decodeNodePrefix_encodeNode_append (node : Node) (rest : List Bool) :
    decodeNodePrefix (encodeNode node ++ rest) = some (node, rest) := by
  cases node <;> simp [decodeNodePrefix, encodeNode, List.append_assoc,
    decodeNatPrefix_encodeNat_append, decodeWordPrefix_encodeWord_append]

private theorem decodeRegisterPrefix_encodeRegister_append (value : Option Nat)
    (rest : List Bool) : decodeRegisterPrefix (encodeRegister value ++ rest) =
      some (value, rest) := by
  cases value with
  | none => rfl
  | some value => simp [encodeRegister, decodeRegisterPrefix, decodeBoolPrefix,
      decodeNatPrefix_encodeNat_append]

private theorem decodeNatPairPrefix_encodeNatPair_append (pair : Nat × Nat)
    (rest : List Bool) : decodeNatPairPrefix (encodeNatPair pair ++ rest) =
      some (pair, rest) := by
  cases pair with
  | mk first second =>
      simp [encodeNatPair, decodeNatPairPrefix, decodeNatPrefix_encodeNat_append,
        List.append_assoc]

private theorem decodeConstantPrefix_encodeConstant_append (constant : Nat × BitVec 64)
    (rest : List Bool) : decodeConstantPrefix (encodeConstant constant ++ rest) =
      some (constant, rest) := by
  cases constant with
  | mk register value =>
      simp [encodeConstant, decodeConstantPrefix, decodeNatPrefix_encodeNat_append,
        decodeWordPrefix_encodeWord_append, List.append_assoc]

private theorem decodeListNodes_encode_append (values : List Node) (rest : List Bool) :
    decodeListPrefix decodeNodePrefix (encodeList encodeNode values ++ rest) =
      some (values, rest) :=
  decodeListPrefix_encodeList_append decodeNodePrefix encodeNode
    decodeNodePrefix_encodeNode_append values rest

private theorem decodeListNats_encode_append (values : List Nat) (rest : List Bool) :
    decodeListPrefix decodeNatPrefix (encodeList encodeNat values ++ rest) =
      some (values, rest) :=
  decodeListPrefix_encodeList_append decodeNatPrefix encodeNat
    (fun value tail => decodeNatPrefix_encodeNat_append value tail) values rest

private theorem decodeListRegisters_encode_append (values : List (Option Nat))
    (rest : List Bool) : decodeListPrefix decodeRegisterPrefix
      (encodeList encodeRegister values ++ rest) = some (values, rest) :=
  decodeListPrefix_encodeList_append decodeRegisterPrefix encodeRegister
    decodeRegisterPrefix_encodeRegister_append values rest

private theorem decodeListConstants_encode_append (values : List (Nat × BitVec 64))
    (rest : List Bool) : decodeListPrefix decodeConstantPrefix
      (encodeList encodeConstant values ++ rest) = some (values, rest) :=
  decodeListPrefix_encodeList_append decodeConstantPrefix encodeConstant
    decodeConstantPrefix_encodeConstant_append values rest

private theorem decodeListPairs_encode_append (values : List (Nat × Nat)) (rest : List Bool) :
    decodeListPrefix decodeNatPairPrefix (encodeList encodeNatPair values ++ rest) =
      some (values, rest) :=
  decodeListPrefix_encodeList_append decodeNatPairPrefix encodeNatPair
    decodeNatPairPrefix_encodeNatPair_append values rest

private theorem decodeTablesPrefix_encodeTables_append (tables : Tables) (rest : List Bool) :
    decodeTablesPrefix (encodeTables tables ++ rest) = some (tables, rest) := by
  cases tables with
  | mk revisionOk regs registers consts slotRegs layers failSuffixMin =>
      simp [encodeTables, decodeTablesPrefix, decodeBoolPrefix_encodeBool_append,
        decodeNatPrefix_encodeNat_append, decodeListRegisters_encode_append,
        decodeListConstants_encode_append, decodeListPairs_encode_append,
        decodeListNats_encode_append, List.append_assoc]

private theorem decodeInputPrefix_encodeInput_append (input : Input) (rest : List Bool) :
    decodeInputPrefix (encodeInput input ++ rest) = some (input, rest) := by
  cases input with
  | mk nodes layerCount failNodes layerOf tables =>
      simp [encodeInput, decodeInputPrefix, decodeListNodes_encode_append,
        decodeNatPrefix_encodeNat_append,
        decodeListNats_encode_append, decodeTablesPrefix_encodeTables_append,
        List.append_assoc]

private theorem decodeInputPrefix_encodeInput (input : Input) :
    decodeInputPrefix (encodeInput input) = some (input, []) := by
  simpa using decodeInputPrefix_encodeInput_append input []

/-- The total decoder round-trips every canonical input encoding. -/
theorem decodeInput_encodeInput (input : Input) : decodeInput (encodeInput input) = some input := by
  unfold decodeInput
  rw [decodeInputPrefix_encodeInput]
  rfl

/-- Canonical encodings are injective, including the lengths and every table entry. -/
theorem encodeInput_injective : Function.Injective encodeInput := by
  intro first second same
  have decoded := congrArg decodeInput same
  rw [decodeInput_encodeInput, decodeInput_encodeInput] at decoded
  exact Option.some.inj decoded

/-- Validation of an encoded input is exactly the existing finite-table acceptance predicate
on its total decoded graph and declaration. -/
theorem validateEncoded_agrees (input : Input) :
    validateEncoded (encodeInput input) = acceptedInput input := by
  simp [validateEncoded, decodeInput_encodeInput]

/-- The size measure is exactly the emitted encoding's bit length. -/
theorem encodedSize_is_bit_length (input : Input) :
    encodedSize input = (encodeInput input).length := rfl

end NobleFoundation.PackedLayoutEncoded
