import InteractionNets
import NobleFoundation.NetI64
import NobleFoundation.PackedSchedule

namespace NobleFoundation.NetI64Corpus

open NobleFoundation.NetI64
open NobleFoundation.PackedSchedule

private def compareId : List Nat → List Nat → Ordering
  | [], [] => .eq
  | [], _ :: _ => .lt
  | _ :: _, [] => .gt
  | left :: leftTail, right :: rightTail =>
      match compare left right with
      | .eq => compareId leftTail rightTail
      | order => order

private def comparePort (left right : INet.Port) : Ordering :=
  match compareId left.1 right.1 with
  | .eq => compare left.2 right.2
  | order => order

private def compareWire (left right : INet.Port × INet.Port) : Ordering :=
  match comparePort left.1 right.1 with
  | .eq => comparePort left.2 right.2
  | order => order

private def canonicalNet (net : INet.XNet NetSym) : INet.XNet NetSym :=
  { agents := net.agents.mergeSort (fun left right => compareId left.1 right.1 == .lt)
    free := net.free.mergeSort (fun left right => comparePort left right == .lt)
    wires := net.wires.mergeSort (fun left right => compareWire left right == .lt) }

private def normalizeReadBack (program : Program) (inputs : List (BitVec 64)) :=
  (Program.translateChecked program inputs).bind fun source =>
    (XNet.normalize xsys (Program.interactionCount program) source).map fun (net, steps) =>
      (Program.readBack program net, steps)

-- Rust-produced input nets, differential outputs and interaction counts follow.

-- graph=5f75dea072950d515d5e190ec619958f32eed8e1a7e353c6de5cc68edf86a389; contract=ngraph-net-i64/3; lowering=ngraph-net-i64-lowering/0; build_key=54b080bffdb5997cbf588cdbea49f5c76a69b0d939f861f81b5e03d493a02bdb; inputs=[9223372036854775807,-1,17]
def fanout_overflowProgram : Program := { inputArity := 3, nodes := [.input 0, .input 1, .input 2, .constant (BitVec.ofNat 64 3), .add 0 3, .sub 1 0, .mul 4 5, .eq 0 1], outputs := [6, 7, 6] }
def fanout_overflowInputs : List (BitVec 64) := [BitVec.ofNat 64 9223372036854775807, BitVec.ofNat 64 18446744073709551615, BitVec.ofNat 64 17]
def fanout_overflowRustNet : INet.XNet NetSym := {
    agents := [([0, 0, 0], .num (BitVec.ofNat 64 9223372036854775807)),
    ([0, 0, 1], .num (BitVec.ofNat 64 18446744073709551615)),
    ([0, 0, 2], .num (BitVec.ofNat 64 17)),
    ([0, 0, 3], .num (BitVec.ofNat 64 3)),
    ([0, 1, 4], .op2 .add),
    ([0, 1, 5], .op2 .sub),
    ([0, 1, 6], .op2 .mul),
    ([0, 1, 7], .op2 .eq),
    ([0, 2, 0, 0], .dup),
    ([0, 2, 0, 1], .dup),
    ([0, 2, 1, 0], .dup),
    ([0, 2, 6, 0], .dup),
    ([0, 3, 2], .era)],
    free := [([9, 9, 0], 0), ([9, 9, 1], 0), ([9, 9, 2], 0)],
    wires := [(([0, 0, 0], 0), ([0, 2, 0, 0], 0)),
    (([0, 0, 1], 0), ([0, 2, 1, 0], 0)),
    (([0, 0, 2], 0), ([0, 3, 2], 0)),
    (([0, 0, 3], 0), ([0, 1, 4], 1)),
    (([0, 1, 4], 0), ([0, 2, 0, 0], 1)),
    (([0, 1, 4], 1), ([0, 0, 3], 0)),
    (([0, 1, 4], 2), ([0, 1, 6], 0)),
    (([0, 1, 5], 0), ([0, 2, 1, 0], 1)),
    (([0, 1, 5], 1), ([0, 2, 0, 1], 1)),
    (([0, 1, 5], 2), ([0, 1, 6], 1)),
    (([0, 1, 6], 0), ([0, 1, 4], 2)),
    (([0, 1, 6], 1), ([0, 1, 5], 2)),
    (([0, 1, 6], 2), ([0, 2, 6, 0], 0)),
    (([0, 1, 7], 0), ([0, 2, 0, 1], 2)),
    (([0, 1, 7], 1), ([0, 2, 1, 0], 2)),
    (([0, 1, 7], 2), ([9, 9, 1], 0)),
    (([0, 2, 0, 0], 0), ([0, 0, 0], 0)),
    (([0, 2, 0, 0], 1), ([0, 1, 4], 0)),
    (([0, 2, 0, 0], 2), ([0, 2, 0, 1], 0)),
    (([0, 2, 0, 1], 0), ([0, 2, 0, 0], 2)),
    (([0, 2, 0, 1], 1), ([0, 1, 5], 1)),
    (([0, 2, 0, 1], 2), ([0, 1, 7], 0)),
    (([0, 2, 1, 0], 0), ([0, 0, 1], 0)),
    (([0, 2, 1, 0], 1), ([0, 1, 5], 0)),
    (([0, 2, 1, 0], 2), ([0, 1, 7], 1)),
    (([0, 2, 6, 0], 0), ([0, 1, 6], 2)),
    (([0, 2, 6, 0], 1), ([9, 9, 0], 0)),
    (([0, 2, 6, 0], 2), ([9, 9, 2], 0)),
    (([0, 3, 2], 0), ([0, 0, 2], 0)),
    (([9, 9, 0], 0), ([0, 2, 6, 0], 1)),
    (([9, 9, 1], 0), ([0, 1, 7], 2)),
    (([9, 9, 2], 0), ([0, 2, 6, 0], 2))]
  }
def fanout_overflowRustOutputs : List PackedSchedule.Val := [.i64 (BitVec.ofNat 64 0), .boolean false, .i64 (BitVec.ofNat 64 0)]
def fanout_overflowRustInteractions : Nat := 13

theorem fanout_overflow_translation : (Program.translateChecked fanout_overflowProgram fanout_overflowInputs).map canonicalNet = some (canonicalNet fanout_overflowRustNet) := by decide
theorem fanout_overflow_normalization : normalizeReadBack fanout_overflowProgram fanout_overflowInputs = some (some fanout_overflowRustOutputs, fanout_overflowRustInteractions) := by decide
theorem fanout_overflow_interaction_count : Program.interactionCount fanout_overflowProgram = fanout_overflowRustInteractions := by decide

-- graph=da45e840579a9f8027d09ea89f6893d195b06d487e6721fde8f1fc4e96237873; contract=ngraph-net-i64/3; lowering=ngraph-net-i64-lowering/0; build_key=6591d217d1bcbe731033ea688c7fd03535039e05883af3cb44d7310472523934; inputs=[-9223372036854775808]
def discarded_unitProgram : Program := { inputArity := 1, nodes := [.input 0], outputs := [] }
def discarded_unitInputs : List (BitVec 64) := [BitVec.ofNat 64 9223372036854775808]
def discarded_unitRustNet : INet.XNet NetSym := {
    agents := [([0, 0, 0], .num (BitVec.ofNat 64 9223372036854775808)),
    ([0, 3, 0], .era)],
    free := [],
    wires := [(([0, 0, 0], 0), ([0, 3, 0], 0)),
    (([0, 3, 0], 0), ([0, 0, 0], 0))]
  }
def discarded_unitRustOutputs : List PackedSchedule.Val := []
def discarded_unitRustInteractions : Nat := 1

theorem discarded_unit_translation : (Program.translateChecked discarded_unitProgram discarded_unitInputs).map canonicalNet = some (canonicalNet discarded_unitRustNet) := by decide
theorem discarded_unit_normalization : normalizeReadBack discarded_unitProgram discarded_unitInputs = some (some discarded_unitRustOutputs, discarded_unitRustInteractions) := by decide
theorem discarded_unit_interaction_count : Program.interactionCount discarded_unitProgram = discarded_unitRustInteractions := by decide

-- graph=8288b6543c55fcf9ac2163b909e4825e5847ce3080dd1a40f3633cbc7804c4e0; contract=ngraph-net-i64/3; lowering=ngraph-net-i64-lowering/0; build_key=8895516b8d3932f5f6ec51c00cfb0bde7771cd0e8cff2212b3882f1a289d6db9; inputs=[]
def emptyProgram : Program := { inputArity := 0, nodes := [], outputs := [] }
def emptyInputs : List (BitVec 64) := []
def emptyRustNet : INet.XNet NetSym := {
    agents := [],
    free := [],
    wires := []
  }
def emptyRustOutputs : List PackedSchedule.Val := []
def emptyRustInteractions : Nat := 0

theorem empty_translation : (Program.translateChecked emptyProgram emptyInputs).map canonicalNet = some (canonicalNet emptyRustNet) := by decide
theorem empty_normalization : normalizeReadBack emptyProgram emptyInputs = some (some emptyRustOutputs, emptyRustInteractions) := by decide
theorem empty_interaction_count : Program.interactionCount emptyProgram = emptyRustInteractions := by decide

end NobleFoundation.NetI64Corpus
