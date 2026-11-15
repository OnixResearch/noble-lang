/-
  Selected wrapping-I64 normalization laws for the issue-#32 experiment.
  Atom identities denote previously materialized graph nodes. Their valuation is
  arbitrary, but the same identity always has the same BitVec 64 value. This
  model does not construct a residual Program or prove a Rust correspondence.
-/
import NobleFoundation.Repr
import NobleFoundation.Specialize

namespace NobleFoundation.PartialStatic

/-- A reference to one materialized node, with the sign contributed to a sum.
The list, including repeated references, is never deduplicated or sorted. -/
structure Atom where
  node : Nat
  negative : Bool
deriving DecidableEq, Repr

/-- Reverse a symbolic contribution without changing the referenced node. -/
def flip (atom : Atom) : Atom :=
  { atom with negative := !atom.negative }

/-- A wrapping constant offset followed by an ordered list of signed atoms. -/
structure Form where
  offset : BitVec 64
  atoms : List Atom
deriving DecidableEq, Repr

/-- The expression fragment admitted by these normalization laws. -/
inductive Expr where
  | constant (bits : BitVec 64)
  | atom (node : Nat)
  | add (first second : Expr)
  | sub (first second : Expr)
deriving DecidableEq, Repr

/-- Evaluate an expression with the deeper/first subtraction operand first. -/
def evalExpr (values : Nat → BitVec 64) : Expr → BitVec 64
  | .constant bits => bits
  | .atom node => values node
  | .add first second => evalExpr values first + evalExpr values second
  | .sub first second => evalExpr values first - evalExpr values second

/-- Signed interpretation is over the wrapping word carrier, not integers or F32. -/
def contribution (values : Nat → BitVec 64) (atom : Atom) : BitVec 64 :=
  if atom.negative then -(values atom.node) else values atom.node

/-- Sum the ordered contributions, retaining their list and node identities. -/
def atomSum (values : Nat → BitVec 64) (atoms : List Atom) : BitVec 64 :=
  (atoms.map (contribution values)).sum

/-- Evaluate the offset and symbolic contributions as 64-bit words. -/
def evalForm (values : Nat → BitVec 64) (form : Form) : BitVec 64 :=
  form.offset + atomSum values form.atoms

/-- Addition preserves the left-to-right atom sequence. -/
def addForm (first second : Form) : Form :=
  ⟨first.offset + second.offset, first.atoms ++ second.atoms⟩

/-- Subtract the second offset and invert its signs in their original order. -/
def subForm (first second : Form) : Form :=
  ⟨first.offset - second.offset, first.atoms ++ second.atoms.map flip⟩

/-- Structural normalization, with no reordering, atom elimination or node cloning. -/
def normalize : Expr → Form
  | .constant bits => ⟨bits, []⟩
  | .atom node => ⟨0, [⟨node, false⟩]⟩
  | .add first second => addForm (normalize first) (normalize second)
  | .sub first second => subForm (normalize first) (normalize second)

/-- Negating an atom keeps the node identity and reverses only its sign. -/
theorem flip_node (atom : Atom) : (flip atom).node = atom.node := by
  rfl

/-- Reversing the sign twice preserves the exact atom, not merely its value. -/
theorem flip_involution (atom : Atom) : flip (flip atom) = atom := by
  cases atom with
  | mk node negative => cases negative <;> rfl

/-- An individual sign reversal negates its exact word contribution. -/
theorem contribution_flip (values : Nat → BitVec 64) (atom : Atom) :
    contribution values (flip atom) = -(contribution values atom) := by
  apply NobleFoundation.toZ_injective
  cases atom with
  | mk node negative =>
      cases negative <;>
        simp [flip, contribution, NobleFoundation.toZ_neg]

/-- The atom sum of two concatenated lists is the wrapping sum of each side. -/
theorem atomSum_append (values : Nat → BitVec 64) (first second : List Atom) :
    atomSum values (first ++ second) = atomSum values first + atomSum values second := by
  simp [atomSum, List.map_append, List.sum_append]

/-- Flipping a list negates its sum, including a repeated reference's multiplicity. -/
theorem atomSum_flip (values : Nat → BitVec 64) (atoms : List Atom) :
    atomSum values (atoms.map flip) = -(atomSum values atoms) := by
  induction atoms with
  | nil => simp [atomSum]
  | cons atom rest ih =>
      calc
        atomSum values ((atom :: rest).map flip) =
            contribution values (flip atom) + atomSum values (rest.map flip) := by
              simp [atomSum]
        _ = -(contribution values atom) + -(atomSum values rest) := by
              rw [contribution_flip, ih]
        _ = -(atomSum values (atom :: rest)) := by
              apply NobleFoundation.toZ_injective
              simp only [atomSum, List.map_cons, List.sum_cons,
                NobleFoundation.toZ_add, NobleFoundation.toZ_neg]
              ring

/-- Concatenating atom lists and accumulating offsets evaluates as word addition. -/
theorem addForm_eval (values : Nat → BitVec 64) (first second : Form) :
    evalForm values (addForm first second) =
      evalForm values first + evalForm values second := by
  simp only [evalForm, addForm, atomSum_append]
  apply NobleFoundation.toZ_injective
  simp only [NobleFoundation.toZ_add]
  ring

/-- Subtracting a form flips the RHS atoms and subtracts the RHS offset. -/
theorem subForm_eval (values : Nat → BitVec 64) (first second : Form) :
    evalForm values (subForm first second) =
      evalForm values first - evalForm values second := by
  simp only [evalForm, subForm, atomSum_append, atomSum_flip]
  apply NobleFoundation.toZ_injective
  simp only [NobleFoundation.toZ_add, NobleFoundation.toZ_sub, NobleFoundation.toZ_neg]
  ring

/-- Updating only the offset accumulates one wrapping constant. -/
theorem increment_offset_eval (values : Nat → BitVec 64) (form : Form)
    (bits : BitVec 64) :
    evalForm values { form with offset := form.offset + bits } =
      evalForm values form + bits := by
  change (form.offset + bits) + atomSum values form.atoms =
    (form.offset + atomSum values form.atoms) + bits
  apply NobleFoundation.toZ_injective
  simp only [NobleFoundation.toZ_add]
  ring

/-- Adding a symbolic atom uses its shared node's value. -/
theorem add_atom_eval (values : Nat → BitVec 64) (form : Form) (node : Nat) :
    evalForm values (addForm form ⟨0, [⟨node, false⟩]⟩) =
      evalForm values form + values node := by
  rw [addForm_eval]
  simp [evalForm, atomSum, contribution]

/-- Subtracting a symbolic atom subtracts its value, not the left operand. -/
theorem sub_atom_eval (values : Nat → BitVec 64) (form : Form) (node : Nat) :
    evalForm values (subForm form ⟨0, [⟨node, false⟩]⟩) =
      evalForm values form - values node := by
  rw [subForm_eval]
  simp [evalForm, atomSum, contribution]

/-- Normalizing every expression in the fragment preserves its wrapping value. -/
theorem normalize_sound (values : Nat → BitVec 64) (expr : Expr) :
    evalForm values (normalize expr) = evalExpr values expr := by
  induction expr with
  | constant bits => simp [normalize, evalForm, atomSum, evalExpr]
  | atom node => simp [normalize, evalForm, atomSum, contribution, evalExpr]
  | add first second ihFirst ihSecond =>
      simp only [normalize, evalExpr, addForm_eval, ihFirst, ihSecond]
  | sub first second ihFirst ihSecond =>
      simp only [normalize, evalExpr, subForm_eval, ihFirst, ihSecond]

/-- The same law holds with original ordered inputs reconstructed by rank/merge. -/
theorem normalize_sound_merge (static : Nat → Option (BitVec 64))
    (dynamic : Nat → BitVec 64) (expr : Expr) :
    evalForm (Specialize.merge static dynamic) (normalize expr) =
      evalExpr (Specialize.merge static dynamic) expr :=
  normalize_sound (Specialize.merge static dynamic) expr

/-- A dynamic input at any position, not just a suffix, uses its ranked slot. -/
theorem dynamic_atom_merge (static : Nat → Option (BitVec 64))
    (dynamic : Nat → BitVec 64) (slot : Nat) (hslot : static slot = none) :
    evalForm (Specialize.merge static dynamic) (normalize (.atom slot)) =
      dynamic (Specialize.rank static slot) := by
  rw [normalize_sound]
  simp [evalExpr, Specialize.merge, hslot]

/-- A statically supplied input keeps its original slot and exact bits. -/
theorem static_atom_merge (static : Nat → Option (BitVec 64))
    (dynamic : Nat → BitVec 64) (slot : Nat) (bits : BitVec 64)
    (hslot : static slot = some bits) :
    evalForm (Specialize.merge static dynamic) (normalize (.atom slot)) = bits := by
  rw [normalize_sound]
  simp [evalExpr, Specialize.merge, hslot]

/-- When the ordered graph node's operands are I64, add agrees with its
normalization against the very same node valuation. -/
theorem packed_add_step (graph : PackedSchedule.Graph)
    (inputs values : Nat → BitVec 64) (prior : Nat → Option PackedSchedule.Val)
    (index first second : Nat)
    (hnode : graph.node index = .add first second)
    (hfirst : prior first = some (.i64 (values first)))
    (hsecond : prior second = some (.i64 (values second))) :
    PackedSchedule.evalStep graph prior inputs index =
      some (.i64 (evalForm values (normalize (.add (.atom first) (.atom second))))) := by
  rw [normalize_sound]
  simp [PackedSchedule.evalStep, hnode, hfirst, hsecond, evalExpr]

/-- The packed graph's first-minus-second operand order agrees with the
signed-atom normalization; swapping the operands is not assumed sound. -/
theorem packed_sub_step (graph : PackedSchedule.Graph)
    (inputs values : Nat → BitVec 64) (prior : Nat → Option PackedSchedule.Val)
    (index first second : Nat)
    (hnode : graph.node index = .sub first second)
    (hfirst : prior first = some (.i64 (values first)))
    (hsecond : prior second = some (.i64 (values second))) :
    PackedSchedule.evalStep graph prior inputs index =
      some (.i64 (evalForm values (normalize (.sub (.atom first) (.atom second))))) := by
  rw [normalize_sound]
  simp [PackedSchedule.evalStep, hnode, hfirst, hsecond, evalExpr]

/-- The concrete mixed expression collapses two known offsets to five. -/
theorem fold_offset_2_3 (node : Nat) :
    normalize (.add (.add (.atom node) (.constant 2)) (.constant 3)) =
      ⟨5, [⟨node, false⟩]⟩ := by
  simp [normalize, addForm]

/-- The offset wraps modulo 2^64, without saturating or signed overflow refusal. -/
theorem wrapping_offset_boundary (node : Nat) :
    normalize (.add (.add (.atom node)
      (.constant (BitVec.ofNat 64 (2 ^ 64 - 1)))) (.constant 1)) =
      ⟨0, [⟨node, false⟩]⟩ := by
  simp [normalize, addForm]

/-- A wrapped offset does not change the symbolic input's value. -/
theorem wrapping_offset_eval (values : Nat → BitVec 64) (node : Nat) :
    evalForm values (normalize (.add (.add (.atom node)
      (.constant (BitVec.ofNat 64 (2 ^ 64 - 1)))) (.constant 1))) = values node := by
  rw [wrapping_offset_boundary]
  simp [evalForm, atomSum, contribution]

/-- Subtraction preserves left-first order and the negative RHS atom tag. -/
theorem subtraction_atom_order (first second : Nat) :
    (normalize (.sub (.atom first) (.atom second))).atoms =
      [⟨first, false⟩, ⟨second, true⟩] := by
  rfl

/-- Repeated references retain two occurrences of the same node identity. -/
theorem repeated_atom_identity (node : Nat) :
    (normalize (.add (.atom node) (.atom node))).atoms =
      [⟨node, false⟩, ⟨node, false⟩] := by
  rfl

/-- Repeated references consume one valuation of one node, with multiplicity two. -/
theorem repeated_atom_eval (values : Nat → BitVec 64) (node : Nat) :
    evalForm values (normalize (.add (.atom node) (.atom node))) =
      values node + values node := by
  rw [normalize_sound]
  rfl

/-- A static position between dynamic slots does not make them a prefix. -/
theorem non_prefix_atom_merge (dynamic : Nat → BitVec 64) :
    evalForm (Specialize.merge Specialize.nonPrefixStatic dynamic)
      (normalize (.sub (.atom 3) (.atom 2))) = dynamic 2 - 100 := by
  rw [normalize_sound]
  simp [evalExpr, Specialize.merge, Specialize.rank, Specialize.nonPrefixStatic]

/-- Exchanging the operands of subtraction fails on distinct concrete values. -/
theorem reversed_subtraction_control :
    evalForm (fun slot => if slot = 0 then 9 else 4)
        (normalize (.sub (.atom 0) (.atom 1))) ≠
      evalForm (fun slot => if slot = 0 then 9 else 4)
        (normalize (.sub (.atom 1) (.atom 0))) := by
  decide

end NobleFoundation.PartialStatic
