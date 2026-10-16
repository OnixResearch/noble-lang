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

## Final execution packet (2026-10-16; promotion held)

The immutable [acceptance receipt](acceptance.json) has SHA-256
`cace59bc617e30b2a763b0298402e613115823eeec79aeae21d746571c5273a2`.
It records `result: passed`, empty `failures` and `integrity_failures`, a frozen
staged projection, and 74 projected built-source files equal to the tested
commit. It contains 264 commands, 31 byte-identical CORE comparison rows,
36 gate-log-bound library tests and 100 scenario commands repeated twice.
The external raw directory is
`/home/brittonr/.cache/noble-issues/proof/ngraph-acceptance-20261016-final/acceptance`;
its manifest SHA-256 is
`3373a8e944fbd8b0bca59ea29439aa022489402332d38d99211f2b6fb9588dd7`.

[Build provenance](build-provenance.json) records the exact commands, hashes,
source trees, ignored 10⁷ soft-float tier and per-case coverage boundaries.
Its corrected SHA-256 is
`b7d618b65fc84f57d2a8cee6779911b8d043b6e2d5127b7639dd6fad6cd76f7b`.
Each `builds.*.sha256` hashes the binary at `builds.*.file`; the separate
`builds.*.log_sha256` hashes its build log. The superseded provenance and
original external integrity check are preserved; the corrected check is
`/home/brittonr/.cache/noble-issues/proof/ngraph-acceptance-20261016-final/packet-integrity-check-corrected.json`.
The release binary and ignored tier were freshly built/run at published source
`0eb5c0478ebf3419de6f94b8144c2ac7520356ca`, not literally at the historical
gate revision `bc10df848ed81d53e849c440c1f639979e40d38d`. The complete
`crates`, Cargo, policy, Nix, flake and dylint source manifests (707 files)
are byte-identical between those revisions. `--tested-commit` binds that
equivalent built-source projection and the original gate log; it does not
relabel the old logs as a new gate run. The base binary was freshly rebuilt
at `66eb7cdbf86059a4e475178cb6ef03c74cd11e8b` in a separate worktree.

Only missing `expected.stage`, top-level outcome and rejection-trace schema
fields were added before freezing the unexecuted cases. Original bytes and
the exact schema-only delta are retained in the external packet's parent;
the prepromotion case-file SHA-256 is
`57a785b1c9148bec0575ed1cf20f32ba22127b00bad5fd7cab66de006473e8eb`.
Existing semantic expectations, case states and evidence were not changed.
This is `prepromotion_case_file_sha256`, not a case-record hash. The historical
external schema-completion record used the shorter field name for this
whole-file digest and remains unchanged. [Prepromotion case records](prepromotion-cases.json)
(SHA-256 `96150208b0f6984919a7016ef4f2b825657e8a4ab809eaaf537fd11dd654b545`)
retain all fifteen normalized, unpromoted records and their individual
`prepromotion_case_sha256` values. Each hashes UTF-8 `JSON.stringify` of the
entire record after recursively sorting object keys in ECMAScript UTF-16
code-unit order, preserving array order, with no whitespace, BOM or trailing
newline. Future `evidence.configuration.prepromotion_case_sha256` must use
the corresponding record digest, never the whole-file digest.

**Runner passage is not lifecycle promotion or a completed claims review.**
CASE-01 and CASE-15 require the separate independent review. The 31 CORE
rows are finite coverage: CORE-03/05/09 are compile-only in this runner and
CORE-15/16 are name-bound cargo controls. CASE-04's zero evaluated nodes is
structural accounting of `check`, not an observed CLI counter; its cited
`refusals_before_evaluation_evaluate_no_node` test exercises eval refusals,
not a direct check-node measurement. Guest/protected-operation counters are
also structural accounting, not measured report fields. CASE-02 remains
not-run without collected zero-use capability facts. No case state has been
promoted, no sync/archive performed and no CALC state changed.

### Director disposition: direct observations only

The runner's variant results are executable evidence, not complete case
acceptance. The director holds all seven static/admission cases at
`absent/not-run/open/unassessed` with `evidence: []`:

| Case | Unobserved acceptance field or additional gap |
|---|---|
| 02 | Guest/protected counters are structural; collected zero forbidden-capability facts are also missing. |
| 03 | Guest/protected counters are structural, not directly observed. |
| 04 | Guest/protected counters and check-node zero are not directly measured. |
| 05 | Guest/protected counters are structural, not directly observed. |
| 09 | Guest/protected counters are structural, not directly observed. |
| 11 | Guest/protected counters are structural, not directly observed. |
| 12 | Guest/protected counters are structural, not directly observed. |

Only 01/06/07/08/10/13/14/15 are candidate promotions, subject to final
independent audit and director release. The director reports that the audit
has supplementary actual CORE-03/05/09 runtime observations for CASE-01;
these are separate from this runner's compile-only rows. CASE-08 still
requires the audit's exact library node-zero coverage verdict. No telemetry,
tautological test, semantic expectation or case-kind change was added to
manufacture case acceptance. Registration and promotion remain unstarted.
This disposition does not claim full feature acceptance or archive readiness.

The original main gates remain **10 passed, four known failed**:
boundary-controls (#26/#27), Octet (#26), source-coverage and native-assurance
(#25). They were not rerun here. NG-PO-A..H remain open; the Phase C W3-L
0.131× host-native STOP remains unchanged. This packet makes no proof,
performance, GPU, backend-selection or cross-platform claim.
