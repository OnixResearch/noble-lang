# Numeric graph profile (#14) Phase B receipts

Phase B of [numeric-graph-profile](../../.cairn/archive/2026-10-16-numeric-graph-profile/design.md) (approved on #14 on 2026-10-15; decisions P1–P6 are in the design section "Phase B approval and decisions"). The experiment lives in the isolated crate [noble-ngraph-experiment](../../crates/noble-ngraph-experiment/Cargo.toml). It changes no noble-kernel, noble-contracts, noble-wasm or noble-cli source.

Since the `numeric-graph-v1` change, the graph library is the `no_std` crate [noble-ngraph](../../crates/noble-ngraph/Cargo.toml) and `noble-ngraph-experiment` is its evidence harness (bins, L1, oracles, workloads, fixtures), with the BLAKE3 identity scheme injected. The receipts below were produced before that split, at the source revisions they record. After the split, `tests/identity_agreement.rs` reproduces every identity of the pre-split record `crates/noble-ngraph-experiment/tests/fixtures/identities-66eb7cd.txt` (including B5), and `ngraph-phase-b --out <tmp>` at `bc34ed0` reported `result: passed` (8/8 records) with the B5 identities below; that run is a check of the cutover, not a new receipt.

## Certified-rewrite quality corpus (issue #64)

[rewrite-quality-v1.txt](rewrite-quality-v1.txt) is the frozen optimization-quality corpus for the
certified graph rewrites of `crates/noble-ngraph/src/cert.rs`: its schema is
`noble-rewrite-quality/v1`, it binds the one rule its proposers emit (`fold-add-constants`; the
identity rules are recorded only by `cert::emit`, which no proposer here uses, and the acceptance
test requires the bound set to equal the emitted set) and declares, per fixture, the
residual property its result must reach (`no-foldable-sites`). Its seven raw-graph fixtures name
independent and chained opportunities, ordered outputs, shared operands, an `i64::MAX + 1` wrapping
boundary and a constant in the right operand. Names are not executable-coverage evidence: the v2
admission diagnostic below found only one output in every fixture and two inadmissible sources.
Fixtures are frozen before any tuning, and the lane pins the
reviewed corpus itself (`REVIEWED_FIXTURES`): a fixture that disappears, is renamed, is relabelled
unsupported or has its declared nodes weakened fails the lane instead of improving coverage.

`cargo test -p noble-ngraph --test rewrite_quality` measures certificate validity and optimization
coverage **separately**, with four proposer controls over the same validator:

| Control | Measured |
|---|---|
| `Identity` | valid certificate over identical endpoints, residual equals the untouched site count — **correctness alone fails the quality target** |
| `FirstEligible` | valid certificate, one fold applied, sites left behind on every multi-site fixture (`independent-folds`, `ordered-outputs`) |
| `SinglePass` | valid certificate; reaches the target on fixtures whose folds all consume original constants |
| `FixedPoint` | reaches the target on a chained fixture too, but that derivation is **refused** `Malformed`: the certificate format binds every premise to the original graph, so a step consuming an earlier step's result is unsupported rather than silently accepted |

That last row is the honest boundary of this corpus: chained opportunities are *possible* (the
residual reaches zero under `FixedPoint`) and *uncertified* (the checker refuses it). No optimization
claim is made about them, and `SinglePass` passing the frozen fixtures is not evidence that a
one-pass proposer is sufficient in general.

### Historical cost accounting (v1; issue #64 items 5-7)

The quality lane above prices nothing. The original cost lane
(`crates/noble-ngraph/tests/rewrite_cost.rs`) produced the immutable
[rewrite-cost-2026-10-27.json](rewrite-cost-2026-10-27.json): schema
`noble-rewrite-cost-receipt/v1`, source `8ce33f961`, clean source, `rustc 1.96.0-nightly`
(2026-03-20), host `Linux 6.18.54 x86_64` (shared, load-dependent), 200 warm samples per row, 28
rows (seven fixtures x four proposers). The receipt also carries `lens`: the SHA-256 of each of the
25 `.rs` files the quality manifest declares as its lens crate (`noble-ngraph`, because that crate
is dependency-free and cannot hash itself), so the corpus binding names a source identity rather
than a path.

The continuation thresholds are **pre-registered** in
[rewrite-cost-v1.txt](rewrite-cost-v1.txt) before any measurement: retention requires the declared
residual target to be reached *and* the warm certificate-check cost per derivation step to stay
under 50 µs. The residual condition is a hard gate; the cost ceiling is reported per row as
`check_threshold_met` and is deliberately generous because this host is shared.

Measured, warm medians with interquartile ranges, 28 rows on a shared host:

| Quantity | Value |
| --- | --- |
| Proposal/search (`cert::propose`) | 11.25-17.56 µs per call (IQR at most 7.41 µs; shared-host medians are informational) |
| Certificate checking (`cert::check`) | 10.86-25.56 µs per call (IQR at most 10.64 µs) |
| Certificate check cost per derivation step | 7.82-16.48 µs, derived from `check_us / steps` for the 21 step-bearing rows; ceiling 50 µs; see erratum below |
| In-memory structural derivation footprint | 125 B (zero steps), 165 B (one step), 205 B (two steps) |
| Coverage | 28 attempts, 27 accepted, 1 refused (`Malformed`, the chained fixed-point derivation), 16 reaching the residual target |
| Residual target reached | `SinglePass` and `FixedPoint` reach zero sites on 6 of 7 fixtures; `FirstEligible` on the 4 single-site fixtures (`single-fold`, `shared-operand`, `wrapping-boundary`, `operand-order`) |

**2026-10-29 erratum (issue #72):** the original lane converted `check_us` a
second time when computing `check_per_step_us`, dividing by 1000 before rounding
to two decimals. The immutable `8ce33f961` receipt retains those incorrect
per-step values and threshold booleans; the earlier table's `0.01 µs` claim was
wrong. Recomputing from its retained `check_us` and `steps` gives the range above.
All 21 step-bearing rows remain below 50 µs, but that is a new arithmetic
observation, not validation of the old threshold calculation. Rounded values
in the bad column cannot simply be multiplied by 1000 to recover the originals.
The per-call ranges above also replace stale summary numbers with the actual
28 receipt rows. No historical receipt or measurement has been rewritten.
The corrected helper and deterministic unit/zero-step regression preserve the
documented microsecond field semantics in new reports.

**2026-10-29 checker change (issue #42):** every `cert::check` timing in this
document, in both cost lanes, was measured before `check` began replaying the
folds on a copy of the original and comparing the result graph with that replay.
The earlier `check` bound the result only by its digest and accepted a
substituted result that the producer re-digested. These timings therefore describe
the earlier checker and are not re-measured here. The accepted/refused coverage is
unaffected for the honest proposers: their results equal the replay, and the
`rewrite_cost` and `rewrite_quality` lanes still pass, with only a `result-mismatch`
label added to the cost lane's refusal names.

The wrapping-boundary and operand-order fixtures are new in this record: both reach the residual
target under every step-bearing proposer, and `Identity` still reaches none of it. They raised the
cost lane's denominator from 20 to 28 attempts, so coverage is reported against the larger corpus.

**2026-11-13 issue #42 remeasurement.** After the result-replay checker and Rust token codec landed,
the same frozen v2 plan and all 28 fixture/proposer rows were rerun from clean source commit
`8aef588c61455d3da4ff8cbb92118d537a26e707`. The fresh [receipt](rewrite-cost-2026-11-13.json)
retains its runner evidence alongside it. `cert::check` now replays each derivation against the
result endpoint. Per-call medians are 15.07–27.75 µs and per-step medians are 9.09–27.75 µs across
the 21 step-bearing rows (informational shared-host debug timings, not a retention or speedup claim).
The measurement now records `cert::encode` token-envelope lengths per
row: 28 B for zero steps, 42–78 B for one step (fixture-dependent decimal node/value token widths),
and 57–59 B for two steps. The corresponding structural in-memory estimates are 161, 201 and 241 B.
The v2 plan's `certificate_wire_bytes` pipeline stage remains **unmeasured**, as pre-registered:
these row fields measure encoded size only, not a timed or retained pipeline stage. Its historical
reason text and bytes are untouched. The v2 runner now treats `wire_bytes` as an optional extra
field, so previously retained v2 reports without it remain valid.

The comparison of the unchanged and certified paths is on identical fixture inputs: the identity row
carries **no steps** and reaches no target, so every certificate-bearing row pays check cost with no
measured saving. Retaining the baseline is valid, and nothing here shows the certified path pays for
itself.

At the time of these v1 measurements, **unmeasured, with the reason, never zero**: input admission,
certificate wire bytes (the Rust token codec had not yet been implemented; the structural footprint
above is not wire bytes), decoding/replay, candidate admission, schedule validation, backend
preparation, first execution, warm execution, retained memory (no allocator/RSS measurement is
installed). The later v2 row measurements below do not retroactively alter this historical report.
**Break-even is
unmeasured** because this lane never executes a program, so there is no per-run saving to amortize
check cost against. "Cold" is not reported separately: the library caches nothing across calls, so a
cold figure would price page faults rather than work.

Those v1 measurements did not contain an integrated total or a per-run saving.

### Integrated in-memory accounting (v2)

[rewrite-cost-v2.txt](rewrite-cost-v2.txt), committed as `e7ccedcbf` before integrated timings,
keeps every historical fixture and proposer. The runner revision bound by the v2 receipt measured
the real library pipeline, not a source compiler, Wasm executable, Lean replay or deployed BLAKE3 path:

- Baseline: exact source admission, packed preparation, actual-layout export/validation, first execution.
- Certified: the same source admission, proposal/construction, exact endpoint certificate check,
  exact candidate admission, packed preparation, actual-layout export/validation, first execution.
- No pruning or repair of certificate endpoints. A refusal stops subsequent stages; skipped durations
  are `null` with a reason, not zero. The directly timed refusal prefix includes cleanup. Stage medians
  are never summed into a purported observed total. Proposal and certificate construction share one
  non-additive measurement.
- Sixty-four alternating fresh priming pairs precede 200 alternating measured pairs. Fresh means no
  admitted graph, compiled artifact or workspace reuse, **not** OS-cache cold. Warm execution primes
  and reuses an admitted compiled object/workspace for 64 unrecorded calls followed by 200 samples.
  Every baseline/candidate execution call is counted, including priming. Semantic agreement is checked outside
  the timers; I64 output evidence is encoded as exact decimal strings.
- Both paths use identical empty fixture inputs and agree with the source reference. All elapsed
  values are informational debug-build, shared-host observations, including instrumentation overhead.
  Library graph/code size estimates and scratch leases are separately labeled; allocator/RSS and
  decoding remain unmeasured. The row-level `wire_bytes` field records the Rust token encoder's
  output length, but the frozen plan's end-to-end `certificate_wire_bytes` stage is unmeasured.
  Layout validation does not establish instruction-byte correspondence.

The direct admission diagnostic exposed a boundary hidden by the old certificate-only report:
`independent-folds` and `ordered-outputs` contain dangling nodes; every non-identity candidate is
refused by certificate checking or exact candidate admission. The unchanged 28-case corpus yields
8 source refusals, 1 certificate refusal, 14 candidate refusals and 5 executable identities.
Every refused candidate has zero execution calls. **No executable optimized candidate reaches a
quality target, so there is no measured optimization benefit or eligible break-even estimate.**
The historical 27 accepted certificates and 16 residual targets are not 27 executable optimizations.
In particular, the old `ordered-outputs` name does not discharge ordered/repeated-output coverage.
Those corpus and admitted-rewrite obligations keep #64 open; they are not bypassed to improve a score.

The [v2 receipt](rewrite-cost-2026-10-29.json) binds clean source `0a0592f1e` and the
pre-registered plan `e7ccedcbf`. Its sibling
[raw stdout](rewrite-cost-2026-10-29.json.evidence/raw/stdout),
[raw stderr](rewrite-cost-2026-10-29.json.evidence/raw/stderr), process record and source snapshots
are byte-identical retained copies; the receipt preserves their original absolute capture paths
and hashes. The executable itself is identified by path, SHA-256, byte count and mode, not checked
into the repository. The real runner completed all seven regression/report tests.

Measured over 200 samples per path after the stated priming, with medians and IQRs in every row:

| Observation | Result |
|---|---|
| Certificate-only coverage | 28 attempts, 27 accepted, 1 refused, 16 residual targets reached |
| Integrated coverage | 8 source refusals, 1 certificate refusal, 14 candidate refusals, 5 executable identities |
| Direct fresh baseline total, five executable identity rows | 13.405–15.389 µs |
| Direct fresh certified total, those same identity rows | 45.936–53.229 µs |
| Warm baseline execution, admitted rows | 0.241–0.271 µs |
| Warm certified execution, executable identities | 0.250–0.271 µs |
| Certificate-check cost per step, step-bearing rows | 6.21–13.54 µs; informational, not an execution acceptance criterion |
| Actual execution calls, including priming | 10,560 baseline; 2,640 certified; zero for refused candidates |
| Break-even | Unmeasured in all 28 rows; no executable optimized candidate reaches a quality target |

Every observed fresh/warm output agrees exactly with the admitted source reference. The wrapping
case retains `-9223372036854775808` as a decimal string. Scratch leases balance and retain zero live
bytes after each observed execution; this is not allocator/RSS accounting. Refused-prefix totals
remain in the receipt and are not compared as successful execution times. These shared-host debug
figures do not establish a production speedup or amortization benefit. An overwrite attempt returned
exit 2 without changing the receipt; a dirty-source attempt refused before Cargo execution.

To reproduce the historical v2 receipt, check out its source commit `0a0592f1e` and use that
revision's runner. The current runner is v3-bound; these v2 totals are not a fresh v2 rerun.

Run from a clean committed checkout, using the selected Node executable in `policy/tool-selection.json`:

```text
node verification/ngraph/rewrite-cost.mjs --output /absolute/outside-checkout/receipt.json
```

The output and sibling `.evidence` directory must not exist. The runner invokes pinned offline Cargo
directly with an isolated Cargo home/target and disabled Rust wrappers. It retains exact argv, raw
stdout/stderr, process status and hashes; checks commit/tree/status and source hashes before/after;
and validates all 28 rows and both coverage denominators. Dirty source refuses before Cargo runs.
Failed attempts remain failures in their own evidence directories; historical receipts are unchanged.

### Issue #64 v2 corpus and deferred ranker

The frozen [v1 corpus](rewrite-quality-v1.txt) is unchanged. The new
[v2 corpus](rewrite-quality-v2.txt) names every ordered output explicitly, including repeated
node indices. Its `independent-folds` outputs are `[2,5]`; `ordered-outputs` is `[4,2]`; and
`repeated-outputs` is `[2,2]`. The existing integration-test parser preserves that order and
multiplicity. The cost lane checks that all three graphs pass source admission and that reference
evaluation returns the corresponding exact output vectors. Other v1 node declarations and targets
are retained and pinned by the cost test.

[rewrite-cost-v3.txt](rewrite-cost-v3.txt) was pre-registered and committed as `d9b62eaf` before
measurement. It fixes the eight-fixture/four-proposer workload (32 attempts), inherited 50 µs/step
checker ceiling, execution and cost eligibility criteria, coverage denominators, and unmeasured
stages. The runner ([rewrite-cost.mjs](rewrite-cost.mjs)) binds the v3 plan and v2 corpus and rejects
missing rows or changed output vectors. Historical v1/v2 plans and receipts remain unchanged.

The certificate decision is to adopt `ngraph-cert/1` as a new revision with certified dead-node
removal (an explicit old-to-new node map) and sequential premises, while retaining
`ngraph-cert/0` and its pinned regressions. That implementation remains in #42's scope; this slice
does not edit `cert.rs`.

The learned/circuit ranker is **NO-GO / deferred**. The current lane has no independently admitted
non-identity rewrite, so ranking has no useful work to prioritize; with the present confluent,
fold-only rules, rewrite order does not change the result. Training a scorer or comparing rankings
now would manufacture a choice the accepted candidate universe does not provide. Reopen only when
all of these hold:

1. #42's new `ngraph-cert/1` supports certified dead-node removal with an explicit old-to-new node
   map and sequential premises, without changing the pinned `ngraph-cert/0` regressions.
2. The unchanged checker and candidate-admission pipeline independently admits at least one
   non-identity rewrite that reaches a frozen quality target.
3. A deterministic baseline exposes at least two eligible choices whose ordering can change useful
   work under the same frozen total-work budget; otherwise record NO-GO without training.
4. The comparison freezes the candidate universe, features and scorer identity before held-out
   evaluation; ranking may only reorder those choices. It cannot add rules, alter premises, repair
   endpoints, weaken refusals, raise budgets or bypass certificate/admission/layout checks.

Training and circuit search remain in RSI Lab #73; the optional LLVM comparison in compiler-lab
#176 is outside Noble's scope. This deferral is not an implementation of either consumer.

### Issue #64 v3 clean-source measurement

The [v3 receipt](rewrite-cost-v3-2026-11-13.json) binds the clean source and plan commit
`d9b62eaf64091c26808eb21592c346d9fa9c4c98`; its source tree is `0bfb0e47cf20e8db6ad2e349859eb5595fbae7ce`.
The sibling `.evidence` directory retains matching [source-before](rewrite-cost-v3-2026-11-13.json.evidence/source-before.json)
and [source-after](rewrite-cost-v3-2026-11-13.json.evidence/source-after.json) snapshots, the
[process record](rewrite-cost-v3-2026-11-13.json.evidence/process.json), [raw stdout](rewrite-cost-v3-2026-11-13.json.evidence/raw/stdout),
[raw stderr](rewrite-cost-v3-2026-11-13.json.evidence/raw/stderr) and selected Cargo config; the receipt
records their hashes. The runner completed all 32 fixture/proposer rows (64 unrecorded priming
iterations and 200 samples per row).

| Observation | Result |
|---|---|
| Source admission | All 8 fixture graphs admitted; explicit-output reference results include `[12,7]` for `independent-folds`, `[12,5]` for `ordered-outputs` and `[12,12]` for `repeated-outputs`. |
| Raw certificate coverage | 32 attempts, 31 accepted, 1 refused, 19 residual targets reached. |
| Integrated pipeline | 0 input refusals, 1 certificate refusal (`chained-folds`/`fixed-point`), 23 candidate-admission refusals, 0 layout/execution refusals, 8 executed rows. |
| Executed candidates | All 8 are `identity`; no non-identity candidate executed. The 23 candidate refusals are `E-GRAPH-MALFORMED` for dead operand nodes reaching no output and no guard (`NodeId(0)` or `NodeId(1)`). |
| Execution calls | 16,896 baseline calls; 4,224 certified calls; refused candidates receive zero execution calls. Calls include priming and measured fresh/warm execution. |
| Fresh full-attempt medians across executed identity rows | Baseline 33.262–45.165 µs; certified 105.486–145.221 µs. |
| Warm-execution medians across executed identity rows | Baseline 0.421–0.752 µs; certified 0.431–0.761 µs. |
| Certificate-check cost per step | 10.54–33.14 µs for step-bearing rows, below the 50 µs/step informational ceiling. |
| Break-even / optimization benefit | Unmeasured: no executed non-identity candidate reaches the residual target; the raw target-reaching proposals were refused at candidate admission. |

The source, baseline and certified outputs agree exactly on all 8 executed identity rows, including
wrapping `-9223372036854775808` and the ordered/repeated output vectors. Timings are debug-build,
shared-host medians after priming, not OS-cache-cold; they do not establish a production speedup.
Wire encoding and retained memory (allocator/RSS) remain unmeasured.

Reproduce from a clean committed checkout, using the selected Node executable in
`policy/tool-selection.json`:

```text
node verification/ngraph/rewrite-cost.mjs --output /absolute/outside-checkout/receipt.json
```

The runner refuses dirty source or pre-existing output/evidence paths and retains the raw process
streams and source snapshots beside the receipt. This v3 result leaves the executable-optimization
item open pending #42's `ngraph-cert/1`; it does not close #64.

### Issue #64 effects-as-laws controls

`crates/noble-kernel/tests/acceptance/rewrite/effect/laws.rs` adds controls to the existing kernel
acceptance target. The test harness accepts the original first, then constructs the candidate's
expected stack interface and allowed effect bound from that original `Checked.interface`; it carries
the original declared `stack_height` limit forward. A candidate cannot supply these expectations.
The controls show that an added `test.emit` is refused by effect inclusion, while a dropped effect
kind can pass the kernel's upper-bound inclusion check but is refused by an independent exact-set
equality law, even when both accepted interfaces return `Unit`. Changed output types and exceeding
the original stack-height limit also refuse before execution. A counterfactual candidate-shaped
request demonstrates why using the candidate's own output claim would be unsound.

These are acceptance/interface controls, not runtime semantic equivalence tests. Effect bounds are
idempotent upper sets of effect identities: exact equality detects dropping an identity, not
dropping an occurrence. Allocation, hidden state, caching and runtime depth are not laws represented
by this kernel interface, and no guarantee about them is made.

## Files

| File | Content |
|---|---|
| [net-decision.md](net-decision.md) | Revision-3 #37 GO/NO-GO against the frozen net-continuation criterion, including the complete #117 run, all-row reference coverage and the end-to-end construction cost. |
| [phase-b-extract-net-2026-11-13.json.gz](phase-b-extract-net-2026-11-13.json.gz) | Losslessly compressed, unedited receipt from source revision `ee34d988390a904080f137a9ec321cfe3108f66a`; adds the revision-3 net differential leg to B2 while retaining the B1 and B3 records. `gzip -t` passes. |
| [phase-b-extract-net-2026-11-14.json.gz](phase-b-extract-net-2026-11-14.json.gz) | Losslessly compressed, unedited B1/B2/B3 receipt from source revision `9b07d413121f6109e2b2226f1641a56be08d1a84`; the debug CLI's revision-3 net leg passes on all recorded vectors, alongside L1 `--opt off/on`. `gzip -t` passes. |
| [phase-b-core.json](phase-b-core.json) | Core receipt (schema `noble-ngraph-phase-b-core-evidence/v1`): BE-CONFIG-01 run fields and one EV-STATE-01 record per item, each with `implementation`, `execution`, `proof`, `trust`, open obligations, trusted boundaries and its result. |
| [phase-b-extract.json](phase-b-extract.json) | `I64` extraction-leg receipt (schema `noble-ngraph-phase-b-extract-evidence/v1`): BE-CONFIG-01 run fields (including the pinned CLI and Node with their SHA-256) and the EV-STATE-01 records `B1-I64-lowering`, `B2-I64-differential` and `B3-I64-source-controls`. |
| [phase-b-extract-2026-11-12.json](phase-b-extract-2026-11-12.json) | The same extraction leg rerun at source revision `83ff2654` with a release `noble` built from that commit (SHA-256 `6d7a4b1d…60c8`), after B2 began requiring every L1 program to have at least one agreement and every L1 vector to be an agreement or a recorded quota outcome. All three records pass. L1 agrees on 562 of 562 vectors in both `--opt` legs, with 0 quota outcomes and 3 session restarts; `random-1`, whose 24 vectors the `eccbc02` CLI of `phase-b-extract.json` refused as `exhausted`, agrees on all 24, as the 2026-10-30 re-check found for newer CLIs. Under the current rule the 2026-10-15 run would record `B2-I64-differential` as failed, because `random-1` had no ordinary-path agreement. `phase-b-extract.json` stays the baseline that `l1-attribution-2026-10-30.json` binds. |
| [l1-attribution-2026-10-30.json](l1-attribution-2026-10-30.json) | L1 attribution re-check for issue #75 (schema `noble-ngraph-l1-attribution-evidence/v1`), produced by [l1-attribution.mjs](l1-attribution.mjs). The hardened adapter ran against two real CLIs, in both `--opt` legs; their 69 L1 and B3 ordinary-path records are compared with `phase-b-extract.json`. The receipt also holds the rule-mutation result of [l1-attribution-mutations.mjs](l1-attribution-mutations.mjs). |
| [test-log.txt](test-log.txt) | `cargo test -p noble-ngraph-experiment` output (default suite, debug) at the receipt's source revision. Historical: it predates the `numeric-graph-v1` split. |

The `I64` extraction leg covers `I1`–`I4` extracted from Core source by D7, L1 (the ordinary Noble Wasm/V8 path through the real CLI), L2 BigInt, L3 reference, L4 packed, L5 revision-3 net, and the source-level B3 controls. The core receipt covers the `F32` legs and the authored-IR controls.

## Reproduce

From the repository root, inside the pinned devShell (`rust-toolchain.toml`, nightly-2026-03-21):

```text
cargo test -p noble-ngraph -p noble-ngraph-experiment
cargo run --release -p noble-ngraph-experiment --bin ngraph-phase-b -- \
  --out verification/ngraph/phase-b-core.json \
  --source-rev "$(git rev-parse HEAD)" --toolchain "$(rustc -V)"
cargo run --release -p noble-ngraph-experiment --bin ngraph-phase-b-extract -- \
  --cli /abs/noble --node "$NOBLE_NODE/bin/node" \
  --out verification/ngraph/phase-b-extract.json \
  --source-rev "$(git rev-parse HEAD)" --toolchain "$(rustc -V)"
```

Core defaults: 8 random graphs × 20,000 input vectors (48 operations each), 10^6 operand pairs per operation, 2,000 gradient samples, 500 training steps. The whole run took 0.66 s of wall time; the default test suite runs in about 1 s in debug.

Extraction-leg defaults: seed `0x1404b201`, 64 random Core programs × 10,000 vectors, L1 on 16 of them × 24 vectors, `I4` for 1,000 host steps. `--cli` is a release `noble` built from `origin/main` (the receipt used `noble-integration-eccbc02-release`, whose SHA-256 it records). The run took 232 s. The L2 BigInt smoke in `extract_b2.rs` is `#[ignore]` because it needs `NOBLE_NODE`; it was explicitly run for this slice and passed (8 seeded programs × 1,000 rows, plus the boundary fixtures). Reproduce it with `NOBLE_NODE=/absolute/path/to/pinned-node-installation cargo test -p noble-ngraph-experiment --test extract_b2 --offline --locked l2_node_bigint_oracle_agrees_with_l3_l4_l5 -- --ignored --exact`.

Set `TMPDIR` to a directory that no other principal can write, and whose ancestors no other principal can write either. The B3 live-slot controls write their authority file there, and `noble live slot` refuses an authority file or ancestor that another principal can write. With the world-writable `/tmp`, both live-slot controls record `refused`.

The L1 attribution re-check (issue #75) reruns the extraction leg with `--vectors 200`. That option sizes only the L2–L4 campaign: L1 still sees the same 16 programs × 24 vectors and the same 1,000 transition steps.

Run the leg once per CLI. The receipt used a release CLI built from `eccbc02` (the baseline's CLI revision) and a CLI built from the adapter's own revision. Then build the receipt from a clean tree:

```text
node verification/ngraph/l1-attribution-mutations.mjs --cargo "$(command -v cargo)" --out /abs/mutations.json
node verification/ngraph/l1-attribution.mjs \
  --baseline verification/ngraph/phase-b-extract.json --baseline-cli-revision "$(git rev-parse eccbc02)" \
  --run cli-eccbc02 /abs/extract-eccbc02.json "$(git rev-parse eccbc02)" \
  --run cli-head /abs/extract-head.json "$(git rev-parse HEAD)" \
  --mutations /abs/mutations.json --out verification/ngraph/l1-attribution-YYYY-MM-DD.json
```

The mutation runner edits `crates/noble-ngraph-experiment/src/l1/mod.rs` in place, one rule at a time. It restores the file and verifies the restore by SHA-256, including on a signal.

## Run (from the receipt)

- Source revision `f5b9ce5180350796dc11474b3007b003b90c40de`, `rustc 1.96.0-nightly (ac7f9ec7d 2026-03-20)`, release profile (overflow checks on), target x86_64-linux.
- Host: AMD RYZEN AI MAX+ 395, 32 logical CPUs, **shared**: load average 4.72/5.12/5.72 at start and 4.66/5.10/5.71 at end. Timings in the receipt are informational only; Phase B makes no performance claim.
- Oracle `rustc_apfloat =0.2.3+llvm-462a31f5a5ab`; PRNG `splitmix64/0`; identities BLAKE3 (`blake3 =1.8.7`, pure).
- Wasm features (GC, function references, multi-value, tail call, component model): not applicable, because these legs execute no Wasm. Hardware counters: unsupported (not reported as zero).
- Extraction leg: source revision `f5806a0f2dec22c59ad53d5b446a8c4db800e32c`, same toolchain, profile and host (load average 4.89–7.57). L1 runs `noble session --framed --opt off|on` of the pinned CLI (SHA-256 `f7f02dfb…85ec`); its Wasm features are fixed by that runtime (managed-linear-memory backend; `--opt on` runs `wasm-opt --enable-gc --enable-reference-types --enable-bulk-memory -O2 --strip-debug`). L2 is a BigInt modulo-2^64 oracle under Node v24.13.0 (SHA-256 `ad2d5c03…8f3b`). Extraction budgets are the `/0` defaults (work units 4,194,304, expanded nodes 262,144, inline depth 64).

## Results

| Item | Record | Execution | Result |
|---|---|---|---|
| B2 (revision-3 net, 2026-11-13) | `B2-I64-differential` | passed | 15 named graphs (120,003 vectors) and 64 seeded random Core programs × 10,000 vectors (640,000): all 760,003 L2–L5 comparisons agree with 0 mismatches. L1 agrees on 685/685 vectors in both `--opt off` and `--opt on`, with no ordinary-path quota outcomes. The same receipt's B1, B2 and B3 records pass. |
| B1 (extraction) | `B1-I64-lowering` | passed | `I1`–`I4` are prepared by the ordinary Core frontend, re-accepted by the kernel and extracted to checked `ngraph-i64/0` graphs (8, 4, 5 and 5 nodes) that give the design results 24, 44, 25 and (13, 13) on L3 and L4. `I2` reads its one shared capture (`2`) twice. `I3` calls the same `sq` identity twice. `I4` runs 1,000 host steps on L3 and L4 and matches `s·k+i`. |
| B1 (core) | `B1-core F1-F3` | passed | `F1` (2→3→1 ReGLU, 22 params), `F2` (transition, 1,000 host steps) and `F3` (`index` over `[a, a, b, -0]` under all 9 rule pairs) pass the checker and agree on L2, B2-ORACLE, L3 and L4. |
| B2 (extraction) | `B2-I64-differential` | passed | L2 (BigInt), L3 and L4 agree on 10 named graphs (`I1`–`I4`; operand order `5 3 -`; overflow `MAX 1 +` and `MIN -1 *`, each as literals and over inputs) and 64 seeded random Core programs (up to 57 live nodes, inline depth 4, 349 pruned nodes in total), × 10,000 vectors each, with 0 mismatches. L1 ran 26 graphs (the 10 named plus 16 random) on 562 vectors per mode: with `--opt off` and with `--opt on`, 538 agree, 0 disagree, and 24 are ordinary-path quota outcomes. Those 24 are every vector of one random program (`random-1`): the CLI refuses its root at acceptance as `exhausted` ("independent source compilation refused"), while D7 extracts it. They are reported as a separate limit class (BE-LIMIT-01), not as agreement. `I4` runs 1,000 host steps through L1 in both modes, and every step agrees. |
| B2 (core) | `B2-core boundary vectors` | passed | All 14 vectors match the expected bits on every leg: reassociation `2^24`, FMA `+0`, lone `-0`, `relu(-0)=+0`, `reglu(-1,0)=-0`, `select`/`index` copy `-0`, `2^127·2=+∞`, `reglu(∞,0)=NaN`, NaN candidate, `2^-126·0.5=2^-127`, repeated reads `2a`. Index boundaries `-1, 0, n−1, n, ±(2^24−1)` agree under every rule. A simulated FTZ backend and a fused evaluation are both detected. |
| B2-ORACLE | `B2-ORACLE double rounding` | passed | 10^6 random pairs per operation: `rn32(x op64 y)` = APFloat = host binary32 for `+`, `−`, `×`, with 0 mismatches. This is finite evidence for the D3 inference, not a proof. |
| B2 (core) | `B2-core seeded random F32 graphs` | passed | 8 graphs (seeds 1–8, 55–57 nodes, 7–12 layers) × 20,000 vectors. 0 disagreements across 160,000 vectors: 41,784 `Normal`, 81,075 `GuardFailed` (first failing guard in canonical order matches) and 37,141 `E-FACT-INPUT`. Outputs included 86,172 NaN, 141,086 zero, 11,318 subnormal and 51,204 infinite values. |
| B3 (extraction) | `B3-I64-source-controls` | passed | 16 source-level controls, each yielding its D6 code with 0 graph nodes evaluated: `test.emit` and `test.emit` inside `run` (`E-EFFECT`); a `Resource` input and a `LiveRef` input (`E-RESOURCE`); live-slot dispatch (`E-LIVE-SLOT`); `run` of a Program input (`E-DYNAMIC-RUN`); self and mutual recursion (`E-RECURSION`, or `E-SUBJECT` when the subject's dependency record states the cycle); `reflect` (`E-REFLECT`); `if`, `case` and `list.case` (`E-CONTROL`); a Program output (`E-PROGRAM-OUTPUT`); and the `k = 20` chain `dₖ = [ dₖ₋₁ dₖ₋₁ ]` under the root `0 d20 drop 7` (`E-BUDGET-EXPANSION`, observed 262,145 > 262,144 although the result is dropped). Each control also records what the ordinary path does with the same source. |
| B3 (core) | `B3-core rejections on authored IR` | passed | 35 controls, each yielding its D6 code. Facts: caller-asserted range/grid/bit/one-hot, a missing fact, an underivable `derived` claim and an `evidence` fact (P5) give `E-FACT-UNCHECKED`; a violating input gives `E-FACT-INPUT`; `0.5` held in a bit gives `G-FACT-FALSE`. Indices: `2.5`, a missing integral fact, a range reaching `2^24`, out of range under `refuse`, NaN under a guard. Malformed IR: cycle, forward reference, type mix ×2, dangling node, unknown node, unknown op, inexact `0.1`, NaN constant ×2, infinite coefficient, duplicate slot, `param` in `i64`, unknown sub-profile. Budgets: live nodes, layers (lowered and the 16,384 default), 4,097 affine terms, 65,537 table entries, raising a budget …
| B4 | `B4 gradients (F1 + MSE)` | passed | 2,000 points (seed `0xb40001`), 48,000 slot checks each. G1 against central differences: worst error 0.014 of the tolerance. G2 against G1: worst error 0.096 of the bound; 47,604 checks also meet the plain `|g1|`-relative bound. Corner, boolean (`E-DERIV-BOOLEAN`, straight-through, clamped-linear), index (candidate `a ← 2g`, reject, zero, backward-difference surrogate), `E-DERIV-I64` and `E-PARAM-UNDECLARED` are separate tests in [b4_gradients.rs](../../crates/noble-ngraph-experiment/tests/b4_gradients.rs). |
| B5 | `B5 training F1 with adam/0` | passed | Seed `0x14140005`, data seed `0x1414da7a`, 64 points (digest `7c40293c…0692`), 500 steps. MSE `0.30594` → `0.00052` (0.17% of initial). `GraphLogicalId` `6065e93f…d661` is unchanged, and so are the IR text and the data. `S₀` evaluates identically on all 64 points. Replay reproduces snapshot `075f58ca…7f9d`. Refusals: wrong graph `E-SNAPSHOT-STALE`, permuted layout `E-SNAPSHOT-LAYOUT`, stale revision `E-SNAPSHOT-STALE`. Old-subject evidence does not apply to the captured subject (`OtherSubject`). An infinite target gives `E-NONFINITE-UPDATE` with 0 published steps. |
| B6 | `B6 ownership and cleanup` | passed | 26 evaluation cases (cancellation at each of the 9 layers of `F1`, fuel exhaustion, guard failure, preflight refusal, normal return, each on L3 and L4) show scratch acquired = released and 0 live bytes; preflight refusals acquire none. Snapshot owners release in both orders. Training cancelled before step 4 publishes nothing new (last = the 3-step snapshot; leases 3/3/0). Retained digests are unchanged. |

## Investigations

- **G2 bound scale.** The first G2 run measured the error relative to the reverse pass over |forward values|, and 20 of 1,440 slot checks failed (worst ratio 3.6). The cause was cancellation in `d = y − t` near the target: the FP32 error of `d` scales with `|y|+|t|`, not with `|d|`. The bound `4·depth·2^-24` is now relative to the absolute-value magnitude of the whole computation (forward `|a|+|b|` and `|a|·|b|`, reverse over |local derivatives|). This is the standard first-order rounding scale, and the receipt also counts checks that meet the plain `|g1|`-relative bound. The tolerance itself was not changed.
- **Sensitivity.** Reversing the `affine` term order in L4 made the boundary-vector and random-graph tests fail. This was a manual mutation check during development; the mutation is not committed.

## Extraction-leg deviations and interpretations

These are differences between the design text and what the extraction leg executed. None changes a D6 code's meaning or a B-item's expected result.

- **No `(block …)` syntax.** Current Core source has no `(block IN OUT [ body ])` form. Each root body is prepared by the ordinary frontend with the block's input types on its stack, which is what `noble session` submits after pushing those values. `sq = [ dup * ]` is written `def sq [ dup * ]`.
- **Stage of three refusals.** D7 places `E-DYNAMIC-RUN` (a `run` of an unknown Program) and `E-PROGRAM-OUTPUT` in symbolic execution (stage 2). The implementation raises both at admission stage 1(c), because a Program-typed value in the root's input or output interface already decides them. `E-LIVE-SLOT` is raised while the closure is recomputed (stage 1(b)), because a live-slot dispatch has no static closure.
- **Recursion is unreachable from Core source.** The Core frontend refuses recursive definitions ("recursive definitions require a signature and are outside Core-Bootstrap"), so the recursion controls are minimally forged, kernel-accepted submissions. `E-RECURSION` comes from admission stage 1(b), from a depth-first search over definition identities, rather than from the named-call expansion stack. When the forged subject's `Env::deps` also records the cycle, the kernel's own acceptance refuses the body first, and admission reports `E-SUBJECT`. Both variants are recorded.
- **The `k = 20` chain is hand-assembled.** The Core frontend stops the chain at `d10` with an `exhausted` "frontend node limit exceeded". The `k = 20` subject is therefore assembled from the frontend's `d0` submission, with one shared row per level, and accepted by the kernel. The ordinary path's own outcome on the same source is recorded next to it.
- **L1 runs in chunked sessions.** A Core session retains every prepared module, so long runs use fresh session processes of at most 240 submissions, re-committing the definitions in each. A transition carries its state across sessions on the host side. A quota outcome caused by accumulated session state restarts the vector in a fresh session and is counted (`session_restarts_after_ordinary_quota`); it is not an observation of the root.
- **L1 reports are attributed before they count (issue #75).** The adapter now validates each line from `noble session --framed` before it counts:
  - The schema, profile and backend must be the expected ones.
  - The session-local `submission` ordinal must equal the line's position. A fresh process, after a quota restart or at a chunk boundary, starts again at 1.
  - A runtime report must carry its status, stack, session state and module record. The record must name the same ordinal, the SHA-256 of the exact submitted text, and the `--opt` leg.
  - A definition never reports `normal`, and an input or root never reports `defined`. So a replayed root report, or an input report in a root position, is rejected even when its value is equal.
  - The exit status must equal the maximum of the CLI's per-outcome statuses.
  - Fewer reports than submissions, or undelivered input, is legitimate only after a terminal outcome.
  - Extra lines, lines after a terminal outcome, and lines over 1 MiB are rejected.

  Any violation kills and reaps the child. B2 then records the failure as its own class, `session_failure`, with its class (protocol, process or transport), its reason, the validated count, the last 32 reports, and the child's exit code or signal. It is a failed check, never agreement and never a numeric disagreement. A fresh, correctly attributed report with a wrong value is still accepted, so the numeric comparison stays separate.

  `phase-b-extract.json` predates these checks. The 2026-10-30 re-check reproduces all 69 of its L1 and B3 ordinary-path records with a CLI built from `eccbc02`, with no session failure. With a CLI built from the adapter's own revision, 65 of 69 records are identical. The 4 that differ are random programs 1 and 11 in both legs: 24/24 agreement instead of 24 static `exhausted` outcomes, and 1 restart instead of 23. These are CLI-version changes, because the `eccbc02` run reproduces them exactly.

  The checks bind reports to the submitted sequence and to the self-reported module identity. They do not attest a malicious child or the semantics of the pinned CLI or engine.
- **Codes for unspecified cases.** Where the design names no code, the leg chose one and states the reason in the diagnostic detail. D7 1(c) requires `P` scalars in the root interface but names no code: a Program-typed root input gives `E-DYNAMIC-RUN` and a Program-typed root output gives `E-PROGRAM-OUTPUT`. D5 says raising a `/0` budget needs a new profile policy revision but names no code: the request is refused with `E-PROFILE`.
- **Subject identity is a producer digest.** The subject digest in `GraphLogicalId` is BLAKE3 over the kernel's `Debug` rendering of the subject. It identifies bytes; it is not independent validation.
- **Only L1 checks extraction against source.** L2 (BigInt), L3 and L4 all evaluate the *extracted* graph. L1 is the only leg that observes the source independently of extraction, and it covers a recorded sample (562 vectors per mode and the 1,000-step `I4` transition), not every vector.

## Assumptions and boundaries

- `F32` graphs are authored IR (B0(b)). There is no ordinary-execution leg for `F32`, only independent references (Risks in the design).
- B2-ORACLE computes in binary64 and rounds once with Rust's `as f32`, which is RNE, the same rounding as `Math.fround`.
- Trusted boundaries: the rustc/LLVM codegen (no contraction), the host CPU's IEEE behaviour (no FTZ/DAZ), `rustc_apfloat` as the L2 oracle, BLAKE3 for identities (a hash identifies bytes), and this crate's checker, evaluators and trainer.
- B6 instrumentation uses scratch-lease counters, not a global allocator: a counting allocator needs `unsafe`, which the workspace forbids.

## Non-claims

- Finite tests and evaluator agreement do not prove the compiler, the graph checker (NG-PO-B), the compiled evaluator (NG-PO-C), the binary32 model (NG-PO-D), fact transfer (NG-PO-E), the derivative policy (NG-PO-F) or snapshot binding (NG-PO-G). NG-PO-A..H remain open and are not added to `obligations.json`.
- **Established slices, not discharges.** Three obligations now carry a recorded slice while staying open (`specs/STATUS.json` → `numeric_graph.proof_slices`):
  - **NG-PO-C** — the Lean validator is checked: `PackedSchedule.validate` with `validate_sound` over every well-formed graph, the kernel checking two real packed emissions in `PackedScheduleCorpus.lean`, and `schedule::check` tested in `crates/noble-ngraph`. No theorem relates the Rust packed executor to `run`, so the compiled-evaluator obligation stays open.
  - **NG-PO-D** — `NobleFoundation.Binary32` is a proved kernel-reducible model, the 26 retained expectations replay through the packed reference and the compiled legs, and the `sem.rs` correspondence is a reviewed trusted, tested assumption on `x86_64-unknown-linux-gnu` (issue #38). Correspondence is not proved; the obligation stays open.
  - **NG-PO-F** — the real-valued reverse-AD model is proved (transpose, adjoint), the ordered binary32 model is proved, and `grad.rs` is tested against it on 306 generated cases. No refinement of `grad.rs` exists; the obligation stays open.
- B5's threshold is a smoke test, not a convergence claim. Replay is shown on this platform only.
- No performance, GPU or backend-selection claim; that is Phase C.

# Phase C (C1–C3) measurements

The design's Phase C plan was approved on 2026-10-15 with the C3 thresholds kept.

## Files

| File | Content |
|---|---|
| [phase-c-core-run1.json](phase-c-core-run1.json) | Primary run (schema `noble-ngraph-phase-c-core-evidence/v1`). It records run pins, W3 (3 sizes), W4, the W1/W2 CPU legs, C2 claim separation and the C3 decision. |
| [phase-c-core-run2.json](phase-c-core-run2.json), [phase-c-core-run3.json](phase-c-core-run3.json) | Back-to-back repeats with the same command. They show run-to-run stability; the key ratios are given below. |
| [phase-c-l1.json](phase-c-l1.json) | W1/W2 from Core source (schema `noble-ngraph-phase-c-l1-evidence/v1`). It records extraction timing per D7 stage, L3/L4 on the extracted graphs, host-native, and the ordinary Noble Wasm/V8 baseline (L1, `--opt off` and `on`), with run pins and the CLI SHA-256. |

## Reproduce

```text
cargo build --release -p noble-ngraph-experiment --bin ngraph-phase-c
taskset -c 2 target/release/ngraph-phase-c --out verification/ngraph/phase-c-core-run1.json \
  --source-rev "$(git rev-parse HEAD)" --toolchain "$(rustc -V)" --pinned-cpu 2
cargo build --release -p noble-ngraph-experiment --bin ngraph-phase-c-l1
target/release/ngraph-phase-c-l1 --cli /abs/noble --out verification/ngraph/phase-c-l1.json \
  --source-rev "$(git rev-parse HEAD)" --toolchain "$(rustc -V)"
```

The harnesses are separate release binaries and are not part of the default test suite. The workload shapes, and bitwise agreement between each native baseline and L3/L4, are default tests in [c1_workloads.rs](../../crates/noble-ngraph-experiment/tests/c1_workloads.rs). One `ngraph-phase-c` run takes about 5 s. One `ngraph-phase-c-l1` run took 2.7 h, almost all of it W2 through L1 with `--opt on`.

## Run conditions

- Source `2358a74730d5fc14888c6a1c10085e6b0ed69575`; `rustc 1.96.0-nightly (ac7f9ec7d 2026-03-20)`; release profile (overflow checks on); x86-64 default target features, so no FMA instructions, and rustc does not contract.
- AMD RYZEN AI MAX+ 395 (32 logical CPUs), with the process pinned to CPU 2 using `taskset`. Governor `performance`, driver `amd-pstate-epp`.
- The host is **shared**. The load average was 6.3–8.0 on 32 CPUs across the three runs, which is the lowest observed during this session. Timings are wall-clock (`Instant`). Hardware counters, RSS, cache and energy are recorded as **unsupported**, not as zero. Byte figures are element-count estimates.
- Warm latency: N = 2,000 samples, each a calibrated batch of at least 20 µs divided by the batch size, so the values are per-evaluation medians and IQRs. Cold latency is the median of 21 repetitions of check + compile + first evaluation. Throughput is a 0.3 s loop. Training is measured as 15 points per step.
- Before any timing, the host-native baselines agree with L3 and L4 under `≡obs`: on 200 input vectors per W3 size, including random specials; on 200 W4 index vectors, including out-of-range refusals; on the W1 value; and on the W2 final state after 10^5 steps.

## C1 results (run 1)

Runs 2 and 3 match run 1 within ±10%. The exceptions are sub-0.05 ms cold figures (W3-S admission 0.036 ms in run 1 versus 0.025 ms in runs 2–3) and the W3-S native training step in run 2 (1.17 µs). Both vary by up to 30% on the shared host.

| Workload | Live nodes / depth | Admission (check) | Build (compile) | Cold total | Warm L4 median [IQR] | Warm L3 | Warm native | L4 ÷ native speed |
|---|---|---|---|---|---|---|---|---|
| W3-S (4→4→1, 45 params, 76 ops) | 125 / 12 | 0.036 ms | 0.005 ms | 0.049 ms | 332 ns [6] | 843 ns | 22.7 ns | 0.068× |
| W3-M (4→22→1, 243 params, 418 ops) | 665 / 30 | 0.121 ms | 0.014 ms | 0.160 ms | 1,468 ns [19] | 3,858 ns | 89.2 ns | 0.061× |
| W3-L (W3-M × 15 points, 243 params, 6,270 ops) | 6,573 / 30 | 1.097 ms | 0.221 ms | 1.358 ms | 9,711 ns [70] | 34,604 ns | 1,271 ns | **0.131×** |
| W4 (32 reads of a 10,000-entry table) | 10,065 / 3 | 9.80 ms | 1.54 ms | 11.35 ms | 706 ns [13] | 38,562 ns | 128 ns | 0.182× |
| W1 = `I1` (hand-built IR) | 8 / 4 | — | — | 4.2 µs | 65 ns | 76 ns | 0.70 ns | 0.011× |
| W2 = `I4`, 10^5 host steps (total) | 5 / 3 | — | — | — | 7.48 ms | 8.30 ms | 0.134 ms | 0.018× |

- **Bytes** (params / L3 activations / L4 register file / retained graph / retained compiled code):
  - W3-S: 180 / 2,000 / 504 / 12,318 / 2,864.
  - W3-M: 972 / 10,640 / 2,664 / 64,554 / 15,032.
  - W3-L: 972 / 105,168 / 26,296 / 631,890 / 179,560.
  - W4: 0 / 161,040 / 40,264 / 2,252,028 / 2,721,604. L4 copies the table operand list into each of the 32 index instructions; this is a measured inefficiency of the current L4.
- **Throughput** (evaluations/s, L4 / L3 / native):
  - W3-S: 2.99 M / 1.19 M / 41.8 M.
  - W3-M: 0.68 M / 0.25 M / 11.0 M.
  - W3-L: 0.10 M / 0.028 M / 0.75 M.
  - W4: 1.38 M / 0.025 M / 7.7 M.
- **Training step, 15 points** (ours = FP32 reference backward + `adam/0` + immutable snapshot publication; native = hand-written FP32 backward + binary64 Adam, updated in place):

  | Size | Ours | Native | Ours slower by |
  |---|---|---|---|
  | W3-S | 18.1 µs | 0.92 µs | 19.7× |
  | W3-M | 80.5 µs | 4.87 µs | 16.5× |
  | W3-L | 60.0 µs | 4.84 µs | 12.4× |

  Backward scratch estimates: 1,032 / 5,352 / 53,176 B.
- **Extraction time** does not apply to W3/W4: they are authored IR under B0(b), and graph authoring took 0.03–0.48 ms. For W1/W2, see the next section.
- **Ordinary Noble Wasm/V8 baseline**: none exists for W3/W4, because Noble's ordinary path has no `F32` under B0(b). For W1/W2, see the next section. The W1/W2 rows above use hand-built IR, which has the same operations and outputs as the extracted `I1`/`I4` graphs (`phase-c-l1.json` checks this).
- **Pinned handwired under pinned Bun** (optional, informational): not run.

## C1 W1/W2 from source against both baselines (`phase-c-l1.json`)

Run conditions: source `e60d0d86706de8d11d3aa2deced631c89748766c`, same toolchain and profile. The CLI was `noble-integration-eccbc02-release` (`origin/main` code, SHA-256 in the receipt). The process was **not** pinned, because L1 spawns multi-threaded Node/V8 and wasm-opt processes. The host was more heavily loaded than for the core runs: load average 21.2 at start and 19.8–21.4 at end, on 32 CPUs. As a result, the CPU-leg figures here are 1.5–2.7× slower than the pinned core run above (W1 L4 137 ns versus 65 ns; W2 L4 20.6 ms versus 7.48 ms). The ratios below compare legs from the same run. W1 ran `--opt off` and then `--opt on`; the two W2 modes ran concurrently.

Agreement before timing: every L1 observation is compared with L4 on the extracted graph. W1: 1,000 vectors per mode (the design vector `(5, 4)` plus 999 `splitmix64/0` vectors, seed `0x1414c101`), and all 1,000 agree in both modes. W2: all 100,000 steps agree in both modes, and the final state `-6083400928511612821` equals L3, L4 and native. There were no quota outcomes or session restarts.

- **Extraction** (median of 21; W1 / W2): frontend prepare 140 / 148 µs; admission (kernel re-check) 53 / 60 µs; extraction including the graph checker 7.6 / 9.1 µs; L4 build 1.3 / 1.3 µs; source to first L4 result 206 / 232 µs. The extractor counters for W1 are 8 work units, 8 expanded nodes, 0 pruned and inline depth 0. Live nodes / depth: 8 / 4 and 5 / 3.
- **L1 cost model.** `noble session` prepares, compiles, instantiates and runs every submission, so one L1 evaluation is one root submission, and one W2 host step is an input submission plus a root submission. The receipt times both from report-line arrival. These are the costs of that host interface, not of a call into an already instantiated Wasm function, which the CLI does not expose. Guest-side metrics for the W1 design vector: 768 B allocated, 48 B operand peak, 18 execution steps, 1 MiB linear memory. Wasm module bytes, V8 heap and RSS are unsupported.

| Workload | L4 (compiled graph) | L3 | Host-native | L1 `--opt off` | L1 `--opt on` | L4 ÷ native speed | L4 ÷ L1 speed (off / on) |
|---|---|---|---|---|---|---|---|
| W1 warm, per evaluation (median [IQR]) | 137 ns [26] | 162 ns | 1.06 ns | 31.0 ms [5.0] | 42.3 ms [4.6] | 0.0077× | 2.3×10^5 / 3.1×10^5 |
| W1 cold (spawn or source to first result) | 206 µs | — | — | 388 ms | 263 ms | — | — |
| W2, 10^5 host steps (total) | 20.6 ms | 18.6 ms | 0.135 ms | 3,633 s | 9,710 s | 0.0066× | 1.8×10^5 / 4.7×10^5 |
| W2 per host step (L1 warm median [IQR]) | — | — | — | 33.0 ms (root 16.6 [2.6]) | 87.0 ms (root 43.4 [5.4]) | — | — |

On W1 and W2 the compiled graph is about five orders of magnitude faster than the ordinary path's per-submission host interface. It is still 130–150× slower than host-native, so it is not at least 2× faster than both baselines (`l4_at_least_2x_faster_than_both_baselines: false` for both workloads). Host loop throughput through L1 was 14.9 / 10.4 W1 evaluations/s and 27.5 / 10.3 W2 steps/s (off / on). There is no training step for these workloads (`E-DERIV-I64`).

## C2 claim separation

`phase-c-core-run1.json` → `c2` keeps the following as separate fields:

- reference agreement (the per-workload agreement checks and Phase B's L2/B2-ORACLE/L3/L4);
- finite coverage (finite only);
- theorem obligations (NG-PO-A..H, each `open`);
- trusted boundaries (codegen, host IEEE, oracle, BLAKE3, this crate and the native baselines, timer and shared host);
- unsupported instrumentation.

No hash match or agreement is reported as discharging an obligation.

## C3 recommendation: **stop**

- **Threshold test.** Warm W3-L: the compiled CPU graph (L4) is 0.131× the speed of the host-native baseline (0.131 / 0.131 / 0.131 across the three runs), about 7.6× **slower**. The kept threshold requires at least 2× faster, so the stop condition holds.
- **Admission.** Admission at W3 scale is 1.1 ms, within the 100 ms threshold. Training value was demonstrated (B5). Neither changes the outcome, because the speed condition alone triggers stop.
- **"Both baselines" for W3.** W3 is `ngraph-f32/0` authored IR (B0(b)), and Noble's ordinary Wasm/V8 path has no `F32`, so that baseline does not exist for W3. I applied the test to the one existing baseline, host-native Rust. A missing baseline cannot make the outcome more favourable: passing would have required beating host-native by 2× and also an ordinary-path comparison that cannot be made. W1/W2, where the ordinary path exists, are reported separately and do not stand in for W3.
- **GPU (#2).** Blocked. No transfer or launch cost was measured or modelled, and with a stop recommendation there is no compiled-CPU specialization to compare a GPU against.
- **W1/W2 against both baselines** (`phase-c-l1.json`). These do not change the outcome. L4 is far faster than the ordinary path's per-submission interface but 0.0066–0.0077× host-native, so it is not at least 2× faster than both baselines on any workload. W1/W2 are `I64` and do not stand in for the W3 test.
- **Interpretation, not measured.** The compiled evaluator interprets one enum-dispatched instruction per node, about 1.5 ns per node on W3-L. A straightforward native loop is 7.6–16.5× faster on these shapes. Only a design change could plausibly reach the 2× threshold, such as batch-vectorized evaluation or native code generation. Doing so would be a new experiment and is not claimed here.

# Supported-profile CPU follow-on (2026-10-16)

This is a separate, source-pinned measurement after the owner adopted the profile;
it does not replace the frozen Phase B/C receipts above or the lifecycle acceptance
and review packets in `verification/numeric-graph-v1/`. It makes no GPU, backend
selection, theorem, or claim of beating native Rust.

## Source, method and limits

The comparison uses the existing Phase C W1/W2/W3-S/M/L/W4 constructors and exact
input, parameter, dataset and optimizer bits, with the selected locked/offline
vendor and the same rustc/Cargo release profile in both lanes. The library baseline
is byte-identical to published anchor `cd76c608721f25ce7f7bc896dfc7cdeb54e9b004`.
Baseline harness commit `f03aa4d1a4d8612721fbed9a1577ae65befffa93` adds measurement
only; final implementation/harness commit is
`7e1de8f1eccdece595dea295a6e00a14c93b2e62`. The intermediate selected
`5ca3665` lane is retained, including its measured observer overhead, not silently
substituted for the final lane. Raw receipts, exact hashes, source manifests and
the reproduction configuration are in [`cpu-20261016/receipt.json`](cpu-20261016/receipt.json).

Each selected lane has three complete runs with the unchanged harness defaults
`--samples 2000 --cold-reps 21`
and one `--phase-profile true` function-level wall-time run; measurements use
CPU 2 via `taskset`, `nice -n 10`, and the campaign Cargo lock. `perf` was absent
from PATH and `perf_event_paranoid` was 2, so the profile is **real function-level
wall timing and structural retained/scratch-byte accounting**, not sampled CPU
stacks or an allocation-count profile. No unsafe global allocator was installed.
The host was shared, not idle or isolated: final-run load averages were lower
than baseline. The lock excluded campaign builds, not all host activity. Tiny
latency differences and overlapping ranges are not evidence of improvement.

The initial pre-edit/exploratory baseline remains contextual evidence only: its
then-used dependency/vendor context is not retrospectively called the selected
context. The final ratios below use only the paired selected receipts.

## Measured changes

Ranges below are the three runs' reported medians, not confidence intervals.
Admission/check, cold compile, warm evaluation and training are separate costs.
The one-step and derivative-admission rows come from the separate function-profile
runs and are not the steady 100-step measurement.

| Cost | Selected baseline | Final | Interpretation |
| --- | ---: | ---: | --- |
| W4 graph check | 5.979–6.209 ms | 6.156–6.434 ms | No checker speedup |
| W4 cold compile | 0.919–1.012 ms | 0.085–0.124 ms | 7.4–12.0× lower compile time across the ranges |
| W4 total cold check + compile + first evaluation | 6.893–7.267 ms | 6.259–6.573 ms | Checker still dominates cold admission |
| W4 retained compiled-code byte estimate | 2,721,604 B | 161,472 B | 16.85× smaller; structural estimate, not allocator/RSS measurement |
| W3-L derivative admission | 115.606 µs | 30.357 µs | 3.81× lower wall time in the function-profile pair |
| W3-L inclusive one-step training call | 196.467 µs | 103.003 µs | 1.91× lower wall time; includes admission/setup/finalization |
| W3-L steady training, per step, 15 points | 35.912–39.392 µs | 35.547–37.001 µs | Overlap; no steady-training throughput claim |
| W3-L warm compiled evaluation | 5.879–6.107 µs | 5.808–6.487 µs | Overlap; no warm-evaluation speedup claim |

The measured bottlenecks justify three boring implementation changes: compiled
table instructions borrow the single retained checked-graph table instead of
copying it into every instruction; derivative admission propagates parameter
influence once in canonical dependency order instead of repeating recursive
reachability walks; and observed/unobserved private instruction loops specialize
once per layer so the default path does not pay an optional observer branch per
instruction. There is no new backend, dependency, workspace target, numerical
profile, optimizer or scheduler/compiler revision. The separately approved
caller-owned safety observer hooks are exercised both with and without an observer.

On the final W3-L runs L4 remains only 0.142–0.155× host-native Rust speed, about
6.5–7.0× slower. These observations do not reverse the historical stop result or
establish that this profile is faster than either required baseline.

## Numerical and resource semantics

All 12 selected receipts (baseline, intermediate and final, including their
function-profile runs) retain exact W3 parameter/input/data bits, graph and packed
layout identities, final loss bits, snapshot identities and optimizer-state
hashes. The W3/W4 per-run L3/L4 agreements pass; I64 layouts and W2 final-state
agreements also match. Register/scratch estimates are unchanged; the intentional
retained-code reduction is reported separately. This is finite execution evidence,
not a discharged proof obligation.

Guard ordering and fuel/cancellation checkpoints are not moved. Compiled execution
still materializes input slots before its first layer fuel/cancellation check;
those writes are actual work and observer counters include them even when fuel
is zero. Historical `Report::evaluated_nodes` charges are not redefined as actual
events. Training still computes all pure model forward values before scanning
for the first canonical failure, as the anchor did; a failed point stops the
step and publishes no snapshot, but this is **not** a no-later-work cutoff claim.
Scratch leases and cancellation-before-step behavior remain independently
observed boundaries. See the resource proofs and production CLI smoke linked
from the receipt, rather than inferring work counts from output agreement.

The machine-readable index is [`cpu-20261016/receipt.json`](cpu-20261016/receipt.json).
It hashes every retained raw receipt, source manifest, harness/probe source and
recorded runner, and identifies benchmark/CLI/probe binaries and external frozen
source archives. The scripts are exact **host-specific recorded invocations**,
not automatic source-lane selectors: reproduce the specified source in separately
owned worktrees, retain the selected vendor/toolchain/configuration, and choose
new output paths rather than overwriting frozen evidence. Hold the campaign lock
around both the build and timing command. `run-selected-comparison.sh` records
the baseline/intermediate run; `run-selected-final.sh` records the final run.
The old environment capture supplies compiler version details; selected Cargo
home and vendor overrides are explicitly recorded separately in the receipt.

## Executed quality and production smoke

On the final selected source, `run-selected-quality.sh` completed with exit 0:
the complete `noble-ngraph` and `noble-ngraph-experiment` suites plus the existing
CLI `ngraph` suite passed (106 passed, two existing ignored tests), followed by
three-package all-target clippy with `-D warnings`, three-package format check,
and a release build of the production `noble` executable. A separate selected
baseline release ownership check passed all five tests; the combined log therefore
has 111 passes and two ignored tests. Commands and logs are retained with the receipt.

The actual final release CLI created a seed snapshot, trained ten steps from it,
saved the child, replay-verified it from the parent, and evaluated saved parameters
through both evaluators with exactly equal reports. Assertions check signed zero,
NaN, infinity and finite output encodings, missing-snapshot binding refusal,
canonical guard refusal, fuel-zero refusal, lineage and unchanged parent bytes.
The external native-oracle table probe also passed 612 explicit boundary,
NaN and refusal cases. The training refusal probe observed four forward/validation
events versus the reference evaluator's three-node cutoff, no publication,
unchanged snapshot bytes, and scratch `(acquired, released, live) = (1, 1, 0)`;
cancellation before a step observed zero events/publications/leases. These are
finite, specific exercised cases, not global no-work or resource-proof claims.

## The packed instruction stream itself (issue #43)

`layout::check` validates a *declaration* only: compiler revision, build key, register map, constant
and slot tables, register count, layer ranges and failure-suffix minima.
- **Compiler revision.** The declared revision must be the selected compiler's
  (`Refusal::StaleCompilerRevision`).
- **Build key.** The declared key must equal the key the checker computed for the graph,
  `CheckedGraph::packed_layout_key`, under the selected compiler revision and layout options
  (`Refusal::LayoutKey`). A key the layout supplies is never its own expected value, so a
  structurally identical layout compiled for another graph is refused (#98).

It never reads an instruction body, so a stream could name the right registers and still compute
the wrong operation. `crates/noble-ngraph/src/schedule.rs` closes that for the total I64/Bool
fragment:

| Obligation | What is compared |
|---|---|
| declarations | `layout::check` runs inside `schedule::check`, and its refusal keeps its own class (`StreamRefusal::Declarations(_)`); this includes layer coverage, each declared layer range starting where the previous one ends and holding exactly that layer's instruction count (#95) |
| ordered bodies | position `i` in the stream must be exactly the body the graph node at position `i` supplies under the declared register map: operation identity, **ordered** source registers, destination register |
| shape | the stream length is the number of instruction-emitting nodes |

Definition-before-use, single-writer registers and ordered output retention are *implied* for this
slice (acyclic layers + injective declared register map + matching body), and the module says so
instead of adding checks that a matching stream could never reach.

The stream is a public type (`I64Instruction`), reachable through `Compiled::i64_instructions()`,
which is what makes an independent validator possible at all. There is still **no versioned
serialization** of it, so the Lean validator in `proofs/foundation/NobleFoundation/PackedSchedule.lean`
cannot consume arbitrary streams. It consumes the exported artifacts below, one at a time; the
general gap stays open and is named in the foundation README.

`crates/noble-ngraph/tests/schedule_correspondence.rs` (11 tests, all passing) validates the stream
of the real `Compiled::new` output for an interleaved two-branch I64 graph, then mutates one
obligation at a time:

| Mutation | Refusal |
|---|---|
| addition replaced by subtraction | `BodyMismatch` |
| a wrapping addition's operands swapped | `BodyMismatch` |
| a multiplication's destination moved to a register the stream does not write | `BodyMismatch` |
| two independent instructions of one layer swapped | `BodyMismatch` (positions are compared against their own node, not against the layer as a set) |
| one instruction dropped, one duplicated | `StreamShape` |
| the last layer's end shortened | `Declarations(LayerCoverage)` |
| a layout revision from another release | `Declarations(StaleRevision)` |
| two output registers swapped in the declaration | the declaration stays admissible, and the stream is refused: `BodyMismatch` |
| a checked graph's layout and stream paired with a different graph | refused, while each graph's own pair validates |
| an F32 graph | `UnsupportedProfile` |

Two fixture mutations were wrong when first written and were fixed rather than relaxed: shortening a
boundary on a zero-width first layer mutated nothing, and a destination mutation that matched no
multiplication left the stream untouched. Both are recorded because a control that silently does
nothing is the failure mode this lane exists to prevent.

### The same artifacts, validated by the Lean kernel (2026-11-07)

`crates/noble-ngraph/tests/schedule_lean_dir/corpus.rs` exports the real declaration and stream of
two graphs into the vocabulary of `NobleFoundation.PackedSchedule`, and generates
`proofs/foundation/NobleFoundation/PackedScheduleCorpus.lean` from them:

| Artifact | What it exercises | Mutated copies the kernel refuses |
|---|---|---|
| `interleaved` (the graph above) | shared inputs, a negative constant, all four operations, a Bool equality, two outputs | wrong operation, swapped operands, a within-layer reorder, a dropped instruction, swapped output registers |
| `lateInput` | an input created after an addition it does not feed, moved ahead of it in packed order (`[0, 1, 3, 2, 4]`), and a repeated output | wrong operation, a dropped instruction |

For each artifact the foundation lane kernel-checks `validate` (`*_validated`). Through
`validate_sound`, it also proves that the exported stream returns the graph reference at every output
for every input (`*_sound`). `the_lean_file_is_the_real_emission` fails when `Compiled::new` emits
something else, until the file is regenerated (the ignored `write_the_lean_file`) and the lane is
rerun.

That gives item 4's three-way comparison at five boundary inputs per artifact, including
`i64::MAX + 5` wrapping onto `i64::MIN + 4`. The generator runs the real packed machine
(`Compiled::evaluate`), asserts that the reference interpreter (`eval::reference`) agrees, and writes
the outputs into `*_execution_*_agrees` theorems. The kernel evaluates the model's `run` to the same
outputs, and by `*_sound` that is also the graph reference. The agreement is tested at those inputs,
not proved for the executor.

The exporter is untrusted: it claims which node each instruction computes, and a wrong claim is
refused rather than accepted. One translation is trusted. The packed machine preloads the slot and
constant tables into their registers before the first layer, and the export writes those preloads as
`setInput`/`setConst` at their packed positions. This is per-artifact validation of two artifacts. It
is not a theorem about the scheduler, `schedule::check`, the Rust executor, the host or the engine,
and it adds no decoder or versioned serialization.

### Validation cost at the default budgets (2026-11-05)

Issue #43 asks for bounded checking work and measured validation time and scratch bytes. Measured
at the graph checker's default budgets (262,144 nodes, 16,384 layers), both validators were
quadratic:
- `layout::check` searched every earlier register for each node, and rescanned the graph once per
  declared constant and slot entry.
- `schedule::check` rescanned every node once per layer to order the stream, then again to count
  coverage.

The fixes:
- **`layout::check`.** One pass plus one sort of `(register, node)` pairs finds the first register
  fault, and one counting pass handles each table. The cost is `O((n + k) log n)`.
- **`schedule::check`.** One counting pass and one placing pass bucket the stream by layer. After the
  declaration check the cost is linear in nodes, layers and stream length.

Every scratch allocation is sized by the graph. The reported refusal is still the one at the
earliest node. `the_earlier_register_fault_is_the_refusal` pins both orders of an alias and an
out-of-range register, and it fails a "last fault wins" mutant.

[`validation-cost.mjs`](validation-cost.mjs) runs one ignored harness,
`crates/noble-ngraph/tests/schedule_cost_dir/measure.rs`, in release mode twice over an archived
copy of the source. The baseline run swaps in the validator files of a named baseline commit. The
retained receipt is [`validation-cost-2026-11-05.json`](validation-cost-2026-11-05.json): clean
source `dd639ee8`, baseline `f5e55f38`, medians of five runs.

| Shape | Nodes | Layers | Check | Compile | `layout::check` before → after | `schedule::check` before → after | First-validation resident growth |
|---|---|---|---|---|---|---|---|
| deepest chain | 16,384 | 16,384 | 2.29 ms | 389 µs | 7.94 ms → 173 µs | 322.79 ms → 303 µs | 0.2 → 0.3 MiB |
| four deepest chains | 65,533 | 16,384 | 8.66 ms | 1.79 ms | 118.33 ms → 2.17 ms | 1.39 s → 2.62 ms | 0.2 → 1.1 MiB |
| sixteen deepest chains | 262,129 | 16,384 | 36.17 ms | 14.50 ms | 1.78 s → 11.21 ms | 6.57 s → 14.91 ms | 1.0 → 5.0 MiB |
| balanced tree | 262,144 | 19 | 37.34 ms | 5.63 ms | 1.74 s → 3.16 ms | 1.74 s → 5.14 ms | 1.0 → 5.0 MiB |

`schedule::check` includes its own `layout::check`. Validation of the largest admitted graph now
costs about what checking and compiling it cost, instead of about 400 times more.

The extra scratch is the sorted pairs and the table counts, and it is freed when validation
returns. Resident-set growth is the only memory instrument here, because the workspace forbids
`unsafe_code` and so no counting allocator can exist. It is also an upper bound: it includes what
the allocator retains.

### A caller's cap on validation work (2026-11-08)

`schedule::check_within(checked, layout, stream, cap)` validates under a caller-selected cap.
- **The measure.** `schedule::declared_work` sums every size validation iterates over: graph nodes,
  the register map, the constant and slot tables, the layer ranges, the failure-suffix table and the
  stream. It reads only lengths, so a padded declaration is counted before any of it is walked.
- **Exhaustion.** Inputs that declare more work than the cap are refused as
  `StreamRefusal::Exhausted { work, cap }`. Only the constant-time profile check comes first, and
  nothing else is read. Exhaustion is therefore never success and never a verdict on the stream.
- **Within the cap**, the result is exactly `schedule::check`'s.

The controls (`tests/schedule_cost_dir/limits.rs`):
- the declared work validates, and one unit less is exhausted;
- a stream with a duplicated instruction, which `check` refuses as `StreamShape`, is exhausted under
  the honest cap;
- a declaration padded with 1000 constant rows, which `check` refuses as a declaration fault, is
  exhausted under the honest cap;
- a wrong operation under an unbounded cap keeps its `BodyMismatch` class.

Still open from the same acceptance item:
- There is no decoder, so no decoding cost exists to bound.
- One host and one run bound nothing on other hardware.

### The source-bound receipt (issue #43, item 6)

[`schedule-receipt.mjs`](schedule-receipt.mjs) writes a receipt for the real packed artifacts the
kernel validates. It reads them from the generated `PackedScheduleCorpus.lean`, which
`the_lean_file_is_the_real_emission` pins to what `Compiled::new` emits.

For each artifact the receipt binds:
- **the graph** and **the packed artifact**, by digest of exactly the terms the kernel checked: the
  node list and graph, then the register map, packed order, instruction stream, outputs and output
  registers;
- **the accepted claim** (`*_validated`), the semantic theorem (`*_sound`), well-formedness, the
  refused mutations and the agreeing executions, with the statements and axioms a clean-source
  foundation receipt recorded for them.

**Revisions.**
- The exporter is bound by its generator sources and the generated file.
- The checker is bound by `PackedSchedule.lean`, `FirstProjection.lean` and the Lean executable.
- The interpreters are bound by the packed machine's sources with its compiler and layout revisions,
  and by the reference interpreter's sources.

**Premises and edges.** The receipt states the premises (the untrusted exporter, the trusted preload
and node reading, and the execution rows checked against the reference interpreter). It also lists
the edges that stay open: no executor theorem, no decoder, guards and failure-suffix ordering, and
per-artifact scope only. The foundation receipt must come from an ancestor commit whose corpus and
checker model are byte-identical to the checkout's.

```sh
node verification/ngraph/schedule-receipt.mjs \
  --foundation-receipt verification/foundation/receipts/<dir> --output NEW_EXTERNAL_FILE
```

The retained receipt is [`schedule-receipt-29fc1e17.json`](schedule-receipt-29fc1e17.json), from the clean
commit `29fc1e17`. It covers the two artifacts (`interleaved`, `lateInput`) and all 23 corpus theorems,
taken from the foundation receipt of `a41435fd`. Both claims report only `propext` and `Quot.sound`.

**Erratum (2026-11-13, issue #43).** The retained receipt's `open_edges` entry reads "the kernel
extraction that could carry it predates these sources (#97)". That dependency is **wrong**, and the
receipt is immutable, so the correction is recorded here rather than edited into it. #97 is the
re-extraction of the existing Aeneas subjects (`noble-contracts`, `noble-kernel`, `noble-wasm`) at
Aeneas `1238879f` to end the VT-PIN-02 library exception; it adds no subject and never covers
`crates/noble-ngraph`. No extraction subject includes `crates/noble-ngraph/src/schedule.rs`, so the
packed executor's correspondence is not blocked by #97 and does not depend on it. What is open is
that no executor theorem exists and no extraction of the executor is planned; a future refinement
would need its own extraction subject and its own obligation, not #97's re-extraction.

# CPU batch continuation decision (2026-11-01, issue #41)

[`batch-continuation-criteria.md`](batch-continuation-criteria.md) froze the continuation rule before
anything was measured: agreement, 1.15× the compiled single-row loop at 65536 rows, no medium-size
regression, and resident growth bounds. [`batch-decision.md`](batch-decision.md) applies it to the
one release run retained in [`batch-measurements-2026-11-01.json`](batch-measurements-2026-11-01.json):
batching was 1.7× **slower** per row than the compiled single-row loop with about 172 bytes of report
per row, so the decision is **NO-GO** and no batching work (SIMD, `F32`, chunked staging) starts on
that issue. The lane lives in the existing `batch_row_reference` target as
`tests/batch_rows/measure.rs`, is `#[ignore]`d by default, and reports allocation counts as an
unavailable instrument because the workspace forbids `unsafe_code`.
The cold, staging, output-copy and byte breakdown that item 6 asks for is retained in
[`batch-breakdown-2026-11-07.json`](batch-breakdown-2026-11-07.json), with its table in
[`batch-decision.md`](batch-decision.md). The per-row reports are about five times the staged inputs.
