import NobleM2.SizeChangeMatrixAssoc

/-!
# Joining against and between folds (issue #101)

The remaining factor of `composeMatrix_assoc` is the interchange of the two folds a product of
products introduces. These are the laws that interchange needs.

Note the asymmetry with `compose`: `unknown` is **neutral** for `join`, so an accumulator has to be
the fold's *initial value* rather than something joined on afterwards. My first statement of these
lemmas got that wrong and was refuted by the nil case; the shapes below are the corrected ones.

- `join_fold_left` / `join_fold_right`: joining a value against a fold is a fold with that value as
  the initial accumulator;
- `join_fold_zip`: joining two folds elementwise is a fold of the elementwise joins.

The double-fold interchange itself, `fold_join_comm`, is the next lemma and is not proved here.
-/

namespace NobleM2.SizeChange

/-- Joining a value against a fold is a fold with that value as the initial accumulator. -/
theorem join_fold_left {β : Type} (f : β → Rel4) :
    ∀ (l : List β) (c : Rel4),
      c.join (l.foldr (fun b acc => (f b).join acc) .unknown)
        = l.foldr (fun b acc => (f b).join acc) c
  | [], c => by
      show c.join Rel4.unknown = c
      exact Rel4.join_unknown_right c
  | a :: rest, c => by
      show c.join ((f a).join (rest.foldr (fun b acc => (f b).join acc) .unknown))
        = (f a).join (rest.foldr (fun b acc => (f b).join acc) c)
      rw [← Rel4.join_assoc, Rel4.join_comm c (f a), Rel4.join_assoc,
        join_fold_left f rest c]

/-- The same, joining on the right. -/
theorem join_fold_right {β : Type} (f : β → Rel4) :
    ∀ (l : List β) (c : Rel4),
      (l.foldr (fun b acc => (f b).join acc) .unknown).join c
        = l.foldr (fun b acc => (f b).join acc) c
  | [], c => by
      show Rel4.unknown.join c = c
      exact join_unknown_left c
  | a :: rest, c => by
      show ((f a).join (rest.foldr (fun b acc => (f b).join acc) .unknown)).join c
        = (f a).join (rest.foldr (fun b acc => (f b).join acc) c)
      rw [Rel4.join_assoc, join_fold_right f rest c]

/-- Swapping the middle two factors of a join of joins. -/
theorem join_swap (a b c d : Rel4) : (a.join b).join (c.join d) = (a.join c).join (b.join d) := by
  cases a <;> cases b <;> cases c <;> cases d <;> rfl

/-- Joining two folds elementwise is a fold of the elementwise joins. -/
theorem join_fold_zip {β : Type} (f g : β → Rel4) :
    ∀ (l : List β),
      (l.foldr (fun b acc => (f b).join acc) .unknown).join
          (l.foldr (fun b acc => (g b).join acc) .unknown)
        = l.foldr (fun b acc => ((f b).join (g b)).join acc) .unknown
  | [] => by
      show Rel4.unknown.join Rel4.unknown = Rel4.unknown
      rfl
  | a :: rest => by
      show ((f a).join (rest.foldr (fun b acc => (f b).join acc) .unknown)).join
          ((g a).join (rest.foldr (fun b acc => (g b).join acc) .unknown))
        = ((f a).join (g a)).join (rest.foldr (fun b acc => ((f b).join (g b)).join acc) .unknown)
      rw [join_swap]
      rw [join_fold_zip f g rest]

end NobleM2.SizeChange
