# Woxi lane results (#24)

These are test observations recorded on 2026-10-19 on aspen3. They change no
canonical case `state` or `evidence`, and CALC-12 stays `absent` / `not-run`.
The lane is described in [README.md](README.md). Compact receipts are in
`verification/calculator-v1/woxi/receipts/`; every file there is listed
with its SHA-256 in [`receipts/sha256.txt`](receipts/sha256.txt).

## Fresh Woxi reference (2026-10-19)

The Wolfram kernel was replaced by [Woxi](https://github.com/ad-si/Woxi)
0.3.0, the Rust Wolfram Language interpreter. No kernel, licence or
Wolfram client is involved. The helper
([`helper/src/main.rs`](helper/src/main.rs)) links the `woxi` crate and calls
`interpret_with_stdout` on exact `Rational[n, d]` expressions.

| Item | Value |
|---|---|
| Noble source | `6617abddab3cebc8022b27725112acddf2559aee` (release build) |
| Binary SHA-256 | `218ec4bf9659d9ecfed1b53fa95738b3b6260ef1857156993a52337404be3338` |
| Corpus | seed 2424, 12 seeded + 36 edge/wide cases; 48 run, 1 malformed excluded |
| Woxi / helper | `0.3.0` / `0.1.0`, `Cargo.lock` committed |
| Reference | [`receipts/references/woxi-seed2424-a12a538-20261019.json`](receipts/references/woxi-seed2424-a12a538-20261019.json), status `ok` |
| Comparison | [`receipts/comparisons/woxi-seed2424-6617abd-20261019.json`](receipts/comparisons/woxi-seed2424-6617abd-20261019.json) |
| Run receipt | [`receipts/runs/woxi-seed2424-6617abd-20261019/receipt.json`](receipts/runs/woxi-seed2424-6617abd-20261019/receipt.json) |

**Raw probes observed on Woxi 0.3.0** (all six match the policy premise):
`0^0 = Indeterminate`, `1/0 = ComplexInfinity`, `0/0 = Indeterminate`,
`0^-1 = ComplexInfinity`, `4^(1/2) = 2`, `2^(1/2) = Sqrt[2]`.

**Comparison summary:** 39 `agree`, 9 `budget-divergence`, 2 `unsupported`,
**0 `disagree`**. The nine budget divergences are the wide/u64 cases where
Noble reports `ResourceLimit` at the 2 GiB runner bound (`edge/u64-squared`,
`edge/wide-quotient`, `seed-2424/calc12-07`, `seed-2424/wide-01..06`). The two
unsupported entries are the acceptance runner's control vectors, not corpus
cases. By the lane's own definition a run with non-agreements and no
disagreement is `incomplete`, not `pass`; the report says exactly that.

### What changed from the Wolfram provider

- The reference is a Rust process built from the committed `Cargo.lock`; the
  `woxi` crate is AGPL-3.0-or-later and is not linked into production crates.
- `TimeConstrained` is a no-op in Woxi and `MemoryConstrained` is a symbol, so
  the adapter enforces its own exponent/scale bounds (`1 << 20`) and the lane
  keeps the runner's wall-clock and memory limits. A bound hit is
  `oracle-timeout`, never Noble's deterministic `ResourceLimit`.
- Provenance records `woxi_version` and `helper_version`; the raw probes are
  now observed rather than documentation-only.

### Previous provider (historical)

The 2026-10-15 attempt used a Wolfram kernel and recorded
`oracle-unavailable`: no kernel or licence was available. Those receipts and
the fault-model results below are retained as history; they are not Woxi
results. The prior `reference-woxi-unavailable.json` input is kept under
`receipts/inputs/` only as a record of that attempt.

## What ran instead

Each result carries its label, as described in the README.

### Harness-level CALC-12 fault models

For this run, each #21 fault model in [`faults.mjs`](../oracle/faults.mjs)
stood in as the implementation. It was judged against the
`stand-in-reference` on the 48-case seed-2424 corpus, with bounded in-process
shrinking: at most 4 rounds of at most 6 candidates. The reports are in
`receipts/harness/`. Every disagreement is attributed to that fault's model
alone.

| Implementation | `lane_result` | Disagreements | Probe | Probe output | Reference | Shrunk to |
|---|---|---|---|---|---|---|
| reversed-subtraction | `fail` | 10 (`value-mismatch`) | `10 - 3` | `-7` | `7` | `1 - 3` |
| left-associated-power | `fail` | 2 (`value-mismatch`) | `2^3^2` | `64` | `512` | `2^1^2` |
| non-normalized-rational | `fail` | 12 (11 `non-normalized-display`, 1 `category-mismatch`) | `2 / 4` | `2/4` | `1/2` | `2 / 2` |
| integer-division | `fail` | 15 (10 `value-mismatch`, 5 `category-mismatch`) | `7 / 2` | `3` | `7/2` | `1 / 2` |
| binary-float-decimal-parser | `fail` | 16 (`value-mismatch`) | `0.1 + 0.2` | `10808639105689191/36028797018963968` | `3/10` | `0.1` |
| I64-wrapping | `fail` | 12 (`value-mismatch`) | `9223372036854775807 + 1` | `-9223372036854775808` | `9223372036854775808` | unchanged |
| failure-mutates-session | `stand-in-reference-agree` | 0 | none (stateful fault) | | | |
| #21 BigInt oracle (sanity) | `stand-in-reference-agree` | 0 | | | | |
| reversed-subtraction against the historical unavailable Wolfram reference | `oracle-unavailable` | 48 `oracle-unavailable` | | | | |

The category mismatches are real, for example:

- Under non-normalized-rational, `4^(2 / 2)` reports `NonIntegerExponent`,
  because the exponent `2/2` is no longer seen as integral.
- Under integer-division, `(2 / 3)^-2` reports `DivisionByZero`.

The shrinker preserves the domain premise. A `DivisionByZero` disagreement
never shrinks to a value case. A candidate whose reference is unavailable is
never accepted.

**Re-run against the fresh Woxi reference (2026-10-19, `6617abd`).** The
same six fault implementations and the stateful control were judged against
`receipts/references/woxi-seed2424-a12a538-20261019.json`, with the Woxi
helper supplying shrink-candidate references (`--bridge`). Every count
reproduces the table exactly: reversed-subtraction 10, left-associated-power
2, non-normalized-rational 12, integer-division 15,
binary-float-decimal-parser 16 and I64-wrapping 12 disagreements, each
attributed only to its own model; `failure-mutates-session` agrees on 48/48.
Receipts: `receipts/harness/woxi-seed2424-6617abd-*.json`.

### Real #21 mutant binaries (retained raw I/O, offline fixture replay)

This lane re-judged the raw stdin, stdout, stderr and exit status that #21
retained from its real mutant runs (ffcce53 + patch, release profile). The
expectations came from
[`fixtures/edge-expected-stand-in.json`](fixtures/edge-expected-stand-in.json),
labeled `offline-fixture-replay` / `python-fractions-stand-in`. The reports
are in `receipts/mutant-replay/`.

| Mutant run | Probe | Observed | Verdict | Mismatch | Attribution |
|---|---|---|---|---|---|
| reversed-subtraction (`…172118Z`) | `10 - 3` | `-7` | `disagree` | `value-mismatch` | `[reversed-subtraction]` |
| left-associated-power (`…182129Z`) | `2^3^2` | `64` | `disagree` | `value-mismatch` | `[left-associated-power]` |
| non-normalized-rational (`…173239Z`) | `2 / 4` | `2/4` | `disagree` | `non-normalized-display` | `[non-normalized-rational]` |
| integer-division (`…210028Z`) | `7 / 2` | `InfrastructureFailure …` | `not-comparable` | | |
| I64-wrapping (`…210028Z`) | `9223372036854775807 + 1` | `InfrastructureFailure …` | `not-comparable` | | |
| binary-float-decimal-parser (`…210028Z`) | `0.1` | `InfrastructureFailure …` | `not-comparable` | | |
| failure-mutates-session (`…180239Z`) | the 3-line session | | `unsupported` (outside the lane) | | |
| control (`…184937Z`) | the six single-line probes | equal to the reference | 6 `agree`; the session probe is `unsupported` | | |

### Bounded shrinking against a real binary

Both rounds use the reversed-subtraction mutant
(`22f2b547…a6e9`) and the `stand-in-reference`. Each round is a `shrink`
corpus followed by `compare`, and `compare` reports the `shrink_selection`.

| Round | Noble execution | Input | Candidates (observed) | Kept |
|---|---|---|---|---|
| 1 | #21's retained run `calc12-shrink-reversed-subtraction-20261015T181002Z`. `lane.mjs shrink --input '5 / 4 - 6 / 2' --shrink-candidates 3` produces exactly its four inputs. | `5 / 4 - 6 / 2` → `-7/4`; reference `7/4` | `5 / 4`, `6 / 2` agree; `4 - 6 / 2` gives `-1` (reference `1`) | `4 - 6 / 2` |
| 2 | a fresh run made by this lane, `wolfram24-shrink-reversed-subtraction-20261015T221149Z` (4 processes, 100–406 s each) | `4 - 6 / 2` → `-1` | `4` agrees; `4 - 2` gives `-2`, and `4 - 6` gives `2` | `4 - 2` |

Every kept candidate has the original's signature,
`disagree:value-mismatch:value:-`, and is attributed only to
`[reversed-subtraction]`. The `minimal.replay` object of each comparison holds
two things:

- the one-vector runner file and its command, to replay the Noble execution;
- the bridge request and its command, to replay the oracle computation. The
  historical comparison has `available_in_this_report: false`; the fresh
  Woxi report above has it `true`.

### The integrated calculator on the seed-2424 corpus

The corpus is
[`receipts/inputs/corpus-seed2424.json`](receipts/inputs/corpus-seed2424.json),
built by `lane.mjs corpus --seed 2424 --count 12 --wide 6`. It holds 48
cases:

- the 31 edge fixtures;
- 11 cases from the #21 generator (seed 2424). A 12th case, `11 + 5.96 / 3 8`
  from the `malformed` family, is excluded, and the exclusion is recorded;
- 6 `wide-literal` cases.

Each case ran as one #19 runner process on the integrated release binary
`noble-integration-eccbc02-release`:

- SHA-256 `f7f02dfb…85ec`, source `eccbc024c837f21caf26ffe2d6be7e6c1d6bb1fb`;
- runner commit `05a50b0` with no tracked changes;
- worker Node v24.13.0 and `noble-calculator-wat 1.243.0`;
- concurrency 6 and `MemoryMax=2G`, under default guest budgets;
- run `wolfram24-seed2424-eccbc02-release-20261015T220627Z`, from 22:06:28Z to
  22:24:05Z;
- 67–205 s per process, 5,940 s in total.

Both runner negative controls classified as required.

| Comparison | Label | `lane_result` (exit status) | Summary |
|---|---|---|---|
| [`seed2424-eccbc02-fresh-oracle.json`](receipts/comparisons/seed2424-eccbc02-fresh-oracle.json) | `fresh-oracle` | `oracle-unavailable` (3) | 48 `oracle-unavailable`; the 2 runner controls are `unsupported` |
| [`seed2424-eccbc02-stand-in.json`](receipts/comparisons/seed2424-eccbc02-stand-in.json) | `stand-in-reference` | `incomplete` (2) | 39 `agree`, 9 `budget-divergence`, 2 `unsupported` |
| [`edge-eccbc02-fixture-replay.json`](receipts/comparisons/edge-eccbc02-fixture-replay.json) | `offline-fixture-replay` (`python-fractions-stand-in`) | `incomplete` (2) | the 31 edge cases: 29 `agree`, 2 `budget-divergence`; the 17 seeded processes and the 2 controls are `unsupported` for the edge-only corpus |
| [`seed2424-eccbc02-wrong-binary-pin.json`](receipts/comparisons/seed2424-eccbc02-wrong-binary-pin.json) | `stand-in-reference`, pinned to the 44329dc binary hash | `stale-provenance` (4) | no verdicts |

Under default budgets, the stand-in agrees with the binary on these cases:

- exact decimals: `0.1 + 0.2` → `3/10`, `0.1` → `1/10`,
  `0.00000000000000000001`, `3.14159 - 3` → `14159/100000`;
- values beyond I64: `9223372036854775807 + 1` → `9223372036854775808` and
  `-9223372036854775808 - 1` → `-9223372036854775809`;
- `9007199254740993 - 9007199254740992` → `1`;
- every intentional difference: `0^0` → `1`; `1 / 0`, `0 / 0` and `0^-1` →
  `DivisionByZero`; `4^(1 / 2)` and `2^(1 / 2)` → `NonIntegerExponent`;
- both first-error orderings;
- the six CALC-12 arithmetic probes;
- the 11 generated cases, other than `2^64 + 3`.

The 9 `budget-divergence` cases are `2^64 + 3`, `u64-squared`,
`wide-quotient` and the six `wide-literal` cases. Each one stopped with
`ResourceLimit EvaluationWork` under the default `--evaluation-work 4000000`.
Under this lane's rules that is a divergence, not a disagreement.

**The wide cases with a larger work budget.** The 9 budget-divergent cases
were run again on the same binary with the runner flag
`--calculator-arg=--evaluation-work --calculator-arg=256000000`
(`wolfram24-wide-eccbc02-release-ew256M-20261015T222435Z`; report
[`wide-eccbc02-ew256M-stand-in.json`](receipts/comparisons/wide-eccbc02-ew256M-stand-in.json)).

- Two cases now agree with the stand-in:
  - `2^64 + 3` → `18446744073709551619`;
  - `59039983304222846951 - 801507105833160313081525` →
    `-801448065849856090234574`.
- The other seven are `not-comparable`. The host stopped them with
  `NativeFailure: {… "outcome":"runtime-exhausted", "quota_reason":"execution work" …}`.
- At `--evaluation-work 16000000` the same seven end the same way
  (`wolfram24-wide-eccbc02-release-ew16M-20261015T222746Z`; report
  [`wide-eccbc02-ew16M-stand-in.json`](receipts/comparisons/wide-eccbc02-ew16M-stand-in.json)).

The integrated build cannot evaluate these products, quotients and
long-fraction sums of 20–30-digit literals under any budget that was tried:
the guest budget runs out at the default, and the host runtime quota stops
the larger budgets. This is a runtime-coverage observation for #20, not an
arithmetic disagreement. This lane does not judge it either way.

## Receipts

`verification/calculator-v1/woxi/receipts/` holds the following, each
listed in [`receipts/sha256.txt`](receipts/sha256.txt), a file that
`sha256sum -c` accepts:

- `inputs/`: the seed-2424 corpus, its runner vector file, the edge-only
  corpus, the stand-in reference and the `oracle-unavailable` Woxi
  reference;
- `comparisons/`, `harness/`, `mutant-replay/` and `shrink/`: every report
  cited above;
- `runs/<run>/`: the runner `receipt.json`, `progress.jsonl` and runner
  stdout, stderr and exit status for this lane's four runs;
- [`raw-io.tar.gz`](receipts/raw-io.tar.gz): every process's stdin, stdout,
  stderr, supervisor spec and result for those four runs. Its names are
  sorted, its mtimes are 0 and it was written with `gzip -n`.

The #21 mutant runs that were replayed above are retained in #21's own
receipts.

## Reproducing

```sh
L=verification/calculator-v1/woxi/lane.mjs
node $L corpus --seed 2424 --count 12 --wide 6 --out C.json     # same cases as receipts/inputs/corpus-seed2424.json
node $L vectors --corpus C.json --out V.json
node verification/calculator-v1/acceptance.mjs --binary noble-integration-eccbc02-release \
  --binary-sha256 f7f02dfbc9f6723a10879fd269abd4319a8736aa9d952ebb06f5d9bc2c5085ec \
  --source-id eccbc024c837f21caf26ffe2d6be7e6c1d6bb1fb --build-profile release \
  --out RUN --memory-max 2G --concurrency 6 --vector-timeout 2400 --vectors V.json
node $L reference --corpus C.json --arithmetic woxi --bridge helper/target/release/noble-woxi-bridge --out REF.json
node $L compare --corpus C.json --run RUN --reference REF.json \
  --expect-binary-sha256 f7f02dfbc9f6723a10879fd269abd4319a8736aa9d952ebb06f5d9bc2c5085ec \
  --expect-source-id eccbc024c837f21caf26ffe2d6be7e6c1d6bb1fb --out REPORT.json
```

The `stand-in-reference` rows use `--arithmetic python-fractions` in place
of `woxi`. The `offline-fixture-replay` rows use
`--fixtures verification/calculator-v1/woxi/fixtures/edge-expected-stand-in.json`
in place of `--reference`. The `harness`, `shrink` and mutant-replay commands
are listed in the [README](README.md).

## Status against the acceptance criteria

| Criterion | Status |
|---|---|
| Document the subset, the interchange, the adapter and the intentional differences | Done ([README](README.md)) |
| Compare against independent exact Woxi results on edge fixtures and a seeded corpus, including values beyond I64 and `0.1 + 0.2 = 3/10` | **Done** for the 48-case seed-2424 corpus at `6617abd`: 39 agree, 9 budget-divergence, 2 runner controls unsupported, 0 disagree. The wide cases hit Noble's 2 GiB runner bound, so the lane reports `incomplete`, honestly. |
| Seeded faults fail for their intended reasons; document coverage | Done with the fresh Woxi reference and with the retained #21 real-mutant raw I/O: six fault models fail with intended attribution and the stateful control agrees 48/48. Coverage is mapped in the README. |
| Bounded-shrink artifacts with replay commands | Done: two real-binary rounds plus harness shrinks, each with a replay object. Oracle replay now runs the Woxi helper. |
| Pin versions, seeds and policy; reject stale provenance | Done, and shown by the wrong-binary-pin report and the tests. `woxi_version` and `helper_version` are pinned by `--expect-woxi-version` / `--expect-helper-version`. |
| Default builds and offline replay work without Woxi; unavailable, error and unsupported states are distinct | Done |
| Production dependencies and certification authority unchanged; tested results distinguished from proofs | Done |

## Non-claims

- Agreement on finitely many inputs is test evidence only. No Woxi result
  was obtained. The stand-in agreements are not a Woxi comparison.
- No proof is made or checked. No `Certified` status, canonical case state or
  `tasks.md` item changes.
- Grammar, spans, sessions, budgets and host failures are not judged here.
