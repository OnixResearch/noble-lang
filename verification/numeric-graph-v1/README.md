# Numeric-Graph-v1 lifecycle evidence

The registered [NGRAPH cases](../../specs/conformance/numeric-graph-cases.json)
and [profile status](../../specs/STATUS.json) record **8 passed, 7 not-run**,
with proof open for every case. The immutable [acceptance receipt](acceptance.json)
and [independent review receipt](review-receipt.json) bind the finite execution
and review evidence. The [implementation change](../../.cairn/changes/numeric-graph-v1/design.md)
stays active and is not archived; this is not whole-feature or global-gate acceptance.

`acceptance.mjs` records observations for `NGRAPH-CASE-01..15`. It never edits
case state or evidence; it writes one receipt, `acceptance.json`, and keeps
every raw output in an external directory bound by sha256. A runner result
is not by itself acceptance of every case contract.

## Historical acceptance runner interface

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
- The runner omits case 02 (it needs Octet/native-assurance capability facts).
  The final disposition below also keeps six other incomplete static/admission
  contracts not-run despite their observed variant outputs.

The report schema has no evaluated-node or guest/protected-operation counters.
Structural field accounting is not a direct zero observation. Only the
specific approved library counter mapping described below supplies CASE-08's
otherwise unreported node-zero field; it does not supply counters for the
seven held cases.

## Inputs

`inputs/` holds every input byte the runner stages; `FIXTURES` in the runner
pins each sha256 and origin. F1, F2, the MSE loss and the eight NGRAPH-CASE-07
B2 graphs are `noble_ngraph::format::graph::print` of the
`noble-ngraph-experiment` fixtures. `b5.points` is the B5 set of
`crates/noble-cli/tests/ngraph/b5.points`. The F1 refusal variants and
`f1-fact.ngraph` are single edits of `f1.ngraph`, checked at plan time. The
training refusal fixtures are those of `crates/noble-cli/tests/ngraph.rs`.

## Immutable prepromotion execution packet (2026-10-16)

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
newline. Each `evidence.configuration.prepromotion_case_sha256` uses the
corresponding record digest, never the whole-file digest.

### Independent review and supplemental runtime

The immutable [claims review](review.json), SHA-256
`3596ce7ac2cc8bdda428bf4d30b8438ccd17b71976ef151ca9f40afdb590b563`,
and [review receipt](review-receipt.json) record `result: passed`, no failures,
and the exact eight-case acceptance/seven-case hold. The review receipt
checks 1,245 acceptance raw files and 246 supplemental runtime artifact files.
Its original external output is
`/home/brittonr/.cache/noble-issues/proof/ngraph-independent-claims-20261016-bound`.
The supplemental runtime packet is bound directly by the `runtime` entries
of both review files:
`/home/brittonr/.cache/noble-issues/proof/ngraph-independent-claims-20261016-core/runtime.json`,
SHA-256 `9a9308c4ebf372e3cf8a3b2a4a2e6745b76e6e89419fbf420523190e775921fc`.

CASE-01 combines the acceptance runner's 31 byte-identical CORE rows with
12 additional actual CORE-03/05/09 runtime observations per binary (6/4/2),
using the m4 driver and pinned Node host. The acceptance runner's own
CORE-03/05/09 rows remain compile-only; its immutable receipt is not
rewritten to include the supplemental observations. CORE-15/16 remain
name-bound cargo controls and source review, not invented CLI runtime rows.
This is a finite ordinary-entry-point review, not universal semantic equivalence.
CASE-15 combines the independent review of the README, actual help, report
field names and historical experiment documentation with 100 command pairs
whose stdout, stderr, exit status and published bytes match.

CASE-08 passes its exact CLI variant codes, outcomes, exits and line counts
plus the reviewed library mapping in `review.json` `case08.node_zero`.
`refusals_before_evaluation_evaluate_no_node` directly measures both
evaluators on `common::linear(false, true)`: `x=5.0` with a valid snapshot
gives `FactInput`; `Value::I64(1)` for the F32 interface gives `Interface`;
`x=1.0` without a snapshot gives `SnapshotLayout`. Each reports
`evaluated_nodes == 0`. These are the three exact named refusal classes,
not measurements on the CLI's guard/F1 file bytes, and **not CLI counters**.
Guard traps, fuel exhaustion and mixed normal/guard-failing rows are not
asserted to evaluate zero nodes. This mapping does not measure CASE-04
`check` or any guest/protected-operation counter.

CASE-06 is finite mapped library coverage: canonical/noisy text over the
full identity corpus, with authored-name/evidence parser variants on the
applicable zoo subjects using the test SipHash scheme, not a Cartesian
product over every corpus graph. CASE-14 includes the retained ignored
10⁷ bit-pattern tier for bit-input routines, NaNs compared as one class,
and exhaustive integer exponents −1100..1100 for `pow2_f64`; it is not
a universal float-correctness proof.

The historical `review.mjs` verifier deliberately binds prepromotion docs
and the old active case path. Do not rerun it against the recorded-state
documents or moved case file; preserve its original review receipt and raw
outputs unchanged.

## Registered disposition: 8 passed, 7 not-run

Cases **01/06/07/08/10/13/14/15** are
`implemented/passed/open/explicit` with receipt-linked evidence at source
`sha256:2f4ebb1dc161687fc9844e9ab69cb1f992caf27bd6fd6ecd83aa0ff9d30f6f9f`.
All seven static/admission cases below remain
`absent/not-run/open/unassessed` with `evidence: []`; passing variant outputs
do not satisfy their unobserved fields:

| Case | Unobserved acceptance field or additional gap |
|---|---|
| 02 | Guest/protected counters are structural, not directly observed; complete zero-use capability closure is missing, with 637 relevant unknowns ([retained gate deltas](gate-deltas.json)). |
| 03 | Guest/protected counters are structural, not directly observed. |
| 04 | Guest/protected counters and check-node zero are not directly measured. |
| 05 | Guest/protected counters are structural, not directly observed. |
| 09 | Guest/protected counters are structural, not directly observed. |
| 11 | Guest/protected counters are structural, not directly observed. |
| 12 | Guest/protected counters are structural, not directly observed. |

CASE-04's zero evaluated nodes is structural accounting of `check`, not an
observed counter: the cited library test measures eval refusals, not check.
CASE-12's parse/integrity paths likewise have no complete directly measured
node count. No telemetry, semantic expectation, case kind, input, profile or
requirement was changed to manufacture acceptance. The original immutable
acceptance, review, provenance, normalized prepromotion cases and raw outputs
retain their historical prepromotion wording and bytes. CALC records and the
proof ledger are unchanged. Seven incomplete contracts and the retained
global failures keep archive and whole-feature completion open.

The original main gates remain **10 passed, four known failed**:
boundary-controls (#26/#27), Octet (#26), source-coverage and native-assurance
(#25). They were not rerun here. NG-PO-A..H remain open; the Phase C W3-L
0.131× host-native STOP remains unchanged. This packet makes no proof,
performance, GPU, backend-selection or cross-platform claim.

## Bounded postpromotion verification

`postpromotion.mjs` verifies the staged lifecycle documents against the exact
independently audited anchor `16fd5369a6625d1d166a25190bbb02532832432a`.
Use the policy-selected Node with no execution overrides, the original
external receipts (not their repository copies), and a new external output
directory whose parent already exists:

```sh
NODE=$(jq -r .tool_paths.node.output policy/tool-selection.json)/bin/node
"$NODE" verification/numeric-graph-v1/postpromotion.mjs plan \
  --anchor 16fd5369a6625d1d166a25190bbb02532832432a \
  --acceptance /home/brittonr/.cache/noble-issues/proof/ngraph-acceptance-20261016-final/acceptance/acceptance.json \
  --review /home/brittonr/.cache/noble-issues/proof/ngraph-independent-claims-20261016-bound/review-receipt.json \
  --out "$OUT"
# Require ready:true and prerequisites:[] before executing the same arguments:
"$NODE" verification/numeric-graph-v1/postpromotion.mjs run \
  --anchor 16fd5369a6625d1d166a25190bbb02532832432a \
  --acceptance /home/brittonr/.cache/noble-issues/proof/ngraph-acceptance-20261016-final/acceptance/acceptance.json \
  --review /home/brittonr/.cache/noble-issues/proof/ngraph-independent-claims-20261016-bound/review-receipt.json \
  --out "$OUT"
```

`plan` is read-only and creates no directory. `run` writes only its new
external directory: a terminal `receipt.json`, `source-snapshot.json`, the
complete 707-file build manifest, immutable packet copies and before/after
snapshots for changed reviewed documents. A passing receipt requires empty
`failures` and `integrity_failures`, the exact eight passed/seven held case
records, renderer-derived views and ledger, unchanged proof/CALC records,
and terminally frozen index/worktree and original external evidence.
All thirteen canonical specs and compatibility views must match the pinned
renderer exactly. In addition to the three approved normative delta files,
case registration regenerates only scenario regions in the existing safety,
verification and backend-experiments specs; their normative bytes remain
identical to the audit anchor. No other canonical file gains a change exemption.
The verifier rehashes the 1,245 acceptance raw files and 246 supplemental
runtime artifacts; it does not rerun the historical review against changed
documents, re-execute acceptance, rebuild, run global gates, sync, archive,
commit or publish. Its receipt is bounded lifecycle verification, not a
replacement for the immutable acceptance/review or a globally green gate.
