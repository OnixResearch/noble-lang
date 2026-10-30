import Mathlib.Data.List.Basic

/-!
# Bounded ISOP Boolean core (issue #35)

This is a standalone, kernel-checked Boolean model.  A valuation is a small
truth-table row, represented by the indexed `Valuation` type; a Boolean
function is a function from those rows to `Bool`.  The recurrence is the exact
(`L = U = F`) Shannon/ISOP split: each cofactor is recursively covered and
the splitting literal is attached to every resulting cube.  It is deliberately
bounded and does not claim a source integration, a checked Boolean DAG, or a
lowering to `Program`.

The independent checker enumerates the model's valuation rows directly.  It
does not call the recurrence, so the mutation fixtures can reject bad
cofactors and complemented literals without relying on the implementation
under test.
-/

namespace NobleFoundation.Isop

/-! ## Truth-table rows and formula syntax -/

/-- A truth-table row for exactly `n` Boolean variables, with variable zero at
the head. -/
inductive Valuation : Nat → Type
  | nil : Valuation 0
  | cons {n : Nat} (head : Bool) (tail : Valuation n) : Valuation (n + 1)
  deriving DecidableEq, Repr

/-- A Boolean function over exactly `n` variables. -/
def BFun (n : Nat) := Valuation n → Bool

/-- Lookup a variable in a truth-table row. -/
def Valuation.get : {n : Nat} → Valuation n → Fin n → Bool
  | _, .nil, i => Fin.elim0 i
  | _, .cons head tail, i => Fin.cases head (fun j => Valuation.get tail j) i

/-- The formula AST used by the fixtures. -/
inductive Formula (n : Nat) where
  | const (value : Bool)
  | var (index : Fin n)
  | and (left right : Formula n)
  | or (left right : Formula n)
  | not (body : Formula n)
  deriving DecidableEq, Repr

/-- Evaluate a formula on one truth-table row. -/
def Formula.eval : Formula n → Valuation n → Bool
  | .const value, _ => value
  | .var index, valuation => valuation.get index
  | .and left right, valuation => left.eval valuation && right.eval valuation
  | .or left right, valuation => left.eval valuation || right.eval valuation
  | .not body, valuation => !(body.eval valuation)

/-- Turn formula syntax into the corresponding Boolean function. -/
def Formula.function (formula : Formula n) : BFun n := formula.eval

/-! ## Cubes, covers, and the exact recurrence -/

/-- A cube is an assignment with optional literals, again with variable zero at
the head. -/
inductive Cube : Nat → Type
  | nil : Cube 0
  | cons {n : Nat} (head : Option Bool) (tail : Cube n) : Cube (n + 1)
  deriving DecidableEq, BEq, Repr

abbrev Cover (n : Nat) := List (Cube n)

/-- Whether a cube matches a valuation. -/
def Cube.matches : Cube n → Valuation n → Bool
  | .nil, .nil => true
  | .cons requirement tail, .cons value row =>
      (match requirement with
      | none => true
      | some expected => decide (value = expected)) && tail.matches row

/-- Evaluate a cover as the disjunction of its cubes. -/
def coverEval : Cover n → Valuation n → Bool
  | [], _ => false
  | cube :: cover, valuation => cube.matches valuation || coverEval cover valuation

/-- Every row of a truth table, generated without a recurrence or a checker. -/
def valuations : (n : Nat) → List (Valuation n)
  | 0 => [.nil]
  | n + 1 => (valuations n).flatMap (fun row => [.cons false row, .cons true row])

theorem valuation_mem (n : Nat) (row : Valuation n) : row ∈ valuations n := by
  induction row with
  | nil => simp [valuations]
  | cons head tail ih =>
      cases head <;> simp [valuations, ih]

/-- An exhaustive Boolean equivalence checker over all model rows. -/
def equivOn (n : Nat) (f g : BFun n) : Bool :=
  (valuations n).all (fun row => decide (f row = g row))

/-- The exhaustive checker is sound for every valuation, independently of the
ISOP recurrence. -/
theorem equivOn_sound (n : Nat) (f g : BFun n)
    (hchecked : equivOn n f g = true) : ∀ row, f row = g row := by
  intro row
  have hmem : row ∈ valuations n := valuation_mem n row
  have hrow := (List.all_eq_true.mp hchecked) row hmem
  exact of_decide_eq_true hrow

private theorem coverEval_append (left right : Cover n) (row : Valuation n) :
    coverEval (left ++ right) row = (coverEval left row || coverEval right row) := by
  induction left with
  | nil => simp [coverEval]
  | cons cube rest ih =>
      simp only [List.cons_append, coverEval]
      rw [ih]
      cases cube.matches row <;> cases coverEval rest row <;>
        cases coverEval right row <;> rfl

private theorem map_false_eval_false (cover : Cover n) (row : Valuation n) :
    coverEval (cover.map (Cube.cons (some false))) (.cons false row) =
      coverEval cover row := by
  induction cover with
  | nil => rfl
  | cons cube rest ih => simp [coverEval, Cube.matches, ih]

private theorem map_false_eval_true (cover : Cover n) (row : Valuation n) :
    coverEval (cover.map (Cube.cons (some false))) (.cons true row) = false := by
  induction cover with
  | nil => rfl
  | cons cube rest ih => simp [coverEval, Cube.matches, ih]

private theorem map_none_eval_false (cover : Cover n) (row : Valuation n) :
    coverEval (cover.map (Cube.cons none)) (.cons false row) =
      coverEval cover row := by
  induction cover with
  | nil => rfl
  | cons cube rest ih => simp [coverEval, Cube.matches, ih]

private theorem map_none_eval_true (cover : Cover n) (row : Valuation n) :
    coverEval (cover.map (Cube.cons none)) (.cons true row) =
      coverEval cover row := by
  induction cover with
  | nil => rfl
  | cons cube rest ih => simp [coverEval, Cube.matches, ih]

private theorem map_true_eval_false (cover : Cover n) (row : Valuation n) :
    coverEval (cover.map (Cube.cons (some true))) (.cons false row) = false := by
  induction cover with
  | nil => rfl
  | cons cube rest ih => simp [coverEval, Cube.matches, ih]

private theorem map_true_eval_true (cover : Cover n) (row : Valuation n) :
    coverEval (cover.map (Cube.cons (some true))) (.cons true row) =
      coverEval cover row := by
  induction cover with
  | nil => rfl
  | cons cube rest ih => simp [coverEval, Cube.matches, ih]

/-- The cofactor at the head variable. -/
def cofactor {n : Nat} (f : BFun (n + 1)) (value : Bool) : BFun n :=
  fun row => f (.cons value row)

/-- Attach a splitting literal to every cube in a cofactor cover. -/
def attach (value : Bool) (cover : Cover n) : Cover (n + 1) :=
  cover.map (Cube.cons (some value))

/-- Attach no literal to a cofactor cube, representing the common part of the
two cofactors. -/
def attachNone (cover : Cover n) : Cover (n + 1) :=
  cover.map (Cube.cons none)

/-- Exact-mode Minato--Morreale/ISOP split.  The two recursively covered
cofactors contribute a common cover without a literal, then the low-only and
high-only cofactors are attached to the complementary literals.  The exact
mode has `L = U = F`; no care-set or approximation is introduced here. -/
def isop : (n : Nat) → BFun n → Cover n
  | 0, f => if f .nil = true then [.nil] else []
  | n + 1, f =>
      attachNone (isop n (fun row => cofactor f false row && cofactor f true row)) ++
        attach false (isop n (fun row =>
          cofactor f false row && !(cofactor f true row))) ++
        attach true (isop n (fun row =>
          cofactor f true row && !(cofactor f false row)))

theorem isop_exact (n : Nat) (f : BFun n) :
    ∀ row, coverEval (isop n f) row = f row := by
  induction n with
  | zero =>
      intro row
      cases row
      cases h : f .nil <;> simp [isop, h, coverEval, Cube.matches]
  | succ n ih =>
      intro row
      cases row with
      | cons value tail =>
          cases value with
          | false =>
              cases hLow : f (.cons false tail) <;>
                cases hHigh : f (.cons true tail) <;>
                simp [isop, attach, attachNone, cofactor, coverEval_append,
                  map_none_eval_false, map_false_eval_false,
                  map_true_eval_false, ih, hLow, hHigh]
          | true =>
              cases hLow : f (.cons false tail) <;>
                cases hHigh : f (.cons true tail) <;>
                simp [isop, attach, attachNone, cofactor, coverEval_append,
                  map_none_eval_true, map_false_eval_true,
                  map_true_eval_true, ih, hLow, hHigh]

/-! ## Support intervals -/

/-- Pointwise Boolean implication, written as the order on characteristic
functions. -/
def pointwiseLe (lower upper : BFun n) : Prop :=
  ∀ row, lower row = true → upper row = true

/-- In exact mode, every support interval `L ≤ F ≤ U` contains the generated
cover pointwise: `L ≤ cover ≤ U`. -/
theorem isop_bounds (n : Nat) (lower f upper : BFun n)
    (hLower : pointwiseLe lower f) (hUpper : pointwiseLe f upper) :
    pointwiseLe lower (coverEval (isop n f)) ∧
      pointwiseLe (coverEval (isop n f)) upper := by
  have hexact : ∀ row, coverEval (isop n f) row = f row := isop_exact n f
  constructor
  · intro row hrow
    rw [hexact row]
    exact hLower row hrow
  · intro row hrow
    rw [hexact row] at hrow
    exact hUpper row hrow

/-! ## Ordered views and mutation controls -/

private def appendVal {n : Nat} : Valuation n → Bool → Valuation (n + 1)
  | .nil, value => .cons value .nil
  | .cons head tail, value => .cons head (appendVal tail value)

/-- Reverse a truth-table row; this is the second bounded variable order used
by the fixtures. -/
def reverseValuation : Valuation n → Valuation n
  | .nil => .nil
  | .cons head tail => appendVal (reverseValuation tail) head

private def appendCube {n : Nat} : Cube n → Option Bool → Cube (n + 1)
  | .nil, value => .cons value .nil
  | .cons head tail, value => .cons head (appendCube tail value)

/-- Reverse the literal order of a cube. -/
def reverseCube : Cube n → Cube n
  | .nil => .nil
  | .cons head tail => appendCube (reverseCube tail) head

def reverseCover (cover : Cover n) : Cover n := cover.map reverseCube

/-- A fixed-order view: `false` uses head-first splitting, `true` uses the
reverse order by transporting rows and cubes. -/
def orderedCover (reverse : Bool) (f : BFun n) : Cover n :=
  if reverse then
    reverseCover (isop n (fun row => f (reverseValuation row)))
  else isop n f

/-- A seeded recurrence with the low cofactor used for both branches. -/
def wrongCofactor : (n : Nat) → BFun n → Cover n
  | 0, f => if f .nil = true then [.nil] else []
  | n + 1, f =>
      attach false (wrongCofactor n (cofactor f false)) ++
        attach true (wrongCofactor n (cofactor f false))

/-- A seeded recurrence which puts each cofactor under the complementary
literal. -/
def wrongComplement : (n : Nat) → BFun n → Cover n
  | 0, f => if f .nil = true then [.nil] else []
  | n + 1, f =>
      attach true (wrongComplement n (cofactor f false)) ++
        attach false (wrongComplement n (cofactor f true))

def checker (n : Nat) (f : BFun n) (cover : Cover n) : Bool :=
  equivOn n f (coverEval cover)

/-! ## Formula fixtures -/

def x0 : Formula 1 := .var ⟨0, by decide⟩
def x1 : Formula 2 := .var ⟨1, by decide⟩
def x2 : Formula 3 := .var ⟨2, by decide⟩

def xorFormula (left right : Formula n) : Formula n :=
  .and (.or left right) (.not (.and left right))

def contradiction : Formula 1 := .and x0 (.not x0)
def repeatedAtom : Formula 1 := .and x0 x0
def parity2 : Formula 2 := xorFormula (.var ⟨0, by decide⟩) x1
def parity3 : Formula 3 :=
  xorFormula
    (xorFormula (.var ⟨0, by decide⟩) (.var ⟨1, by decide⟩))
    (.var ⟨2, by decide⟩)

def trueConstant : Formula 0 := .const true
def falseConstant : Formula 0 := .const false

theorem constant_true_checked :
    checker 0 trueConstant.function (isop 0 trueConstant.function) = true := by
  decide

theorem constant_false_checked :
    checker 0 falseConstant.function (isop 0 falseConstant.function) = true := by
  decide

theorem contradiction_checked :
    checker 1 contradiction.function (isop 1 contradiction.function) = true := by
  decide

theorem repeated_atom_checked :
    checker 1 repeatedAtom.function (isop 1 repeatedAtom.function) = true := by
  decide

theorem parity3_checked :
    checker 3 parity3.function (isop 3 parity3.function) = true := by
  decide

/-- Irredundancy is intentionally recorded as a fixture computation, not as a
universal theorem about all ISOP inputs. -/
def irredundant (cover : Cover n) : Bool :=
  cover.all (fun cube =>
    (valuations n).any (fun row =>
      cube.matches row && !(coverEval (cover.erase cube) row)))

theorem contradiction_irredundant_fixture :
    irredundant (isop 1 contradiction.function) = true := by
  decide

theorem repeated_atom_irredundant_fixture :
    irredundant (isop 1 repeatedAtom.function) = true := by
  decide

theorem parity3_irredundant_fixture :
    irredundant (isop 3 parity3.function) = true := by
  decide

theorem parity3_cover_growth : (isop 3 parity3.function).length = 4 := by
  decide

theorem parity3_orders_differ :
    orderedCover false parity3.function ≠ orderedCover true parity3.function := by
  decide

theorem parity3_orders_equivalent :
    checker 3 parity3.function (orderedCover true parity3.function) = true := by
  decide

theorem wrong_cofactor_rejected :
    checker 1 repeatedAtom.function (wrongCofactor 1 repeatedAtom.function) = false := by
  decide

theorem wrong_complement_rejected :
    checker 1 repeatedAtom.function (wrongComplement 1 repeatedAtom.function) = false := by
  decide

end NobleFoundation.Isop
