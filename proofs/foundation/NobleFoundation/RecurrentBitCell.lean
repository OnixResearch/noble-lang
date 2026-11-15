import Batteries.Data.BitVec.Lemmas
import Init.Data.BitVec.Bitblast

/-!
# Exact-bit recurrent one-step model (issue #78)

The source carrier is `BitVec 64`, with wrapping addition and a two-word swap. The
separate target is a little-endian Boolean cell function. Its adder copies the
select order in `crates/noble-ngraph/tests/models/recurrent/programs.rs`: first
invert b, select a XOR b, select the sum with the incoming carry, then select
(b AND carry) or (b OR carry) according to a. `BitVec.iunfoldr` connects these
Boolean gates from bit zero through bit 63; its final carry is discarded.
The swap target routes the second word's cells before the first word's cells.

These definitions model the selected graph algorithm, not Rust graph construction,
ONNX bytes, binary32 arithmetic, guard/refusal behavior or runtime execution.
The correspondence of the checked Rust graph and ONNX Scan body to these Boolean
functions is a separate trusted translation boundary.
-/

namespace NobleFoundation.RecurrentBitCell

abbrev WordCells := Fin 64 → Bool
abbrev StackCells := WordCells × WordCells

/-- Cell zero is the least significant bit of the source word. -/
def encodeWord (word : BitVec 64) : WordCells := fun i => word.getLsb i

/-- Decode exactly 64 indexed cells, with no signed or floating-point cast. -/
def decodeWord (cells : WordCells) : BitVec 64 := BitVec.ofFnLE cells

theorem decode_encode_word (word : BitVec 64) :
    decodeWord (encodeWord word) = word := by
  apply BitVec.eq_of_getLsbD_eq
  intro i hi
  change (BitVec.ofFnLE (fun j : Fin 64 => word.getLsb j)).getLsbD i = word.getLsbD i
  simp only [BitVec.getLsbD_eq_getElem hi]
  exact BitVec.getLsb_ofFnLE (fun j : Fin 64 => word.getLsb j) ⟨i, hi⟩

theorem encode_decode_word (cells : WordCells) :
    encodeWord (decodeWord cells) = cells := by
  funext i
  exact BitVec.getLsb_ofFnLE cells i

/-- The precise off/on ordering of the checked graph's F32Select, at bit values. -/
def choose (bit off on : Bool) : Bool := if bit then on else off

def xorGate (a b : Bool) : Bool := choose a b (choose b true false)

def sumGate (a b carry : Bool) : Bool :=
  choose (xorGate a b) carry (choose carry true false)

def carryGate (a b carry : Bool) : Bool :=
  choose a (choose b false carry) (choose b carry true)

theorem sumGate_xor (a b carry : Bool) :
    sumGate a b carry = (a ^^ (b ^^ carry)) := by
  cases a <;> cases b <;> cases carry <;> rfl

theorem carryGate_majority (a b carry : Bool) :
    carryGate a b carry = Bool.atLeastTwo a b carry := by
  cases a <;> cases b <;> cases carry <;> rfl

/-- A ripple through the actual select-gate cell equations; this is not addition
by definition. The input and state bits are indexed in little-endian order. -/
def graphAdder (state input : WordCells) : WordCells :=
  encodeWord ((BitVec.iunfoldr
    (fun (i : Fin 64) (carry : Bool) =>
      (carryGate (state i) (input i) carry,
       sumGate (state i) (input i) carry)) false).2)

/-- Equivalence to the separately proved BitVec ripple-adder specification,
including the carry at every bit, for arbitrary 64-cell state and input. -/
theorem graphAdder_correct (state input : WordCells) :
    graphAdder state input = encodeWord (decodeWord state + decodeWord input) := by
  have hstate (i : Fin 64) :
      (decodeWord state).getLsbD i.val = state i := by
    rw [BitVec.getLsbD_eq_getElem i.isLt]
    exact BitVec.getLsb_ofFnLE state i
  have hinput (i : Fin 64) :
      (decodeWord input).getLsbD i.val = input i := by
    rw [BitVec.getLsbD_eq_getElem i.isLt]
    exact BitVec.getLsb_ofFnLE input i
  have hgate :
      (fun (i : Fin 64) (carry : Bool) =>
        (carryGate (state i) (input i) carry,
         sumGate (state i) (input i) carry)) =
      (fun (i : Fin 64) (carry : Bool) =>
        BitVec.adcb ((decodeWord state).getLsbD i.val)
          ((decodeWord input).getLsbD i.val) carry) := by
    funext i carry
    rw [hstate i, hinput i, carryGate_majority, sumGate_xor]
    rfl
  calc
    graphAdder state input =
        encodeWord (BitVec.adc (decodeWord state) (decodeWord input) false).2 := by
      unfold graphAdder BitVec.adc
      rw [hgate]
    _ = encodeWord (decodeWord state + decodeWord input) := by
      rw [← BitVec.add_eq_adc 64 (decodeWord state) (decodeWord input)]

theorem graphAdder_decodes (state input : WordCells) :
    decodeWord (graphAdder state input) = decodeWord state + decodeWord input := by
  rw [graphAdder_correct, decode_encode_word]

/-- The graph emits the same post-step cells as next state and observation. -/
def graphAccumulator (state input : WordCells) : WordCells × WordCells :=
  let next := graphAdder state input
  (next, next)

/-- Source-state initialization is the little-endian bit encoding. -/
theorem accumulator_initialization (state : BitVec 64) :
    decodeWord (encodeWord state) = state := decode_encode_word state

/-- Universal one-step state and observation simulation; `BitVec 64` addition
is unsigned addition modulo 2^64, not binary32 word arithmetic. -/
theorem accumulator_one_step (state input : BitVec 64) :
    decodeWord (graphAccumulator (encodeWord state) (encodeWord input)).1 = state + input ∧
    decodeWord (graphAccumulator (encodeWord state) (encodeWord input)).2 = state + input := by
  simp [graphAccumulator, graphAdder_decodes, decode_encode_word]

def encodeStack (state : BitVec 64 × BitVec 64) : StackCells :=
  (encodeWord state.1, encodeWord state.2)

def decodeStack (state : StackCells) : BitVec 64 × BitVec 64 :=
  (decodeWord state.1, decodeWord state.2)

/-- Exactly the checked swap graph's ordered output routing, duplicated for
next state and observation. The ONNX Scan's exact +0 dummy token is host ABI,
not an input to this state-only graph. -/
def graphSwap (state : StackCells) : StackCells × StackCells :=
  let next := (state.2, state.1)
  (next, next)

theorem stack_initialization (state : BitVec 64 × BitVec 64) :
    decodeStack (encodeStack state) = state := by
  rcases state with ⟨left, right⟩
  simp [encodeStack, decodeStack, decode_encode_word]

/-- The two distinct source words are routed, not reinterpreted or added. -/
theorem stack_swap_one_step (left right : BitVec 64) :
    decodeStack (graphSwap (encodeStack (left, right))).1 = (right, left) ∧
    decodeStack (graphSwap (encodeStack (left, right))).2 = (right, left) := by
  simp [graphSwap, encodeStack, decodeStack, decode_encode_word]

/-- The abstract 1,000-completed-step cap. The runtime's request-admission,
cancellation, snapshot and guard checks are not modelled by this predicate. -/
def mayStep (completed : Nat) : Bool := decide (completed < 1000)

theorem step_bound_before_limit (completed : Nat) (h : completed < 1000) :
    mayStep completed = true := by
  simp [mayStep, h]

theorem step_bound_halts_at_limit (completed : Nat) (h : 1000 ≤ completed) :
    mayStep completed = false := by
  have hnot : ¬ completed < 1000 := Nat.not_lt.mpr h
  simp [mayStep, hnot]

end NobleFoundation.RecurrentBitCell
