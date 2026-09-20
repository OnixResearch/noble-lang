import NobleContractImpl.Types
import NobleKernel.Funs

open Aeneas Aeneas.Std Result
open Aeneas.Data.ListN

set_option maxHeartbeats 1000000
set_option maxRecDepth 2048

namespace noble_contracts

/-! The dependency types emitted for this crate are distinct Lean datatypes from
those in the inherited kernel extraction. This boundary changes only their
representation: no scalar is narrowed, no vector is truncated or reordered, and
no checker, word table, witness resolver, or resource meter is replaced.

Nested types follow the existing m3 embedding's Vec/Slice/ListN recursion.
Here both sides have the same bounded representation, so structural recursion
needs neither fuel nor an overflow fallback. -/
namespace KernelBridge

/-- Length-preserving, order-preserving mapping of the extracted list storage. -/
def mapListN {α β : Type} (f : α → β) {n : Nat} : ListN α n → ListN β n
  | .nil => .nil
  | .cons x xs => .cons (f x) (mapListN f xs)

def mapSlice {α β : Type} (f : α → β) (s : Slice α) : Slice β :=
  ⟨s.leng, mapListN f s.list, s.bound⟩

def mapVec {α β : Type} (f : α → β) (v : alloc.vec.Vec α) : alloc.vec.Vec β :=
  ⟨mapSlice f v.slice⟩

@[simp] theorem mapListN_mapListN {α β γ : Type} (f : α → β) (g : β → γ)
    {n : Nat} (xs : ListN α n) :
    mapListN g (mapListN f xs) = mapListN (fun x => g (f x)) xs := by
  induction xs with
  | nil => rfl
  | cons x xs ih => simp only [mapListN, ih]

@[simp] theorem mapListN_id {α : Type} {n : Nat} (xs : ListN α n) :
    mapListN (fun x => x) xs = xs := by
  induction xs with
  | nil => rfl
  | cons x xs ih => simp only [mapListN, ih]

@[simp] theorem mapSlice_mapSlice {α β γ : Type} (f : α → β) (g : β → γ)
    (s : Slice α) :
    mapSlice g (mapSlice f s) = mapSlice (fun x => g (f x)) s := by
  simp only [mapSlice, mapListN_mapListN]

@[simp] theorem mapSlice_id {α : Type} (s : Slice α) :
    mapSlice (fun x => x) s = s := by
  cases s
  simp only [mapSlice, mapListN_id]

@[simp] theorem mapVec_mapVec {α β γ : Type} (f : α → β) (g : β → γ)
    (v : alloc.vec.Vec α) :
    mapVec g (mapVec f v) = mapVec (fun x => g (f x)) v := by
  simp only [mapVec, mapSlice_mapSlice]

@[simp] theorem mapVec_id {α : Type} (v : alloc.vec.Vec α) :
    mapVec (fun x => x) v = v := by
  cases v
  simp only [mapVec, mapSlice_id]

@[simp] theorem mapListN_toList {α β : Type} (f : α → β)
    {n : Nat} (xs : ListN α n) :
    (mapListN f xs).toList = xs.toList.map f := by
  induction xs with
  | nil => rfl
  | cons x xs ih => simp only [mapListN, ListN.toList, List.map, ih]

@[simp] theorem mapVec_val {α β : Type} (f : α → β) (v : alloc.vec.Vec α) :
    (mapVec f v).val = v.val.map f := by
  simp only [mapVec, mapSlice, alloc.vec.Vec.val, Slice.val, mapListN_toList]

mutual
  def toTy : noble_kernel.types.Ty → _root_.noble_kernel.types.Ty
    | .UnitType => .UnitType
    | .BoolType => .BoolType
    | .I64Type => .I64Type
    | .TextType => .TextType
    | .SyntaxType => .SyntaxType
    | .PairType a b => .PairType (toTy a) (toTy b)
    | .SumType a b => .SumType (toTy a) (toTy b)
    | .ListType a => .ListType (toTy a)
    | .ProgramType i o e => .ProgramType (toTyVec i) (toTyVec o) e
    | .ResourceType k => .ResourceType k
    termination_by x => sizeOf x
    decreasing_by
      all_goals
        simp only [noble_kernel.types.Ty.PairType.sizeOf_spec,
        noble_kernel.types.Ty.SumType.sizeOf_spec,
        noble_kernel.types.Ty.ListType.sizeOf_spec,
        noble_kernel.types.Ty.ProgramType.sizeOf_spec]
        omega

  def toTyVec (v : alloc.vec.Vec noble_kernel.types.Ty) : alloc.vec.Vec _root_.noble_kernel.types.Ty :=
    match v with
    | ⟨s⟩ => ⟨toTySlice s⟩
    termination_by sizeOf v
    decreasing_by
      simp only [alloc.vec.Vec.mk.sizeOf_spec]
      omega

  def toTySlice (s : Slice noble_kernel.types.Ty) : Slice _root_.noble_kernel.types.Ty :=
    match s with
    | ⟨n, xs, bound⟩ => ⟨n, toTyListN n xs, bound⟩
    termination_by sizeOf s
    decreasing_by
      simp only [Slice.mk.sizeOf_spec]
      omega

  def toTyListN (n : Nat) (xs : ListN noble_kernel.types.Ty n) : ListN _root_.noble_kernel.types.Ty n :=
    match xs with
    | .nil => .nil
    | .cons x rest => .cons (toTy x) (toTyListN _ rest)
    termination_by sizeOf xs
    decreasing_by
      all_goals
        simp only [ListN.cons.sizeOf_spec]
        omega
end

mutual
  def fromTy : _root_.noble_kernel.types.Ty → noble_kernel.types.Ty
    | .UnitType => .UnitType
    | .BoolType => .BoolType
    | .I64Type => .I64Type
    | .TextType => .TextType
    | .SyntaxType => .SyntaxType
    | .PairType a b => .PairType (fromTy a) (fromTy b)
    | .SumType a b => .SumType (fromTy a) (fromTy b)
    | .ListType a => .ListType (fromTy a)
    | .ProgramType i o e => .ProgramType (fromTyVec i) (fromTyVec o) e
    | .ResourceType k => .ResourceType k
    termination_by x => sizeOf x
    decreasing_by
      all_goals
        simp only [_root_.noble_kernel.types.Ty.PairType.sizeOf_spec,
        _root_.noble_kernel.types.Ty.SumType.sizeOf_spec,
        _root_.noble_kernel.types.Ty.ListType.sizeOf_spec,
        _root_.noble_kernel.types.Ty.ProgramType.sizeOf_spec]
        omega

  def fromTyVec (v : alloc.vec.Vec _root_.noble_kernel.types.Ty) : alloc.vec.Vec noble_kernel.types.Ty :=
    match v with
    | ⟨s⟩ => ⟨fromTySlice s⟩
    termination_by sizeOf v
    decreasing_by
      simp only [alloc.vec.Vec.mk.sizeOf_spec]
      omega

  def fromTySlice (s : Slice _root_.noble_kernel.types.Ty) : Slice noble_kernel.types.Ty :=
    match s with
    | ⟨n, xs, bound⟩ => ⟨n, fromTyListN n xs, bound⟩
    termination_by sizeOf s
    decreasing_by
      simp only [Slice.mk.sizeOf_spec]
      omega

  def fromTyListN (n : Nat) (xs : ListN _root_.noble_kernel.types.Ty n) : ListN noble_kernel.types.Ty n :=
    match xs with
    | .nil => .nil
    | .cons x rest => .cons (fromTy x) (fromTyListN _ rest)
    termination_by sizeOf xs
    decreasing_by
      all_goals
        simp only [ListN.cons.sizeOf_spec]
        omega
end

mutual
  theorem fromTy_toTy (x : noble_kernel.types.Ty) : fromTy (toTy x) = x := by
    cases x with
    | UnitType =>
      simp only [toTy, fromTy]
    | BoolType =>
      simp only [toTy, fromTy]
    | I64Type =>
      simp only [toTy, fromTy]
    | TextType =>
      simp only [toTy, fromTy]
    | SyntaxType =>
      simp only [toTy, fromTy]
    | PairType a b =>
      simp only [toTy, fromTy, fromTy_toTy a, fromTy_toTy b]
    | SumType a b =>
      simp only [toTy, fromTy, fromTy_toTy a, fromTy_toTy b]
    | ListType a =>
      simp only [toTy, fromTy, fromTy_toTy a]
    | ProgramType i o e =>
      simp only [toTy, fromTy, fromTyVec_toTyVec i, fromTyVec_toTyVec o]
    | ResourceType k =>
      simp only [toTy, fromTy]
    termination_by sizeOf x
    decreasing_by
      all_goals
        simp only [noble_kernel.types.Ty.PairType.sizeOf_spec,
        noble_kernel.types.Ty.SumType.sizeOf_spec,
        noble_kernel.types.Ty.ListType.sizeOf_spec,
        noble_kernel.types.Ty.ProgramType.sizeOf_spec]
        omega

  theorem fromTyVec_toTyVec (v : alloc.vec.Vec noble_kernel.types.Ty) :
      fromTyVec (toTyVec v) = v := by
    cases v with
    | mk s => simp only [toTyVec, fromTyVec, fromTySlice_toTySlice s]
    termination_by sizeOf v
    decreasing_by
      simp only [alloc.vec.Vec.mk.sizeOf_spec]
      omega

  theorem fromTySlice_toTySlice (s : Slice noble_kernel.types.Ty) :
      fromTySlice (toTySlice s) = s := by
    cases s with
    | mk n xs bound =>
      simp only [toTySlice, fromTySlice, fromTyListN_toTyListN n xs]
    termination_by sizeOf s
    decreasing_by
      simp only [Slice.mk.sizeOf_spec]
      omega

  theorem fromTyListN_toTyListN (n : Nat) (xs : ListN noble_kernel.types.Ty n) :
      fromTyListN n (toTyListN n xs) = xs := by
    cases xs with
    | nil => simp only [toTyListN, fromTyListN]
    | @cons n x rest =>
      simp only [toTyListN, fromTyListN, fromTy_toTy x,
        fromTyListN_toTyListN n rest]
    termination_by sizeOf xs
    decreasing_by
      all_goals
        simp only [ListN.cons.sizeOf_spec]
        omega
end

attribute [simp] fromTy_toTy fromTyVec_toTyVec
  fromTySlice_toTySlice fromTyListN_toTyListN

mutual
  theorem toTy_fromTy (x : _root_.noble_kernel.types.Ty) : toTy (fromTy x) = x := by
    cases x with
    | UnitType =>
      simp only [fromTy, toTy]
    | BoolType =>
      simp only [fromTy, toTy]
    | I64Type =>
      simp only [fromTy, toTy]
    | TextType =>
      simp only [fromTy, toTy]
    | SyntaxType =>
      simp only [fromTy, toTy]
    | PairType a b =>
      simp only [fromTy, toTy, toTy_fromTy a, toTy_fromTy b]
    | SumType a b =>
      simp only [fromTy, toTy, toTy_fromTy a, toTy_fromTy b]
    | ListType a =>
      simp only [fromTy, toTy, toTy_fromTy a]
    | ProgramType i o e =>
      simp only [fromTy, toTy, toTyVec_fromTyVec i, toTyVec_fromTyVec o]
    | ResourceType k =>
      simp only [fromTy, toTy]
    termination_by sizeOf x
    decreasing_by
      all_goals
        simp only [_root_.noble_kernel.types.Ty.PairType.sizeOf_spec,
        _root_.noble_kernel.types.Ty.SumType.sizeOf_spec,
        _root_.noble_kernel.types.Ty.ListType.sizeOf_spec,
        _root_.noble_kernel.types.Ty.ProgramType.sizeOf_spec]
        omega

  theorem toTyVec_fromTyVec (v : alloc.vec.Vec _root_.noble_kernel.types.Ty) :
      toTyVec (fromTyVec v) = v := by
    cases v with
    | mk s => simp only [fromTyVec, toTyVec, toTySlice_fromTySlice s]
    termination_by sizeOf v
    decreasing_by
      simp only [alloc.vec.Vec.mk.sizeOf_spec]
      omega

  theorem toTySlice_fromTySlice (s : Slice _root_.noble_kernel.types.Ty) :
      toTySlice (fromTySlice s) = s := by
    cases s with
    | mk n xs bound =>
      simp only [fromTySlice, toTySlice, toTyListN_fromTyListN n xs]
    termination_by sizeOf s
    decreasing_by
      simp only [Slice.mk.sizeOf_spec]
      omega

  theorem toTyListN_fromTyListN (n : Nat) (xs : ListN _root_.noble_kernel.types.Ty n) :
      toTyListN n (fromTyListN n xs) = xs := by
    cases xs with
    | nil => simp only [fromTyListN, toTyListN]
    | @cons n x rest =>
      simp only [fromTyListN, toTyListN, toTy_fromTy x,
        toTyListN_fromTyListN n rest]
    termination_by sizeOf xs
    decreasing_by
      all_goals
        simp only [ListN.cons.sizeOf_spec]
        omega
end

attribute [simp] toTy_fromTy toTyVec_fromTyVec
  toTySlice_fromTySlice toTyListN_fromTyListN

def toVariableKind : noble_kernel.words.VariableKind → _root_.noble_kernel.words.VariableKind
  | .Stack => .Stack
  | .Value => .Value
  | .Effect => .Effect

def fromVariableKind : _root_.noble_kernel.words.VariableKind → noble_kernel.words.VariableKind
  | .Stack => .Stack
  | .Value => .Value
  | .Effect => .Effect

@[simp] theorem fromVariableKind_toVariableKind (x : noble_kernel.words.VariableKind) :
    fromVariableKind (toVariableKind x) = x := by
  cases x <;> simp [fromVariableKind, toVariableKind]

@[simp] theorem toVariableKind_fromVariableKind (x : _root_.noble_kernel.words.VariableKind) :
    toVariableKind (fromVariableKind x) = x := by
  cases x <;> simp [toVariableKind, fromVariableKind]

def toEffectSlot : noble_kernel.shapes.EffectSlot → _root_.noble_kernel.shapes.EffectSlot
  | .Effect id => .Effect id
  | .Var id => .Var id

def fromEffectSlot : _root_.noble_kernel.shapes.EffectSlot → noble_kernel.shapes.EffectSlot
  | .Effect id => .Effect id
  | .Var id => .Var id

@[simp] theorem fromEffectSlot_toEffectSlot (x : noble_kernel.shapes.EffectSlot) :
    fromEffectSlot (toEffectSlot x) = x := by
  cases x <;> simp [fromEffectSlot, toEffectSlot]

@[simp] theorem toEffectSlot_fromEffectSlot (x : _root_.noble_kernel.shapes.EffectSlot) :
    toEffectSlot (fromEffectSlot x) = x := by
  cases x <;> simp [toEffectSlot, fromEffectSlot]

mutual
  def toPattern : noble_kernel.shapes.Pattern → _root_.noble_kernel.shapes.Pattern
    | .UnitPattern => .UnitPattern
    | .BoolPattern => .BoolPattern
    | .I64Pattern => .I64Pattern
    | .TextPattern => .TextPattern
    | .SyntaxPattern => .SyntaxPattern
    | .PairPattern a b => .PairPattern (toPattern a) (toPattern b)
    | .SumPattern a b => .SumPattern (toPattern a) (toPattern b)
    | .ListPattern a => .ListPattern (toPattern a)
    | .ProgramPattern i o e => .ProgramPattern (toPatternVec i) (toPatternVec o) (mapVec toEffectSlot e)
    | .ResourcePattern k => .ResourcePattern k
    | .VarPattern id => .VarPattern id
    | .StackVarPattern id => .StackVarPattern id
    termination_by x => sizeOf x
    decreasing_by
      all_goals
        simp only [noble_kernel.shapes.Pattern.PairPattern.sizeOf_spec,
        noble_kernel.shapes.Pattern.SumPattern.sizeOf_spec,
        noble_kernel.shapes.Pattern.ListPattern.sizeOf_spec,
        noble_kernel.shapes.Pattern.ProgramPattern.sizeOf_spec]
        omega

  def toPatternVec (v : alloc.vec.Vec noble_kernel.shapes.Pattern) : alloc.vec.Vec _root_.noble_kernel.shapes.Pattern :=
    match v with
    | ⟨s⟩ => ⟨toPatternSlice s⟩
    termination_by sizeOf v
    decreasing_by
      simp only [alloc.vec.Vec.mk.sizeOf_spec]
      omega

  def toPatternSlice (s : Slice noble_kernel.shapes.Pattern) : Slice _root_.noble_kernel.shapes.Pattern :=
    match s with
    | ⟨n, xs, bound⟩ => ⟨n, toPatternListN n xs, bound⟩
    termination_by sizeOf s
    decreasing_by
      simp only [Slice.mk.sizeOf_spec]
      omega

  def toPatternListN (n : Nat) (xs : ListN noble_kernel.shapes.Pattern n) : ListN _root_.noble_kernel.shapes.Pattern n :=
    match xs with
    | .nil => .nil
    | .cons x rest => .cons (toPattern x) (toPatternListN _ rest)
    termination_by sizeOf xs
    decreasing_by
      all_goals
        simp only [ListN.cons.sizeOf_spec]
        omega
end

mutual
  def fromPattern : _root_.noble_kernel.shapes.Pattern → noble_kernel.shapes.Pattern
    | .UnitPattern => .UnitPattern
    | .BoolPattern => .BoolPattern
    | .I64Pattern => .I64Pattern
    | .TextPattern => .TextPattern
    | .SyntaxPattern => .SyntaxPattern
    | .PairPattern a b => .PairPattern (fromPattern a) (fromPattern b)
    | .SumPattern a b => .SumPattern (fromPattern a) (fromPattern b)
    | .ListPattern a => .ListPattern (fromPattern a)
    | .ProgramPattern i o e => .ProgramPattern (fromPatternVec i) (fromPatternVec o) (mapVec fromEffectSlot e)
    | .ResourcePattern k => .ResourcePattern k
    | .VarPattern id => .VarPattern id
    | .StackVarPattern id => .StackVarPattern id
    termination_by x => sizeOf x
    decreasing_by
      all_goals
        simp only [_root_.noble_kernel.shapes.Pattern.PairPattern.sizeOf_spec,
        _root_.noble_kernel.shapes.Pattern.SumPattern.sizeOf_spec,
        _root_.noble_kernel.shapes.Pattern.ListPattern.sizeOf_spec,
        _root_.noble_kernel.shapes.Pattern.ProgramPattern.sizeOf_spec]
        omega

  def fromPatternVec (v : alloc.vec.Vec _root_.noble_kernel.shapes.Pattern) : alloc.vec.Vec noble_kernel.shapes.Pattern :=
    match v with
    | ⟨s⟩ => ⟨fromPatternSlice s⟩
    termination_by sizeOf v
    decreasing_by
      simp only [alloc.vec.Vec.mk.sizeOf_spec]
      omega

  def fromPatternSlice (s : Slice _root_.noble_kernel.shapes.Pattern) : Slice noble_kernel.shapes.Pattern :=
    match s with
    | ⟨n, xs, bound⟩ => ⟨n, fromPatternListN n xs, bound⟩
    termination_by sizeOf s
    decreasing_by
      simp only [Slice.mk.sizeOf_spec]
      omega

  def fromPatternListN (n : Nat) (xs : ListN _root_.noble_kernel.shapes.Pattern n) : ListN noble_kernel.shapes.Pattern n :=
    match xs with
    | .nil => .nil
    | .cons x rest => .cons (fromPattern x) (fromPatternListN _ rest)
    termination_by sizeOf xs
    decreasing_by
      all_goals
        simp only [ListN.cons.sizeOf_spec]
        omega
end

mutual
  theorem fromPattern_toPattern (x : noble_kernel.shapes.Pattern) : fromPattern (toPattern x) = x := by
    cases x with
    | UnitPattern =>
      simp only [toPattern, fromPattern]
    | BoolPattern =>
      simp only [toPattern, fromPattern]
    | I64Pattern =>
      simp only [toPattern, fromPattern]
    | TextPattern =>
      simp only [toPattern, fromPattern]
    | SyntaxPattern =>
      simp only [toPattern, fromPattern]
    | PairPattern a b =>
      simp only [toPattern, fromPattern, fromPattern_toPattern a, fromPattern_toPattern b]
    | SumPattern a b =>
      simp only [toPattern, fromPattern, fromPattern_toPattern a, fromPattern_toPattern b]
    | ListPattern a =>
      simp only [toPattern, fromPattern, fromPattern_toPattern a]
    | ProgramPattern i o e =>
      simp only [toPattern, fromPattern, fromPatternVec_toPatternVec i, fromPatternVec_toPatternVec o, mapVec_mapVec, fromEffectSlot_toEffectSlot, mapVec_id]
    | ResourcePattern k =>
      simp only [toPattern, fromPattern]
    | VarPattern id =>
      simp only [toPattern, fromPattern]
    | StackVarPattern id =>
      simp only [toPattern, fromPattern]
    termination_by sizeOf x
    decreasing_by
      all_goals
        simp only [noble_kernel.shapes.Pattern.PairPattern.sizeOf_spec,
        noble_kernel.shapes.Pattern.SumPattern.sizeOf_spec,
        noble_kernel.shapes.Pattern.ListPattern.sizeOf_spec,
        noble_kernel.shapes.Pattern.ProgramPattern.sizeOf_spec]
        omega

  theorem fromPatternVec_toPatternVec (v : alloc.vec.Vec noble_kernel.shapes.Pattern) :
      fromPatternVec (toPatternVec v) = v := by
    cases v with
    | mk s => simp only [toPatternVec, fromPatternVec, fromPatternSlice_toPatternSlice s]
    termination_by sizeOf v
    decreasing_by
      simp only [alloc.vec.Vec.mk.sizeOf_spec]
      omega

  theorem fromPatternSlice_toPatternSlice (s : Slice noble_kernel.shapes.Pattern) :
      fromPatternSlice (toPatternSlice s) = s := by
    cases s with
    | mk n xs bound =>
      simp only [toPatternSlice, fromPatternSlice, fromPatternListN_toPatternListN n xs]
    termination_by sizeOf s
    decreasing_by
      simp only [Slice.mk.sizeOf_spec]
      omega

  theorem fromPatternListN_toPatternListN (n : Nat) (xs : ListN noble_kernel.shapes.Pattern n) :
      fromPatternListN n (toPatternListN n xs) = xs := by
    cases xs with
    | nil => simp only [toPatternListN, fromPatternListN]
    | @cons n x rest =>
      simp only [toPatternListN, fromPatternListN, fromPattern_toPattern x,
        fromPatternListN_toPatternListN n rest]
    termination_by sizeOf xs
    decreasing_by
      all_goals
        simp only [ListN.cons.sizeOf_spec]
        omega
end

attribute [simp] fromPattern_toPattern fromPatternVec_toPatternVec
  fromPatternSlice_toPatternSlice fromPatternListN_toPatternListN

mutual
  theorem toPattern_fromPattern (x : _root_.noble_kernel.shapes.Pattern) : toPattern (fromPattern x) = x := by
    cases x with
    | UnitPattern =>
      simp only [fromPattern, toPattern]
    | BoolPattern =>
      simp only [fromPattern, toPattern]
    | I64Pattern =>
      simp only [fromPattern, toPattern]
    | TextPattern =>
      simp only [fromPattern, toPattern]
    | SyntaxPattern =>
      simp only [fromPattern, toPattern]
    | PairPattern a b =>
      simp only [fromPattern, toPattern, toPattern_fromPattern a, toPattern_fromPattern b]
    | SumPattern a b =>
      simp only [fromPattern, toPattern, toPattern_fromPattern a, toPattern_fromPattern b]
    | ListPattern a =>
      simp only [fromPattern, toPattern, toPattern_fromPattern a]
    | ProgramPattern i o e =>
      simp only [fromPattern, toPattern, toPatternVec_fromPatternVec i, toPatternVec_fromPatternVec o, mapVec_mapVec, toEffectSlot_fromEffectSlot, mapVec_id]
    | ResourcePattern k =>
      simp only [fromPattern, toPattern]
    | VarPattern id =>
      simp only [fromPattern, toPattern]
    | StackVarPattern id =>
      simp only [fromPattern, toPattern]
    termination_by sizeOf x
    decreasing_by
      all_goals
        simp only [_root_.noble_kernel.shapes.Pattern.PairPattern.sizeOf_spec,
        _root_.noble_kernel.shapes.Pattern.SumPattern.sizeOf_spec,
        _root_.noble_kernel.shapes.Pattern.ListPattern.sizeOf_spec,
        _root_.noble_kernel.shapes.Pattern.ProgramPattern.sizeOf_spec]
        omega

  theorem toPatternVec_fromPatternVec (v : alloc.vec.Vec _root_.noble_kernel.shapes.Pattern) :
      toPatternVec (fromPatternVec v) = v := by
    cases v with
    | mk s => simp only [fromPatternVec, toPatternVec, toPatternSlice_fromPatternSlice s]
    termination_by sizeOf v
    decreasing_by
      simp only [alloc.vec.Vec.mk.sizeOf_spec]
      omega

  theorem toPatternSlice_fromPatternSlice (s : Slice _root_.noble_kernel.shapes.Pattern) :
      toPatternSlice (fromPatternSlice s) = s := by
    cases s with
    | mk n xs bound =>
      simp only [fromPatternSlice, toPatternSlice, toPatternListN_fromPatternListN n xs]
    termination_by sizeOf s
    decreasing_by
      simp only [Slice.mk.sizeOf_spec]
      omega

  theorem toPatternListN_fromPatternListN (n : Nat) (xs : ListN _root_.noble_kernel.shapes.Pattern n) :
      toPatternListN n (fromPatternListN n xs) = xs := by
    cases xs with
    | nil => simp only [fromPatternListN, toPatternListN]
    | @cons n x rest =>
      simp only [fromPatternListN, toPatternListN, toPattern_fromPattern x,
        toPatternListN_fromPatternListN n rest]
    termination_by sizeOf xs
    decreasing_by
      all_goals
        simp only [ListN.cons.sizeOf_spec]
        omega
end

attribute [simp] toPattern_fromPattern toPatternVec_fromPatternVec
  toPatternSlice_fromPatternSlice toPatternListN_fromPatternListN

def toScheme (x : noble_kernel.words.Scheme) : _root_.noble_kernel.words.Scheme :=
  { var_kinds := mapVec toVariableKind x.var_kinds
    stack_in := toPatternVec x.stack_in
    stack_out := toPatternVec x.stack_out
    effects := mapVec toEffectSlot x.effects }

def fromScheme (x : _root_.noble_kernel.words.Scheme) : noble_kernel.words.Scheme :=
  { var_kinds := mapVec fromVariableKind x.var_kinds
    stack_in := fromPatternVec x.stack_in
    stack_out := fromPatternVec x.stack_out
    effects := mapVec fromEffectSlot x.effects }

@[simp] theorem fromScheme_toScheme (x : noble_kernel.words.Scheme) :
    fromScheme (toScheme x) = x := by
  cases x
  simp [fromScheme, toScheme]

@[simp] theorem toScheme_fromScheme (x : _root_.noble_kernel.words.Scheme) :
    toScheme (fromScheme x) = x := by
  cases x
  simp [toScheme, fromScheme]

def toBehavior : noble_kernel.contracts.Behavior → _root_.noble_kernel.contracts.Behavior
  | .DupBehavior => .DupBehavior
  | .DropBehavior => .DropBehavior
  | .SwapBehavior => .SwapBehavior
  | .DipBehavior => .DipBehavior
  | .ArithBehavior => .ArithBehavior
  | .EqualsBehavior => .EqualsBehavior
  | .QuoteBehavior => .QuoteBehavior
  | .ComposeBehavior => .ComposeBehavior
  | .RunBehavior => .RunBehavior
  | .ReflectBehavior => .ReflectBehavior
  | .UnitBehavior => .UnitBehavior
  | .PairBehavior => .PairBehavior
  | .UnpairBehavior => .UnpairBehavior
  | .InlBehavior => .InlBehavior
  | .InrBehavior => .InrBehavior
  | .CaseBehavior => .CaseBehavior
  | .IfBehavior => .IfBehavior
  | .NilBehavior => .NilBehavior
  | .ConsBehavior => .ConsBehavior
  | .ListCaseBehavior => .ListCaseBehavior
  | .TestEmitBehavior => .TestEmitBehavior
  | .NamedBehavior => .NamedBehavior

def fromBehavior : _root_.noble_kernel.contracts.Behavior → noble_kernel.contracts.Behavior
  | .DupBehavior => .DupBehavior
  | .DropBehavior => .DropBehavior
  | .SwapBehavior => .SwapBehavior
  | .DipBehavior => .DipBehavior
  | .ArithBehavior => .ArithBehavior
  | .EqualsBehavior => .EqualsBehavior
  | .QuoteBehavior => .QuoteBehavior
  | .ComposeBehavior => .ComposeBehavior
  | .RunBehavior => .RunBehavior
  | .ReflectBehavior => .ReflectBehavior
  | .UnitBehavior => .UnitBehavior
  | .PairBehavior => .PairBehavior
  | .UnpairBehavior => .UnpairBehavior
  | .InlBehavior => .InlBehavior
  | .InrBehavior => .InrBehavior
  | .CaseBehavior => .CaseBehavior
  | .IfBehavior => .IfBehavior
  | .NilBehavior => .NilBehavior
  | .ConsBehavior => .ConsBehavior
  | .ListCaseBehavior => .ListCaseBehavior
  | .TestEmitBehavior => .TestEmitBehavior
  | .NamedBehavior => .NamedBehavior

@[simp] theorem fromBehavior_toBehavior (x : noble_kernel.contracts.Behavior) :
    fromBehavior (toBehavior x) = x := by
  cases x <;> simp [fromBehavior, toBehavior]

@[simp] theorem toBehavior_fromBehavior (x : _root_.noble_kernel.contracts.Behavior) :
    toBehavior (fromBehavior x) = x := by
  cases x <;> simp [toBehavior, fromBehavior]

def toSchemaDecl (x : noble_kernel.contracts.SchemaDecl) : _root_.noble_kernel.contracts.SchemaDecl :=
  { id := x.id
    scheme := toScheme x.scheme
    recursive := x.recursive }

def fromSchemaDecl (x : _root_.noble_kernel.contracts.SchemaDecl) : noble_kernel.contracts.SchemaDecl :=
  { id := x.id
    scheme := fromScheme x.scheme
    recursive := x.recursive }

@[simp] theorem fromSchemaDecl_toSchemaDecl (x : noble_kernel.contracts.SchemaDecl) :
    fromSchemaDecl (toSchemaDecl x) = x := by
  cases x
  simp [fromSchemaDecl, toSchemaDecl]

@[simp] theorem toSchemaDecl_fromSchemaDecl (x : _root_.noble_kernel.contracts.SchemaDecl) :
    toSchemaDecl (fromSchemaDecl x) = x := by
  cases x
  simp [toSchemaDecl, fromSchemaDecl]

def toEnv (x : noble_kernel.contracts.Env) : _root_.noble_kernel.contracts.Env :=
  { defs := mapVec toScheme x.defs
    kinds := mapVec toBehavior x.kinds
    deps := x.deps
    schemas := mapVec toSchemaDecl x.schemas
    effects := x.effects }

def fromEnv (x : _root_.noble_kernel.contracts.Env) : noble_kernel.contracts.Env :=
  { defs := mapVec fromScheme x.defs
    kinds := mapVec fromBehavior x.kinds
    deps := x.deps
    schemas := mapVec fromSchemaDecl x.schemas
    effects := x.effects }

@[simp] theorem fromEnv_toEnv (x : noble_kernel.contracts.Env) :
    fromEnv (toEnv x) = x := by
  cases x
  simp [fromEnv, toEnv]

@[simp] theorem toEnv_fromEnv (x : _root_.noble_kernel.contracts.Env) :
    toEnv (fromEnv x) = x := by
  cases x
  simp [toEnv, fromEnv]

def toBinding : noble_kernel.words.Binding → _root_.noble_kernel.words.Binding
  | .Stack stack => .Stack (toTyVec stack)
  | .Value ty => .Value (toTy ty)
  | .Effect effects => .Effect effects
  | .Ref id => .Ref id

def fromBinding : _root_.noble_kernel.words.Binding → noble_kernel.words.Binding
  | .Stack stack => .Stack (fromTyVec stack)
  | .Value ty => .Value (fromTy ty)
  | .Effect effects => .Effect effects
  | .Ref id => .Ref id

@[simp] theorem fromBinding_toBinding (x : noble_kernel.words.Binding) :
    fromBinding (toBinding x) = x := by
  cases x <;> simp [fromBinding, toBinding]

@[simp] theorem toBinding_fromBinding (x : _root_.noble_kernel.words.Binding) :
    toBinding (fromBinding x) = x := by
  cases x <;> simp [toBinding, fromBinding]

def toInst (x : noble_kernel.words.Inst) : _root_.noble_kernel.words.Inst :=
  { bindings := mapVec toBinding x.bindings }

def fromInst (x : _root_.noble_kernel.words.Inst) : noble_kernel.words.Inst :=
  { bindings := mapVec fromBinding x.bindings }

@[simp] theorem fromInst_toInst (x : noble_kernel.words.Inst) :
    fromInst (toInst x) = x := by
  cases x
  simp [fromInst, toInst]

@[simp] theorem toInst_fromInst (x : _root_.noble_kernel.words.Inst) :
    toInst (fromInst x) = x := by
  cases x
  simp [toInst, fromInst]

def toLit : noble_kernel.untrusted.Lit → _root_.noble_kernel.untrusted.Lit
  | .I64Lit value => .I64Lit value
  | .BoolLit value => .BoolLit value
  | .TextLit => .TextLit
  | .UnitLit => .UnitLit

def fromLit : _root_.noble_kernel.untrusted.Lit → noble_kernel.untrusted.Lit
  | .I64Lit value => .I64Lit value
  | .BoolLit value => .BoolLit value
  | .TextLit => .TextLit
  | .UnitLit => .UnitLit

@[simp] theorem fromLit_toLit (x : noble_kernel.untrusted.Lit) :
    fromLit (toLit x) = x := by
  cases x <;> simp [fromLit, toLit]

@[simp] theorem toLit_fromLit (x : _root_.noble_kernel.untrusted.Lit) :
    toLit (fromLit x) = x := by
  cases x <;> simp [toLit, fromLit]

def toNode : noble_kernel.untrusted.Node → _root_.noble_kernel.untrusted.Node
  | .Literal lit inst => .Literal (toLit lit) (toInst inst)
  | .Invocation id inst => .Invocation id (toInst inst)
  | .Quotation body inst => .Quotation body (toInst inst)

def fromNode : _root_.noble_kernel.untrusted.Node → noble_kernel.untrusted.Node
  | .Literal lit inst => .Literal (fromLit lit) (fromInst inst)
  | .Invocation id inst => .Invocation id (fromInst inst)
  | .Quotation body inst => .Quotation body (fromInst inst)

@[simp] theorem fromNode_toNode (x : noble_kernel.untrusted.Node) :
    fromNode (toNode x) = x := by
  cases x <;> simp [fromNode, toNode]

@[simp] theorem toNode_fromNode (x : _root_.noble_kernel.untrusted.Node) :
    toNode (fromNode x) = x := by
  cases x <;> simp [toNode, fromNode]

def toCandidate (x : noble_kernel.untrusted.Candidate) : _root_.noble_kernel.untrusted.Candidate :=
  { format := x.format
    revision := x.revision
    nodes := mapVec toNode x.nodes
    body := x.body }

def fromCandidate (x : _root_.noble_kernel.untrusted.Candidate) : noble_kernel.untrusted.Candidate :=
  { format := x.format
    revision := x.revision
    nodes := mapVec fromNode x.nodes
    body := x.body }

@[simp] theorem fromCandidate_toCandidate (x : noble_kernel.untrusted.Candidate) :
    fromCandidate (toCandidate x) = x := by
  cases x
  simp [fromCandidate, toCandidate]

@[simp] theorem toCandidate_fromCandidate (x : _root_.noble_kernel.untrusted.Candidate) :
    toCandidate (fromCandidate x) = x := by
  cases x
  simp [toCandidate, fromCandidate]

def toLimits (x : noble_kernel.untrusted.Limits) : _root_.noble_kernel.untrusted.Limits :=
  { bytes := x.bytes
    nodes := x.nodes
    depth := x.depth
    type_size := x.type_size
    stack_height := x.stack_height
    work := x.work
    diagnostics := x.diagnostics }

def fromLimits (x : _root_.noble_kernel.untrusted.Limits) : noble_kernel.untrusted.Limits :=
  { bytes := x.bytes
    nodes := x.nodes
    depth := x.depth
    type_size := x.type_size
    stack_height := x.stack_height
    work := x.work
    diagnostics := x.diagnostics }

@[simp] theorem fromLimits_toLimits (x : noble_kernel.untrusted.Limits) :
    fromLimits (toLimits x) = x := by
  cases x
  simp [fromLimits, toLimits]

@[simp] theorem toLimits_fromLimits (x : _root_.noble_kernel.untrusted.Limits) :
    toLimits (fromLimits x) = x := by
  cases x
  simp [toLimits, fromLimits]

def toExpected (x : noble_kernel.untrusted.Expected) : _root_.noble_kernel.untrusted.Expected :=
  { stack_in := toTyVec x.stack_in
    stack_out := toTyVec x.stack_out
    allowed_effects := x.allowed_effects }

def fromExpected (x : _root_.noble_kernel.untrusted.Expected) : noble_kernel.untrusted.Expected :=
  { stack_in := fromTyVec x.stack_in
    stack_out := fromTyVec x.stack_out
    allowed_effects := x.allowed_effects }

@[simp] theorem fromExpected_toExpected (x : noble_kernel.untrusted.Expected) :
    fromExpected (toExpected x) = x := by
  cases x
  simp [fromExpected, toExpected]

@[simp] theorem toExpected_fromExpected (x : _root_.noble_kernel.untrusted.Expected) :
    toExpected (fromExpected x) = x := by
  cases x
  simp [toExpected, fromExpected]

def toRequest (x : noble_kernel.untrusted.Request) : _root_.noble_kernel.untrusted.Request :=
  { input_bytes := x.input_bytes
    expected := toExpected x.expected
    limits := toLimits x.limits }

def fromRequest (x : _root_.noble_kernel.untrusted.Request) : noble_kernel.untrusted.Request :=
  { input_bytes := x.input_bytes
    expected := fromExpected x.expected
    limits := fromLimits x.limits }

@[simp] theorem fromRequest_toRequest (x : noble_kernel.untrusted.Request) :
    fromRequest (toRequest x) = x := by
  cases x
  simp [fromRequest, toRequest]

@[simp] theorem toRequest_fromRequest (x : _root_.noble_kernel.untrusted.Request) :
    toRequest (fromRequest x) = x := by
  cases x
  simp [toRequest, fromRequest]

def toInterface (x : noble_kernel.untrusted.Interface) : _root_.noble_kernel.untrusted.Interface :=
  { stack_in := toTyVec x.stack_in
    stack_out := toTyVec x.stack_out
    effects := x.effects }

def fromInterface (x : _root_.noble_kernel.untrusted.Interface) : noble_kernel.untrusted.Interface :=
  { stack_in := fromTyVec x.stack_in
    stack_out := fromTyVec x.stack_out
    effects := x.effects }

@[simp] theorem fromInterface_toInterface (x : noble_kernel.untrusted.Interface) :
    fromInterface (toInterface x) = x := by
  cases x
  simp [fromInterface, toInterface]

@[simp] theorem toInterface_fromInterface (x : _root_.noble_kernel.untrusted.Interface) :
    toInterface (fromInterface x) = x := by
  cases x
  simp [toInterface, fromInterface]

def toDerivation (x : noble_kernel.untrusted.Derivation) : _root_.noble_kernel.untrusted.Derivation :=
  { node := x.node
    interface := toInterface x.interface }

def fromDerivation (x : _root_.noble_kernel.untrusted.Derivation) : noble_kernel.untrusted.Derivation :=
  { node := x.node
    interface := fromInterface x.interface }

@[simp] theorem fromDerivation_toDerivation (x : noble_kernel.untrusted.Derivation) :
    fromDerivation (toDerivation x) = x := by
  cases x
  simp [fromDerivation, toDerivation]

@[simp] theorem toDerivation_fromDerivation (x : _root_.noble_kernel.untrusted.Derivation) :
    toDerivation (fromDerivation x) = x := by
  cases x
  simp [toDerivation, fromDerivation]

def toChecked (x : noble_kernel.untrusted.Checked) : _root_.noble_kernel.untrusted.Checked :=
  { interface := toInterface x.interface
    derivations := mapVec toDerivation x.derivations }

def fromChecked (x : _root_.noble_kernel.untrusted.Checked) : noble_kernel.untrusted.Checked :=
  { interface := fromInterface x.interface
    derivations := mapVec fromDerivation x.derivations }

@[simp] theorem fromChecked_toChecked (x : noble_kernel.untrusted.Checked) :
    fromChecked (toChecked x) = x := by
  cases x
  simp [fromChecked, toChecked]

@[simp] theorem toChecked_fromChecked (x : _root_.noble_kernel.untrusted.Checked) :
    toChecked (fromChecked x) = x := by
  cases x
  simp [toChecked, fromChecked]

def toConstraint : noble_kernel.untrusted.Constraint → _root_.noble_kernel.untrusted.Constraint
  | .StackJoin => .StackJoin
  | .StackOrder => .StackOrder
  | .EffectInclusion id => .EffectInclusion id
  | .Eligibility ty => .Eligibility (toTy ty)
  | .UnknownEffect id => .UnknownEffect id
  | .InstantiationKind => .InstantiationKind
  | .InstantiationArity => .InstantiationArity
  | .MalformedReference id => .MalformedReference id
  | .UnknownDefinition id => .UnknownDefinition id
  | .CyclicWitness => .CyclicWitness

def fromConstraint : _root_.noble_kernel.untrusted.Constraint → noble_kernel.untrusted.Constraint
  | .StackJoin => .StackJoin
  | .StackOrder => .StackOrder
  | .EffectInclusion id => .EffectInclusion id
  | .Eligibility ty => .Eligibility (fromTy ty)
  | .UnknownEffect id => .UnknownEffect id
  | .InstantiationKind => .InstantiationKind
  | .InstantiationArity => .InstantiationArity
  | .MalformedReference id => .MalformedReference id
  | .UnknownDefinition id => .UnknownDefinition id
  | .CyclicWitness => .CyclicWitness

@[simp] theorem fromConstraint_toConstraint (x : noble_kernel.untrusted.Constraint) :
    fromConstraint (toConstraint x) = x := by
  cases x <;> simp [fromConstraint, toConstraint]

@[simp] theorem toConstraint_fromConstraint (x : _root_.noble_kernel.untrusted.Constraint) :
    toConstraint (fromConstraint x) = x := by
  cases x <;> simp [toConstraint, fromConstraint]

def toDiagnostic (x : noble_kernel.untrusted.Diagnostic) : _root_.noble_kernel.untrusted.Diagnostic :=
  { node := x.node
    «def» := x.«def»
    expected := toTyVec x.expected
    actual := toTyVec x.actual
    constraint := toConstraint x.constraint
    provenance_available := x.provenance_available
    truncated := x.truncated }

def fromDiagnostic (x : _root_.noble_kernel.untrusted.Diagnostic) : noble_kernel.untrusted.Diagnostic :=
  { node := x.node
    «def» := x.«def»
    expected := fromTyVec x.expected
    actual := fromTyVec x.actual
    constraint := fromConstraint x.constraint
    provenance_available := x.provenance_available
    truncated := x.truncated }

@[simp] theorem fromDiagnostic_toDiagnostic (x : noble_kernel.untrusted.Diagnostic) :
    fromDiagnostic (toDiagnostic x) = x := by
  cases x
  simp [fromDiagnostic, toDiagnostic]

@[simp] theorem toDiagnostic_fromDiagnostic (x : _root_.noble_kernel.untrusted.Diagnostic) :
    toDiagnostic (fromDiagnostic x) = x := by
  cases x
  simp [toDiagnostic, fromDiagnostic]

def toUnsupportedKind : noble_kernel.untrusted.UnsupportedKind → _root_.noble_kernel.untrusted.UnsupportedKind
  | .FormatRevision => .FormatRevision
  | .NodeForm => .NodeForm
  | .SchemeForm => .SchemeForm
  | .RecursiveDependency id => .RecursiveDependency id
  | .RecursiveSchema id => .RecursiveSchema id

def fromUnsupportedKind : _root_.noble_kernel.untrusted.UnsupportedKind → noble_kernel.untrusted.UnsupportedKind
  | .FormatRevision => .FormatRevision
  | .NodeForm => .NodeForm
  | .SchemeForm => .SchemeForm
  | .RecursiveDependency id => .RecursiveDependency id
  | .RecursiveSchema id => .RecursiveSchema id

@[simp] theorem fromUnsupportedKind_toUnsupportedKind (x : noble_kernel.untrusted.UnsupportedKind) :
    fromUnsupportedKind (toUnsupportedKind x) = x := by
  cases x <;> simp [fromUnsupportedKind, toUnsupportedKind]

@[simp] theorem toUnsupportedKind_fromUnsupportedKind (x : _root_.noble_kernel.untrusted.UnsupportedKind) :
    toUnsupportedKind (fromUnsupportedKind x) = x := by
  cases x <;> simp [toUnsupportedKind, fromUnsupportedKind]

def toLimitKind : noble_kernel.untrusted.LimitKind → _root_.noble_kernel.untrusted.LimitKind
  | .Bytes => .Bytes
  | .Nodes => .Nodes
  | .Depth => .Depth
  | .TypeSize => .TypeSize
  | .StackHeight => .StackHeight
  | .Work => .Work
  | .Diagnostics => .Diagnostics

def fromLimitKind : _root_.noble_kernel.untrusted.LimitKind → noble_kernel.untrusted.LimitKind
  | .Bytes => .Bytes
  | .Nodes => .Nodes
  | .Depth => .Depth
  | .TypeSize => .TypeSize
  | .StackHeight => .StackHeight
  | .Work => .Work
  | .Diagnostics => .Diagnostics

@[simp] theorem fromLimitKind_toLimitKind (x : noble_kernel.untrusted.LimitKind) :
    fromLimitKind (toLimitKind x) = x := by
  cases x <;> simp [fromLimitKind, toLimitKind]

@[simp] theorem toLimitKind_fromLimitKind (x : _root_.noble_kernel.untrusted.LimitKind) :
    toLimitKind (fromLimitKind x) = x := by
  cases x <;> simp [toLimitKind, fromLimitKind]

def toOutcome : noble_kernel.untrusted.Outcome → _root_.noble_kernel.untrusted.Outcome
  | .Accepted checked => .Accepted (toChecked checked)
  | .Invalid diagnostic => .Invalid (toDiagnostic diagnostic)
  | .Unsupported kind => .Unsupported (toUnsupportedKind kind)
  | .Exhausted kind => .Exhausted (toLimitKind kind)
  | .InternalFailure => .InternalFailure

def fromOutcome : _root_.noble_kernel.untrusted.Outcome → noble_kernel.untrusted.Outcome
  | .Accepted checked => .Accepted (fromChecked checked)
  | .Invalid diagnostic => .Invalid (fromDiagnostic diagnostic)
  | .Unsupported kind => .Unsupported (fromUnsupportedKind kind)
  | .Exhausted kind => .Exhausted (fromLimitKind kind)
  | .InternalFailure => .InternalFailure

@[simp] theorem fromOutcome_toOutcome (x : noble_kernel.untrusted.Outcome) :
    fromOutcome (toOutcome x) = x := by
  cases x <;> simp [fromOutcome, toOutcome]

@[simp] theorem toOutcome_fromOutcome (x : _root_.noble_kernel.untrusted.Outcome) :
    toOutcome (fromOutcome x) = x := by
  cases x <;> simp [toOutcome, fromOutcome]

def toDefect : noble_kernel.shapes.Defect → _root_.noble_kernel.shapes.Defect
  | .UnknownVariable => .UnknownVariable
  | .KindMismatch => .KindMismatch

def fromDefect : _root_.noble_kernel.shapes.Defect → noble_kernel.shapes.Defect
  | .UnknownVariable => .UnknownVariable
  | .KindMismatch => .KindMismatch

@[simp] theorem fromDefect_toDefect (x : noble_kernel.shapes.Defect) :
    fromDefect (toDefect x) = x := by
  cases x <;> simp [fromDefect, toDefect]

@[simp] theorem toDefect_fromDefect (x : _root_.noble_kernel.shapes.Defect) :
    toDefect (fromDefect x) = x := by
  cases x <;> simp [toDefect, fromDefect]

def toInstError : noble_kernel.words.InstError → _root_.noble_kernel.words.InstError
  | .KindMismatch => .KindMismatch
  | .UnknownVariable => .UnknownVariable
  | .ArityMismatch => .ArityMismatch
  | .OversizedStack => .OversizedStack
  | .OversizedType => .OversizedType
  | .OversizedEffects => .OversizedEffects
  | .CyclicWitness => .CyclicWitness
  | .WalkExhausted => .WalkExhausted

def fromInstError : _root_.noble_kernel.words.InstError → noble_kernel.words.InstError
  | .KindMismatch => .KindMismatch
  | .UnknownVariable => .UnknownVariable
  | .ArityMismatch => .ArityMismatch
  | .OversizedStack => .OversizedStack
  | .OversizedType => .OversizedType
  | .OversizedEffects => .OversizedEffects
  | .CyclicWitness => .CyclicWitness
  | .WalkExhausted => .WalkExhausted

@[simp] theorem fromInstError_toInstError (x : noble_kernel.words.InstError) :
    fromInstError (toInstError x) = x := by
  cases x <;> simp [fromInstError, toInstError]

@[simp] theorem toInstError_fromInstError (x : _root_.noble_kernel.words.InstError) :
    toInstError (fromInstError x) = x := by
  cases x <;> simp [toInstError, fromInstError]

end KernelBridge

/-! Every external call below delegates to the already extracted kernel.
The outer Result bind retains failures and divergence; only successful values
are converted. Constants retain the values from that same extraction. -/
open KernelBridge

/-- The real inherited acceptance checker, through the structural bridge. -/
def noble_kernel.acceptance.check (env : noble_kernel.contracts.Env)
    (request : noble_kernel.untrusted.Request) (candidate : noble_kernel.untrusted.Candidate) :
    Result noble_kernel.untrusted.Outcome := do
  let outcome ← _root_.noble_kernel.acceptance.check
    (toEnv env) (toRequest request) (toCandidate candidate)
  ok (fromOutcome outcome)

def noble_kernel.contracts.Env.scheme (env : noble_kernel.contracts.Env)
    (definition : noble_kernel.contracts.Definition) :
    Result (Option noble_kernel.words.Scheme) := do
  let scheme ← _root_.noble_kernel.contracts.Env.scheme (toEnv env) definition
  ok (scheme.map fromScheme)

def noble_kernel.contracts.environment :
    Result (core.result.Result noble_kernel.contracts.Env noble_kernel.shapes.Defect) := do
  let result ← _root_.noble_kernel.contracts.environment
  match result with
  | .Ok env => ok (.Ok (fromEnv env))
  | .Err error => ok (.Err (fromDefect error))

def noble_kernel.types.Ty.Insts.CoreCloneClone.clone (ty : noble_kernel.types.Ty) :
    Result noble_kernel.types.Ty := do
  let cloned ← _root_.noble_kernel.types.Ty.Insts.CoreCloneClone.clone (toTy ty)
  ok (fromTy cloned)

def noble_kernel.types.Ty.Insts.CoreCmpPartialEqTy.eq
    (left right : noble_kernel.types.Ty) : Result Bool :=
  _root_.noble_kernel.types.Ty.Insts.CoreCmpPartialEqTy.eq (toTy left) (toTy right)

def noble_kernel.types.Ty.Insts.CoreFmtDebug.fmt
    (ty : noble_kernel.types.Ty) (formatter : core.fmt.Formatter) :
    Result ((core.result.Result Unit core.fmt.Error) × core.fmt.Formatter) :=
  _root_.noble_kernel.types.Ty.Insts.CoreFmtDebug.fmt (toTy ty) formatter

def noble_kernel.types.EffSet.empty : Result noble_kernel.types.EffSet :=
  _root_.noble_kernel.types.EffSet.empty

def noble_kernel.types.EffSet.is_empty (effects : noble_kernel.types.EffSet) : Result Bool :=
  _root_.noble_kernel.types.EffSet.is_empty effects

def noble_kernel.types.Ty.program
    (stack_in stack_out : alloc.vec.Vec noble_kernel.types.Ty)
    (effects : noble_kernel.types.EffSet) : Result noble_kernel.types.Ty := do
  let ty ← _root_.noble_kernel.types.Ty.program (toTyVec stack_in) (toTyVec stack_out) effects
  ok (fromTy ty)

def noble_kernel.types.Ty.size (ty : noble_kernel.types.Ty) : Result (Option U32) :=
  _root_.noble_kernel.types.Ty.size (toTy ty)

def noble_kernel.untrusted.CANDIDATE_FORMAT : Result U32 :=
  ok _root_.noble_kernel.untrusted.CANDIDATE_FORMAT

def noble_kernel.untrusted.SEMANTIC_REVISION : Result U32 :=
  ok _root_.noble_kernel.untrusted.SEMANTIC_REVISION

def noble_kernel.untrusted.Lit.ty (lit : noble_kernel.untrusted.Lit) :
    Result noble_kernel.types.Ty := do
  let ty ← _root_.noble_kernel.untrusted.Lit.ty (toLit lit)
  ok (fromTy ty)

def noble_kernel.untrusted.Candidate.Insts.CoreCloneClone.clone
    (candidate : noble_kernel.untrusted.Candidate) : Result noble_kernel.untrusted.Candidate := do
  let cloned ← _root_.noble_kernel.untrusted.Candidate.Insts.CoreCloneClone.clone (toCandidate candidate)
  ok (fromCandidate cloned)

def noble_kernel.untrusted.Candidate.Insts.CoreFmtDebug.fmt
    (candidate : noble_kernel.untrusted.Candidate) (formatter : core.fmt.Formatter) :
    Result ((core.result.Result Unit core.fmt.Error) × core.fmt.Formatter) :=
  _root_.noble_kernel.untrusted.Candidate.Insts.CoreFmtDebug.fmt (toCandidate candidate) formatter

def noble_kernel.untrusted.Request.Insts.CoreCloneClone.clone
    (request : noble_kernel.untrusted.Request) : Result noble_kernel.untrusted.Request := do
  let cloned ← _root_.noble_kernel.untrusted.Request.Insts.CoreCloneClone.clone (toRequest request)
  ok (fromRequest cloned)

def noble_kernel.untrusted.Request.Insts.CoreFmtDebug.fmt
    (request : noble_kernel.untrusted.Request) (formatter : core.fmt.Formatter) :
    Result ((core.result.Result Unit core.fmt.Error) × core.fmt.Formatter) :=
  _root_.noble_kernel.untrusted.Request.Insts.CoreFmtDebug.fmt (toRequest request) formatter

def noble_kernel.untrusted.Checked.Insts.CoreCloneClone.clone
    (checked : noble_kernel.untrusted.Checked) : Result noble_kernel.untrusted.Checked := do
  let cloned ← _root_.noble_kernel.untrusted.Checked.Insts.CoreCloneClone.clone (toChecked checked)
  ok (fromChecked cloned)

def noble_kernel.untrusted.Checked.Insts.CoreFmtDebug.fmt
    (checked : noble_kernel.untrusted.Checked) (formatter : core.fmt.Formatter) :
    Result ((core.result.Result Unit core.fmt.Error) × core.fmt.Formatter) :=
  _root_.noble_kernel.untrusted.Checked.Insts.CoreFmtDebug.fmt (toChecked checked) formatter

/-- Resolve the same witness graph with the same fuel; preserve the returned
residual fuel and every error constructor exactly. -/
def noble_kernel.words.resolve.bindings
    (kinds : Slice noble_kernel.words.VariableKind) (inst : noble_kernel.words.Inst) (fuel : U32) :
    Result (core.result.Result (noble_kernel.words.Inst × U32) noble_kernel.words.InstError) := do
  let result ← _root_.noble_kernel.words.resolve.bindings
    (mapSlice toVariableKind kinds) (toInst inst) fuel
  match result with
  | .Ok (resolved, remaining) => ok (.Ok (fromInst resolved, remaining))
  | .Err error => ok (.Err (fromInstError error))

namespace KernelBridge

/-- Distinct generated types, candidates, and requests cannot collapse under
conversion to the inherited kernel's representation. -/
theorem toTy_injective : Function.Injective toTy := by
  intro x y h
  have same := congrArg fromTy h
  simpa only [fromTy_toTy] using same

theorem toCandidate_injective : Function.Injective toCandidate := by
  intro x y h
  have same := congrArg fromCandidate h
  simpa only [fromCandidate_toCandidate] using same

theorem toRequest_injective : Function.Injective toRequest := by
  intro x y h
  have same := congrArg fromRequest h
  simpa only [fromRequest_toRequest] using same

/-- On inherited inputs the boundary is exactly the inherited extracted checker
followed by the lossless outcome conversion, including its Result effects.
This is a linkage theorem, not a claim about a replacement handwritten checker. -/
theorem check_from_inputs (env : _root_.noble_kernel.contracts.Env)
    (request : _root_.noble_kernel.untrusted.Request)
    (candidate : _root_.noble_kernel.untrusted.Candidate) :
    noble_kernel.acceptance.check (fromEnv env) (fromRequest request) (fromCandidate candidate) =
      (do
        let outcome ← _root_.noble_kernel.acceptance.check env request candidate
        ok (fromOutcome outcome)) := by
  simp only [noble_kernel.acceptance.check, toEnv_fromEnv, toRequest_fromRequest,
    toCandidate_fromCandidate]

end KernelBridge
end noble_contracts
