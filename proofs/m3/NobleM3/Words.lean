/-
Reference model: the full bootstrap word table (M3 fragment v1).

Every bootstrap word's rank-1 scheme, from the language specification's
word contracts: wrapping arithmetic (K-NUM-01), stack manipulation (7.1),
pairs and sums (7.2), branches (7.3), lists (7.4), quotation (6.3-6.5),
reflection (11.3), and the host emission fixture.
-/

import NobleM2.Judgment

namespace NobleM2

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

end NobleM2
