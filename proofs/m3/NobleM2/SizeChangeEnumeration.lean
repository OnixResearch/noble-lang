import NobleM2.SizeChangeMonoid

/-!
# Enumerating call graphs (issue #101)

The bound argument needs a finite enumeration of `Graph n = Fin n → Rel`. This project imports no
`Fintype`, so the enumeration is built by hand: `Rel.all` lists the three relations, and `graphsAux`
extends a list of functions on `Fin n` to all functions on `Fin (n + 1)` by fixing the new argument
to each relation in turn.

`graphsAux_length` counts the result: three times the previous count, so `3 ^ n` functions.
Completeness - that every function appears in the list - is `graphsAux_complete` below, by the
three-way case split on the new argument through `mem_succ_of_mem`. The enumeration therefore
exhausts the search space.
-/

namespace NobleM2.SizeChange

/-- The three relations, in a fixed order. -/
def Rel.all : List Rel := [.decrease, .equal, .unknown]

/-- Every relation appears in the enumeration. -/
theorem Rel.mem_all (r : Rel) : r ∈ Rel.all := by
  cases r <;> simp [Rel.all]

/-- All functions `Fin n → Rel`, built by extending the `Fin n` list one argument at a time. -/
def graphsAux : (n : Nat) → List (Graph n)
  | 0 => [fun i => Fin.elim0 i]
  | n + 1 =>
      let previous := graphsAux n
      previous.map (fun f => Fin.cases .decrease f) ++
        (previous.map (fun f => Fin.cases .equal f) ++
          previous.map (fun f => Fin.cases .unknown f))

/-- The empty domain has exactly one function. -/
theorem graphsAux_zero : (graphsAux 0).length = 1 := rfl

/-- Each domain size triples the count: `3 ^ n` functions on `Fin n`. -/
theorem graphsAux_length (n : Nat) : (graphsAux n).length = 3 ^ n := by
  induction n with
  | zero => rfl
  | succ n ih =>
      simp [graphsAux, List.length_append, List.length_map, ih]
      omega


/-- Extending an enumerated function by one argument stays in the next enumeration. -/
theorem mem_succ_of_mem {n : Nat} {r : Rel} {f : Fin n → Rel} (hf : f ∈ graphsAux n) :
    Fin.cases r f ∈ graphsAux (n + 1) := by
  have hlist : graphsAux (n + 1) =
      (graphsAux n).map (fun f => Fin.cases .decrease f) ++
        ((graphsAux n).map (fun f => Fin.cases .equal f) ++
          (graphsAux n).map (fun f => Fin.cases .unknown f)) := rfl
  rw [hlist]
  cases r with
  | decrease =>
      rw [List.mem_append]; left
      exact List.mem_map.mpr ⟨f, hf, rfl⟩
  | equal =>
      rw [List.mem_append]; right; rw [List.mem_append]; left
      exact List.mem_map.mpr ⟨f, hf, rfl⟩
  | unknown =>
      rw [List.mem_append]; right; rw [List.mem_append]; right
      exact List.mem_map.mpr ⟨f, hf, rfl⟩

/-- **Completeness**: every function `Fin n → Rel` appears in the enumeration. -/
theorem graphsAux_complete (n : Nat) (g : Graph n) : g ∈ graphsAux n := by
  induction n with
  | zero =>
      have h : g = fun i => Fin.elim0 i := by funext i; exact Fin.elim0 i
      rw [h]; simp [graphsAux]
  | succ n ih =>
      have decomp : g = Fin.cases (g 0) (fun i => g i.succ) := by
        funext i; cases i using Fin.cases <;> simp
      rw [decomp]
      exact mem_succ_of_mem (r := g 0) (f := fun i => g i.succ) (ih _)

end NobleM2.SizeChange
