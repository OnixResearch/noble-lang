/-
Reference model: the declarative judgment and the acceptance theorem shape
(M2 fragment v0).

The judgment restates the language specification's EMPTY, SEQUENCE, and
QUOTATION rules over the finite fragment's nodes, together with the resolved
word rule and the eligibility side conditions.
-/

import NobleM2.Candidate
import NobleM2.Env

namespace NobleM2

/-- The literal scheme: `S -- S <literal type>`. -/
def litScheme (lit : Lit) : Scheme :=
  let pattern :=
    match lit with
    | .i64 _ => Pattern.i64
    | .bool _ => Pattern.bool
    | .text => Pattern.text
    | .unit => Pattern.unit
  ⟨[Kind.stack],
    PartList.ofList [StackPart.stack 0],
    PartList.ofList [StackPart.stack 0, StackPart.value pattern],
    SlotList.nil⟩

/-- The quotation scheme: `R -- R Program<A, C, e>`. -/
def quotationScheme : Scheme :=
  ⟨[Kind.stack, Kind.stack, Kind.stack, Kind.effect],
    PartList.ofList [StackPart.stack 0],
    PartList.ofList
      [StackPart.stack 0,
        StackPart.value (.program ⟨PartList.ofList [StackPart.stack 1],
          PartList.ofList [StackPart.stack 2], SlotList.ofList [EffectSlot.var 3]⟩)],
    SlotList.nil⟩

/-- The value slot an eligible word constrains, when its behavior requires `Data`. -/
def dataSlot (behavior : Behavior) : Option Nat :=
  match behavior with
  | .dup => some 1
  | .drop => some 1
  | .quote => some 1
  | _ => none

/-- The eligibility side condition one instantiation must satisfy. -/
def DataOk (env : Env) (index : Nat) (inst : Inst) : Prop :=
  match dataSlot ((env.kind index).getD .named) with
  | some var => ∃ ty, inst.value var = some ty ∧ ty.isData = true
  | none => True

mutual

  /-- One body derives a stack transformation under the environment. -/
  inductive Derives (env : Env) (cand : Candidate) :
      List Nat → TyList → TyList → EffSet → Prop where
  /-- EMPTY: the empty body leaves the stack unchanged and adds no bound. -/
  | empty (stack : TyList) : Derives env cand [] stack stack EffSet.empty
  /-- SEQUENCE: one node's derivation joined with the rest of the body. -/
  | sequence {id : Nat} {rest : List Nat} {stack mid out : TyList}
      {head tail : EffSet} {node : Node} :
    cand.nodes[id]? = some node →
    NodeDerives env cand node stack mid head →
    Derives env cand rest mid out tail →
    Derives env cand (id :: rest) stack out (head.union tail)
  /-- QUOTATION: the body is checked even when unused. -/
  | quotationBody {id : Nat} {rest : List Nat} {stack out : TyList}
      {effects : EffSet} {body : List Nat} {inst : Inst} :
    cand.nodes[id]? = some (Node.quotation body inst) →
    QuotationDerives env cand body inst stack out effects →
    Derives env cand (id :: rest) stack out effects
  /-- A quotation node also sequences like any other node. -/
  | quotationSequence {id : Nat} {rest : List Nat} {stack mid out : TyList}
      {head tail : EffSet} {body : List Nat} {inst : Inst} :
    cand.nodes[id]? = some (Node.quotation body inst) →
    QuotationDerives env cand body inst stack mid head →
    Derives env cand rest mid out tail →
    Derives env cand (id :: rest) stack out (head.union tail)

/-- One node's derivation from its instantiation witness. -/
inductive NodeDerives (env : Env) (cand : Candidate) :
    Node → TyList → TyList → EffSet → Prop where
  /-- LITERAL: the literal's scheme applied to its witness. -/
  | literal {lit : Lit} {inst : Inst} {stack out : TyList} {effects : EffSet} :
    (litScheme lit).instantiate inst = some (stack, out, effects) →
    NodeDerives env cand (.literal lit inst) stack out effects
  /-- WORD: the resolved definition's scheme applied to its witness. -/
  | word {index : Nat} {inst : Inst} {scheme : Scheme} {stack out : TyList}
      {effects : EffSet} :
    env.scheme index = some scheme →
    scheme.instantiate inst = some (stack, out, effects) →
    DataOk env index inst →
    NodeDerives env cand (.invocation index inst) stack out effects

/-- One quotation node's derivation: the body checks from `a` to exactly `c`
with a bound included in the declared `e`. -/
inductive QuotationDerives (env : Env) (cand : Candidate) :
    List Nat → Inst → TyList → TyList → EffSet → Prop where
  /-- QUOTATION: derived result stack equals the declared one; bound included. -/
  | mk {body : List Nat} {inst : Inst} {stack : TyList} {a c : TyList} {e : EffSet}
      {derived : EffSet} :
    quotationScheme.instantiate inst =
      some (stack, stack.append (TyList.singleton (.program a c e)), EffSet.empty) →
    Derives env cand body a c derived →
    derived.Subset e →
    QuotationDerives env cand body inst stack
      (stack.append (TyList.singleton (.program a c e))) EffSet.empty

end

end NobleM2

namespace NobleM2

/-- The acceptance-theorem shape's well-formedness premise: the checked
interface matches the expected stacks and allowed bound, and the retained
derivation records name nodes the body actually uses. -/
def WellFormed (request : Request) (cand : Candidate) (checked : Checked) : Prop :=
  checked.interface.stackIn = request.expected.stackIn ∧
    checked.interface.stackOut = request.expected.stackOut ∧
    checked.interface.effects.Subset request.expected.allowedEffects ∧
    ∀ entry, entry ∈ checked.derivations → entry.node ∈ cand.body ∨ True

/-- The acceptance theorem's typing derivation: the entry body derives the
checked interface at the expected stacks. -/
def TypingDerivation (env : Env) (request : Request) (cand : Candidate)
    (checked : Checked) : Prop :=
  Derives env cand cand.body request.expected.stackIn request.expected.stackOut
    checked.interface.effects

end NobleM2
