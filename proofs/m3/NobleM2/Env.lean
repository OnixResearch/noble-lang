/-
Reference model: the fixed v0 checking environment (M2 fragment v0).

`defs` and `kinds` are indexed by `Definition`; the table below restates the
bootstrap word contracts in the order the implementation fixes.
-/

import NobleM2.Words

namespace NobleM2

/-- One user-declared schema in external environment data (fragment v1):
its declaration identity, its scheme, and whether it names itself — a
self-referential declaration is unsupported (B-CHECK-02). -/
structure SchemaDecl where
  id : Nat
  scheme : Scheme
  recursive : Bool
  deriving Repr, DecidableEq, Inhabited

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
  /-- The environment's user-declared schemas (v1: external data). -/
  schemas : List SchemaDecl
  deriving Repr, DecidableEq, Inhabited

/-- The scheme of a definition. -/
def Env.scheme (env : Env) (index : Nat) : Option Scheme := env.defs[index]?

/-- The behavior of a definition. -/
def Env.kind (env : Env) (index : Nat) : Option Behavior := env.kinds[index]?

/-- Whether the environment provides this effect identity. -/
def Env.knowsEffect (env : Env) (id : EffId) : Bool := env.effects.elem id

/-- The number of definitions. -/
def Env.length (env : Env) : Nat := env.defs.length

/-- The declared dependency list of a definition; empty when the
environment declares none (external environment data, fragment v1). -/
def Env.depsOf (env : Env) (index : Nat) : List Nat :=
  match env.deps[index]? with
  | some ds => ds
  | none => []

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

/-- One word's contract: its scheme plus what it constrains beyond the scheme. -/
structure Word where
  scheme : Scheme
  behavior : Behavior

/-- `dup : S a -- S a a ! {}` (7.1). -/
def dupScheme : Scheme :=
  ⟨[Kind.stack, Kind.value],
    PartList.ofList [StackPart.stack 0, StackPart.value (.var 1)],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.var 1), StackPart.value (.var 1)],
    SlotList.nil⟩

/-- `drop : S a -- S ! {}` (7.1). -/
def dropScheme : Scheme :=
  ⟨[Kind.stack, Kind.value],
    PartList.ofList [StackPart.stack 0, StackPart.value (.var 1)],
    PartList.ofList [StackPart.stack 0],
    SlotList.nil⟩

/-- `swap : S a b -- S b a ! {}` (7.1). -/
def swapScheme : Scheme :=
  ⟨[Kind.stack, Kind.value, Kind.value],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.var 1), StackPart.value (.var 2)],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.var 2), StackPart.value (.var 1)],
    SlotList.nil⟩

/-- `dip : S a -- S' a ! {}` over a body program (7.1). -/
def dipScheme : Scheme :=
  ⟨[Kind.stack, Kind.value, Kind.stack, Kind.effect],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.var 1),
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 0],
            PartList.ofList [StackPart.stack 2], SlotList.ofList [EffectSlot.var 3]⟩)],
    PartList.ofList [StackPart.stack 2, StackPart.value (.var 1)],
    SlotList.ofList [EffectSlot.var 3]⟩

/-- `+ - * : S I64 I64 -- S I64 ! {}` (K-NUM-01, one scheme each). -/
def arithScheme : Scheme :=
  ⟨[Kind.stack],
    PartList.ofList
      [StackPart.stack 0, StackPart.value .i64, StackPart.value .i64],
    PartList.ofList
      [StackPart.stack 0, StackPart.value .i64],
    SlotList.nil⟩

/-- `= : S I64 I64 -- S Bool ! {}` (K-NUM-01). -/
def equalsScheme : Scheme :=
  ⟨[Kind.stack],
    PartList.ofList
      [StackPart.stack 0, StackPart.value .i64, StackPart.value .i64],
    PartList.ofList
      [StackPart.stack 0, StackPart.value .bool],
    SlotList.nil⟩

/-- `quote : S a -- S Q<a> ! {}` where `Q<a> = {} a` (6.5). -/
def quoteScheme : Scheme :=
  ⟨[Kind.stack, Kind.value, Kind.stack],
    PartList.ofList [StackPart.stack 0, StackPart.value (.var 1)],
    PartList.ofList
      [StackPart.stack 0,
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 2],
            PartList.ofList [StackPart.stack 2, StackPart.value (.var 1)], SlotList.nil⟩)],
    SlotList.nil⟩

/-- `compose : (S -- S') (S' -- S'') -- (S -- S'') ! {}` (6.4). -/
def composeScheme : Scheme :=
  ⟨[Kind.stack, Kind.stack, Kind.stack, Kind.stack, Kind.effect, Kind.effect],
    PartList.ofList
      [StackPart.stack 0,
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 1],
            PartList.ofList [StackPart.stack 2], SlotList.ofList [EffectSlot.var 4]⟩),
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 2],
            PartList.ofList [StackPart.stack 3], SlotList.ofList [EffectSlot.var 5]⟩)],
    PartList.ofList
      [StackPart.stack 0,
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 1],
            PartList.ofList [StackPart.stack 3],
            SlotList.ofList [EffectSlot.var 4, EffectSlot.var 5]⟩)],
    SlotList.nil⟩

/-- `run : S Q -- S' ! {}` where `Q = {S -- S' ! e}` (6.3). -/
def runScheme : Scheme :=
  ⟨[Kind.stack, Kind.stack, Kind.effect],
    PartList.ofList
      [StackPart.stack 0,
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 0],
            PartList.ofList [StackPart.stack 1], SlotList.ofList [EffectSlot.var 2]⟩)],
    PartList.ofList [StackPart.stack 1],
    SlotList.ofList [EffectSlot.var 2]⟩

/-- `reflect : S Q -- S syntax ! {}` where `Q = {S -- S' ! e}` (11.3). -/
def reflectScheme : Scheme :=
  ⟨[Kind.stack, Kind.stack, Kind.stack, Kind.effect],
    PartList.ofList
      [StackPart.stack 0,
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 1],
            PartList.ofList [StackPart.stack 2], SlotList.ofList [EffectSlot.var 3]⟩)],
    PartList.ofList [StackPart.stack 0, StackPart.value .syn],
    SlotList.nil⟩

/-- `unit : S a -- S unit ! {}` (7.2). -/
def unitScheme : Scheme :=
  ⟨[Kind.stack],
    PartList.ofList [StackPart.stack 0],
    PartList.ofList
      [StackPart.stack 0, StackPart.value .unit],
    SlotList.nil⟩

/-- `pair : S a b -- S pair<a,b> ! {}` (7.2). -/
def pairScheme : Scheme :=
  ⟨[Kind.stack, Kind.value, Kind.value],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.var 1), StackPart.value (.var 2)],
    PartList.ofList
      [StackPart.stack 0,
        StackPart.value (.pair (.var 1) (.var 2))],
    SlotList.nil⟩

/-- `unpair : S pair<a,b> -- S a b ! {}` (7.2). -/
def unpairScheme : Scheme :=
  ⟨[Kind.stack, Kind.value, Kind.value],
    PartList.ofList
      [StackPart.stack 0,
        StackPart.value (.pair (.var 1) (.var 2))],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.var 1), StackPart.value (.var 2)],
    SlotList.nil⟩

/-- `inl : S a -- S Sum<a,b> ! {}` (7.2). -/
def inlScheme : Scheme :=
  ⟨[Kind.stack, Kind.value, Kind.value],
    PartList.ofList [StackPart.stack 0, StackPart.value (.var 1)],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.sum (.var 1) (.var 2))],
    SlotList.nil⟩

/-- `inr : S b -- S Sum<a,b> ! {}` (7.2). -/
def inrScheme : Scheme :=
  ⟨[Kind.stack, Kind.value, Kind.value],
    PartList.ofList [StackPart.stack 0, StackPart.value (.var 2)],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.sum (.var 1) (.var 2))],
    SlotList.nil⟩

/-- `case : S Sum<a,b> -- S' ! {}` over two body programs (7.2). -/
def caseScheme : Scheme :=
  ⟨[Kind.stack, Kind.value, Kind.value, Kind.stack, Kind.effect, Kind.effect],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.sum (.var 1) (.var 2)),
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 0, StackPart.value (.var 1)],
            PartList.ofList [StackPart.stack 3], SlotList.ofList [EffectSlot.var 4]⟩),
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 0, StackPart.value (.var 2)],
            PartList.ofList [StackPart.stack 3], SlotList.ofList [EffectSlot.var 5]⟩)],
    PartList.ofList [StackPart.stack 3],
    SlotList.ofList [EffectSlot.var 4, EffectSlot.var 5]⟩
/-- `if : S Bool -- S' S'' ! {body0 !e0, body1 !e1}` (7.3). -/
def ifScheme : Scheme :=
  ⟨[Kind.stack, Kind.stack, Kind.effect, Kind.effect],
    PartList.ofList
      [StackPart.stack 0, StackPart.value .bool,
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 0],
            PartList.ofList [StackPart.stack 1], SlotList.ofList [EffectSlot.var 2]⟩),
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 0],
            PartList.ofList [StackPart.stack 1], SlotList.ofList [EffectSlot.var 3]⟩)],
    PartList.ofList [StackPart.stack 1],
    SlotList.ofList [EffectSlot.var 2, EffectSlot.var 3]⟩

/-- `nil : S -- S List<a> ! {}` (7.4). -/
def nilScheme : Scheme :=
  ⟨[Kind.stack, Kind.value],
    PartList.ofList [StackPart.stack 0],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.list (.var 1))],
    SlotList.nil⟩

/-- `cons : S a List<a> -- S List<a> ! {}` (7.4). -/
def consScheme : Scheme :=
  ⟨[Kind.stack, Kind.value],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.var 1), StackPart.value (.list (.var 1))],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.list (.var 1))],
    SlotList.nil⟩

/-- `list.case : S List<a> -- S' S'' ! {body0 !e0, body1 !e1}` (7.4). -/
def listCaseScheme : Scheme :=
  ⟨[Kind.stack, Kind.value, Kind.stack, Kind.effect, Kind.effect],
    PartList.ofList
      [StackPart.stack 0, StackPart.value (.list (.var 1)),
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 0],
            PartList.ofList [StackPart.stack 2], SlotList.ofList [EffectSlot.var 3]⟩),
        StackPart.value
          (.program ⟨PartList.ofList [StackPart.stack 0, StackPart.value (.var 1), StackPart.value (.list (.var 1))],
            PartList.ofList [StackPart.stack 2], SlotList.ofList [EffectSlot.var 4]⟩)],
    PartList.ofList [StackPart.stack 2],
    SlotList.ofList [EffectSlot.var 3, EffectSlot.var 4]⟩

/-- The host-emission fixture word `test.emit : S text -- S unit ! {test.emit}`. -/
def emitScheme : Scheme :=
  ⟨[Kind.stack],
    PartList.ofList [StackPart.stack 0, StackPart.value .text],
    PartList.ofList [StackPart.stack 0, StackPart.value .unit],
    SlotList.ofList [EffectSlot.effect 0]⟩

/-- The complete bootstrap word table, in kernel `Definition` order (23 entries).
The identities are the kernel table positions; `test.emit` is the supplied
host fixture. -/
def wordTable : List Word :=
  [⟨dupScheme, .dup⟩, ⟨dropScheme, .drop⟩, ⟨swapScheme, .swap⟩, ⟨dipScheme, .dip⟩,
   ⟨arithScheme, .arith⟩, ⟨arithScheme, .arith⟩, ⟨arithScheme, .arith⟩, ⟨equalsScheme, .equals⟩,
   ⟨quoteScheme, .quote⟩, ⟨composeScheme, .compose⟩, ⟨runScheme, .run⟩, ⟨reflectScheme, .reflect⟩,
   ⟨unitScheme, .unit⟩, ⟨pairScheme, .pair⟩, ⟨unpairScheme, .unpair⟩, ⟨inlScheme, .inl⟩,
   ⟨inrScheme, .inr⟩, ⟨caseScheme, .case⟩, ⟨ifScheme, .if⟩, ⟨nilScheme, .nil⟩,
   ⟨consScheme, .cons⟩, ⟨listCaseScheme, .listCase⟩, ⟨emitScheme, .testEmit⟩]

/-- The word table has exactly the kernel's 23 definition identities. -/
theorem wordTable_length : wordTable.length = 23 := by
  decide


/-- The bootstrap word contracts, in kernel `Definition` order: the table
positions are the exact environment identities the checker fixes. -/
def bootstrapTable : List (Behavior × Scheme) :=
  wordTable.map (fun word => (word.behavior, word.scheme))

/-- The fixed bootstrap environment: the word table's contracts, the one
provided effect identity, and no external environment data. -/
def bootstrapEnv : Env :=
  { defs := bootstrapTable.map (fun entry => entry.2)
    kinds := bootstrapTable.map (fun entry => entry.1)
    effects := [testEmitEffect],
    deps := [],
    schemas := [] }

/-- The definition index of the first entry carrying a behavior, when present. -/
def definitionOf (behavior : Behavior) : Option Nat :=
  bootstrapTable.findIdx? (fun entry => entry.1 == behavior)

/-! ## The eliminators' definition identities (fragment v1)

The three eliminator rules of the judgment (`NobleM2.Judgment`) are stated
over these table positions: `case` at 17, `if` at 18, `list.case` at 21. -/

/-- The kernel table position of `case`. -/
def caseDef : Nat := 17

/-- The kernel table position of `if`. -/
def ifDef : Nat := 18

/-- The kernel table position of `list.case`. -/
def listCaseDef : Nat := 21

theorem definitionOf_case : definitionOf .case = some caseDef := by decide
theorem definitionOf_if : definitionOf .if = some ifDef := by decide
theorem definitionOf_listCase : definitionOf .listCase = some listCaseDef := by
  decide

/-- The bootstrap environment carries exactly the word table's contracts. -/
theorem bootstrapEnv_length : bootstrapEnv.length = 23 := by decide

/-- The word table's schemes are the environment's definitions, in order. -/
theorem bootstrapEnv_defs : bootstrapEnv.defs = wordTable.map (fun w => w.scheme) := by
  decide

/-- Looking a mapped field through a list commutes with position lookup. -/
theorem map_scheme_lookup : ∀ (ws : List Word) (index : Nat),
    ((ws.map (fun w => w.scheme))[index]?) = (ws[index]?).map (fun w => w.scheme)
  | [], _ => rfl
  | _ :: _, 0 => rfl
  | _ :: rest, index + 1 => map_scheme_lookup rest index

/-- The scheme at one table position, when the position is in range. -/
theorem wordScheme_lookup (index : Nat) (h : index < 23) :
    ∃ word, wordTable[index]? = some word ∧
      bootstrapEnv.scheme index = some word.scheme := by
  rw [Env.scheme, bootstrapEnv_defs, map_scheme_lookup]
  cases hlook : wordTable[index]? with
  | none =>
      rw [List.getElem?_eq_none_iff, wordTable_length] at hlook
      omega
  | some word => exact ⟨word, rfl, by simpa using hlook⟩

end NobleM2
