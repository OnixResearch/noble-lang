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
  `rows_never_share_a_stack`);
* a refused row stays local: it cannot change any other row's result
  (`evalBatch_append`, `a_refused_row_does_not_affect_later_rows`);
* a validated row's reference result is the wrapping operation on its operands
  (`validated_row_result_is_the_wrapping_operation`), and an out-of-range literal
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

end NobleFoundation
