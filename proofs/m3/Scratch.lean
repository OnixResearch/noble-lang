import NobleM2.CheckSoundness
open NobleM2
def tl2 : List Ty → TyList
  | [] => .nil
  | t :: ts => .cons t (tl2 ts)
def eeSum2 : Ty := .sum .i64 .i64
def eeProg2 : Ty := .program (tl2 [.i64, .i64]) (tl2 [.i64, .i64, .i64]) EffSet.empty
#eval quotationScheme.instantiate ⟨[.stack (tl2 [.i64, eeSum2]), .value .i64, .stack (tl2 [.i64, .i64])]⟩
def eeCand : Candidate :=
  ⟨0, 0,
    [ .literal (.i64 7) ⟨[.stack .nil]⟩,
      .literal (.i64 1) ⟨[.stack (tl2 [.i64])]⟩,
      .invocation 15 ⟨[.stack (tl2 [.i64]), .value .i64, .value .i64]⟩,
      .invocation 8 ⟨[.stack (tl2 [.i64, eeSum2]), .value .i64, .stack (tl2 [.i64, .i64])]⟩,
      .invocation 8 ⟨[.stack (tl2 [.i64, eeSum2, eeProg2]), .value .i64, .stack (tl2 [.i64, .i64])]⟩,
      .invocation 17 ⟨[.stack (tl2 [.i64]), .value .i64, .value .i64, .stack (tl2 [.i64, .i64, .i64]), .effect EffSet.empty, .effect EffSet.empty]⟩ ],
    [0, 1, 2, 3, 4, 5]⟩
def eeReq : Request :=
  ⟨64, ⟨.nil, tl2 [.i64, .i64, .i64], EffSet.empty⟩, ⟨65536, 256, 32, 64, 16, 10000, 64⟩⟩
#eval Repr.repr (check bootstrapEnv eeReq eeCand)
