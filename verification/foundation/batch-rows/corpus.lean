import NobleFoundation.BatchRows

/-!
# Batch-row reference corpus (issue #41)

Prints one row per line as `deeper top op result`, where every operand is an `I64` boundary value and
every result comes from the kernel-checked model `NobleFoundation.BatchRows` and from nothing else.
`crates/noble-ngraph/tests/batch_row_reference.rs` compares the real CPU batch entry point against
this recorded corpus, so the expected results are independent of the implementation under test.

Run:

```text
lake env lean --run batch-rows/corpus.lean > ../../../crates/noble-ngraph/tests/batch_rows/corpus.txt
```
-/

namespace NobleFoundation.BatchRows

def boundaries : List ℤ :=
  [-(2 ^ 63), -(2 ^ 63) + 1, -1, 0, 1, 2 ^ 63 - 2, 2 ^ 63 - 1]

def operations : List AOp := [.add, .sub, .mul]

def operationTag : AOp → String
  | .add => "add"
  | .sub => "sub"
  | .mul => "mul"
  | .push _ => "push"

/-- One corpus row's exact reference result, or `refused` when the model refuses it. -/
def resultOf (operation : AOp) (deeper top : ℤ) : String :=
  match rowRun { operation := operation, deeper := deeper, top := top } with
  | none => "refused"
  | some [result] => toString result
  | some _ => "unexpected"

end NobleFoundation.BatchRows

def main : IO Unit := do
  let rows := NobleFoundation.BatchRows.boundaries.flatMap (fun deeper =>
    NobleFoundation.BatchRows.boundaries.flatMap (fun top =>
      NobleFoundation.BatchRows.operations.map (fun operation =>
        (deeper, top, operation))))
  IO.println s!"# rows: {rows.length}"
  for (deeper, top, operation) in rows do
    let result := NobleFoundation.BatchRows.resultOf operation deeper top
    IO.println s!"{deeper} {top} {NobleFoundation.BatchRows.operationTag operation} {result}"
