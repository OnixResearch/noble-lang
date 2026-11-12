import NobleM2.SizeChangeMatrix

/-!
# Towards matrix-product associativity (issue #101)

Associativity of `composeMatrix` is the last algebraic prerequisite for exponent arithmetic, and it
factors into three lemmas proved here:

- `Rel4.compose_assoc`: composing relations is associative;
- `compose_fold_join_left` and `compose_fold_join_right`: composition distributes over a fold of
  joins, in either position.

What `composeMatrix_assoc` also needs is the interchange of the two folds that a product of products
introduces - a double fold over `List.finRange n` in both orders. That interchange is
`fold_join_comm`, and the associativity itself is `composeMatrix_assoc`, both in
`SizeChangeMatrixJoinFold`.
-/

namespace NobleM2.SizeChange

namespace Rel4

/-- Composing relations is associative. -/
theorem compose_assoc (a b c : Rel4) : (a.compose b).compose c = a.compose (b.compose c) := by
  cases a <;> cases b <;> cases c <;> rfl

end Rel4

/-- Composition distributes over a fold of joins on the left. -/
theorem compose_fold_join_left {α : Type} (f : α → Rel4) (c : Rel4) :
    ∀ (l : List α),
      (l.foldr (fun a acc => (f a).join acc) .unknown).compose c
        = l.foldr (fun a acc => ((f a).compose c).join acc) .unknown
  | [] => by
      show Rel4.unknown.compose c = Rel4.unknown
      cases c <;> rfl
  | a :: rest => by
      rw [List.foldr_cons, List.foldr_cons, Rel4.compose_join_left,
        compose_fold_join_left f c rest]

/-- Composition distributes over a fold of joins on the right. -/
theorem compose_fold_join_right {α : Type} (c : Rel4) (f : α → Rel4) :
    ∀ (l : List α),
      c.compose (l.foldr (fun a acc => (f a).join acc) .unknown)
        = l.foldr (fun a acc => (c.compose (f a)).join acc) .unknown
  | [] => by
      show c.compose Rel4.unknown = Rel4.unknown
      cases c <;> rfl
  | a :: rest => by
      rw [List.foldr_cons, List.foldr_cons, Rel4.compose_join_right,
        compose_fold_join_right c f rest]

end NobleM2.SizeChange
