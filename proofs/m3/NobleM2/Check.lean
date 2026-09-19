/-
Reference model: the reference decision function (M2 fragment v0, extended
to bootstrap fragment v1).

`check` mirrors the finite acceptance rules: it preflights the request, folds
the entry body, and returns the five-way outcome domain. Work is charged
before each bounded step, and a nested body shares the same fold state.

Fragment v1 adds the external-environment-data validation walks (B-CHECK-02:
recursive definition dependencies and user-declared recursive schemas,
rejected in preflight before any candidate body is checked) and resolves
every witness's reference bindings before substitution (B-CHECK-05: a
cyclic type-equation chain rejects invalid; the walk charges before each
hop and fails closed on its budget).
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
    && (inst.bindings.zip scheme.varKinds).all (fun entry =>
      match entry.1.kindOf with
      | none => true
      | some kind => kind == entry.2)
    && inst.bindings.all (fun binding =>
      match binding with
      | .stack segment => stackWithin segment maxHeight maxType
      | .value ty => typeWithin ty maxType
      | .effect _ => true
      | .ref _ => true)

/-- Whether two stack segments hold the same types in some order. -/
def sameMultiset (left right : List Ty) : Bool := decide (left.Perm right)

/-- The constraint one failed tail match reports. -/
def mismatchConstraint (expected actual : List Ty) : Constraint :=
  if expected.length == actual.length && sameMultiset expected actual then
    Constraint.stackOrder
  else
    Constraint.stackJoin

/-- Build one rejection diagnostic under the declared diagnostic budget,
truncating the recorded stacks when they together exceed it. -/
def diagnosticOf (limits : Limits) (expected actual : List Ty)
    (constraint : Constraint) : Diagnostic :=
  if expected.length + actual.length > limits.diagnostics then
    let kept := expected.take (limits.diagnostics - actual.length)
    ⟨none, none, kept, actual.take (limits.diagnostics - kept.length), constraint, false, true⟩
  else ⟨none, none, expected, actual, constraint, false, false⟩

/-- The reference state one body fold threads: work left, derivations so far. -/
structure Fold where
  work : Nat
  derivations : List Derivation
  deriving Repr, Inhabited

/-- A reference failure mapped to its outcome. -/
inductive Failure where
  | invalid : Constraint → List Ty → List Ty → Failure
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

/-- Apply one scheme to its witness (fragment v1): the witness's reference
bindings are resolved first (B-CHECK-05) — a cyclic chain rejects invalid, a
chain that would outrun the declared work limit fails closed — then the v0
discipline applies: bounds, substitution, known identities. The resolved
witness returns beside the interface, so callers read its segments
directly. -/
def applyScheme (env : Env) (limits : Limits) (scheme : Scheme) (inst : Inst) :
    Except Failure (Interface × Inst) :=
  if !instValid scheme inst limits.stackHeight limits.typeSize then
    .error (.invalid .instantiationKind [] [])
  else
    match Inst.resolve scheme.varKinds inst limits.work with
    | .error .cyclic => .error (.invalid .cyclicWitness [] [])
    | .error .exhausted => .error (.exhausted .work)
    | .error .unknown => .error (.invalid .instantiationKind [] [])
    | .error .kindMismatch => .error (.invalid .instantiationKind [] [])
    | .ok (resolved, _) =>
      match scheme.instantiate resolved with
      | none => .error (.invalid .instantiationKind [] [])
      | some (stackIn, stackOut, effects) =>
        if !stackWithin stackIn limits.stackHeight limits.typeSize
          || !stackWithin stackOut limits.stackHeight limits.typeSize then
          .error (.exhausted .typeSize)
        else
          match effects.ids.find? (fun id => !effectKnown env id) with
          | some id => .error (.invalid (.unknownEffect id) [] [])
          | none => .ok (⟨stackIn, stackOut, effects⟩, resolved)

/-! ## External-environment-data validation (fragment v1, B-CHECK-02)

Recursive definition dependencies and user-declared recursive schemas are
unsupported: both are rejected during preflight, before any candidate body
is checked, so external environment data never enters checking unvalidated.
Each walk is bounded and charges the declared work limit before every
dependency edge and schema entry. -/

/-- The outcome of one external-data validation walk. -/
inductive ValidateResult where
  /-- The data validated; the nat is the work budget left. -/
  | ok : Nat → ValidateResult
  /-- A recursive dependency cycle closing on the named definition. -/
  | depCycle : Nat → ValidateResult
  /-- A user-declared recursive schema naming the declaration. -/
  | schemaCycle : Nat → ValidateResult
  /-- A declared scheme outside the accepted form. -/
  | badForm : ValidateResult
  /-- The declared work limit failed closed. -/
  | exhausted : ValidateResult
  deriving Repr, DecidableEq, Inhabited

/-- What one unit of the dependency walk is doing. -/
inductive DepMode where
  /-- Scan the next unvisited root definition. -/
  | roots : DepMode
  /-- Dispatch on the walk stack's top. -/
  | dispatch : DepMode
  /-- Inspect the remaining declared edges of `top` (already on-path). -/
  | edges : Nat → List Nat → DepMode
  deriving Repr

/-- One unit of the dependency walk (B-CHECK-02), mirroring one iteration
of the kernel's `dep_step` loop nest: a root advances, a stack top is
dispatched on (unvisited turns on-path and hands its edges to the scan;
on-path has finished its successors and turns done; done pops), or one
declared edge is inspected — charging work exactly then. An edge to an
already on-path definition closes a cycle and rejects, naming that
definition. `fuel` is the walk's structural budget: every unit is a pop, a
mark, a root advance, or a charged edge, so the generous bound never fires
while the declared work limit holds. -/
def depWalk (env : Env) (count : Nat) :
    Nat → Nat → DepMode → Nat → List Nat → List Nat → List Nat → ValidateResult
  | 0, _, _, _, _, _, _ => .exhausted
  | fuel + 1, next, .roots, work, stack, onPath, done =>
      if count ≤ next then .ok work
      else if done.contains next then
        depWalk env count fuel (next + 1) .roots work stack onPath done
      else
        depWalk env count fuel next .dispatch work (next :: stack) onPath done
  | fuel + 1, next, .dispatch, work, [], onPath, done =>
      depWalk env count fuel next .roots work [] onPath done
  | fuel + 1, next, .dispatch, work, top :: rest, onPath, done =>
      if done.contains top then
        depWalk env count fuel next .dispatch work rest onPath done
      else if onPath.contains top then
        depWalk env count fuel next .dispatch work rest onPath (top :: done)
      else
        depWalk env count fuel next (.edges top (env.depsOf top)) work
          (top :: rest) (top :: onPath) done
  | fuel + 1, next, .edges top ds, work, stack, onPath, done =>
      match ds with
      | [] => depWalk env count fuel next .dispatch work stack onPath done
      | d :: rest =>
        if work = 0 then .exhausted
        else if count ≤ d then
          depWalk env count fuel next (.edges top rest) (work - 1) stack onPath done
        else if onPath.contains d then .depCycle d
        else
          depWalk env count fuel next (.edges top rest) (work - 1)
            (if done.contains d then stack else d :: stack) onPath done
termination_by fuel _ _ _ _ _ _ => fuel

/-- Validate the environment's definition dependencies (B-CHECK-02): the
walk starts at the first root with the full declared work budget. The fuel
bound covers every unit the walk can spend under that budget (each of the
`count` definitions contributes at most a root advance, a dispatch, an edge
hand-off, and a pop; each charged edge is one unit). -/
def validateDeps (env : Env) (work : Nat) : ValidateResult :=
  depWalk env env.defs.length (5 * work + 6 * env.defs.length + 16)
    0 .roots work [] [] []

/-- Validate the environment's user-declared schemas (B-CHECK-02): a
declaration that names itself is a user-declared recursive schema and is
rejected, naming the declaration; a non-recursive declaration must still be
a well-formed scheme. Each entry charges the declared work limit before it
is inspected; the bootstrap environment declares none, so its scan charges
nothing. -/
def validateSchemas (env : Env) : Nat → List SchemaDecl → ValidateResult
  | _, [] => .ok 0
  | work, decl :: rest =>
      if work = 0 then .exhausted
      else if decl.recursive then .schemaCycle decl.id
      else if !schemeValid decl.scheme then .badForm
      else validateSchemas env (work - 1) rest

/-- Join one interface into a running stack and bound. -/
def joinInterface (current : TyList) (derived : EffSet) (iface : Interface)
    (limits : Limits) : Except Failure (TyList × EffSet) :=
  if !tailEquals current iface.stackIn then
    .error (.invalid (mismatchConstraint iface.stackIn.toList
      (tailOf current iface.stackIn.length).toList) iface.stackIn.toList
      (tailOf current iface.stackIn.length).toList)
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

  /-- Fold one body, returning its derived stack and bound (fragment v1:
  every witness resolves its reference bindings inside `applyScheme`, and
  the eligibility side condition is decided on the resolved witness, where
  the kernel's `project` reads it). -/
  def foldBody (env : Env) (cand : Candidate) (limits : Limits) :
      (depth : Nat) → (fuel : Nat) → List Nat → TyList → EffSet → Fold → BodyResult
    | _, 0, _, _, _, _ => .failed (.exhausted .work)
    | _, _ + 1, [], stack, derived, fold => .ok stack derived fold
    | depth, fuel + 1, id :: rest, stack, derived, fold =>
      match cand.nodes[id]? with
      | none => .failed (.invalid (.malformedReference id) [] [])
      | some (.literal lit inst) =>
        match applyScheme env limits (litScheme lit) inst with
        | .error problem => .failed problem
        | .ok (iface, _) =>
          match joinInterface stack derived iface limits with
          | .error problem => .failed problem
          | .ok (joined, bound) =>
            match stepFold fold (schemeCost (litScheme lit) + joinCost iface) id iface with
            | .error problem => .failed problem
            | .ok next => foldBody env cand limits depth fuel rest joined bound next
      | some (.invocation index inst) =>
        match env.scheme index with
        | none => .failed (.invalid (.unknownDefinition index) [] [])
        | some scheme =>
          match applyScheme env limits scheme inst with
          | .error problem => .failed problem
          | .ok (iface, resolved) =>
            if !dataOkAt env index resolved then
              .failed (match dataSlot ((env.kind index).getD .named) with
                | some var =>
                  (resolved.value var).elim (.invalid .instantiationKind [] [])
                    (fun ty => .invalid (.eligibility ty) [] [])
                | none => .invalid .instantiationKind [] [])
            else
              match joinInterface stack derived iface limits with
              | .error problem => .failed problem
              | .ok (joined, bound) =>
                match stepFold fold (schemeCost scheme + joinCost iface) id iface with
                | .error problem => .failed problem
                | .ok next => foldBody env cand limits depth fuel rest joined bound next
      | some (.quotation body inst) =>
        match applyScheme env limits quotationScheme inst with
        | .error problem => .failed problem
        | .ok (iface, resolved) =>
          if depth + 1 > limits.depth then
            .failed (.exhausted .depth)
          else
            match (resolved.stack 1, resolved.stack 2, resolved.effects 3) with
            | (some start, some claimed, some bound) =>
              match foldBody env cand limits (depth + 1) fuel body start EffSet.empty
                  ⟨fold.work, fold.derivations⟩ with
              | .failed problem => .failed problem
              | .ok final finalBound inner =>
                if final != claimed then
                  .failed (.invalid (mismatchConstraint claimed.toList final.toList)
                    claimed.toList final.toList)
                else if !finalBound.subsetOf bound then
                  .failed (.invalid (.effectInclusion (firstExtra finalBound bound |>.getD 0))
                    [] [])
                else
                  match joinInterface stack derived iface limits with
                  | .error problem => .failed problem
                  | .ok (joined, outerBound) =>
                    match stepFold inner (joinCost iface) id iface with
                    | .error problem => .failed problem
                    | .ok next =>
                      foldBody env cand limits depth fuel rest joined outerBound next
            | _ => .failed (.invalid .instantiationKind [] [])

end

/-- Run the reference checker over one candidate and request (fragment v1):
the preflight validates the request, the environment schemes, and the
environment's external data — dependencies and user-declared schemas —
before any candidate body is checked; the fold then runs the v0 discipline
over resolving scheme applications. -/
def check (env : Env) (request : Request) (cand : Candidate) : Outcome :=
  if cand.format ≠ 0 || cand.revision ≠ 0 then
    .unsupported .formatRevision
  else if request.inputBytes > request.limits.bytes then
    .exhausted .bytes
  else if cand.nodes.length > request.limits.nodes then
    .exhausted .nodes
  else if !envValid env then
    .unsupported .schemeForm
  else
    match validateDeps env request.limits.work with
    | .exhausted => .exhausted .work
    | .depCycle d => .unsupported (.recursiveDependency d)
    | .schemaCycle id => .unsupported (.recursiveSchema id)
    | .badForm => .unsupported .schemeForm
    | .ok _ =>
      match validateSchemas env request.limits.work env.schemas with
      | .exhausted => .exhausted .work
      | .depCycle d => .unsupported (.recursiveDependency d)
      | .schemaCycle id => .unsupported (.recursiveSchema id)
      | .badForm => .unsupported .schemeForm
      | .ok _ =>
        if !stackWithin request.expected.stackIn request.limits.stackHeight
            request.limits.typeSize
          || !stackWithin request.expected.stackOut request.limits.stackHeight
            request.limits.typeSize then
          .exhausted .typeSize
        else
          match request.expected.allowedEffects.ids.find? (fun id => !effectKnown env id) with
          | some id => .invalid (diagnosticOf request.limits [] [] (.unknownEffect id))
          | none =>
            match foldBody env cand request.limits 0 request.limits.work cand.body
                request.expected.stackIn EffSet.empty ⟨request.limits.work, []⟩ with
            | .ok final derived fold =>
              if final != request.expected.stackOut then
                .invalid (diagnosticOf request.limits request.expected.stackOut.toList
                  final.toList (mismatchConstraint request.expected.stackOut.toList final.toList))
              else if !derived.subsetOf request.expected.allowedEffects then
                .invalid (diagnosticOf request.limits [] []
                  (.effectInclusion
                    (firstExtra derived request.expected.allowedEffects |>.getD 0)))
              else
                .accepted ⟨⟨request.expected.stackIn, final, derived⟩, fold.derivations⟩
            | .failed (.invalid constraint expected actual) =>
              .invalid (diagnosticOf request.limits expected actual constraint)
            | .failed (.exhausted kind) => .exhausted kind
            | .failed (.unsupported kind) => .unsupported kind

end NobleM2
