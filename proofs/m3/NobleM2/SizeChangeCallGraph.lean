import NobleM2.SizeChangeMatrixPreserve

/-!
# Call-graph reachability and components (issue #101)

The periodic cycle theorem needs to know *which* sequences occur on a cycle. That is a property of the
program's call graph, not of the matrices: an edge lies on a cycle exactly when the target reaches
back to the source.

- `Reaches call i j`: a path of call edges from `i` to `j`, as an inductive relation;
- `SameSCC call i j`: mutual reachability, proved to be an equivalence relation;
- `OnCycleEdge call i j`: a call edge whose target reaches back, proved to place `i` and `j` in the
  same component.

With these, "the strict edge lies on a cycle" is a statement about a component rather than about one
matrix. What is **not** true is that the sequences occurring inside a component are the periodic ones
`periodic_cycle_terminates` covers: a component's runs need not be periodic at all. The lasso
`1, 0, 0, 0, …` is a walk of the complete call graph, so it stays inside one component, yet it has
no period (`not_every_sequence_periodic`). Connecting reachability to the cycle theorem therefore
needs a different argument - a bound on how many in-component steps can elapse before a node
repeats, plus a treatment of the aperiodic prefix - and is not attempted here.
-/

namespace NobleM2.SizeChange

/-- A path of call edges. -/
inductive Reaches {n : Nat} (call : Fin n → Fin n → Bool) : Fin n → Fin n → Prop where
  /-- Every node reaches itself. -/
  | refl (i : Fin n) : Reaches call i i
  /-- One edge, then a path. -/
  | step {i j k : Fin n} : call i j = true → Reaches call j k → Reaches call i k

/-- Reachability is transitive. -/
theorem reaches_trans {n : Nat} {call : Fin n → Fin n → Bool} {i j k : Fin n}
    (hij : Reaches call i j) : Reaches call j k → Reaches call i k := by
  induction hij with
  | refl _ => intro h; exact h
  | step h _ ih => intro hjk; exact Reaches.step h (ih hjk)

/-- Mutual reachability: the same strongly connected component. -/
def SameSCC {n : Nat} (call : Fin n → Fin n → Bool) (i j : Fin n) : Prop :=
  Reaches call i j ∧ Reaches call j i

theorem sameSCC_refl {n : Nat} (call : Fin n → Fin n → Bool) (i : Fin n) :
    SameSCC call i i := ⟨Reaches.refl i, Reaches.refl i⟩

theorem sameSCC_symm {n : Nat} {call : Fin n → Fin n → Bool} {i j : Fin n}
    (h : SameSCC call i j) : SameSCC call j i := ⟨h.2, h.1⟩

theorem sameSCC_trans {n : Nat} {call : Fin n → Fin n → Bool} {i j k : Fin n}
    (hij : SameSCC call i j) (hjk : SameSCC call j k) : SameSCC call i k :=
  ⟨reaches_trans hij.1 hjk.1, reaches_trans hjk.2 hij.2⟩

/-- A call edge whose target reaches back lies on a cycle. -/
def OnCycleEdge {n : Nat} (call : Fin n → Fin n → Bool) (i j : Fin n) : Prop :=
  call i j = true ∧ Reaches call j i

/-- An edge on a cycle places its endpoints in the same component. -/
theorem onCycleEdge_sameSCC {n : Nat} {call : Fin n → Fin n → Bool} {i j : Fin n}
    (h : OnCycleEdge call i j) : SameSCC call i j :=
  ⟨Reaches.step h.1 (Reaches.refl j), h.2⟩

/-- A two-node cycle: `0 → 1 → 0`. -/
def twoCycle : Fin 2 → Fin 2 → Bool := fun i j => i ≠ j

theorem twoCycle_twoCycleEdges :
    OnCycleEdge twoCycle 0 1 ∧ OnCycleEdge twoCycle 1 0 := by
  constructor
  · exact ⟨by simp [twoCycle], Reaches.step (by simp [twoCycle]) (Reaches.refl 0)⟩
  · exact ⟨by simp [twoCycle], Reaches.step (by simp [twoCycle]) (Reaches.refl 1)⟩

theorem twoCycle_sameSCC : SameSCC twoCycle 0 1 :=
  onCycleEdge_sameSCC twoCycle_twoCycleEdges.1

/-! ## Not every in-component sequence is periodic -/

/-- A call graph with an edge between every pair of nodes. -/
def completeCall {n : Nat} : Fin n → Fin n → Bool := fun _ _ => true

/-- The lasso `1, 0, 0, 0, …`: one step to node `0`, then it stays there. -/
def lasso (t : Nat) : Fin 2 := if t = 0 then 1 else 0

/-- The lasso is a walk of the complete call graph, hence stays inside one component. -/
theorem lasso_follows : ∀ t, completeCall (lasso t) (lasso (t + 1)) = true := fun _ => rfl

/-- **A component's runs are not all periodic**: the lasso returns to `0` after its first step and
never leaves, so no period reproduces its first value. `periodic_cycle_terminates` therefore covers
only the runs that happen to be periodic, not every run of a component. -/
theorem not_every_sequence_periodic (p : Nat) (hp : p ≠ 0) :
    ¬ ∀ t : Nat, lasso (t + p) = lasso t := by
  intro hper
  have h0 := hper 0
  have hleft : lasso (0 + p) = (0 : Fin 2) := by simp [lasso, hp]
  have hright : lasso 0 = (1 : Fin 2) := rfl
  rw [hleft, hright] at h0
  exact absurd h0 (by decide)

end NobleM2.SizeChange
