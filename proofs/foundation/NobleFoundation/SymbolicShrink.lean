/-
Shrinking a refuting witness for pure I64/Bool graph pairs (issue #56, acceptance item 2).

`SymbolicSolver.firstIndex` reports the first candidate of a declared universe that
refutes a pair. This file shrinks such a witness toward zero, one slot at a time,
and keeps a shrink only while it still refutes the pair.

A move replaces one slot's exact 64 bits with one of:
- zero;
- the value halved toward zero (`BitVec.sdiv`, which truncates as Rust's `i64`
  division does);
- the value one step toward zero.

A move equal to the value is dropped. `shrinkStep` takes the first move that still
refutes, slot by slot and move by move in that order, and `shrinkTrace` repeats it
under explicit fuel.

What is proved:
- a shrink never loses the refutation (`shrinkTrace_refutes`);
- every step is one of the declared moves (`shrinkStep_is_a_move`);
- a witness the shrinker cannot move is a local minimum, because every one of its
  single-slot moves agrees (`shrinkStep_none_minimal`).

Nothing here claims a global minimum, a smallest witness over all inputs, or
anything about `crates/noble-ngraph/src/extract/symbolic.rs`.
-/
import NobleFoundation.SymbolicSolver

namespace NobleFoundation.SymbolicShrink

open NobleFoundation.SymbolicPair

/-- The inputs a finite list of slot values denotes; a slot beyond the list reads zero. -/
def inputsOf (values : List (BitVec 64)) (slot : Nat) : BitVec 64 :=
  values.getD slot 0

/-- The value one step toward zero: the value minus its sign. -/
def towardZero (value : BitVec 64) : BitVec 64 :=
  if value = 0 then value else if value.slt 0 then value + 1 else value - 1

/-- One slot's moves, nearest zero first: zero, the value halved toward zero, and the
value one step toward zero, without the value itself. -/
def moves (value : BitVec 64) : List (BitVec 64) :=
  [0, value.sdiv 2, towardZero value].filter (fun move => move != value)

/-- Every single-slot move of a witness, slot by slot in order. -/
def neighbours (values : List (BitVec 64)) : List (List (BitVec 64)) :=
  (List.range values.length).flatMap
    (fun slot => (moves (values.getD slot 0)).map (fun move => values.set slot move))

/-- The first single-slot move that still refutes the pair. -/
def shrinkStep (pair : Pair) (values : List (BitVec 64)) : Option (List (BitVec 64)) :=
  (neighbours values).find? (fun next => refuted pair (inputsOf next))

/-- A witness followed by its shrinks, under explicit fuel. -/
def shrinkTrace (pair : Pair) (values : List (BitVec 64)) : Nat → List (List (BitVec 64))
  | 0 => [values]
  | fuel + 1 =>
      match shrinkStep pair values with
      | some next => values :: shrinkTrace pair next fuel
      | none => [values]

/-- A shrink step keeps the refutation. -/
theorem shrinkStep_refutes (pair : Pair) (values next : List (BitVec 64))
    (hstep : shrinkStep pair values = some next) :
    refuted pair (inputsOf next) = true := by
  unfold shrinkStep at hstep
  have hfound := List.find?_some hstep
  exact hfound

/-- A shrink step is one of the declared single-slot moves. -/
theorem shrinkStep_is_a_move (pair : Pair) (values next : List (BitVec 64))
    (hstep : shrinkStep pair values = some next) :
    next ∈ neighbours values := by
  unfold shrinkStep at hstep
  exact List.mem_of_find?_eq_some hstep

/-- A witness the shrinker cannot move is a local minimum: every single-slot move
agrees with the pair. -/
theorem shrinkStep_none_minimal (pair : Pair) (values : List (BitVec 64))
    (hstep : shrinkStep pair values = none) :
    ∀ next ∈ neighbours values, agreesOn pair (inputsOf next) := by
  intro next hmem
  unfold shrinkStep at hstep
  have hnot := List.find?_eq_none.mp hstep next hmem
  exact (not_refuted_iff_agrees pair (inputsOf next)).mp (by simpa using hnot)

/-- A trace starts at its witness. -/
theorem shrinkTrace_head (pair : Pair) (values : List (BitVec 64)) (fuel : Nat) :
    (shrinkTrace pair values fuel).head? = some values := by
  cases fuel with
  | zero => rfl
  | succ fuel =>
      simp only [shrinkTrace]
      cases shrinkStep pair values <;> rfl

/-- Shrinking never loses the refutation: from a refuting witness, every element of
the trace refutes the pair. -/
theorem shrinkTrace_refutes (pair : Pair) (values : List (BitVec 64)) (fuel : Nat)
    (hstart : refuted pair (inputsOf values) = true) :
    ∀ step ∈ shrinkTrace pair values fuel, refuted pair (inputsOf step) = true := by
  induction fuel generalizing values with
  | zero =>
      intro step hmem
      simp only [shrinkTrace, List.mem_singleton] at hmem
      exact hmem ▸ hstart
  | succ fuel ih =>
      intro step hmem
      simp only [shrinkTrace] at hmem
      cases hnext : shrinkStep pair values with
      | none =>
          simp only [hnext, List.mem_singleton] at hmem
          exact hmem ▸ hstart
      | some next =>
          simp only [hnext, List.mem_cons] at hmem
          rcases hmem with hsame | hlater
          · exact hsame ▸ hstart
          · exact ih next (shrinkStep_refutes pair values next hnext) step hlater

end NobleFoundation.SymbolicShrink
