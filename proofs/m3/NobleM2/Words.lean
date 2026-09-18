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
  /-- The witness of another variable of the same instantiation: a type
  equation between two variables (fragment v1). A chain of references must
  be finite and well founded; a cycle is rejected (B-CHECK-05). -/
  | ref : Nat → Binding
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

/-- The kind a direct binding carries; a reference binding carries none
(fragment v1: its kind is checked at the terminal binding after
resolution, exactly as the kernel's `check_binding` passes `Ref`). -/
def Binding.kindOf : Binding → Option Kind
  | .stack _ => some .stack
  | .value _ => some .value
  | .effect _ => some .effect
  | .ref _ => none

/-! ## Reference-binding resolution (fragment v1, B-CHECK-05)

A `ref` binding states a type equation: this variable's witness is another
variable's witness. Chains must be finite and well founded — a chain that
returns to a variable already on its own path is cyclic and rejects. The
walk below mirrors the kernel's `words::resolve`: charge before hop, and a
hop count passing the witness length names a cycle without a visited set
(pigeonhole). -/

/-- Why a witness's reference bindings cannot be resolved. -/
inductive ResolveErr where
  /-- A chain that returns to a variable on its own path. -/
  | cyclic : ResolveErr
  /-- The walk would outrun its declared hop budget; fail closed. -/
  | exhausted : ResolveErr
  /-- A hop to a binding position the witness does not carry. -/
  | unknown : ResolveErr
  /-- The terminal binding's kind does not match the referrer's. -/
  | kindMismatch : ResolveErr
  deriving Repr, DecidableEq, Inhabited

namespace Inst

/-- Follow one reference chain to its terminal binding position: the hop
count passing the witness length names a cycle (pigeonhole), every hop is
charged before it is taken (`remaining` is the budget left), and a lookup
outside the witness's positions is unknown. Returns the terminal position
and the leftover budget. -/
def followFrom (witness : Inst) (current : Nat) (remaining : Nat) (hops : Nat) :
    Except ResolveErr (Nat × Nat) :=
  if hops + 1 > witness.bindings.length then
    .error .cyclic
  else if remaining = 0 then
    .error .exhausted
  else
    match witness.bindings[current]? with
    | none => .error .unknown
    | some (.ref next) => witness.followFrom next (remaining - 1) (hops + 1)
    | some _ => .ok (current, remaining)
termination_by remaining

/-- Resolve one declared binding (at `index`, carrying `binding`) against
the witness's reference graph: a direct binding is kept; a reference
follows its chain and keeps the terminal binding when its kind matches the
referrer's declared kind. Returns the resolved binding and leftover budget. -/
def resolveOne (kinds : List Kind) (witness : Inst) (index : Nat)
    (binding : Binding) (remaining : Nat) : Except ResolveErr (Binding × Nat) :=
  match binding with
  | .ref var =>
    match witness.followFrom var remaining 0 with
    | .error e => .error e
    | .ok (terminal, left) =>
      match kinds[index]? with
      | none => .error .unknown
      | some kind =>
        match witness.bindings[terminal]? with
        | none => .error .unknown
        | some term =>
          if term.kindOf == some kind then .ok (term, left)
          else .error .kindMismatch
  | direct => .ok (direct, remaining)

/-- The resolution pass over the witness's bindings, in position order,
threading the shared hop budget exactly as the kernel's walk state does:
each position resolves (charging its hops), then the pass continues with
what budget is left. -/
def resolveList (kinds : List Kind) (witness : Inst) :
    Nat → Nat → List Binding → Except ResolveErr (List Binding × Nat)
  | _, remaining, [] => .ok ([], remaining)
  | index, remaining, binding :: rest =>
    match resolveOne kinds witness index binding remaining with
    | .error e => .error e
    | .ok (resolved, left) =>
      match resolveList kinds witness (index + 1) left rest with
      | .error e => .error e
      | .ok (resolvedRest, left') => .ok (resolved :: resolvedRest, left')

/-- Resolve every reference binding of one witness (B-CHECK-05): the result
carries only direct bindings — every reference replaced by its terminal
binding — together with the leftover hop budget. -/
def resolve (kinds : List Kind) (witness : Inst) (fuel : Nat) :
    Except ResolveErr (Inst × Nat) :=
  match resolveList kinds witness 0 fuel witness.bindings with
  | .error e => .error e
  | .ok (bindings, left) => .ok (⟨bindings⟩, left)

end Inst

end NobleM2
