# Numeric-Graph-v1 acceptance runner

`acceptance.mjs` records the executed scenario cases of
`numeric-graph-v1` (`NGRAPH-CASE-01..15`). It never edits case state or
evidence; it writes one receipt, `acceptance.json`, and keeps every raw
output in an external directory bound by sha256.

## Run

Use the policy-selected Node (`policy/tool-selection.json`
`tool_paths.node.output` + `/bin/node`) with no Node flags.

```sh
NODE=$(jq -r .tool_paths.node.output policy/tool-selection.json)/bin/node
"$NODE" verification/numeric-graph-v1/acceptance.mjs plan "$OUT"
"$NODE" verification/numeric-graph-v1/acceptance.mjs run "$OUT" \
  --binary <noble built at $INT> --base-binary <noble built at 66eb7cd> \
  --base-commit 66eb7cdbf86059a4e475178cb6ef03c74cd11e8b \
  --gate-receipt verification/main-gates/receipt.json --tested-commit "$INT" \
  --soft-float-log <release log of soft_float --ignored at $INT>
```

- `plan` needs a new directory outside the repository. It freezes the case
  file (`--cases FILE`, default: whichever of
  `specs/conformance/numeric-graph-cases.json` or the change's
  `cases/numeric-graph-cases.json` exists), copies `inputs/`, and writes
  `plan.json` with every argv, the expected fields and the fixture hashes.
- `run` executes a planned directory once. It writes
  `verification/numeric-graph-v1/acceptance.json` unless `--receipt FILE` names
  another path; an existing receipt is never overwritten. Optional
  `--binary-sha256` and `--base-binary-sha256` pin the binaries.
- `--base-commit` must be an ancestor of `--tested-commit` without
  `crates/noble-ngraph`. The receipt is `passed` only with `failures: []` and
  `integrity_failures: []`; the source projection must be staged (frozen).

## What it observes

- `noble ngraph` cases 03, 04, 05, 07, 08, 10, 11, 12 and 13 run with their
  exact case inputs on a frozen copy of `--binary`, each in a fresh directory:
  F1 `--fuel 1`, an F2 snapshot against F1, the B5 500-step snapshot of F1 and
  its follow-ups, and `verify --replay --from` of that run's 10-step child.
  Forged snapshots come from the runner's own D6/BLAKE3 encoding, which must
  first reproduce the unforged snapshot byte for byte.
- Case 15: every planned command runs twice; stdout, stderr, exit, published
  bytes and the final directories must be identical. `noble --help` must list
  `ngraph` and `calculator`. The claims review is a separate record.
- Case 01: the CORE-01..18 workload, driven as `verification/m4/gate.mjs`
  does, on both binaries, compared byte for byte, plus the kernel, contracts
  and wasm diff, the `main.rs` dispatch, the `noble_ngraph` references and the
  K-NUM-01 text. The case is a review; the receipt supplies its inputs.
- Library cases 06, 09 and 14 and the library halves of the others: every
  mapped test (`LIBRARY_TESTS`) appears exactly once as `test NAME ... ok` in
  the gate `rust-tests` log of `--tested-commit` and is defined once in its file
  at that commit. Case 14 also needs `ten_million_bit_patterns_match_std` in
  `--soft-float-log`.
- Not run: case 02 (it needs Octet/native-assurance capability facts).

Every `expected` field is either observed or accounted for: `evaluated_nodes`
and the guest counters are not in `noble-ngraph-report/v1` lines, so they are
recorded as not observable with the covering library test.

## Inputs

`inputs/` holds every input byte the runner stages; `FIXTURES` in the runner
pins each sha256 and origin. F1, F2, the MSE loss and the eight NGRAPH-CASE-07
B2 graphs are `noble_ngraph::format::graph::print` of the
`noble-ngraph-experiment` fixtures. `b5.points` is the B5 set of
`crates/noble-cli/tests/ngraph/b5.points`. The F1 refusal variants and
`f1-fact.ngraph` are single edits of `f1.ngraph`, checked at plan time. The
training refusal fixtures are those of `crates/noble-cli/tests/ngraph.rs`.
