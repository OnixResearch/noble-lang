/-
Embedding and projection layer between the reference model (`NobleM2.*`) and
the Aeneas-extracted checker (`NobleKernel.*`, namespace `noble_kernel`).

The reference model uses unbounded `Nat` identifiers and cons-list stacks; the
extracted checker uses fixed-width machine scalars and Rust-style vectors.
The translations below are total:

* `Nat` identifiers are reduced modulo `2^32` (two's-complement wraparound,
  matching the generated `U32` representation). Every fragment identifier is
  far below the bound, so the reduction is the identity on all real inputs.
* `Int` literal payloads are embedded with `BitVec.ofInt 64`, i.e. two's
  complement on 64 bits, matching the generated `I64` representation.
* Lists become vectors through `vecOf`. Lean lists carry no a-priori length
  bound while the extracted `Vec` mirrors Rust's allocation-finite vectors,
  so an input longer than `Usize.max` — unreachable for every fragment value —
  falls back to the empty vector. In range, `vecOf f l` is `Vec.from (l.map f)`.

The recursive translations are fuel-parameterized: the pure `Vec` builders go
through `vecOf`'s higher-order map, and Lean's termination checker cannot
relate an unapplied mutual reference through it. An explicit depth budget
(one decrement per data level, discharged by the pattern variable) makes every
recursive call trivially decreasing. The size measures bound the true depth of
the value they measure — `partListSize` counts whole value-pattern subtrees,
not just the list spine — and the public wrappers add a constant margin, so
the fuel-exhausted fallback arms never fire on any well-formed input: for
every reference value, embedding followed by projection is the identity on
the fragment's identifiers and shapes.
-/

import NobleKernel
import NobleM2.Env
import NobleM2.Types
import NobleM2.Candidate

open Aeneas Aeneas.Std

namespace NobleM2

/-! ## List → Vec helper -/

/-- Build an extracted vector from a reference list, mapping each entry.
    See the module docstring for the (unreachable) overflow fallback. -/
def vecOf {α β : Type} (f : α → β) (l : List α) : alloc.vec.Vec β :=
  if h : l.length ≤ Usize.max then
    alloc.vec.Vec.from (l.map f) (by simpa using h)
  else
    alloc.vec.Vec.new β

/-! ## Scalars -/

/-- Reference identifier → extracted `U32`, modulo `2^32`. -/
def embedU32 (n : Nat) : Std.U32 := ⟨BitVec.ofNat 32 n⟩

/-- Reference literal payload → extracted `I64`, two's complement on 64 bits. -/
def embedI64 (i : Int) : Std.I64 := ⟨BitVec.ofInt 64 i⟩

/-- Extracted `U32` → its natural value. -/
def projectU32 (x : Std.U32) : Nat := x.val

/-! ## Types, stacks, effect bounds: embedding -/

mutual

  /-- Fuel-driven type embedding: `fuel` decrements once per data level. -/
  def embedTyFuel (fuel : Nat) (t : Ty) : noble_kernel.types.Ty :=
    match fuel, t with
    | 0, _ => .UnitType
    | _ + 1, .unit => .UnitType
    | _ + 1, .bool => .BoolType
    | _ + 1, .i64 => .I64Type
    | _ + 1, .text => .TextType
    | _ + 1, .syn => .SyntaxType
    | fuel + 1, .pair a b => .PairType (embedTyFuel fuel a) (embedTyFuel fuel b)
    | fuel + 1, .sum a b => .SumType (embedTyFuel fuel a) (embedTyFuel fuel b)
    | fuel + 1, .list a => .ListType (embedTyFuel fuel a)
    | fuel + 1, .program i o e =>
      .ProgramType (embedTyListFuel fuel i) (embedTyListFuel fuel o)
        (vecOf embedU32 e.ids)
    | _ + 1, .resource k => .ResourceType (embedU32 k)
    termination_by fuel

  /-- Fuel-driven stack (list) embedding; the whole stack shares the budget. -/
  def embedTyListFuel (fuel : Nat) (l : TyList) :
      alloc.vec.Vec noble_kernel.types.Ty :=
    match fuel with
    | 0 => alloc.vec.Vec.new noble_kernel.types.Ty
    | fuel + 1 => vecOf (embedTyFuel fuel) l.toList
    termination_by fuel

end

/-- Reference type → extracted type. The budget exceeds the nesting depth of
    any type of that size, so the fallback arm is unreachable here. -/
def embedTy (t : Ty) : noble_kernel.types.Ty := embedTyFuel (2 * Ty.size t + 2) t

/-- Reference stack (bottom first) → extracted vector of types. -/
def embedTyList (l : TyList) : alloc.vec.Vec noble_kernel.types.Ty :=
  embedTyListFuel (2 * TyList.size l + 2) l

/-- Reference effect bound → extracted effect set. -/
def embedEffSet (s : EffSet) : noble_kernel.types.EffSet := vecOf embedU32 s.ids

/-! ## Patterns: embedding -/

mutual

  /-- Size of a stack pattern: one point per constructor node, counting the
      value-pattern subtrees (a `value` entry's pattern is part of the size,
      so the fuel budget dominates every nested pattern). -/
  def partListSize : PartList → Nat
    | .nil => 1
    | .cons (.stack _) rest => 1 + partListSize rest
    | .cons (.value p) rest => 1 + patternSize p + partListSize rest
    termination_by l => sizeOf l

  /-- Size of a value pattern: one point per constructor node; program
      signatures count their full stack patterns and effect slots. -/
  def patternSize : Pattern → Nat
    | .unit => 1
    | .bool => 1
    | .i64 => 1
    | .text => 1
    | .syn => 1
    | .resource _ => 1
    | .var _ => 1
    | .pair a b => 1 + patternSize a + patternSize b
    | .sum a b => 1 + patternSize a + patternSize b
    | .list a => 1 + patternSize a
    | .program ⟨stackIn, stackOut, effects⟩ =>
      1 + partListSize stackIn + partListSize stackOut + effects.length
    termination_by p => sizeOf p
    decreasing_by all_goals simp +arith

end

/-- Reference effect slot → extracted slot. -/
def embedEffectSlot : EffectSlot → noble_kernel.shapes.EffectSlot
  | .effect id => .Effect (embedU32 id)
  | .var v => .Var (embedU32 v)

mutual

  /-- Fuel-driven pattern embedding. -/
  def embedPatternFuel (fuel : Nat) (p : Pattern) : noble_kernel.shapes.Pattern :=
    match fuel, p with
    | 0, _ => .UnitPattern
    | _ + 1, .unit => .UnitPattern
    | _ + 1, .bool => .BoolPattern
    | _ + 1, .i64 => .I64Pattern
    | _ + 1, .text => .TextPattern
    | _ + 1, .syn => .SyntaxPattern
    | _ + 1, .resource k => .ResourcePattern (embedU32 k)
    | _ + 1, .var v => .VarPattern (embedU32 v)
    | fuel + 1, .pair a b =>
      .PairPattern (embedPatternFuel fuel a) (embedPatternFuel fuel b)
    | fuel + 1, .sum a b =>
      .SumPattern (embedPatternFuel fuel a) (embedPatternFuel fuel b)
    | fuel + 1, .list a => .ListPattern (embedPatternFuel fuel a)
    | fuel + 1, .program sig =>
      .ProgramPattern (embedPartListFuel fuel sig.stackIn)
        (embedPartListFuel fuel sig.stackOut)
        (vecOf embedEffectSlot sig.effects.toList)
    termination_by fuel

  /-- Fuel-driven stack-entry embedding. -/
  def embedStackPartFuel (fuel : Nat) (s : StackPart) :
      noble_kernel.shapes.Pattern :=
    match fuel, s with
    | 0, .stack v => .StackVarPattern (embedU32 v)
    | 0, .value _ => .UnitPattern
    | _ + 1, .stack v => .StackVarPattern (embedU32 v)
    | fuel + 1, .value p => embedPatternFuel fuel p
    termination_by fuel

  /-- Fuel-driven stack-pattern embedding; the whole list shares the budget. -/
  def embedPartListFuel (fuel : Nat) (l : PartList) :
      alloc.vec.Vec noble_kernel.shapes.Pattern :=
    match fuel with
    | 0 => alloc.vec.Vec.new noble_kernel.shapes.Pattern
    | fuel + 1 => vecOf (embedStackPartFuel fuel) l.toList
    termination_by fuel

end

/-- Reference value pattern → extracted pattern (budget dominates depth). -/
def embedPattern (p : Pattern) : noble_kernel.shapes.Pattern :=
  embedPatternFuel (2 * patternSize p + 2) p

/-- Reference stack pattern → extracted vector of patterns. -/
def embedPartList (l : PartList) : alloc.vec.Vec noble_kernel.shapes.Pattern :=
  embedPartListFuel (2 * partListSize l + 2) l

/-- Reference variable kind → extracted kind. -/
def embedKind : Kind → noble_kernel.words.VariableKind
  | .stack => .Stack
  | .value => .Value
  | .effect => .Effect

/-- Reference scheme → extracted scheme. -/
def embedScheme (s : Scheme) : noble_kernel.words.Scheme :=
  { var_kinds := vecOf embedKind s.varKinds
    stack_in := embedPartList s.stackIn
    stack_out := embedPartList s.stackOut
    effects := vecOf embedEffectSlot s.effects.toList }

/-! ## Words: bindings, instantiations, literals, nodes -/

/-- Reference binding → extracted binding. -/
def embedBinding : Binding → noble_kernel.words.Binding
  | .stack segment => .Stack (embedTyList segment)
  | .value ty => .Value (embedTy ty)
  | .effect bound => .Effect (embedEffSet bound)

/-- Reference instantiation → extracted instantiation. -/
def embedInst (inst : Inst) : noble_kernel.words.Inst :=
  ⟨vecOf embedBinding inst.bindings⟩

/-- Reference literal → extracted literal. -/
def embedLit : Lit → noble_kernel.untrusted.Lit
  | .i64 n => .I64Lit (embedI64 n)
  | .bool b => .BoolLit b
  | .text => .TextLit
  | .unit => .UnitLit

/-- Reference node → extracted node. -/
def embedNode : Node → noble_kernel.untrusted.Node
  | .literal lit inst => .Literal (embedLit lit) (embedInst inst)
  | .invocation d inst => .Invocation (embedU32 d) (embedInst inst)
  | .quotation body inst => .Quotation (vecOf embedU32 body) (embedInst inst)

/-! ## Candidate, request, environment: embedding -/

/-- Reference candidate → extracted candidate. The reference model format
    marker `0` names the supported revision; the extracted crate supported
    revision advanced to `1` when reference bindings entered the candidate
    schema (fragment v1), so the supported marker shifts with it and every
    foreign revision stays foreign: the reference rejects any nonzero
    format, and every nonzero reference format embeds to a nonzero — and,
    for every fragment value, mod-2^32-unrepresentable — extracted format. -/
def embedCandidate (cand : Candidate) : noble_kernel.untrusted.Candidate :=
  { format := embedU32 (cand.format + 1)
    revision := embedU32 cand.revision
    nodes := vecOf embedNode cand.nodes
    body := vecOf embedU32 cand.body }

/-- Reference expected interface → extracted expected interface. -/
def embedExpected (e : Expected) : noble_kernel.untrusted.Expected :=
  { stack_in := embedTyList e.stackIn
    stack_out := embedTyList e.stackOut
    allowed_effects := embedEffSet e.allowedEffects }

/-- Reference limits → extracted limits. -/
def embedLimits (l : Limits) : noble_kernel.untrusted.Limits :=
  { bytes := embedU32 l.bytes
    nodes := embedU32 l.nodes
    depth := embedU32 l.depth
    type_size := embedU32 l.typeSize
    stack_height := embedU32 l.stackHeight
    work := embedU32 l.work
    diagnostics := embedU32 l.diagnostics }

/-- Reference request → extracted request. -/
def embedRequest (req : Request) : noble_kernel.untrusted.Request :=
  { input_bytes := embedU32 req.inputBytes
    expected := embedExpected req.expected
    limits := embedLimits req.limits }

/-- Reference behavior → extracted behavior. -/
def embedBehavior : Behavior → noble_kernel.contracts.Behavior
  | .dup => .DupBehavior
  | .drop => .DropBehavior
  | .swap => .SwapBehavior
  | .dip => .DipBehavior
  | .arith => .ArithBehavior
  | .equals => .EqualsBehavior
  | .quote => .QuoteBehavior
  | .compose => .ComposeBehavior
  | .run => .RunBehavior
  | .reflect => .ReflectBehavior
  | .unit => .UnitBehavior
  | .pair => .PairBehavior
  | .unpair => .UnpairBehavior
  | .inl => .InlBehavior
  | .inr => .InrBehavior
  | .case => .CaseBehavior
  | .if => .IfBehavior
  | .nil => .NilBehavior
  | .cons => .ConsBehavior
  | .listCase => .ListCaseBehavior
  | .testEmit => .TestEmitBehavior
  | .named => .NamedBehavior

/-- Reference environment → extracted environment. -/
def embedEnv (env : Env) : noble_kernel.contracts.Env :=
  { defs := vecOf embedScheme env.defs
    kinds := vecOf embedBehavior env.kinds
    -- Fragment v0 reference environments carry no external environment
    -- data: no dependencies and no user-declared schemas embed.
    deps := alloc.vec.Vec.new _
    schemas := alloc.vec.Vec.new _
    effects := vecOf embedU32 env.effects }

/-! ## Projection: extracted values → reference model -/

/-- Extracted effect bound → reference effect set (normalized sorted, repeat
    free, exactly as `EffSet.ofIds` builds it). -/
def projectEffSet (s : noble_kernel.types.EffSet) : EffSet :=
  EffSet.ofIds (s.val.map projectU32)

/-- The element list of a `ProgramType` node's first field vector is smaller
    than the node itself. `Vec`/`Slice`/`ListN` nest the elements behind three
    projections, which is the one non-trivial sizeOf fact needed to terminate
    `genTySize` below. -/
theorem sizeOf_vec1_lt_programType (i o : alloc.vec.Vec noble_kernel.types.Ty)
    (e : noble_kernel.types.EffSet) :
    sizeOf i.slice.list < sizeOf (noble_kernel.types.Ty.ProgramType i o e) := by
  have h3 : sizeOf (noble_kernel.types.Ty.ProgramType i o e)
      = 1 + sizeOf i + sizeOf o + sizeOf e := by
    rw [noble_kernel.types.Ty.ProgramType.sizeOf_spec]
  obtain ⟨⟨leng, list, bound⟩⟩ := i
  have hN : sizeOf leng = leng := rfl
  have hv := alloc.vec.Vec.mk.sizeOf_spec
    (Slice.mk (α := noble_kernel.types.Ty) leng list bound)
  have hs := Slice.mk.sizeOf_spec (α := noble_kernel.types.Ty) leng list bound
  simp only [hv, hs, hN] at h3 ⊢
  omega

/-- The element list of a `ProgramType` node's second field vector is smaller
    than the node itself; same argument as `sizeOf_vec1_lt_programType`. -/
theorem sizeOf_vec2_lt_programType (i o : alloc.vec.Vec noble_kernel.types.Ty)
    (e : noble_kernel.types.EffSet) :
    sizeOf o.slice.list < sizeOf (noble_kernel.types.Ty.ProgramType i o e) := by
  have h3 : sizeOf (noble_kernel.types.Ty.ProgramType i o e)
      = 1 + sizeOf i + sizeOf o + sizeOf e := by
    rw [noble_kernel.types.Ty.ProgramType.sizeOf_spec]
  obtain ⟨⟨leng, list, bound⟩⟩ := o
  have hN : sizeOf leng = leng := rfl
  have hv := alloc.vec.Vec.mk.sizeOf_spec
    (Slice.mk (α := noble_kernel.types.Ty) leng list bound)
  have hs := Slice.mk.sizeOf_spec (α := noble_kernel.types.Ty) leng list bound
  simp only [hv, hs, hN] at h3 ⊢
  omega

/-! A size measure for extracted types: one point per constructor node. Its
    `ProgramType` arm recurses through the `Vec`/`Slice`/`ListN` field chain,
    discharged by `sizeOf_programType_lt`. The body never touches `sizeOf`, so
    the size instances of the extracted module are not needed at run time. -/

mutual

  def genTySize : noble_kernel.types.Ty → Nat
    | .UnitType => 1
    | .BoolType => 1
    | .I64Type => 1
    | .TextType => 1
    | .SyntaxType => 1
    | .PairType a b => 1 + genTySize a + genTySize b
    | .SumType a b => 1 + genTySize a + genTySize b
    | .ListType a => 1 + genTySize a
    | .ProgramType i o _ =>
      1 + genTyListSizeN i.slice.leng i.slice.list
        + genTyListSizeN o.slice.leng o.slice.list
    | .ResourceType _ => 1
    termination_by t => sizeOf t
    decreasing_by
      all_goals
        first
          | exact sizeOf_vec1_lt_programType _ _ _
          | exact sizeOf_vec2_lt_programType _ _ _
          | (rw [noble_kernel.types.Ty.PairType.sizeOf_spec]; omega)
          | (rw [noble_kernel.types.Ty.SumType.sizeOf_spec]; omega)
          | (rw [noble_kernel.types.Ty.ListType.sizeOf_spec]; omega)

  /-- A size measure over the extracted element list (length-indexed). -/
  def genTyListSizeN (n : Nat) (l : Aeneas.Data.ListN.ListN noble_kernel.types.Ty n) :
      Nat :=
    match l with
    | .nil => 1
    | .cons t ts => 1 + genTySize t + genTyListSizeN _ ts
    termination_by sizeOf l

end

/- A size measure over the extracted type list. -/
def genTyListSize : List noble_kernel.types.Ty → Nat
  | [] => 1
  | t :: ts => 1 + genTySize t + genTyListSize ts

mutual

  /-- Fuel-driven type projection. -/
  def projectTyFuel (fuel : Nat) (t : noble_kernel.types.Ty) : Ty :=
    match fuel, t with
    | 0, _ => .unit
    | _ + 1, .UnitType => .unit
    | _ + 1, .BoolType => .bool
    | _ + 1, .I64Type => .i64
    | _ + 1, .TextType => .text
    | _ + 1, .SyntaxType => .syn
    | fuel + 1, .PairType a b => .pair (projectTyFuel fuel a) (projectTyFuel fuel b)
    | fuel + 1, .SumType a b => .sum (projectTyFuel fuel a) (projectTyFuel fuel b)
    | fuel + 1, .ListType a => .list (projectTyFuel fuel a)
    | fuel + 1, .ProgramType i o e =>
      .program (projectTyListFuel fuel i.val) (projectTyListFuel fuel o.val)
        (projectEffSet e)
    | _ + 1, .ResourceType k => .resource (projectU32 k)
    termination_by fuel

  /-- Fuel-driven stack projection over the extracted type list. -/
  def projectTyListFuel (fuel : Nat) (l : List noble_kernel.types.Ty) : TyList :=
    match fuel, l with
    | 0, _ => .nil
    | _ + 1, [] => .nil
    | fuel + 1, t :: ts => .cons (projectTyFuel fuel t) (projectTyListFuel fuel ts)
    termination_by fuel

end

/-- Extracted type → reference type (budget dominates depth). -/
def projectTy (t : noble_kernel.types.Ty) : Ty :=
  projectTyFuel (2 * genTySize t + 2) t

/-- Extracted vector of types → reference stack. -/
def projectTyList (v : alloc.vec.Vec noble_kernel.types.Ty) : TyList :=
  projectTyListFuel (2 * genTyListSize v.val + 2) v.val

/-- Extracted interface → reference interface. -/
def projectInterface (i : noble_kernel.untrusted.Interface) : Interface :=
  { stackIn := projectTyList i.stack_in
    stackOut := projectTyList i.stack_out
    effects := projectEffSet i.effects }

/-- Extracted derivation → reference derivation. -/
def projectDerivation (d : noble_kernel.untrusted.Derivation) : Derivation :=
  { node := projectU32 d.node
    interface := projectInterface d.interface }

/-- Extracted checked result → reference checked result. -/
def project_checked (c : noble_kernel.untrusted.Checked) : Checked :=
  { interface := projectInterface c.interface
    derivations := c.derivations.val.map projectDerivation }

/-- Extracted constraint → reference constraint. -/
def projectConstraint : noble_kernel.untrusted.Constraint → Constraint
  | .StackJoin => .stackJoin
  | .StackOrder => .stackOrder
  | .EffectInclusion id => .effectInclusion (projectU32 id)
  | .Eligibility ty => .eligibility (projectTy ty)
  | .UnknownEffect id => .unknownEffect (projectU32 id)
  | .InstantiationKind => .instantiationKind
  | .InstantiationArity => .instantiationArity
  | .MalformedReference n => .malformedReference (projectU32 n)
  | .UnknownDefinition d => .unknownDefinition (projectU32 d)
  | .CyclicWitness => .cyclicWitness

/-- Extracted diagnostic → reference diagnostic. -/
def projectDiagnostic (d : noble_kernel.untrusted.Diagnostic) : Diagnostic :=
  { node := d.node.map projectU32
    definition := d.«def».map projectU32
    expected := d.expected.val.map projectTy
    actual := d.actual.val.map projectTy
    constraint := projectConstraint d.constraint
    provenanceAvailable := d.provenance_available
    truncated := d.truncated }

/-- Extracted unsupported kind → reference unsupported kind. -/
def projectUnsupportedKind : noble_kernel.untrusted.UnsupportedKind → UnsupportedKind
  | .FormatRevision => .formatRevision
  | .NodeForm => .nodeForm
  | .SchemeForm => .schemeForm
  | .RecursiveDependency d => .recursiveDependency (projectU32 d)
  | .RecursiveSchema id => .recursiveSchema (projectU32 id)

/-- Extracted limit kind → reference limit kind. -/
def projectLimitKind : noble_kernel.untrusted.LimitKind → LimitKind
  | .Bytes => .bytes
  | .Nodes => .nodes
  | .Depth => .depth
  | .TypeSize => .typeSize
  | .StackHeight => .stackHeight
  | .Work => .work
  | .Diagnostics => .diagnostics

/-- Extracted outcome → reference outcome. -/
def project_outcome : noble_kernel.untrusted.Outcome → Outcome
  | .Accepted checked => .accepted (project_checked checked)
  | .Invalid diag => .invalid (projectDiagnostic diag)
  | .Unsupported kind => .unsupported (projectUnsupportedKind kind)
  | .Exhausted kind => .exhausted (projectLimitKind kind)
  | .InternalFailure => .internalFailure

/-- The extracted checker's `Result`-wrapped outcome, projected onto the
    reference outcome domain. A terminating successful run observes `ok` and
    projects its outcome; a monadic `fail` or divergence — not produced by any
    bounded run — projects to the reference's reserved `.internalFailure`. -/
def project_result (r : Result noble_kernel.untrusted.Outcome) : Outcome :=
  match r.match with
  | .ok o => project_outcome o
  | .vis _ _ => .internalFailure
  | .div => .internalFailure

end NobleM2
