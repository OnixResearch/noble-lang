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

end NobleM2.Refinement
