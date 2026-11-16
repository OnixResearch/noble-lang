import NobleFoundation.NetI64PlanLowering
import NobleFoundation.NetI64PlanCorpus

/-!
# Finite differential between a physical Lean lowerer and actual Rust bytes

The Rust corpus is emitted by its compiler/checker; this module independently
rebuilds the exact two-slot physical inputs, both pair orientations and both
storage policies. These finite equalities do not establish universal Rust
compiler correspondence or general physical semantic admission.
-/

namespace NobleFoundation.NetI64PlanLoweringCorpus

open INet
open NobleFoundation.NetI64
open NobleFoundation.NetI64Plans
open NobleFoundation.NetI64PlanCorpus

private def selectedPair : PhysicalPair :=
  { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }

/-- Reconstruct the producer's two-slot corpus input independently in the
physical Lean model; output labels are stable while their targets depend on
which orientation holds the operation. -/
private def physicalFixture (rule : NetSym) (value : BitVec 64)
    (reverse : Bool) : PhysicalState := Id.run do
  let ruleSlot := if reverse then 1 else 0
  let valueSlot := if reverse then 0 else 1
  let ruleId : Id := if reverse then [2] else [1]
  let valueId : Id := if reverse then [1] else [2]
  let rulePorts := (List.range (NetI64.arity rule + 1)).map fun index =>
    if index == 0 then some (valueSlot, 0) else none
  let rulePhysical : PhysicalSlot :=
    { agent := some (ruleId, rule), generation := 0,
      queuedGeneration := none, ports := rulePorts }
  let valuePhysical : PhysicalSlot :=
    { agent := some (valueId, .num value), generation := 0,
      queuedGeneration := none, ports := [some (ruleSlot, 0)] }
  let outputs := (List.range (NetI64.arity rule)).map fun index =>
    { label := ([100 + index], 0), target := (ruleSlot, index + 1) : PhysicalOutput }
  return { slots := if reverse then #[valuePhysical, rulePhysical]
      else #[rulePhysical, valuePhysical], freeSlots := [], outputs, work := [] }

private def selectedRedex (rule : NetSym) (value : BitVec 64)
    (reverse : Bool) : Option (XRedex NetSym) := do
  let first := if reverse then NetSym.num value else rule
  let second := if reverse then rule else NetSym.num value
  let rhs ← NetI64.xRule first second
  some ([1], [2], first, second, rhs)

private def lowered (rule : NetSym) (value : BitVec 64)
    (reverse reuse : Bool) : Option (List UInt8) := do
  let redex ← selectedRedex rule value reverse
  (physicalFixture rule value reverse).lowerBytes selectedPair redex reuse

/-- Byte-level equality to the retained Rust output, for both orientations
and both storage modes of each selected rule. No producer metadata is
consulted by the Lean lowerer. -/
theorem all_sixteen_bytes_match :
    lowered (.op2 .add) (BitVec.ofNat 64 27) false true =
      some op2ForwardReuseBytes ∧
    lowered (.op2 .add) (BitVec.ofNat 64 27) false false =
      some op2ForwardAllocateBytes ∧
    lowered (.op2 .add) (BitVec.ofNat 64 27) true true =
      some op2ReverseReuseBytes ∧
    lowered (.op2 .add) (BitVec.ofNat 64 27) true false =
      some op2ReverseAllocateBytes ∧
    lowered (.op1 .sub (BitVec.ofNat 64 7)) (BitVec.ofNat 64 13) false true =
      some op1ForwardReuseBytes ∧
    lowered (.op1 .sub (BitVec.ofNat 64 7)) (BitVec.ofNat 64 13) false false =
      some op1ForwardAllocateBytes ∧
    lowered (.op1 .sub (BitVec.ofNat 64 7)) (BitVec.ofNat 64 13) true true =
      some op1ReverseReuseBytes ∧
    lowered (.op1 .sub (BitVec.ofNat 64 7)) (BitVec.ofNat 64 13) true false =
      some op1ReverseAllocateBytes ∧
    lowered .dup (BitVec.ofNat 64 42) false true =
      some dupForwardReuseBytes ∧
    lowered .dup (BitVec.ofNat 64 42) false false =
      some dupForwardAllocateBytes ∧
    lowered .dup (BitVec.ofNat 64 42) true true =
      some dupReverseReuseBytes ∧
    lowered .dup (BitVec.ofNat 64 42) true false =
      some dupReverseAllocateBytes ∧
    lowered .era (BitVec.ofNat 64 42) false true =
      some eraForwardReuseBytes ∧
    lowered .era (BitVec.ofNat 64 42) false false =
      some eraForwardAllocateBytes ∧
    lowered .era (BitVec.ofNat 64 42) true true =
      some eraReverseReuseBytes ∧
    lowered .era (BitVec.ofNat 64 42) true false =
      some eraReverseAllocateBytes := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_,
    ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  all_goals set_option maxRecDepth 3000 in decide

/-- Sixteen additional Rust emissions stress the high I64 payload and zero
cases. They have no independently established whole-state physical fixture. -/
theorem extra_sixteen_bytes_match :
    lowered (.op2 .add) (BitVec.ofNat 64 (UInt64.size - 1)) false true =
      some op2MaxForwardReuseBytes ∧
    lowered (.op2 .add) (BitVec.ofNat 64 (UInt64.size - 1)) false false =
      some op2MaxForwardAllocateBytes ∧
    lowered (.op2 .add) (BitVec.ofNat 64 (UInt64.size - 1)) true true =
      some op2MaxReverseReuseBytes ∧
    lowered (.op2 .add) (BitVec.ofNat 64 (UInt64.size - 1)) true false =
      some op2MaxReverseAllocateBytes ∧
    lowered (.op1 .sub (BitVec.ofNat 64 7)) (BitVec.ofNat 64 0) false true =
      some op1ZeroForwardReuseBytes ∧
    lowered (.op1 .sub (BitVec.ofNat 64 7)) (BitVec.ofNat 64 0) false false =
      some op1ZeroForwardAllocateBytes ∧
    lowered (.op1 .sub (BitVec.ofNat 64 7)) (BitVec.ofNat 64 0) true true =
      some op1ZeroReverseReuseBytes ∧
    lowered (.op1 .sub (BitVec.ofNat 64 7)) (BitVec.ofNat 64 0) true false =
      some op1ZeroReverseAllocateBytes ∧
    lowered .dup (BitVec.ofNat 64 0) false true =
      some dupZeroForwardReuseBytes ∧
    lowered .dup (BitVec.ofNat 64 0) false false =
      some dupZeroForwardAllocateBytes ∧
    lowered .dup (BitVec.ofNat 64 0) true true =
      some dupZeroReverseReuseBytes ∧
    lowered .dup (BitVec.ofNat 64 0) true false =
      some dupZeroReverseAllocateBytes ∧
    lowered .era (BitVec.ofNat 64 0) false true =
      some eraZeroForwardReuseBytes ∧
    lowered .era (BitVec.ofNat 64 0) false false =
      some eraZeroForwardAllocateBytes ∧
    lowered .era (BitVec.ofNat 64 0) true true =
      some eraZeroReverseReuseBytes ∧
    lowered .era (BitVec.ofNat 64 0) true false =
      some eraZeroReverseAllocateBytes := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_,
    ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  all_goals set_option maxRecDepth 3000 in decide

end NobleFoundation.NetI64PlanLoweringCorpus
