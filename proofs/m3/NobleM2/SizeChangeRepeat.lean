import NobleM2.SizeChangeCounting

/-!
# Powers must repeat (issue #101)

`injective_bound` needs no `Nodup` hypothesis on the containing list, and `graphsAux n` contains
every graph on `Fin n` (completeness, proved earlier). So the sequence of powers of any graph cannot
be injective over `(graphsAux n).length + 1` positions - one more than the number of graphs there
are.

That is the pigeonhole step, stated in the negative form that needs no classical choice: the
injectivity hypothesis leads to `3 ^ n + 1 <= 3 ^ n`.

What this does not give is a *concrete* pair of positions. Extracting one needs either classical
reasoning or the decidable scan a checker runs anyway (`decreasesWithin` already searches powers
decidably). The bound on how many composites a checker must visit is still open, and so is the
general soundness theorem.
-/

namespace NobleM2.SizeChange

/-- The number of positions the bound considers: one more than the number of graphs. -/
theorem graphsAux_length_succ (n : Nat) : (graphsAux n).length + 1 = 3 ^ n + 1 := by
  rw [graphsAux_length]

/-- **Powers cannot be injective** over `(graphsAux n).length + 1` positions. -/
theorem powers_not_injective {n : Nat} (g : Graph n) :
    ¬ ∀ i j : Fin ((graphsAux n).length + 1),
        power (i.val + 1) g = power (j.val + 1) g → i = j := by
  intro hinj
  have hall : ∀ i : Fin ((graphsAux n).length + 1), power (i.val + 1) g ∈ graphsAux n :=
    fun i => graphsAux_complete n _
  have hbound := injective_bound (graphsAux n)
    (fun i : Fin ((graphsAux n).length + 1) => power (i.val + 1) g) hall hinj
  have hlen := graphsAux_length n
  omega

/-- The same statement with the count made explicit: `3 ^ n` graphs, `3 ^ n + 1` positions. -/
theorem powers_not_injective_card {n : Nat} (g : Graph n) :
    ¬ ∀ i j : Fin (3 ^ n + 1), power (i.val + 1) g = power (j.val + 1) g → i = j := by
  rw [← graphsAux_length] at *
  exact powers_not_injective g

/-! ## Fixtures -/

/-- On one measure there are three graphs, so four positions cannot all differ. -/
theorem three_graphs : (graphsAux 1).length = 3 := by
  rw [graphsAux_length]

theorem decreasing_repeats :
    ¬ ∀ i j : Fin ((graphsAux 1).length + 1),
        power (i.val + 1) (fun _ => .decrease : Graph 1) = power (j.val + 1) (fun _ => .decrease)
          → i = j :=
  powers_not_injective _

end NobleM2.SizeChange
