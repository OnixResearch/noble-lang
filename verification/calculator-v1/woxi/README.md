# Optional Woxi exact-arithmetic lane (#24)

This directory is an optional, opt-in differential lane for the exact
calculator. It compares what the real `noble calculator` binary printed with
exact results computed by [Woxi](https://github.com/ad-si/Woxi), the Rust
Wolfram Language interpreter. It is a provider under the
[CALC-12 oracle lane](../oracle/README.md) (#21). It does not replace that
lane, the [#19 acceptance runner](../README.md) or any proof. Its results are
test observations only. They never change a canonical case `state` or
`evidence`.

**Prerequisite status.** A fresh Woxi comparison needs only the Rust helper:
`cargo build --release --locked` in [`helper/`](helper/). Woxi is a crates.io
dependency (`woxi = "=0.3.0"`, AGPL-3.0-or-later) and does not require a
Wolfram kernel, licence or client. The CPython `python-fractions` stand-in
needs Python but is never reported as an oracle run.

## What is shared and what is not

| Part | Source |
|---|---|
| Corpus | The #21 seeded generator (`generateExpressions`, mulberry32), plus this lane's `wide-literal` family and the [edge fixtures](fixtures/edge-cases.json) |
| Shrink candidates | The #21 `shrinkCandidates`, restricted to this lane's subset |
| Text → tree | The #21 test re-parser `parseTree`, checked by `render(parseTree(text)) === text` |
| Noble execution | The #19 runner (`acceptance.mjs --vectors`), one process per case; this lane reads only the retained stdin, stdout, stderr and supervisor result |
| Fault models | The #21 [`faults.mjs`](../oracle/faults.mjs) models, used as harness-level implementations and for attribution |
| **Reference** | **`helper/` (the `woxi` crate) for `woxi`; `bridge.py` for the CPython stand-in.** The guest modules, the Rust host and the #21 BigInt oracle never produce an expected value for this lane. The #21 oracle supplies only the runner's per-vector bookkeeping expectation, which this lane ignores. The tests also use it to cross-check retained fixtures. |

## Supported subset and semantic adapter

The subset covers single-line expressions over numeric literals with unary
`-` and binary `+ - * / ^`, in the default display mode. The policy is
`noble-woxi-policy/1` and is implemented in `evaluate()` in
[`bridge.py`](bridge.py) and its Rust counterpart in
[`helper/src/main.rs`](helper/src/main.rs). Operands are evaluated left to
right, and the first diagnostic stops the walk (CALC-PARSE-02).

| Noble operation | Reference computation (Woxi) | Adapter policy |
|---|---|---|
| literal `d…d[.f…f]` | `Times[digits, Power[10, -scale]]` | CALC-NUM-03: digits are sent as an exact integer |
| `-x` | `Times[-1, x]` | none |
| `x + y`, `x - y`, `x * y` | `Plus`, `Subtract`, `Times` | none |
| `x / y` | `Divide[x, y]` | `y = 0` gives `DivisionByZero`; no Woxi call is made |
| `x ^ y` | `Power[x, e]` with integer `e` | `y` not integral by value gives `NonIntegerExponent` (`2/2` counts as integral). `y = 0` gives `1`, including `0^0`. `x = 0` with `e < 0` gives `DivisionByZero`. |

Each step is one Woxi evaluation, wrapped as
`TimeConstrained[MemoryConstrained[step, memory, mark], seconds, mark]`. The
step result is checked before use: it must be an `Integer` or a normalized
`Rational` (`d > 0`, `gcd = 1`). Anything else is an `oracle-error`. That
includes a machine real, `Indeterminate` (which the client turns into `nan`),
`ComplexInfinity`, `DirectedInfinity` and an unevaluated `Power`. The Node
side checks each returned reference again
(`validateReference` in [`interchange.mjs`](interchange.mjs)).

### Exact interchange representation

A case reaches the reference as a JSON tree, never as Noble source text:

```json
{ "op": "add", "x": { "op": "lit", "digits": "1", "scale": 1 }, "y": { "op": "lit", "digits": "2", "scale": 1 } }
```

`digits` is the literal's digit string exactly as written, for example
`"000000000000000000001"` with scale 20. The Rust helper turns it into an
exact decimal string and calls `woxi::interpret_with_stdout` on a generated
`Rational[n, d]` expression. No input text and no machine float crosses the
reference boundary: the helper builds every expression from exact decimal
operands. Results come back as decimal strings
`{ "kind": "value", "n": "-7", "d": "2" }` or as
`{ "kind": "diagnostic", "category": "DivisionByZero", "rule": … }`.

### Intentional differences from raw Woxi

Each fresh Woxi run evaluates these raw expressions (`probes()` in
[`helper/src/main.rs`](helper/src/main.rs)) and records them. If a raw result
differs from the expected one below, the policy's premise no longer holds for
that Woxi version, and the run is an `oracle-error`. The expected strings were
confirmed against Woxi 0.3.0.

| Input | Raw Woxi | Noble contract (adapter) |
|---|---|---|
| `0^0` | `Indeterminate` ([Power](https://reference.wolfram.com/language/ref/Power.html)) | `1` (CALC-NUM-05) |
| `1 / 0` | `ComplexInfinity` | `DivisionByZero` (CALC-NUM-04) |
| `0 / 0` | `Indeterminate` | `DivisionByZero` |
| `0^-1` | `ComplexInfinity` | `DivisionByZero` |
| `4^(1 / 2)` | `2` | `NonIntegerExponent` |
| `2^(1 / 2)` | `Sqrt[2]` | `NonIntegerExponent` |
| a step exceeds `TimeConstrained`/`MemoryConstrained` | aborts | `oracle-timeout`, an oracle failure; never Noble's deterministic `ResourceLimit` |
| a Noble `ResourceLimit` line | (no budget model) | `budget-divergence`, never agreement or disagreement |

### Outside this lane

The CAS does not establish any of the following. They stay with their own
lanes:

- grammar and malformed input, diagnostic spans and provenance text (#21 oracle);
- names, functions, lexical sessions and session rollback (#21 oracle and the
  stateful seeded fault);
- deterministic guest budgets (#20);
- host and transport failures (#19 runner);
- `--finite-decimal` and `--framed` output.

A diagnostic's span is recorded as `spans_not_judged`. A process outside the
subset is listed as `unsupported`. #21's `malformed` family is excluded from
the corpus, and the exclusion is recorded.

## Verdicts, labels and lane results

Each case gets exactly one verdict:

| Verdict | Meaning |
|---|---|
| `agree` | the streams equal the reference: one value line in normalized `n` / `n/d` form with exit 0, or one diagnostic of the reference category with exit 1 |
| `disagree` | the comparison ran and differs. `mismatch` is one of `value-mismatch`, `non-normalized-display` (same value, wrong form), `category-mismatch` or `stream-shape` |
| `budget-divergence` | Noble reported `ResourceLimit` |
| `not-comparable` | timeout, signal, exit status not 0/1, or a non-guest stderr line such as `InfrastructureFailure:` |
| `oracle-unavailable`, `oracle-timeout`, `oracle-error` | the reference could not produce a usable result |
| `no-recorded-expectation` | a replay fixture lacks this case |
| `unsupported` | the process is outside the subset (outside-lane list) |
| `stale-provenance` | the provenance checks failed, so no verdict is issued |

Every report carries a `label`, which says where the expectations came from:

- `fresh-oracle`: a Woxi computation made in this run. This is the only
  label that can give `lane_result: pass`.
- `stand-in-reference`: the same adapter policy over CPython
  `fractions.Fraction` (`--arithmetic python-fractions`). The lane uses it to
  exercise itself without Woxi. It is not Woxi, and agreement with it
  is `stand-in-reference-agree`.
- `offline-fixture-replay`: expectations read from a retained
  `noble-woxi-fixtures/v1` file. The file's own `expectation_source` is
  shown too: `woxi-recorded` or `python-fractions-stand-in`. Agreement is
  `offline-fixture-replay-agree`, never `pass`.

`lane_result` takes the first value that applies:

1. `stale-provenance`;
2. the reference status (`oracle-unavailable` or `oracle-error`);
3. `fail`: any disagreement;
4. `incomplete`: any other non-agreement, including cases of the corpus that
   were not run;
5. `pass` or `<label>-agree`.

`reference`, `compare` and `harness` exit with these statuses:

| Exit status | `lane_result` |
|---|---|
| 0 | pass or agree |
| 1 | `fail` |
| 2 | `incomplete` |
| 3 | `oracle-unavailable` |
| 4 | stale provenance or `oracle-error` |

An opt-in CI job can therefore tell all of these states apart.

## Provenance and staleness

| Recorded | Where |
|---|---|
| Policy version and adapter revision: SHA-256 of `interchange.mjs`, `bridge.py`, the helper sources and `Cargo.lock`, and `../oracle/generate.mjs` | every reference, fixture and report |
| Woxi version, helper version, time and memory settings, raw probe results | reference files (fresh Woxi), copied into recorded fixtures |
| Seed, counts, PRNG, generator file hashes | corpus `generator` |
| Corpus SHA-256 | reference files and reports |
| Noble binary SHA-256, owner-supplied source identity, build profile, runner commit and embedded-source hashes, runner/worker Node, WAT assembler version, run configuration | copied from the #19 run receipt into each report |
| Lane repository commit and tracked changes, Node version | each report |

`compare` and `harness` mark a report `stale-provenance` and issue no
verdicts when any of these holds:

- the policy version or adapter revision differs from the current files;
- a reference file was computed for a different corpus;
- one of the optional pins differs: `--expect-binary-sha256`,
  `--expect-source-id`, `--expect-woxi-version` or
  `--expect-helper-version`.

A fixture without a Woxi version therefore fails a Woxi pin. A fixture
whose interchange hash no longer matches a case gives
`no-recorded-expectation` for that case.

## Commands

Run the Node parts with the pinned Node (`$NOBLE_NODE/bin/node`). Every
output must be a new file outside the repository, except the output of
`record`.

```sh
L=verification/calculator-v1/woxi/lane.mjs
node $L corpus --seed 2424 --count 12 --wide 6 --out C.json   # edge fixtures + seeded cases
node $L vectors --corpus C.json --out V.json                  # #19 runner vector file
node verification/calculator-v1/acceptance.mjs --binary BIN --binary-sha256 SHA \
  --source-id SRC --build-profile release --out RUN_DIR --memory-max 2G --concurrency 6 --vectors V.json
# fresh Woxi reference (exit 3 when the helper is missing)
node $L reference --corpus C.json --arithmetic woxi \
  --bridge verification/calculator-v1/woxi/helper/target/release/noble-woxi-bridge \
  [--timeout 30] [--memory 1073741824] --out REF.json
node $L compare --corpus C.json --run RUN_DIR --reference REF.json \
  --expect-binary-sha256 SHA --expect-source-id SRC --out REPORT.json
# offline replay without Woxi (labeled offline-fixture-replay)
node $L compare --corpus C.json --run RUN_DIR --fixtures FIXTURES.json --out REPORT.json
# retain a fresh run's results as replay fixtures
node $L record --corpus C.json --reference REF.json --out FIXTURES.json
# harness-level #21 fault model as the implementation, with bounded shrinking
node $L harness --corpus C.json --reference REF.json --implementation fault:integer-division \
  --bridge BIN [--shrink-rounds 4] [--shrink-candidates 6] --out H.json
# one real-binary shrink round: the input plus at most N candidates as a corpus
node $L shrink --input '5 / 4 - 6 / 2' --shrink-candidates 3 --out S.json
```

**Helper setup.** Woxi is a crates.io dependency of the standalone helper
crate (`verification/calculator-v1/woxi/helper`), licensed AGPL-3.0-or-later;
it is not linked into any Noble production crate. Build it with the pinned
Rust toolchain:

```sh
cargo build --release --locked --manifest-path verification/calculator-v1/woxi/helper/Cargo.toml
```

The committed `Cargo.lock` pins the full dependency graph. The first build
needs crates.io access; subsequent builds run offline. The helper reports
`oracle-unavailable` when the binary is missing and `oracle-error` when it
crashes. `--arithmetic python-fractions` still uses the CPython stand-in and
needs `python3`, but it never counts as an oracle run.

**A real-binary shrink round.**

1. Run `shrink` to write a corpus: the original input followed by at most
   `N` strictly smaller candidates inside the subset, smallest first.
2. Run that corpus through `vectors`, the #19 runner and `reference`.
3. Run `compare`. Its `shrink_selection` names the smallest candidate whose
   whole classification signature (verdict, mismatch kind, reference outcome
   kind and category) equals the original's.

Repeat the round on that candidate as many times as you choose. The in-process
`harness` shrinker runs at most `--shrink-rounds` rounds of at most
`--shrink-candidates` candidates, and each round costs one bridge call. It
never accepts a candidate whose reference is unavailable, timed out or in
error. Every minimal case carries a `replay` object that holds:

- a one-vector runner file and its command, to replay the Noble execution;
- the bridge request and its command, to replay the oracle computation.

**Tests.** The tests need neither Woxi nor network access:

```sh
node --test verification/calculator-v1/woxi/lane.test.mjs
python3 -m unittest discover -s verification/calculator-v1/woxi -p 'test_*.py'
```

## Retained fixtures

- [`fixtures/edge-cases.json`](fixtures/edge-cases.json): the deterministic
  edge inputs. They include values beyond the I64 range, exact decimals such
  as `0.1 + 0.2`, every intentional difference, and the CALC-12 fault probes
  (`calc12_fault`).
- [`fixtures/edge-expected-stand-in.json`](fixtures/edge-expected-stand-in.json):
  expectations recorded from the python-fractions stand-in
  (`expectation_source: python-fractions-stand-in`). A test checks that this
  file is current for the adapter revision and that each expectation agrees
  with the independent #21 BigInt oracle. Replaying it is
  `offline-fixture-replay`. It is not a Woxi comparison.

No `woxi-recorded` fixture is committed. Before you commit one, check the
redistribution and retention terms of the licence of the kernel that produced
it.

## CALC-12 seeded faults: coverage of this lane

| CALC-12 fault | Covered here? | How |
|---|---|---|
| reversed-subtraction | yes | `10 - 3` gives `value-mismatch` |
| left-associated-power | yes | `2^3^2` gives `value-mismatch` |
| non-normalized-rational | yes | `2 / 4` gives `non-normalized-display` (same value, not normalized) |
| integer-division | yes | `7 / 2` gives `value-mismatch` |
| binary-float-decimal-parser | yes | `0.1` gives `value-mismatch` |
| I64-wrapping | yes | `9223372036854775807 + 1` gives `value-mismatch` |
| failure-mutates-session | **no** | stateful; sessions are outside this lane and stay with the #21 oracle |

For integer-division, binary-float-decimal-parser and I64-wrapping, the #21
mutant binaries could not run (`InfrastructureFailure`), so this lane reports
them as `not-comparable`. Their detection is shown only at the level of the
harness models.

The #21 invalid-evidence controls, grammar and span controls, and the
budget categories are not covered by this lane. [results.md](results.md)
lists the evidence for each row.

## Trust boundary

- Woxi is not a production dependency. Nothing in Cargo, the flake, the
  shipped calculator or the Wasm runtime changes. The bridge runs only in this
  verification lane, and only when you ask for it.
- Reference output is untrusted candidate data. It is validated and then used
  only as an expected test value. It mints no `Certified` status, discharges no
  proof obligation and changes no accepted semantics. The
  [certified-status invariant](../../../proofs/mc2/CertifiedInvariant.lean) is
  untouched.
- A matching result is a tested observation, not an independently checked
  proof. Certificate production and checking are out of scope here and belong
  to #6.
- Every report carries `non_claims` that say this.
