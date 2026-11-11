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

/-- Split a list at an occurrence of `a`: `l = pre ++ a :: suf`. -/
theorem decompose_of_mem {α : Type} : ∀ (l : List α) {a : α}, a ∈ l → ∃ pre suf, l = pre ++ a :: suf
  | [], _, h => by simp at h
  | b :: rest, a, h => by
      rcases List.mem_cons.mp h with rfl | hmem
      · exact ⟨[], rest, rfl⟩
      · obtain ⟨pre, suf, hsplit⟩ := decompose_of_mem rest hmem
        exact ⟨b :: pre, suf, by rw [hsplit]; rfl⟩

/-- A member different from `a` survives deleting one occurrence of `a`. -/
theorem mem_of_ne_of_mem_append_cons {α : Type} {a b : α} :
    ∀ (pre suf : List α), b ∈ pre ++ a :: suf → b ≠ a → b ∈ pre ++ suf
  | [], suf, h, hne => by
      rcases List.mem_cons.mp h with hb | hs
      · exact absurd hb hne
      · exact hs
  | c :: pre, suf, h, hne => by
      rcases List.mem_cons.mp h with hc | hmem
      · subst hc; simp
      · have := mem_of_ne_of_mem_append_cons pre suf hmem hne
        simp [this]

/-- **Counting, constructively**: a nodup list is no longer than any list containing its elements.
The containing list needs no `Nodup` hypothesis - duplicates only make it longer. -/
theorem length_le_of_subset {α : Type} (m : List α) :
    ∀ (l : List α), m.Nodup → (∀ a ∈ m, a ∈ l) → m.length ≤ l.length := by
  induction m with
  | nil => intro l _ _; exact Nat.zero_le _
  | cons a t ih =>
      intro l hm hall
      have hat : a ∉ t := (List.nodup_cons.mp hm).1
      have ht : t.Nodup := (List.nodup_cons.mp hm).2
      obtain ⟨pre, suf, hsplit⟩ := decompose_of_mem l (hall a (by simp))
      have sub : ∀ b ∈ t, b ∈ pre ++ suf := by
        intro b hb
        have hbl : b ∈ l := hall b (by simp [hb])
        have hne : b ≠ a := fun h => hat (h ▸ hb)
        rw [hsplit] at hbl
        exact mem_of_ne_of_mem_append_cons pre suf hbl hne
      have ih' := ih (pre ++ suf) ht sub
      have hlen : l.length = (pre ++ suf).length + 1 := by
        rw [hsplit]
        simp [List.length_append]
        omega
      rw [List.length_cons]
      omega

/-- **Counting bound**: an injective sequence drawn from a list containing every value has at most
`l.length` positions. This is the pigeonhole content the bound argument consumes. -/
theorem injective_bound {α : Type} (l : List α) {m : Nat} (f : Fin m → α)
    (hall : ∀ i, f i ∈ l) (hinj : ∀ i j : Fin m, f i = f j → i = j) : m ≤ l.length := by
  have hmap : ((List.finRange m).map f).Nodup :=
    nodup_map_of_injective (List.nodup_finRange m) (fun a _ b _ hab => hinj a b hab)
  have hsub : ∀ a ∈ (List.finRange m).map f, a ∈ l := by
    intro a ha
    obtain ⟨i, _, rfl⟩ := List.mem_map.mp ha
    exact hall i
  have hlent := length_le_of_subset _ l hmap hsub
  rw [List.length_map, List.length_finRange] at hlent
  exact hlent

end NobleM2.SizeChange
