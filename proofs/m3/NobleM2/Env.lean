/-
Reference model: the fixed v0 checking environment (M2 fragment v0).

`defs` and `kinds` are indexed by `Definition`; the table below restates the
bootstrap word contracts in the order the implementation fixes.
-/

import NobleM2.Words

namespace NobleM2

/-- The reserved identity of the resource-free `test.emit` effect. -/
def testEmitEffect : EffId := 0

/-- The reserved resource kind used by negative eligibility fixtures. -/
def fixtureResource : ResourceKind := 0

/-- What a definition's contract constrains beyond its scheme. -/
inductive Behavior where
  | dup : Behavior
  | drop : Behavior
  | swap : Behavior
  | dip : Behavior
  | arith : Behavior
  | equals : Behavior
  | quote : Behavior
  | compose : Behavior
  | run : Behavior
  | reflect : Behavior
  | unit : Behavior
  | pair : Behavior
  | unpair : Behavior
  | inl : Behavior
  | inr : Behavior
  | case : Behavior
  | if : Behavior
  | nil : Behavior
  | cons : Behavior
  | listCase : Behavior
  | testEmit : Behavior
  /-- An environment-supplied named definition with no extra constraint. -/
  | named : Behavior
  deriving Repr, DecidableEq, Inhabited

/-- The checking environment. -/
structure Env where
  /-- Schemes indexed by definition. -/
  defs : List Scheme
  /-- Behavior kinds indexed by definition. -/
  kinds : List Behavior
  /-- Effect identities this environment provides; `test.emit` is first. -/
  effects : List EffId
  /-- Each definition.s dependency list (v1: external environment data). -/
  deps : List (List Nat)
  /-- Each definition.s user-declared schema id (v1: external data). -/
  schemas : List Nat
  deriving Repr, DecidableEq, Inhabited

/-- The scheme of a definition. -/
def Env.scheme (env : Env) (index : Nat) : Option Scheme := env.defs[index]?

/-- The behavior of a definition. -/
def Env.kind (env : Env) (index : Nat) : Option Behavior := env.kinds[index]?

/-- Whether the environment provides this effect identity. -/
def Env.knowsEffect (env : Env) (id : EffId) : Bool := env.effects.elem id

/-- The number of definitions. -/
def Env.length (env : Env) : Nat := env.defs.length

/-- Build a stack pattern from a list. -/
def PartList.ofList : List StackPart → PartList
  | [] => .nil
  | head :: rest => .cons head (ofList rest)

/-- The entries of a stack pattern. -/
def PartList.toList : PartList → List StackPart
  | .nil => []
  | .cons head rest => head :: rest.toList

/-- Build an effect pattern from a list. -/
def SlotList.ofList : List EffectSlot → SlotList
  | [] => .nil
  | head :: rest => .cons head (ofList rest)

/-- The entries of an effect pattern. -/
def SlotList.toList : SlotList → List EffectSlot
  | .nil => []
  | .cons head rest => head :: rest.toList

/-- A whole-stack variable entry. -/
def stackVar (index : Nat) : StackPart := .stack index

/-- A value-variable entry. -/
def valueVar (index : Nat) : StackPart := .value (.var index)

/-- One concrete pattern entry. -/
def patternPart (pattern : Pattern) : StackPart := .value pattern

/-- One effect-variable slot. -/
def effectVar (index : Nat) : EffectSlot := .var index

/-- One program pattern over three part lists. -/
def programPattern (stackIn stackOut : PartList) (effects : SlotList) : Pattern :=
  .program ⟨stackIn, stackOut, effects⟩

/-- One scheme over four part lists. -/
def schemeOf (varKinds : List Kind) (stackIn stackOut : PartList)
    (effects : SlotList) : Scheme :=
  ⟨varKinds, stackIn, stackOut, effects⟩

/-- The bootstrap word contracts, in definition order. -/
def bootstrapTable : List (Behavior × Scheme) :=
  [ (Behavior.dup,
      schemeOf [Kind.stack, Kind.value]
        (PartList.ofList [stackVar 0, valueVar 1])
        (PartList.ofList [stackVar 0, valueVar 1, valueVar 1])
        SlotList.nil),
    (Behavior.drop,
      schemeOf [Kind.stack, Kind.value]
        (PartList.ofList [stackVar 0, valueVar 1])
        (PartList.ofList [stackVar 0])
        SlotList.nil),
    (Behavior.swap,
      schemeOf [Kind.stack, Kind.value, Kind.value]
        (PartList.ofList [stackVar 0, valueVar 1, valueVar 2])
        (PartList.ofList [stackVar 0, valueVar 2, valueVar 1])
        SlotList.nil),
    (Behavior.dip,
      schemeOf [Kind.stack, Kind.value, Kind.stack, Kind.effect]
        (PartList.ofList
          [stackVar 0, valueVar 1,
            patternPart (programPattern (PartList.ofList [stackVar 0])
              (PartList.ofList [stackVar 2]) (SlotList.ofList [effectVar 3]))])
        (PartList.ofList [stackVar 2, valueVar 1])
        (SlotList.ofList [effectVar 3])),
    (Behavior.arith,
      schemeOf [Kind.stack]
        (PartList.ofList [stackVar 0, patternPart .i64, patternPart .i64])
        (PartList.ofList [stackVar 0, patternPart .i64])
        SlotList.nil),
    (Behavior.arith,
      schemeOf [Kind.stack]
        (PartList.ofList [stackVar 0, patternPart .i64, patternPart .i64])
        (PartList.ofList [stackVar 0, patternPart .i64])
        SlotList.nil),
        (Behavior.equals,
      schemeOf [Kind.stack]
        (PartList.ofList [stackVar 0, patternPart .i64, patternPart .i64])
        (PartList.ofList [stackVar 0, patternPart .bool])
        SlotList.nil),
    (Behavior.arith,
      schemeOf [Kind.stack]
        (PartList.ofList [stackVar 0, patternPart .i64, patternPart .i64])
        (PartList.ofList [stackVar 0, patternPart .i64])
        SlotList.nil),
    (Behavior.quote,
      schemeOf [Kind.stack, Kind.value, Kind.stack]
        (PartList.ofList [stackVar 0, valueVar 1])
        (PartList.ofList
          [stackVar 0,
            patternPart (programPattern (PartList.ofList [stackVar 2])
              (PartList.ofList [stackVar 2, valueVar 1]) SlotList.nil)])
        SlotList.nil),
    (Behavior.compose,
      schemeOf
        [Kind.stack, Kind.stack, Kind.stack, Kind.stack, Kind.effect, Kind.effect]
        (PartList.ofList
          [stackVar 0,
            patternPart (programPattern (PartList.ofList [stackVar 1])
              (PartList.ofList [stackVar 2]) (SlotList.ofList [effectVar 4])),
            patternPart (programPattern (PartList.ofList [stackVar 2])
              (PartList.ofList [stackVar 3]) (SlotList.ofList [effectVar 5]))])
        (PartList.ofList
          [stackVar 0,
            patternPart (programPattern (PartList.ofList [stackVar 1])
              (PartList.ofList [stackVar 3])
              (SlotList.ofList [effectVar 4, effectVar 5]))])
        SlotList.nil),
    (Behavior.run,
      schemeOf [Kind.stack, Kind.stack, Kind.effect]
        (PartList.ofList
          [stackVar 0,
            patternPart (programPattern (PartList.ofList [stackVar 0])
              (PartList.ofList [stackVar 1]) (SlotList.ofList [effectVar 2]))])
        (PartList.ofList [stackVar 1])
        (SlotList.ofList [effectVar 2])),
    (Behavior.reflect,
      schemeOf [Kind.stack, Kind.stack, Kind.stack, Kind.effect]
        (PartList.ofList
          [stackVar 0,
            patternPart (programPattern (PartList.ofList [stackVar 1])
              (PartList.ofList [stackVar 2]) (SlotList.ofList [effectVar 3]))])
        (PartList.ofList [stackVar 0, patternPart .syn])
        SlotList.nil),
    (Behavior.unit,
      schemeOf [Kind.stack]
        (PartList.ofList [stackVar 0])
        (PartList.ofList [stackVar 0, patternPart .unit])
        SlotList.nil),
    (Behavior.pair,
      schemeOf [Kind.stack, Kind.value, Kind.value]
        (PartList.ofList [stackVar 0, valueVar 1, valueVar 2])
        (PartList.ofList
          [stackVar 0, patternPart (.pair (.var 1) (.var 2))])
        SlotList.nil),
    (Behavior.unpair,
      schemeOf [Kind.stack, Kind.value, Kind.value]
        (PartList.ofList [stackVar 0, patternPart (.pair (.var 1) (.var 2))])
        (PartList.ofList [stackVar 0, valueVar 1, valueVar 2])
        SlotList.nil),
    (Behavior.inl,
      schemeOf [Kind.stack, Kind.value, Kind.value]
        (PartList.ofList [stackVar 0, valueVar 1])
        (PartList.ofList [stackVar 0, patternPart (.sum (.var 1) (.var 2))])
        SlotList.nil),
    (Behavior.inr,
      schemeOf [Kind.stack, Kind.value, Kind.value]
        (PartList.ofList [stackVar 0, valueVar 2])
        (PartList.ofList [stackVar 0, patternPart (.sum (.var 1) (.var 2))])
        SlotList.nil),
    (Behavior.case,
      schemeOf
        [Kind.stack, Kind.value, Kind.value, Kind.stack, Kind.effect, Kind.effect]
        (PartList.ofList
          [stackVar 0,
            patternPart (.sum (.var 1) (.var 2)),
            patternPart (programPattern (PartList.ofList [stackVar 0, valueVar 1])
              (PartList.ofList [stackVar 3]) (SlotList.ofList [effectVar 4])),
            patternPart (programPattern (PartList.ofList [stackVar 0, valueVar 2])
              (PartList.ofList [stackVar 3]) (SlotList.ofList [effectVar 5]))])
        (PartList.ofList [stackVar 3])
        (SlotList.ofList [effectVar 4, effectVar 5])),
    (Behavior.if,
      schemeOf [Kind.stack, Kind.stack, Kind.effect, Kind.effect]
        (PartList.ofList
          [stackVar 0,
            patternPart .bool,
            patternPart (programPattern (PartList.ofList [stackVar 0])
              (PartList.ofList [stackVar 1]) (SlotList.ofList [effectVar 2])),
            patternPart (programPattern (PartList.ofList [stackVar 0])
              (PartList.ofList [stackVar 1]) (SlotList.ofList [effectVar 3]))])
        (PartList.ofList [stackVar 1])
        (SlotList.ofList [effectVar 2, effectVar 3])),
    (Behavior.nil,
      schemeOf [Kind.stack, Kind.value]
        (PartList.ofList [stackVar 0])
        (PartList.ofList [stackVar 0, patternPart (.list (.var 1))])
        SlotList.nil),
    (Behavior.cons,
      schemeOf [Kind.stack, Kind.value]
        (PartList.ofList [stackVar 0, valueVar 1, patternPart (.list (.var 1))])
        (PartList.ofList [stackVar 0, patternPart (.list (.var 1))])
        SlotList.nil),
    (Behavior.listCase,
      schemeOf [Kind.stack, Kind.value, Kind.stack, Kind.effect, Kind.effect]
        (PartList.ofList
          [stackVar 0,
            patternPart (.list (.var 1)),
            patternPart (programPattern (PartList.ofList [stackVar 0])
              (PartList.ofList [stackVar 2]) (SlotList.ofList [effectVar 3])),
            patternPart (programPattern
              (PartList.ofList [stackVar 0, valueVar 1, patternPart (.list (.var 1))])
              (PartList.ofList [stackVar 2]) (SlotList.ofList [effectVar 4]))])
        (PartList.ofList [stackVar 2])
        (SlotList.ofList [effectVar 3, effectVar 4])),
    (Behavior.testEmit,
      schemeOf [Kind.stack]
        (PartList.ofList [stackVar 0, patternPart .text])
        (PartList.ofList [stackVar 0, patternPart .unit])
        (SlotList.ofList [.effect testEmitEffect])) ]

/-- The fixed v0 bootstrap environment. -/
def bootstrapEnv : Env :=
  { defs := bootstrapTable.map (fun entry => entry.2)
    kinds := bootstrapTable.map (fun entry => entry.1)
    effects := [testEmitEffect],
    deps := [],
    schemas := [] }

/-- The definition index of the first entry carrying a behavior, when present. -/
def definitionOf (behavior : Behavior) : Option Nat :=
  bootstrapTable.findIdx? (fun entry => entry.1 == behavior)

end NobleM2
