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
  | .unknown, _ => .unknown
  | _, .unknown => .unknown
  | .strict, _ => .strict
  | _, .strict => .strict
  | .nonIncrease, .same => .nonIncrease
  | .same, .nonIncrease => .nonIncrease
  | .nonIncrease, .nonIncrease => .nonIncrease
  | .same, .same => .same

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

/-- A strict relation absorbs composition on either side, provided the other factor is known. -/
theorem compose_strict_left {r : Rel4} (h : r ≠ .unknown) : Rel4.strict.compose r = .strict := by
  cases r <;> simp_all [Rel4.compose]

theorem compose_strict_right {r : Rel4} (h : r ≠ .unknown) : r.compose .strict = .strict := by
  cases r <;> simp_all [Rel4.compose]

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


/-- A fold over joins of all-`unknown` entries is `unknown`. -/
theorem foldr_join_unknown_of_all_unknown {α : Type} (f : α → Rel4) :
    ∀ (l : List α), (∀ x ∈ l, f x = .unknown) →
      l.foldr (fun x acc => (f x).join acc) .unknown = .unknown
  | [], _ => rfl
  | x :: rest, h => by
      rw [List.foldr_cons]
      have hx : f x = .unknown := h x (by simp)
      have hr : rest.foldr (fun y a => (f y).join a) .unknown = .unknown :=
        foldr_join_unknown_of_all_unknown f rest (fun y hy => h y (by simp [hy]))
      rw [hx, hr]
      rfl

/-- `unknown` is neutral for `join` on the left. -/
theorem join_unknown_left (r : Rel4) : Rel4.unknown.join r = r := by
  cases r <;> rfl

/-- A fold over joins with a single non-`unknown` entry equals that entry. -/
theorem foldr_join_of_single {α : Type} [DecidableEq α] (f : α → Rel4) (x : α) :
    ∀ (l : List α), x ∈ l → (∀ y ∈ l, y ≠ x → f y = .unknown) →
      l.foldr (fun y acc => (f y).join acc) .unknown = f x
  | [], hx, _ => absurd hx (by simp)
  | y :: rest, hx, h => by
      rw [List.foldr_cons]
      by_cases hyx : y = x
      · cases hyx
        by_cases hxr : x ∈ rest
        · have hr := foldr_join_of_single f x rest hxr (fun z hz hzx => h z (by simp [hz]) hzx)
          rw [hr]
          cases (f x) <;> rfl
        · have hr : rest.foldr (fun z a => (f z).join a) .unknown = .unknown :=
            foldr_join_unknown_of_all_unknown f rest
              (fun z hz => h z (by simp [hz]) (fun hzx => hxr (hzx ▸ hz)))
          rw [hr]
          exact Rel4.join_unknown (f x)
      · have hxr : x ∈ rest := by
          rcases List.mem_cons.mp hx with hyx' | hxr
          · exact absurd hyx'.symm hyx
          · exact hxr
        have hr := foldr_join_of_single f x rest hxr (fun z hz hzx => h z (by simp [hz]) hzx)
        rw [hr, h y (by simp) hyx]
        exact join_unknown_left (f x)

/-- The identity matrix is a right unit for the product. -/
theorem composeMatrix_id_right {n : Nat} (g : MGraph n) : composeMatrix g idMatrix = g := by
  funext i k
  have hunit : (g i k).compose (idMatrix k k) = g i k := by
    have hid : idMatrix k k = (Rel4.same : Rel4) := by simp [idMatrix]
    rw [hid]
    cases (g i k) <;> rfl
  have hsingle := foldr_join_of_single (fun j => (g i j).compose (idMatrix j k)) k
    (List.finRange n) (List.mem_finRange k)
    (fun y _ hyk => by
      have hid : idMatrix y k = (Rel4.unknown : Rel4) := by simp [idMatrix, hyk]
      rw [hid]
      cases (g i y) <;> rfl)
  simpa [composeMatrix, hunit] using hsingle

/-- Composing with `same` on the left changes nothing. -/
theorem same_compose_left (r : Rel4) : Rel4.same.compose r = r := by
  cases r <;> rfl

/-- The identity matrix is a left unit for the product. -/
theorem composeMatrix_id_left {n : Nat} (g : MGraph n) : composeMatrix idMatrix g = g := by
  funext i k
  have hunit : (idMatrix i i).compose (g i k) = g i k := by
    have hid : idMatrix i i = (Rel4.same : Rel4) := by simp [idMatrix]
    rw [hid]
    exact same_compose_left (g i k)
  have hsingle := foldr_join_of_single (fun j => (idMatrix i j).compose (g j k)) i
    (List.finRange n) (List.mem_finRange i)
    (fun y _ hyi => by
      have hid : idMatrix i y = (Rel4.unknown : Rel4) := by
        have hiy : ¬ i = y := fun h => hyi h.symm
        simp [idMatrix, hiy]
      rw [hid]
      rfl)
  simpa [composeMatrix, hunit] using hsingle

end NobleM2.SizeChange
