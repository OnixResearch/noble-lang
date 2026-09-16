/-
Reference model: fragment v0 scheme patterns and signatures (M2 fragment v0).

A `Scheme` quantifies over stack, value-type, and effect variables. Patterns
nest through `pair`, `sum`, `list`, and `program`; every variable set is
finite and rank-1.
-/

import NobleM2.Types

namespace NobleM2

mutual

  /-- One stack position pattern. -/
  inductive Pattern where
    | unit : Pattern
    | bool : Pattern
    | i64 : Pattern
    | text : Pattern
    /-- The inert `Syntax` type. -/
    | syn : Pattern
    | resource : ResourceKind → Pattern
    /-- A value variable. -/
    | var : Nat → Pattern
    | pair : Pattern → Pattern → Pattern
    | sum : Pattern → Pattern → Pattern
    | list : Pattern → Pattern
    | program : Signature → Pattern
    deriving Repr, DecidableEq, Inhabited

  /-- One entry of a stack pattern: a whole stack variable or one value pattern. -/
  inductive StackPart where
    | stack : Nat → StackPart
    | value : Pattern → StackPart
    deriving Repr, DecidableEq, Inhabited

  /-- One entry of an effect pattern: a concrete identity or an effect variable. -/
  inductive EffectSlot where
    | effect : EffId → EffectSlot
    | var : Nat → EffectSlot
    deriving Repr, DecidableEq, Inhabited

  /-- A finite stack pattern. -/
  inductive PartList where
    | nil : PartList
    | cons : StackPart → PartList → PartList
    deriving Repr, DecidableEq, Inhabited

  /-- A finite effect pattern. -/
  inductive SlotList where
    | nil : SlotList
    | cons : EffectSlot → SlotList → SlotList
    deriving Repr, DecidableEq, Inhabited

  /-- A program pattern: invocation stack, result stack, latent bound. -/
  structure Signature where
    stackIn : PartList
    stackOut : PartList
    effects : SlotList
    deriving Repr, DecidableEq, Inhabited

end

end NobleM2
