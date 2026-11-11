import NobleM2.SizeChangeClosureSound

/-!
# Matrix call graphs for a sound criterion (issue #101)

The vector type `Graph n = Fin n → Rel` cannot express the classical criterion: it records one
relation per measure and no relation between two measures, so "a strict edge on a cycle" has no
referent. `SizeChangeCriterion` proves the consequence - the per-member premise is unsound there.

This module starts the redesign additively: nothing above changes meaning, and the matrix lane lives
on its own. It fixes the type and the algebra; the criterion and its SCC argument are the next step.

## Why four relations

The classical criterion separates *strict* from *non-strict* edges: a cycle is only useful if some
edge on it is strict, and non-increasing edges are what let a path close without progress. With only
`{decrease, equal, unknown}` there is no way to say "does not increase", so a cycle of `unknown`
edges - the counterexample in `SizeChangeCriterion` - cannot be distinguished from a cycle of
non-increasing ones. Hence four:

| Relation | Meaning |
|---|---|
| `strict` | new < old |
| `nonIncrease` | new ≤ old |
| `same` | new = old |
| `unknown` | nothing known |

Composition follows the usual size-change table (transitive closure of the four relations);
`Rel4.join` picks the more informative of two parallel paths, so a matrix product over a finite
index set is defined without choice.
-/

namespace NobleM2.SizeChange

/-- The four size-change relations: strict, non-increasing, equal, unknown. -/
inductive Rel4 where
  /-- `new < old`. -/
  | strict
  /-- `new ≤ old`. -/
  | nonIncrease
  /-- `new = old`. -/
  | same
  /-- Nothing known. -/
  | unknown
deriving DecidableEq, Repr

namespace Rel4

/-- Sequential composition of size-change relations. -/
def compose : Rel4 → Rel4 → Rel4
  | .strict, _ => .strict
  | _, .strict => .strict
  | .nonIncrease, .same => .nonIncrease
  | .same, .nonIncrease => .nonIncrease
  | .nonIncrease, .nonIncrease => .nonIncrease
  | .same, .same => .same
  | _, _ => .unknown

/-- Joining two parallel paths keeps the more informative relation. -/
def join : Rel4 → Rel4 → Rel4
  | .strict, _ => .strict
  | _, .strict => .strict
  | .nonIncrease, _ => .nonIncrease
  | _, .nonIncrease => .nonIncrease
  | .same, _ => .same
  | _, .same => .same
  | _, _ => .unknown

/-- Every relation composes with itself to itself, as usual for transitive relations. -/
theorem compose_self (r : Rel4) : r.compose r = r := by
  cases r <;> rfl

/-- A strict relation absorbs any composition on either side. -/
theorem compose_strict_left (r : Rel4) : Rel4.strict.compose r = .strict := rfl

theorem compose_strict_right (r : Rel4) : r.compose .strict = .strict := by
  cases r <;> rfl

/-- Joining with `unknown` changes nothing: it is the bottom of the information order. -/
theorem join_unknown (r : Rel4) : r.join .unknown = r := by
  cases r <;> rfl

/-- Join is commutative in the informative cases. -/
theorem join_comm_strict (r : Rel4) : Rel4.strict.join r = r.join .strict := by
  cases r <;> rfl

end Rel4

/-- A matrix call graph: entry `(i, j)` relates the old value of measure `i` to the new value of
measure `j`. -/
abbrev MGraph (n : Nat) := Fin n → Fin n → Rel4

/-- The identity matrix: each measure relates to itself by equality, and nothing else is known. -/
def idMatrix {n : Nat} : MGraph n := fun i j => if i = j then .same else .unknown

/-- Matrix product in the size-change semiring. -/
def composeMatrix {n : Nat} (g h : MGraph n) : MGraph n :=
  fun i k => (List.finRange n).foldr (fun j acc => Rel4.join (g i j |>.compose (h j k)) acc) .unknown


end NobleM2.SizeChange
