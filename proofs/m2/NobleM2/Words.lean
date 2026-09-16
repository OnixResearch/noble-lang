/-
Reference model: rank-1 schemes, bindings, and substitution (M2 fragment v0).

An instantiation maps every declared variable to one binding of the matching
kind; substituting a scheme's patterns yields the concrete interface.
-/

import NobleM2.Types
import NobleM2.Shapes

namespace NobleM2

/-- What a scheme variable stands for. -/
inductive Kind where
  /-- A whole stack segment. -/
  | stack : Kind
  /-- One value type occupying one stack position. -/
  | value : Kind
  /-- An effect set. -/
  | effect : Kind
  deriving Repr, DecidableEq, Inhabited

/-- A rank-1 scheme over the fixed variable kinds. -/
structure Scheme where
  varKinds : List Kind
  stackIn : PartList
  stackOut : PartList
  effects : SlotList
  deriving Repr, DecidableEq, Inhabited

/-- One variable's concrete value. -/
inductive Binding where
  | stack : TyList → Binding
  | value : Ty → Binding
  | effect : EffSet → Binding
  deriving Repr, DecidableEq, Inhabited

/-- A concrete instantiation: one binding per declared variable, in order. -/
structure Inst where
  bindings : List Binding
  deriving Repr, DecidableEq, Inhabited

namespace Inst

/-- The stack bound to a stack variable, when the binding has that kind. -/
def stack (inst : Inst) (var : Nat) : Option TyList :=
  match inst.bindings[var]? with
  | some (.stack segment) => some segment
  | _ => none

/-- The type bound to a value variable, when the binding has that kind. -/
def value (inst : Inst) (var : Nat) : Option Ty :=
  match inst.bindings[var]? with
  | some (.value ty) => some ty
  | _ => none

/-- The effect set bound to an effect variable, when the binding has that kind. -/
def effects (inst : Inst) (var : Nat) : Option EffSet :=
  match inst.bindings[var]? with
  | some (.effect set) => some set
  | _ => none

end Inst

mutual

  /-- Substitute one value pattern under an instantiation. -/
  def Scheme.substPattern (scheme : Scheme) (inst : Inst) : Pattern → Option Ty
    | .unit => some .unit
    | .bool => some .bool
    | .i64 => some .i64
    | .text => some .text
    | .syn => some .syn
    | .resource kind => some (.resource kind)
    | .var var => inst.value var
    | .pair left right =>
      match scheme.substPattern inst left, scheme.substPattern inst right with
      | some leftTy, some rightTy => some (.pair leftTy rightTy)
      | _, _ => none
    | .sum left right =>
      match scheme.substPattern inst left, scheme.substPattern inst right with
      | some leftTy, some rightTy => some (.sum leftTy rightTy)
      | _, _ => none
    | .list item => (scheme.substPattern inst item).map (fun ty => .list ty)
    | .program signature =>
      (scheme.substSignature inst signature).map (fun (stackIn, stackOut, effects) =>
        .program stackIn stackOut effects)
  termination_by pattern => sizeOf pattern

  /-- Substitute one program pattern under an instantiation. -/
  def Scheme.substSignature (scheme : Scheme) (inst : Inst) : Signature → Option (TyList × TyList × EffSet)
    | ⟨stackIn, stackOut, effects⟩ =>
      match scheme.substStack inst stackIn,
        scheme.substStack inst stackOut,
        scheme.substEffects inst effects with
      | some inTypes, some outTypes, some bound => some (inTypes, outTypes, bound)
      | _, _, _ => none
  termination_by signature => sizeOf signature

  /-- Substitute one stack pattern under an instantiation. -/
  def Scheme.substStack (scheme : Scheme) (inst : Inst) : PartList → Option TyList
    | .nil => some .nil
    | .cons part rest =>
      match scheme.substStack inst rest with
      | none => none
      | some tail =>
        match part with
        | .stack var => (inst.stack var).map (fun segment => segment.append tail)
        | .value pattern =>
          (scheme.substPattern inst pattern).map (fun ty => .cons ty tail)
  termination_by parts => sizeOf parts

  /-- Substitute one effect pattern under an instantiation. -/
  def Scheme.substEffects (scheme : Scheme) (inst : Inst) : SlotList → Option EffSet
    | .nil => some .empty
    | .cons slot rest =>
      match scheme.substEffects inst rest with
      | none => none
      | some tail =>
        match slot with
        | .effect id => some (.insert id tail)
        | .var var => (inst.effects var).map (fun set => set.union tail)
  termination_by slots => sizeOf slots

end

/-- Substitute one whole scheme into its concrete interface. -/
def Scheme.instantiate (scheme : Scheme) (inst : Inst) : Option (TyList × TyList × EffSet) :=
  match scheme.substStack inst scheme.stackIn,
    scheme.substStack inst scheme.stackOut,
    scheme.substEffects inst scheme.effects with
  | some stackIn, some stackOut, some effects => some (stackIn, stackOut, effects)
  | _, _, _ => none

end NobleM2
