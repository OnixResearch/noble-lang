import NobleM2.SizeChangeBounded

/-!
# Counting bricks for the pigeonhole (issue #101)

The bound argument needs to know that a nodup list cannot be longer than the list that contains all
of its elements. Core supplies `List.Nodup` and `List.Sublist.length_le` but not `Nodup.length_le`
and not a map-injectivity lemma, so this module builds the latter and states the former's gap
explicitly.

`nodup_map_of_injective` is the direction a checker uses: if a list has no duplicates and a function
is injective on that list, the mapped list has no duplicates either. That is what turns a run of
distinct positions into `Fin k → Graph n` with a nodup image, which is the shape the pigeonhole
consumes.

The counting step itself - `Nodup` plus containment implies a length bound - is not proved here.
-/

namespace NobleM2.SizeChange

/-- A nodup list mapped through a function injective on it stays nodup. -/
theorem nodup_map_of_injective {α β : Type} {l : List α} {f : α → β}
    (hl : l.Nodup) (hinj : ∀ a ∈ l, ∀ b ∈ l, f a = f b → a = b) : (l.map f).Nodup := by
  induction l with
  | nil => simp
  | cons a t ih =>
      rw [List.map_cons, List.nodup_cons]
      have hat : a ∉ t := (List.nodup_cons.mp hl).1
      have ht : t.Nodup := (List.nodup_cons.mp hl).2
      refine ⟨?_, ih ht ?_⟩
      · intro hmem
        obtain ⟨b, hb, hfb⟩ := List.mem_map.mp hmem
        have : b = a := (hinj a (by simp) b (by simp [hb]) hfb.symm).symm
        exact hat (this ▸ hb)
      · intro x hx y hy hxy
        exact hinj x (by simp [hx]) y (by simp [hy]) hxy

end NobleM2.SizeChange
