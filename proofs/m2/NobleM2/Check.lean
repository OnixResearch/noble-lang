/-
Reference model: the reference decision function for the M2 fragment v0.

`check` mirrors the finite acceptance rules: it preflights the request, folds
the entry body, and returns the five-way outcome domain. Work is charged
before each bounded step, and a nested body shares the same fold state.
-/

import NobleM2.Candidate
import NobleM2.Judgment

namespace NobleM2

/-- The work one scheme instantiation charges. -/
def schemeCost (scheme : Scheme) : Nat :=
  scheme.stackIn.length + scheme.stackOut.length + 1

/-- The work one join charges. -/
def joinCost (iface : Interface) : Nat :=
  iface.stackIn.length + iface.stackOut.length + 1

/-- Whether one variable use matches its declared kind. -/
def requireKind (kinds : List Kind) (var : Nat) (expected : Kind) : Bool :=
  match kinds[var]? with
  | some kind => kind == expected
  | none => false

mutual

  /-- Whether every stack pattern matches its declared kind. -/
  def partsValid (kinds : List Kind) : PartList → Bool
    | .nil => true
    | .cons (.stack var) rest => requireKind kinds var .stack && partsValid kinds rest
    | .cons (.value pattern) rest => patternsValid kinds pattern && partsValid kinds rest
  termination_by parts => sizeOf parts

  /-- Whether every effect slot matches its declared kind. -/
  def slotsValid (kinds : List Kind) : SlotList → Bool
    | .nil => true
    | .cons (.effect _) rest => slotsValid kinds rest
    | .cons (.var var) rest => requireKind kinds var .effect && slotsValid kinds rest
  termination_by slots => sizeOf slots

  /-- Whether every value pattern matches its declared kind. -/
  def patternsValid (kinds : List Kind) : Pattern → Bool
    | .unit => true
    | .bool => true
    | .i64 => true
    | .text => true
    | .syn => true
    | .resource _ => true
    | .var var => requireKind kinds var .value
    | .pair left right => patternsValid kinds left && patternsValid kinds right
    | .sum left right => patternsValid kinds left && patternsValid kinds right
    | .list item => patternsValid kinds item
    | .program ⟨stackIn, stackOut, effects⟩ =>
      partsValid kinds stackIn && partsValid kinds stackOut && slotsValid kinds effects
  termination_by pattern => sizeOf pattern

end

/-- Whether one scheme's variable uses all match their declared kinds. -/
def schemeValid (scheme : Scheme) : Bool :=
  partsValid scheme.varKinds scheme.stackIn && partsValid scheme.varKinds scheme.stackOut
    && slotsValid scheme.varKinds scheme.effects

/-- Whether every environment scheme is well formed. -/
def envValid (env : Env) : Bool := env.defs.all schemeValid

/-- The kind one binding carries. -/
def bindingKind : Binding → Kind
  | .stack _ => .stack
  | .value _ => .value
  | .effect _ => .effect

/-- Whether one type's size fits the declared bound. -/
def typeWithin (ty : Ty) (maxType : Nat) : Bool := ty.size ≤ maxType

/-- Whether one stack fits the declared height and type-size bounds. -/
def stackWithin (stack : TyList) (maxHeight maxType : Nat) : Bool :=
  stack.length ≤ maxHeight && stack.toList.all (fun ty => typeWithin ty maxType)

/-- Whether one effect identity is known to the environment. -/
def effectKnown (env : Env) (id : EffId) : Bool := env.effects.elem id

/-- Whether one instantiation fits its scheme within the declared bounds. -/
def instValid (scheme : Scheme) (inst : Inst) (maxHeight maxType : Nat) : Bool :=
  inst.bindings.length == scheme.varKinds.length
    && (inst.bindings.zip scheme.varKinds).all (fun entry => bindingKind entry.1 == entry.2)
    && inst.bindings.all (fun binding =>
      match binding with
      | .stack segment => stackWithin segment maxHeight maxType
      | .value ty => typeWithin ty maxType
      | .effect _ => true)

/-- The stack prefix of `count` entries. -/
def prefixStack : Nat → TyList → TyList
  | 0, _ => .nil
  | _ + 1, .nil => .nil
  | count + 1, .cons head rest => .cons head (prefixStack count rest)

/-- The top `count` entries of a stack, or the whole stack when it is shorter. -/
def tailOf (stack : TyList) (count : Nat) : TyList :=
  prefixStack (stack.length - min count stack.length) stack

/-- The stack with its top `expected` segment replaced by the result segment. -/
def replaceTail (stack expected out : TyList) : TyList :=
  (prefixStack (stack.length - expected.length) stack).append out

/-- Whether the expected segment is the stack's top segment. -/
def tailEquals (stack expected : TyList) : Bool :=
  stack.length ≥ expected.length && tailOf stack expected.length == expected

/-- Whether two stack segments hold the same types in some order. -/
def sameMultiset (left right : List Ty) : Bool := decide (left.Perm right)

/-- The constraint one failed tail match reports. -/
def mismatchConstraint (expected actual : List Ty) : Constraint :=
  if expected.length == actual.length && sameMultiset expected actual then
    Constraint.stackOrder
  else
    Constraint.stackJoin

/-- The reference state one body fold threads: work left, derivations so far. -/
structure Fold where
  work : Nat
  derivations : List Derivation
  deriving Repr, Inhabited

/-- A reference failure mapped to its outcome. -/
inductive Failure where
  | invalid : Constraint → Failure
  | exhausted : LimitKind → Failure
  | unsupported : UnsupportedKind → Failure
  deriving Repr, Inhabited

/-- The reference outcome of folding one body. -/
inductive BodyResult where
  /-- The body derived this stack and bound. -/
  | ok : TyList → EffSet → Fold → BodyResult
  /-- The body was rejected. -/
  | failed : Failure → BodyResult
  deriving Repr, Inhabited

/-- Charge work, failing closed before the declared limit is exceeded. -/
def charge (work cost : Nat) : Option Nat :=
  if cost ≤ work then some (work - cost) else none

/-- The eligibility side condition one application checks. -/
def dataOkAt (env : Env) (index : Nat) (inst : Inst) : Bool :=
  match dataSlot ((env.kind index).getD .named) with
  | some var =>
    match inst.value var with
    | some ty => ty.isData
    | none => false
  | none => true

/-- Apply one scheme to its witness: bounds, substitution, known identities. -/
def applyScheme (env : Env) (limits : Limits) (scheme : Scheme) (inst : Inst) :
    Except Failure Interface :=
  if !instValid scheme inst limits.stackHeight limits.typeSize then
    .error (.invalid .instantiationKind)
  else
    match scheme.instantiate inst with
    | none => .error (.invalid .instantiationKind)
    | some (stackIn, stackOut, effects) =>
      if !stackWithin stackIn limits.stackHeight limits.typeSize
        || !stackWithin stackOut limits.stackHeight limits.typeSize then
        .error (.exhausted .typeSize)
      else
        match effects.ids.find? (fun id => !effectKnown env id) with
        | some id => .error (.invalid (.unknownEffect id))
        | none => .ok ⟨stackIn, stackOut, effects⟩

/-- Join one interface into a running stack and bound. -/
def joinInterface (current : TyList) (derived : EffSet) (iface : Interface)
    (limits : Limits) : Except Failure (TyList × EffSet) :=
  if !tailEquals current iface.stackIn then
    .error (.invalid (mismatchConstraint iface.stackIn.toList
      (tailOf current iface.stackIn.length).toList))
  else
    let joined := replaceTail current iface.stackIn iface.stackOut
    if !stackWithin joined limits.stackHeight limits.typeSize then
      .error (.exhausted .stackHeight)
    else
      .ok (joined, derived.union iface.effects)

/-- Charge one node's work and record its derivation. -/
def stepFold (fold : Fold) (cost : Nat) (node : Nat) (iface : Interface) :
    Except Failure Fold :=
  match charge fold.work cost with
  | none => .error (.exhausted .work)
  | some work => .ok ⟨work, fold.derivations ++ [⟨node, iface⟩]⟩

mutual

  /-- Fold one body, returning its derived stack and bound. -/
  def foldBody (env : Env) (cand : Candidate) (limits : Limits) :
      (depth : Nat) → (fuel : Nat) → List Nat → TyList → EffSet → Fold → BodyResult
    | _, 0, _, _, _, _ => .failed (.exhausted .work)
    | _, _ + 1, [], stack, derived, fold => .ok stack derived fold
    | depth, fuel + 1, id :: rest, stack, derived, fold =>
      match cand.nodes[id]? with
      | none => .failed (.invalid (.malformedReference id))
      | some (.literal lit inst) =>
        match applyScheme env limits (litScheme lit) inst with
        | .error problem => .failed problem
        | .ok iface =>
          match joinInterface stack derived iface limits with
          | .error problem => .failed problem
          | .ok (joined, bound) =>
            match stepFold fold (schemeCost (litScheme lit) + joinCost iface) id iface with
            | .error problem => .failed problem
            | .ok next => foldBody env cand limits depth fuel rest joined bound next
      | some (.invocation index inst) =>
        match env.scheme index with
        | none => .failed (.invalid (.unknownDefinition index))
        | some scheme =>
          if !dataOkAt env index inst then
            .failed (.invalid .instantiationKind)
          else
            match applyScheme env limits scheme inst with
            | .error problem => .failed problem
            | .ok iface =>
              match joinInterface stack derived iface limits with
              | .error problem => .failed problem
              | .ok (joined, bound) =>
                match stepFold fold (schemeCost scheme + joinCost iface) id iface with
                | .error problem => .failed problem
                | .ok next => foldBody env cand limits depth fuel rest joined bound next
      | some (.quotation body inst) =>
        match applyScheme env limits quotationScheme inst with
        | .error problem => .failed problem
        | .ok iface =>
          if depth + 1 > limits.depth then
            .failed (.exhausted .depth)
          else
            match (inst.stack 1, inst.stack 2, inst.effects 3) with
            | (some start, some claimed, some bound) =>
              match foldBody env cand limits (depth + 1) fuel body start bound
                  ⟨fold.work, fold.derivations⟩ with
              | .failed problem => .failed problem
              | .ok final finalBound inner =>
                if final != claimed then
                  .failed (.invalid (mismatchConstraint claimed.toList final.toList))
                else if !finalBound.subsetOf bound then
                  .failed (.invalid (.effectInclusion 0))
                else
                  match joinInterface stack derived iface limits with
                  | .error problem => .failed problem
                  | .ok (joined, outerBound) =>
                    match stepFold inner (joinCost iface) id iface with
                    | .error problem => .failed problem
                    | .ok next =>
                      foldBody env cand limits depth fuel rest joined outerBound next
            | _ => .failed (.invalid .instantiationKind)

end

/-- Run the reference checker over one candidate and request. -/
def check (env : Env) (request : Request) (cand : Candidate) : Outcome :=
  if cand.format ≠ 0 || cand.revision ≠ 0 then
    .unsupported .formatRevision
  else if request.inputBytes > request.limits.bytes then
    .exhausted .bytes
  else if cand.nodes.length > request.limits.nodes then
    .exhausted .nodes
  else if !envValid env then
    .unsupported .schemeForm
  else if !stackWithin request.expected.stackIn request.limits.stackHeight
      request.limits.typeSize
    || !stackWithin request.expected.stackOut request.limits.stackHeight
      request.limits.typeSize then
    .exhausted .typeSize
  else
    match request.expected.allowedEffects.ids.find? (fun id => !effectKnown env id) with
    | some id => .invalid ⟨none, none, [], [], .unknownEffect id, false, false⟩
    | none =>
      match foldBody env cand request.limits 0 request.limits.work cand.body
          request.expected.stackIn EffSet.empty ⟨request.limits.work, []⟩ with
      | .ok final derived fold =>
        if fold.work != request.limits.work - (fold.work + 0) - 0 then
          .internalFailure
        else if !derived.subsetOf request.expected.allowedEffects then
          .invalid ⟨none, none, [], [], .effectInclusion 0, false, false⟩
        else if final != request.expected.stackOut then
          .invalid ⟨none, none, [], [], .stackJoin, false, false⟩
        else
          .accepted ⟨⟨request.expected.stackIn, final, derived⟩, fold.derivations⟩
      | .failed (.invalid constraint) =>
        .invalid ⟨none, none, [], [], constraint, false, false⟩
      | .failed (.exhausted kind) => .exhausted kind
      | .failed (.unsupported kind) => .unsupported kind

end NobleM2
