import NobleM2.SizeChangeCallGraph

/-!
# Call sites generate the matrices (issue #101)

The last link between the call-graph layer and the cycle theorem: a program assigns a matrix to each
call edge, and a run walks those edges. If the run's node sequence is periodic with period `p`, the
sequence of matrices it visits is periodic with the same period - so `periodic_cycle_terminates`
applies to it directly.

This is the modelling step, made as small as it can be: the program is a call graph plus one matrix
per edge, and a run carries its node and state sequences with the obligation that each step follows
an edge and satisfies that edge's matrix. What is *not* modelled is how a compiler produces those
matrices from a definition's syntax; that is the front end.

Both hypotheses of the theorem are assumptions, not consequences, so the result is narrower than
"the program terminates". Periodicity is not derived from the call graph - a run's node sequence
need not be periodic at all (`SizeChangeCallGraph.not_every_sequence_periodic`) - and the
strict-diagonal hypothesis is given. The proof never uses `follows`, so the call edges themselves
play no part in it.

What is proved is: call sites → matrices → path products → periodic cycles → termination *of a
periodic run whose period product is strict on a diagonal*. It is not a termination proof for a
program. The path-length bound a composite search needs is `matrixProduct_short_representative`,
which produces a sub-path representative no longer than the matrix enumeration.
-/

namespace NobleM2.SizeChange

/-- A program: a call graph with a matrix on each call edge. -/
structure Program (n m : Nat) where
  /-- Which call edges exist. -/
  call : Fin n → Fin n → Bool
  /-- The matrix of each call edge. -/
  onEdge : Fin n → Fin n → MGraph m

/-- A run of a program: a walk of call edges with states satisfying each edge's matrix. -/
structure CallRun {n m : Nat} (prog : Program n m) where
  /-- The call site visited at each step. -/
  node : Nat → Fin n
  /-- The states between steps. -/
  state : Nat → Fin m → Nat
  /-- Each step follows a call edge. -/
  follows : ∀ t, prog.call (node t) (node (t + 1)) = true
  /-- Each step satisfies the matrix of the edge it follows. -/
  sat : ∀ t, MatrixSatisfies (prog.onEdge (node t) (node (t + 1))) (state t) (state (t + 1))

/-- The matrix a run visits at step `t`. -/
def CallRun.step {n m : Nat} {prog : Program n m} (run : CallRun prog) (t : Nat) : MGraph m :=
  prog.onEdge (run.node t) (run.node (t + 1))

/-- A periodic node sequence makes the visited matrices periodic. -/
theorem step_periodic {n m : Nat} {prog : Program n m} (run : CallRun prog) {p : Nat}
    (hper : ∀ t, run.node (t + p) = run.node t) (t : Nat) : run.step (t + p) = run.step t := by
  unfold CallRun.step
  have h1 : run.node (t + p) = run.node t := hper t
  have h2 : run.node (t + p + 1) = run.node (t + 1) := by
    have hshift := hper (t + 1)
    have harith : t + 1 + p = t + p + 1 := by omega
    rw [harith] at hshift
    exact hshift
  rw [h1, h2]

/-- **Termination for a periodic call run**: if the product of the matrices visited over one period
is strict on a diagonal, the run cannot be infinite. `follows` is not used: the call edges are
carried for the front end, not consumed here. -/
theorem call_run_terminates {n m : Nat} {prog : Program n m} (run : CallRun prog) {p : Nat}
    {i : Fin m} (hper : ∀ t, run.node (t + p) = run.node t)
    (hprod : pathMatrix p run.step 0 i i = .strict) : False :=
  periodic_cycle_terminates (step := run.step) (step_periodic run hper) hprod run.state
    (fun t => run.sat t)

end NobleM2.SizeChange
