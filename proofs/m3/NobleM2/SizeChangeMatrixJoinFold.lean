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
- `join_fold_zip`: joining two folds elementwise is a fold of the elementwise joins;
- `fold_join_comm`: the double-fold interchange, and `composeMatrix_assoc` its consumer.
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

/-- A fold of `unknown` joins is `unknown`. -/
theorem fold_unknown {β : Type} :
    ∀ (l : List β), l.foldr (fun _ acc => Rel4.unknown.join acc) Rel4.unknown = Rel4.unknown
  | [] => rfl
  | _ :: rest => by
      rw [List.foldr_cons, fold_unknown rest]
      rfl

/-- **The double fold commutes**: joining over two indices in either order gives the same value. -/
theorem fold_join_comm {α β : Type} (F : α → β → Rel4) :
    ∀ (l₁ : List α) (l₂ : List β),
      (l₁.foldr (fun a acc => (l₂.foldr (fun b acc' => (F a b).join acc') .unknown).join acc) .unknown)
        = l₂.foldr
            (fun b acc => (l₁.foldr (fun a acc' => (F a b).join acc') .unknown).join acc) .unknown
  | [], l₂ => by
      show Rel4.unknown = l₂.foldr (fun b acc => Rel4.unknown.join acc) Rel4.unknown
      rw [fold_unknown l₂]
  | a :: rest, l₂ => by
      show (l₂.foldr (fun b acc => (F a b).join acc) .unknown).join
          (rest.foldr (fun r acc => (l₂.foldr (fun b acc' => (F r b).join acc') .unknown).join acc)
            .unknown)
        = l₂.foldr
            (fun b acc => ((F a b).join
              (rest.foldr (fun r acc' => (F r b).join acc') .unknown)).join acc) .unknown
      rw [fold_join_comm F rest l₂]
      exact join_fold_zip (fun b => F a b)
        (fun b => rest.foldr (fun r acc => (F r b).join acc) .unknown) l₂

/-- **Matrix products associate**: `(g ∘ h) ∘ k = g ∘ (h ∘ k)`. -/
theorem composeMatrix_assoc {n : Nat} (g h k : MGraph n) :
    composeMatrix (composeMatrix g h) k = composeMatrix g (composeMatrix h k) := by
  funext i l
  simp only [composeMatrix]
  simp only [compose_fold_join_left, compose_fold_join_right, Rel4.compose_assoc]
  exact fold_join_comm (fun r m => (g i m).compose ((h m r).compose (k r l)))
    (List.finRange n) (List.finRange n)

end NobleM2.SizeChange
