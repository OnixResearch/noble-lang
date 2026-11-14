import NobleFoundation.BeltTarget

namespace NobleFoundation.BeltTargetCorpus

open NobleFoundation.PackedSchedule
open NobleFoundation.BeltTarget

def graph : Graph := {
  arity := 6,
  output := 3,
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .input 1
    else if index = 2 then .constant (BitVec.ofNat 64 18446744073709551613)
    else if index = 3 then .add 0 1
    else if index = 4 then .sub 1 0
    else if index = 5 then .mul 3 2
    else .input 0
}

def inputs (slot : Nat) : BitVec 64 := match slot with
  | 0 => BitVec.ofInt 64 (5)
  | 1 => BitVec.ofInt 64 (3)
  | _ => 0

def outputs : List Nat := [3, 4, 5]

def t1Schedule : NobleFoundation.BeltTarget.Schedule := {
  target := .t1,
  horizon := 9,
  events := [
    { cycle := 0, lane := 0, unit := 2, node := 0, operation := .input 0 },
    { cycle := 1, lane := 0, unit := 2, node := 1, operation := .input 1 },
    { cycle := 2, lane := 0, unit := 2, node := 2, operation := .constant (BitVec.ofNat 64 18446744073709551613) },
    { cycle := 3, lane := 0, unit := 0, node := 3, operation := .add 0 1 2 1 },
    { cycle := 4, lane := 0, unit := 2, node := 0, operation := .save 3 0 },
    { cycle := 5, lane := 0, unit := 0, node := 4, operation := .sub 1 0 2 3 },
    { cycle := 6, lane := 0, unit := 1, node := 5, operation := .multiply 3 2 1 2 },
  ],
  outputs := [3, 4, 5]
}

example : validate graph .t1 outputs t1Schedule = true := by decide
example : runOutputs graph inputs t1Schedule = referenceOutputs graph inputs outputs :=
  validate_sound graph inputs outputs .t1 t1Schedule (by decide)
example : runOutputs graph inputs t1Schedule = [some (Val.i64 (BitVec.ofInt 64 (8))), some (Val.i64 (BitVec.ofInt 64 (-2))), some (Val.i64 (BitVec.ofInt 64 (-24)))] := by decide

def t2Schedule : NobleFoundation.BeltTarget.Schedule := {
  target := .t2,
  horizon := 7,
  events := [
    { cycle := 0, lane := 0, unit := 3, node := 0, operation := .input 0 },
    { cycle := 1, lane := 0, unit := 3, node := 1, operation := .input 1 },
    { cycle := 2, lane := 0, unit := 3, node := 2, operation := .constant (BitVec.ofNat 64 18446744073709551613) },
    { cycle := 3, lane := 0, unit := 0, node := 3, operation := .add 0 1 2 1 },
    { cycle := 4, lane := 0, unit := 0, node := 4, operation := .sub 1 0 2 3 },
    { cycle := 5, lane := 0, unit := 2, node := 5, operation := .multiply 3 2 1 2 },
  ],
  outputs := [3, 4, 5]
}

example : validate graph .t2 outputs t2Schedule = true := by decide
example : runOutputs graph inputs t2Schedule = referenceOutputs graph inputs outputs :=
  validate_sound graph inputs outputs .t2 t2Schedule (by decide)
example : runOutputs graph inputs t2Schedule = [some (Val.i64 (BitVec.ofInt 64 (8))), some (Val.i64 (BitVec.ofInt 64 (-2))), some (Val.i64 (BitVec.ofInt 64 (-24)))] := by decide

end NobleFoundation.BeltTargetCorpus
