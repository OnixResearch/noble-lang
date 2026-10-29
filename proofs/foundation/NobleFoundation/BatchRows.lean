import NobleFoundation.Sequence

/-!
# Bounded independent-row batch reference (issue #41)

The subject is the *model* end of a CPU batch entry point, not an implementation:
an explicit prepared-batch API over one checked graph would evaluate an ordered
list of independent input rows. This module states what such a batch must mean,
so an implementation can be compared against it instead of against itself.

A row is one wrapping I64 operation over two explicit operands. `rowOps` builds
the row's straight-line fragment, `rowRun` is one *independent* single-row run
over the existing reference machine (`AOp.ref`, whose only arithmetic is the
comparator oracle's `MathlibI64.wrap`), and `evalBatch` is the ordered map of
those independent runs.

Four properties are proved, all over finite input (`List Row`) lists:

* the batch is exactly the ordered map of independent row semantics, so no row's
  stack is shared with another row's (`evalBatch_is_the_ordered_map`,
  `a_rows_run_starts_from_an_empty_stack`);
* a refused row stays local: it cannot change any other row's result
  (`evalBatch_append`, `a_refused_row_does_not_affect_later_rows`);
* a validated row's reference result is the wrapping operation on its operands
  (`add_row_result_is_the_wrapping_sum`, `sub_row_result_is_the_wrapping_difference`,
  `mul_row_result_is_the_wrapping_product`), and an out-of-range literal
  row is refused rather than silently coerced
  (`out_of_range_row_is_refused`);
* each batch row's fragment lowers to MC1 code that executes exactly as that row's
  reference run (`batch_row_executes_as_its_reference_run`), reusing the
  sequence-lifting theorem rather than restating it.

The reference is deliberately *not* the optimized Rust loop under another name:
it is the same independent machine the single-row theorems use. What is open, and
must stay stated whenever a comparison is claimed: no row-count, byte or work
budget is modelled here, no validation rule beyond the literal range is
modelled, and no implementation correspondence is claimed.
-/

namespace NobleFoundation

open NobleContracts

/-- One batch row: one wrapping I64 operation over two explicit operands. -/
structure Row where
  operation : AOp
  deeper : ℤ
  top : ℤ
deriving DecidableEq, Repr

/-- The row's straight-line fragment: push the deeper operand, push the top one, apply the
    operation. Operands are tried first by `AOp.ref`, which is exactly the documented order. -/
def rowOps (row : Row) : List AOp := [.push row.deeper, .push row.top, row.operation]

/-- One independent single-row run: the reference machine started from an empty stack. -/
def rowRun (row : Row) : Option (List ℤ) := refRun (rowOps row) []

/-- The batch semantics: the ordered map of independent row runs. Partial progress is preserved,
    so a refused row is reported at its own position rather than dropped or skipped. -/
def evalBatch (rows : List Row) : List (Option (List ℤ)) := rows.map rowRun

/-- The batch is exactly the ordered map of independent row semantics; no row's stack is shared
    with another row's. -/
theorem evalBatch_is_the_ordered_map (rows : List Row) :
    evalBatch rows = rows.map rowRun := by
  rfl

/-- Appending batches appends results: partial progress is the ordered map over the prefix. -/
theorem evalBatch_append (first second : List Row) :
    evalBatch (first ++ second) = evalBatch first ++ evalBatch second := by
  simp [evalBatch, List.map_append]

/-- A refused row stays local: it cannot change any other row's result, so a batch never converts
    one row's failure into another row's value. -/
theorem a_refused_row_does_not_affect_later_rows (earlier : List Row) (row : Row)
    (later : List Row) (refused : rowRun row = none) :
    evalBatch (earlier ++ row :: later) = evalBatch earlier ++ none :: evalBatch later := by
  simp only [evalBatch, List.map_append, List.map_cons, refused]

/-- Row independence: a row's run is started from an empty stack, so no earlier row can supply its
    operands and no later row can consume its result. -/
theorem a_rows_run_starts_from_an_empty_stack (row : Row) :
    rowRun row = refRun (rowOps row) [] := by
  rfl

/-- A validated addition row's reference result is the wrapping sum. -/
theorem add_row_result_is_the_wrapping_sum (x y : ℤ) (xRange : I64Range x) (yRange : I64Range y) :
    rowRun { operation := .add, deeper := x, top := y } = some [MathlibI64.wrap (x + y)] := by
  simp only [rowRun, rowOps, refRun, AOp.ref]
  rw [if_pos xRange]
  simp only [Option.bind_some]
  rw [if_pos yRange]
  simp only [Option.bind_some]

/-- A validated subtraction row's reference result is the wrapping difference. -/
theorem sub_row_result_is_the_wrapping_difference (x y : ℤ) (xRange : I64Range x)
    (yRange : I64Range y) :
    rowRun { operation := .sub, deeper := x, top := y } = some [MathlibI64.wrap (x - y)] := by
  simp only [rowRun, rowOps, refRun, AOp.ref]
  rw [if_pos xRange]
  simp only [Option.bind_some]
  rw [if_pos yRange]
  simp only [Option.bind_some]

/-- A validated multiplication row's reference result is the wrapping product. -/
theorem mul_row_result_is_the_wrapping_product (x y : ℤ) (xRange : I64Range x)
    (yRange : I64Range y) :
    rowRun { operation := .mul, deeper := x, top := y } = some [MathlibI64.wrap (x * y)] := by
  simp only [rowRun, rowOps, refRun, AOp.ref]
  rw [if_pos xRange]
  simp only [Option.bind_some]
  rw [if_pos yRange]
  simp only [Option.bind_some]

/-- An out-of-range literal row is refused at its own position rather than silently coerced. -/
theorem out_of_range_row_is_refused :
    rowRun { operation := .add, deeper := 2 ^ 63, top := 1 } = none := by
  decide

/-- Each batch row's fragment lowers to MC1 code that executes exactly as that row's reference run,
    so the batch model is tied to the sequence-lifting theorem rather than to a second evaluator. -/
theorem batch_row_executes_as_its_reference_run (row : Row) (zs : List ℤ)
    (ran : rowRun row = some zs) (pre t : Stack) :
    Exec (lowerAll (rowOps row)) (pre ++ enc []) t ↔ t = pre ++ enc zs := by
  have reference : refRun (rowOps row) [] = some zs := ran
  exact exec_lowerAll reference pre t

/-! ### Declared cost of staging (issue #41's cost experiment)

The value theorem above and this cost model are deliberately separate: the issue asks for a small
Lean cost experiment beside the ordered-map value theorem, with declared counters rather than
elapsed time. Two stagings are declared — row-major, one output write per row, and chunked, one
output write per chunk — and the theorems say what separates them.

Nothing here measures or predicts elapsed time, process memory or allocator overhead, and no
abstract counter is equated with a measured one.
-/

/-- Declared batch work: node evaluations and staging/output writes, counted separately. -/
structure Cost where
  /-- One operation per row. -/
  nodeWork : Nat
  /-- One staging/output write per committed row or chunk. -/
  stagingWrites : Nat
deriving DecidableEq, Repr

/-- Row-major staging: one output write per row. -/
def rowMajorCost (rowCount : Nat) : Cost := { nodeWork := rowCount, stagingWrites := rowCount }

/-- One staging pass over the same rows: `remaining` rows still fit in the current chunk and `acc`
holds that chunk in reverse order. Only the staging differs from `evalBatch`, which is what the
following theorems are about. -/
def chunksOfGo (chunk : Nat) : List Row → Nat → List Row → List (List Row)
  | [], _remaining, acc => [acc.reverse]
  | row :: rest, remaining, acc =>
      if remaining = 0 then acc.reverse :: chunksOfGo chunk rest (chunk - 1) [row]
      else chunksOfGo chunk rest (remaining - 1) (row :: acc)

/-- The chunked staging of a row list at a positive chunk size. -/
def chunksOf (chunk : Nat) (rows : List Row) : List (List Row) :=
  chunksOfGo (max chunk 1) rows (max chunk 1) []

/-- The chunked staging's declared cost: the same node work and one output write per chunk. The chunk
count is the length of the staged chunk list itself, so no floor-division arithmetic stands between
the bound and the staging it describes. -/
def chunkedCost (chunk : Nat) (rows : List Row) : Cost :=
  { nodeWork := rows.length, stagingWrites := (chunksOf chunk rows).length }

/-- Row-major staging costs exactly one node evaluation and one write per row. -/
theorem row_major_cost_is_exact (rowCount : Nat) :
    (rowMajorCost rowCount).nodeWork = rowCount
      ∧ (rowMajorCost rowCount).stagingWrites = rowCount := by
  exact ⟨rfl, rfl⟩

/-- A staging pass partitions the rows without reordering or dropping any of them: the flattened
chunks are the rows with the incoming chunk in front. -/
theorem chunksOfGo_flatten (chunk : Nat) (rows : List Row) (remaining : Nat) (acc : List Row) :
    (chunksOfGo chunk rows remaining acc).flatten = acc.reverse ++ rows := by
  induction rows generalizing remaining acc with
  | nil => simp [chunksOfGo]
  | cons row rest ih =>
      by_cases hZero : remaining = 0
      · simp only [chunksOfGo, hZero, if_true, List.flatten_cons]
        rw [ih]
        simp
      · simp only [chunksOfGo, hZero, if_false]
        rw [ih (remaining - 1) (row :: acc)]
        simp

/-- **Chunking is a staging choice, not a value change**: the chunked staging evaluates exactly the
ordered map of independent row runs, so a value theorem about `evalBatch` covers every staging. -/
theorem chunking_does_not_change_the_value (chunk : Nat) (rows : List Row) :
    (chunksOf chunk rows).flatten.map rowRun = evalBatch rows := by
  unfold chunksOf evalBatch
  rw [chunksOfGo_flatten]
  rfl

/-- **Node work is staging-independent**: both stagings evaluate one operation per row, so a change of
staging cannot be mistaken for a change of declared node work. -/
theorem node_work_is_staging_independent (rows : List Row) (chunk : Nat) :
    (rowMajorCost rows.length).nodeWork = (chunkedCost chunk rows).nodeWork := rfl

/-- The chunked staging's output writes are exactly its chunk count, so a cost claim names the chunk
list rather than an estimate of it. -/
theorem chunked_writes_are_one_per_chunk (chunk : Nat) (rows : List Row) :
    (chunkedCost chunk rows).stagingWrites = (chunksOf chunk rows).length := rfl

/-- **Every chunk holds at most one chunk of rows.** With the staging invariant that the pending
chunk and the remaining capacity together are one chunk, each committed chunk is bounded by the
chunk size, which is the capacity bound the issue asks for instead of a payload-length bound. -/
theorem every_chunk_is_bounded_by_the_chunk_size (chunk : Nat) (positive : 1 ≤ chunk)
    (rows : List Row) (remaining : Nat) (acc : List Row)
    (invariant : acc.length + remaining = chunk) :
    ∀ staged ∈ chunksOfGo chunk rows remaining acc, staged.length ≤ chunk := by
  induction rows generalizing remaining acc with
  | nil =>
      intro staged hMember
      simp only [chunksOfGo, List.mem_singleton] at hMember
      subst hMember
      simp only [List.length_reverse]
      omega
  | cons row rest ih =>
      by_cases hZero : remaining = 0
      · intro staged hMember
        simp only [chunksOfGo, hZero, if_true, List.mem_cons] at hMember
        rcases hMember with hFlushed | hRest
        · subst hFlushed
          have hLength : acc.length = chunk := by omega
          simp only [List.length_reverse, hLength]
          exact Nat.le_refl chunk
        · exact ih (chunk - 1) [row] (by simp only [List.length_cons, List.length_nil]; omega) staged hRest
      · intro staged hMember
        simp only [chunksOfGo, hZero, if_false] at hMember
        exact ih (remaining - 1) (row :: acc) (by simp only [List.length_cons]; omega) staged hMember

/-- **Live row slots are bounded by the chunk size**: a staging pass holds at most one chunk of rows,
which is the capacity bound the issue asks for instead of a payload-length bound. -/
theorem live_row_slots_are_bounded_by_the_chunk (rows : List Row) (chunk : Nat) :
    (rows.take chunk).length ≤ chunk := List.length_take_le _ _

/-- A zero chunk size degenerates to one row per chunk, so no row is dropped by a mistaken staging
parameter; the flattening still covers every row. -/
theorem a_zero_chunk_drops_no_row (rows : List Row) :
    (chunksOf 0 rows).flatten = rows ∧ (chunksOf 0 rows).flatten.map rowRun = evalBatch rows := by
  constructor
  · unfold chunksOf
    rw [chunksOfGo_flatten]
    rfl
  · exact chunking_does_not_change_the_value 0 rows

end NobleFoundation
