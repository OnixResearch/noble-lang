-- GENERATED from the checked Rust belt lowerer; do not hand-edit.
-- Model proof and Rust artifact correspondence are separate evidence.
import NobleFoundation.Belt

namespace NobleFoundation.BeltCorpus

open NobleFoundation.PackedSchedule
open NobleFoundation.Belt

def graph : Graph := {
  arity := 8,
  output := 3,
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .input 1
    else if index = 2 then .constant (BitVec.ofNat 64 9223372036854775808)
    else if index = 3 then .add 0 1
    else if index = 4 then .sub 1 0
    else if index = 5 then .mul 4 2
    else if index = 6 then .eq 3 5
    else if index = 7 then .add 0 0
    else .input 0
}

def outputs : List Nat := [3, 4, 6, 3, 5, 7, 4]
def capacity : Nat := 2
def saveSlots : Nat := 8
def program : List Instr := [
  .input 0,
  .input 1,
  .save 1 0,
  .constant (BitVec.ofNat 64 9223372036854775808),
  .save 1 1,
  .load 0,
  .save 1 0,
  .load 1,
  .save 1 1,
  .add 1 0,
  .save 1 2,
  .load 1,
  .save 1 1,
  .load 2,
  .save 1 2,
  .sub 0 1,
  .load 0,
  .save 1 0,
  .mul 1 0,
  .load 1,
  .save 1 1,
  .eq 0 1,
  .save 1 3,
  .load 2,
  .save 1 2,
  .add 0 0,
  .load 3,
  .output 0,
  .save 1 3,
  .load 0,
  .output 0,
  .save 1 0,
  .load 2,
  .output 0,
  .save 1 2,
  .load 0,
  .output 0,
  .load 1,
  .output 0,
  .load 3,
  .output 0,
  .load 2,
  .output 0,
]

example : validate graph outputs capacity saveSlots program = true := by decide

def inputs0 (slot : Nat) : BitVec 64 := match slot with
  | 0 => BitVec.ofInt 64 (9223372036854775807)
  | 1 => BitVec.ofInt 64 (1)
  | _ => 0
example : runOutputs graph inputs0 outputs capacity saveSlots program =
  some [some (.i64 (BitVec.ofInt 64 (-9223372036854775808))), some (.i64 (BitVec.ofInt 64 (-9223372036854775806))), some (.boolean false), some (.i64 (BitVec.ofInt 64 (-9223372036854775808))), some (.i64 (BitVec.ofInt 64 (0))), some (.i64 (BitVec.ofInt 64 (-2))), some (.i64 (BitVec.ofInt 64 (-9223372036854775806)))] := by decide

def inputs1 (slot : Nat) : BitVec 64 := match slot with
  | 0 => BitVec.ofInt 64 (-9223372036854775808)
  | 1 => BitVec.ofInt 64 (-1)
  | _ => 0
example : runOutputs graph inputs1 outputs capacity saveSlots program =
  some [some (.i64 (BitVec.ofInt 64 (9223372036854775807))), some (.i64 (BitVec.ofInt 64 (9223372036854775807))), some (.boolean false), some (.i64 (BitVec.ofInt 64 (9223372036854775807))), some (.i64 (BitVec.ofInt 64 (-9223372036854775808))), some (.i64 (BitVec.ofInt 64 (0))), some (.i64 (BitVec.ofInt 64 (9223372036854775807)))] := by decide

def inputs2 (slot : Nat) : BitVec 64 := match slot with
  | 0 => BitVec.ofInt 64 (5)
  | 1 => BitVec.ofInt 64 (3)
  | _ => 0
example : runOutputs graph inputs2 outputs capacity saveSlots program =
  some [some (.i64 (BitVec.ofInt 64 (8))), some (.i64 (BitVec.ofInt 64 (-2))), some (.boolean false), some (.i64 (BitVec.ofInt 64 (8))), some (.i64 (BitVec.ofInt 64 (0))), some (.i64 (BitVec.ofInt 64 (10))), some (.i64 (BitVec.ofInt 64 (-2)))] := by decide

end NobleFoundation.BeltCorpus
