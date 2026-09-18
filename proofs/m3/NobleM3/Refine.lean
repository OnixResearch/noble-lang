/-
Reference model: per-word refinement of the extracted bootstrap table (M3).

Each table position's extracted scheme builder agrees componentwise with the
reference model's contract for that word, decided by computation over the
generated module: the kind list, both stacks, and the effect bound agree
position by position.
-/

import NobleKernel
import NobleM3.Words
import NobleM3.Embed

namespace NobleM2.Refinement

open noble_kernel Aeneas Aeneas.Std

deriving instance BEq for noble_kernel.shapes.EffectSlot
deriving instance BEq for noble_kernel.types.ResourceKind
deriving instance BEq for noble_kernel.types.Ty
deriving instance BEq for noble_kernel.shapes.Pattern
deriving instance BEq for noble_kernel.words.VariableKind
deriving instance BEq for noble_kernel.types.EffSet
deriving instance BEq for noble_kernel.words.Variable
deriving instance BEq for Aeneas.Std.alloc.vec.Vec

/-- Run one extracted scheme builder, projecting success to `some` and any
failure or divergence to `none` (the builders are total on the pinned
toolchain, so the failure arms are unobservable). -/
def schemeOf (r : Result noble_kernel.words.Scheme) :
    Option noble_kernel.words.Scheme :=
  match r.match with
  | .ok s => some s
  | .vis _ _ => none
  | .div => none

/-- The extracted table as a plain list of behavior/scheme pairs. -/
def extractedTable :
    List (noble_kernel.contracts.Behavior × noble_kernel.words.Scheme) :=
  match noble_kernel.contracts.bootstrap.data.table.match with
  | .ok v => v.val
  | .vis _ _ => []
  | .div => []

/-- The extracted builders, one per position. -/
def extractedBuilders : List noble_kernel.words.Scheme :=
  List.map Prod.snd extractedTable

/-- Structural equality of two patterns via the derived `BEq` on the
generated constructors. -/
def partAgreesSimple (s t : noble_kernel.shapes.Pattern) : Bool :=
  s == t

/-- Componentwise agreement of one extracted scheme with its reference
contract: the kind list, both stacks, and the effect bound agree position by
position. -/
def schemeAgrees (s t : noble_kernel.words.Scheme) : Bool :=
  s.var_kinds == t.var_kinds
    && s.stack_in == t.stack_in
    && s.stack_out == t.stack_out
    && s.effects == t.effects

/-- One table position's agreement: the extracted builder's scheme and
behavior against the reference word's contract. -/
def positionAgrees (n : Nat) : Bool :=
  match extractedTable[n]? with
  | some (_, scheme) =>
    match NobleM2.wordTable[n]? with
    | some word => schemeAgrees scheme (embedScheme word.scheme)
    | none => false
  | none => false

/-- The full table: every one of the 23 positions' extracted builder agrees
componentwise with the reference contract. -/
theorem table_refines :
    (List.range 23).all (fun n => positionAgrees n) = true := by
  native_decide

/-- The extraction's empty scheme (the builder list is total, so this is
unreachable). -/
def defaultSchemeE : noble_kernel.words.Scheme :=
  { var_kinds := alloc.vec.Vec.new _, stack_in := alloc.vec.Vec.new _,
    stack_out := alloc.vec.Vec.new _, effects := alloc.vec.Vec.new _ }

/-- Extracted substitution: both stacks through the generated walk. -/
def extractedStacks (scheme : noble_kernel.words.Scheme) (inst : noble_kernel.words.Inst) :
    Option (Bool × List noble_kernel.types.Ty × List noble_kernel.types.Ty) :=
  let ra := noble_kernel.words.Scheme.subst_stack scheme scheme.stack_in.deref inst
  let rb := noble_kernel.words.Scheme.subst_stack scheme scheme.stack_out.deref inst
  match ra.match, rb.match with
  | (.ok raI), (.ok rbI) =>
    match raI, rbI with
    | (.Ok va), (.Ok vb) => some (true, va.val, vb.val)
    | _, _ => some (false, [], [])
  | _, _ => some (false, [], [])

/-- Reference substitution through the model's `instantiate`. -/
def referenceStacks (word : NobleM2.Word) (inst : NobleM2.Inst) :
    Option (List noble_kernel.types.Ty × List noble_kernel.types.Ty) :=
  match NobleM2.Scheme.instantiate word.scheme inst with
  | some (a, b, _) => some (List.map embedTy a.toList, List.map embedTy b.toList)
  | none => none

/-- One word's application agreement at an instantiation: the extracted
substitution's stacks equal the reference substitution's embedded stacks. -/
def embedInstE (inst : NobleM2.Inst) : noble_kernel.words.Inst := embedInst inst
def applyAgrees (n : Nat) (inst : NobleM2.Inst) : Bool :=
  match NobleM2.wordTable[n]? with
  | some word =>
    match extractedStacks (List.getD extractedBuilders n defaultSchemeE) (embedInstE inst),
          referenceStacks word inst with
    | some (okA, a, b), some (c, d) => okA && a == c && b == d
    | _, _ => false
  | none => false

end NobleM2.Refinement

namespace NobleM2.Refinement

/-! ## The canonical witness per word

One witness per word that instantiates every declared variable with a
canonical concrete binding, so the extracted and reference substitutions run
on the same finite input. -/

/-- The canonical type for a value variable. -/
def canonTy : NobleM2.Ty := .i64

/-- The canonical effect set for an effect variable. -/
def canonSet : NobleM2.EffSet := EffSet.empty

/-- A canonical instantiation for the word at position `n`: one binding per
declared variable kind (stack = one `canonTy`, value = `canonTy`, effect =
`canonSet`). -/
def canonInst (n : Nat) : NobleM2.Inst :=
  match NobleM2.wordTable[n]? with
  | some word => ⟨word.scheme.varKinds.map (fun k =>
      match k with
      | .stack => .stack (.cons canonTy .nil)
      | .value => .value canonTy
      | .effect => .effect canonSet)⟩
  | none => ⟨[]⟩

/-- The application-agreement rows, one per word. -/
def applyRows : List Bool :=
  (List.range 23).map (fun n => applyAgrees n (canonInst n))

/-- The application-level family: every word's substitution agrees. -/
theorem apply_refines : applyRows = List.replicate 23 true := by
  native_decide

end NobleM2.Refinement
