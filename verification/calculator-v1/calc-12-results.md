# CALC-12 results: independent oracle, seeded faults, invalid evidence

These are implementation observations recorded on 2026-10-15. They do not
change the state or evidence of any canonical case; CALC-12 stays
`absent` / `not-run`.

The lanes are documented separately: the [oracle](oracle/README.md), the
[seeded faults](seeded-faults/README.md) and the
[invalid-evidence controls](evidence-controls/README.md). Receipts are kept
outside the repository under `/home/brittonr/.cache/noble-issues/receipts/`;
the directory names are given below. Each comparison report records the
SHA-256 of the five oracle files and the Node version used (24.13.0, the
pinned `NOBLE_NODE`).

## Subjects

| Binary | SHA-256 | Source |
|---|---|---|
| `noble-44329dc-release` | `e1fb70ba…8127` | `44329dc`, release profile |
| `noble-control` (unpatched mutant base) | `97774049…aae6` | `ffcce53` (= `44329dc` + cargo fmt + vendor re-pin), built in the mutant target directory |
| `noble-integration-eccbc02-release` | `f7f02dfb…85ec` | `eccbc02` (integrated #16/#17/#18/#23) |
| seeded-fault mutants | see the fault table | `ffcce53` + `seeded-faults/patches/FAULT.patch` |

## Oracle comparison

Every row re-derives the expected streams from each process's retained stdin
and argv. Spans are judged only where the CALC-02 examples fix them.

| Input set | Binary | Receipt (#19 runner output) | Result |
|---|---|---|---|
| The cases file's hand-written expectations (no CLI involved) | none | `calc12-comparisons-20261015T205424Z/cases.json` | 38/38 agree |
| CALC-01 (15), CALC-02 (5), CALC-03 (4), CALC-06 session (1), CALC-07 CLI variants (5), CALC-09 (4) | 44329dc release | `calc-44329dc-release-20261015T172022Z` (#19 runner at `1e16953`) | **34/34 agree** |
| The same 34 vectors | eccbc02 integrated | `calc-integration-eccbc02-release-20261015T202105Z` (receipt on branch `verification/calc-integration-eccbc02`, commit `ee5eef7`) | **34/34 agree** |
| Seed 2112: 12 generated expressions and 2 generated sessions | 44329dc release | `calc12-gen2112-44329dc-release-20261015T170140Z` | 13 agree, 1 `budget-divergence` |
| #19 runner negative controls | both | as above | wrong-expected agrees with the oracle (the runner's wrong expectation is not used); forced timeout is `not-comparable` |

Notes:

- **Integrated build.** The integrated build `eccbc02` passed the #19 runner's
  CALC-01/02/03/06/07/09 sweep 34/34, with outputs identical to `44329dc`. The
  oracle comparison of its retained raw I/O gives the same 34/34 agreement.
- **The one budget divergence.** Generated input `2^64 + 1` stops with
  `ResourceLimit EvaluationWork [5,6)` under the default budgets after 1,295 s.
  The oracle has no budget model, so this is reported as a divergence, never
  as agreement or disagreement. The canonical boundary vector
  `9223372036854775807 + 1` (CALC-01/04) does agree:
  `9223372036854775808`.
- **Generated sessions.** Both generated sessions agree, including the
  failed-`let` case: `let x = 3 / 0` reports `DivisionByZero`, and the
  following `x` still prints `8`.
- **Spans the spec does not fix.** These are recorded but not judged. Examples:
  - `DivisionByZero` is reported on the `/` operator;
  - CALC-06's `WrongArity` is observed at `[0,1)`, the callee name;
  - CALC-07's `DefinitionError` is observed at `[8,9)`.

## Seeded incorrect implementations

Each mutant ran its probe as a separate #19-runner process, with
expectations from the oracle. "Rejected for the intended reason" requires two
things: the verdict is `disagree`, and the only fault model that predicts the
observed output is the seeded fault itself (`matches_fault_models`). The
unpatched control ran all seven probes on the same base and target directory.
The four rejected mutants and the control pass the calculator's startup source
check (all eleven modules type-check; empty input, exit 0).

| Fault | Mechanism | Probe | Oracle | Mutant output | Verdict / attribution | Receipt |
|---|---|---|---|---|---|---|
| reversed-subtraction | guest `op_begin_sub` operand swap | `10 - 3` | `7` | `-7` | disagree / `[reversed-subtraction]` | `calc12-fault-reversed-subtraction-20261015T172118Z` |
| left-associated-power | guest `should_pop` pops at equal `^` precedence | `2^3^2` | `512` | `64` | disagree / `[left-associated-power]` | `calc12-fault-left-associated-power-20261015T182129Z` |
| non-normalized-rational | guest `gcd_unit_bits_p` always true | `2 / 4` | `1/2` | `2/4` | disagree / `[non-normalized-rational]` | `calc12-fault-non-normalized-rational-20261015T173239Z` |
| failure-mutates-session | host `Session::submit` clears accepted history on failure | `let x = 20` / `let x = 1 / 0` / `x` | `20`, `DivisionByZero`, `20` | `20`, `DivisionByZero`, `UnknownName [0,1)` | disagree / `[failure-mutates-session]` | `calc12-fault-failure-mutates-session-20261015T180239Z` |
| integer-division | guest `op_divisor_admit`, truncating I64 quotient for integer operands | `7 / 2` | `7/2` | none: `InfrastructureFailure … calculator selected environment clone fanout exceeded` | **not comparable** (backend refused the phase) | `calc12-fault-integer-division-20261015T210028Z` |
| I64-wrapping | guest `op_begin_add/sub/mul`, Core wrapping I64 words for integer operands | `9223372036854775807 + 1` | `9223372036854775808` | none: `InfrastructureFailure … calculator selected environment clone fanout exceeded` | **not comparable** (backend refused the phase) | `calc12-fault-I64-wrapping-20261015T210028Z` |
| binary-float-decimal-parser | guest `parse_decimal_finish`, 2^-55 rounding with I64 words | `0.1` | `1/10` | none: `InfrastructureFailure … independent source compilation refused (exhausted)` | **not comparable** (backend refused the phase) | `calc12-fault-binary-float-decimal-parser-20261015T210028Z` |
| control (unpatched) | none | all seven probes | as above | equal to the oracle | **7/7 agree** | `calc12-fault-control-20261015T184937Z` |

**Bounded shrinking against a real binary.** One shrink round ran on the
reversed-subtraction mutant: the input `5 / 4 - 6 / 2` plus at most three
candidates (receipt
`calc12-shrink-reversed-subtraction-20261015T181002Z`). The input and the
candidate `4 - 6 / 2` still disagree (`7/4` vs `-7/4`, `-1` vs `1`), and both
are attributed to `[reversed-subtraction]`. The subtraction-free candidates
`5 / 4` and `6 / 2` agree. The smallest disagreeing input of that round is
therefore `4 - 6 / 2`.

The same seven faults were also run as harness-level models (`compare.mjs
harness`) on the seed-2112 set. Each model disagrees on at least one generated
vector, and each disagreement is attributed only to itself. In-process
bounded shrinking reduces the disagreements, for example:

- `11 / (2 + 8.68)` to `8.68` for the binary-float model;
- `5 / 4 - 6 / 2` to `1 - 2` for the reversed-subtraction model.

### Three mutants the real binary could not run

For integer-division, I64-wrapping and binary-float-decimal-parser, every
mutant binary passed or failed its startup check as listed below, and every
run that started ended in a host `InfrastructureFailure` when the selected
calculator backend refused to compile the mutated phase. The comparison
classifies such a run `not-comparable`. **None of these runs is counted as a
rejection.** The runs show that these patches break the backend's phase
limits; they do not show that the oracle comparison catches the arithmetic
defect on the real binary.

| Fault | Attempt | Placement | Startup check | Observed on the probe | Receipt |
|---|---|---|---|---|---|
| integer-division | 1 | BigInt long division inlined into `op_first_stage_div_raw` | exit 0 | `independent source compilation refused (exhausted)` | `calc12-fault-integer-division-20261015T174124Z` |
| integer-division | 2 (committed patch) | truncating I64 quotient in `op_divisor_admit` | exit 0 | `calculator selected environment clone fanout exceeded` | `calc12-fault-integer-division-20261015T210028Z` |
| I64-wrapping | 1 | two's-complement wrap from bit-list division in `big_add_finish_add/_sub` | exit 0 | `independent source compilation refused (exhausted)` | `calc12-fault-I64-wrapping-20261015T183523Z` |
| I64-wrapping | 2 (committed patch) | Core wrapping `+ - *` in `op_begin_add/sub/mul` | exit 0 | `calculator selected environment clone fanout exceeded` | `calc12-fault-I64-wrapping-20261015T210028Z` |
| binary-float-decimal-parser | 1 | BigInt multiply/add/divide rounding in `parse_decimal_finish` | exit 0 | `calculator environment row limit exceeded` | `calc12-fault-binary-float-decimal-parser-20261015T180744Z` |
| binary-float-decimal-parser | 2 (committed patch) | the same rounding with I64 words in `parse_decimal_finish` | exit 0 | `independent source compilation refused (exhausted)` | `calc12-fault-binary-float-decimal-parser-20261015T210028Z` |
| binary-float-decimal-parser | 3 | a new `num@1.float_round_raw` definition run as a separate host phase | **exit 2**: `num@1: Check [0,0): module schema/definition count exceeded` | not run | `calc12-fault-binary-float-decimal-parser-20261015T210449Z` |

The startup checks and the attempt-2/3 comparisons are retained in
`calc12-comparisons-attempt2-20261015T210525Z`. That directory's
`startup.txt` lists the attempt-3 binary-float mutant; the attempt-2
binary-float mutant (`d3d8c611…f4dc`) was checked separately and exits 0.

The calculator's selected backend bounds how much one compiled phase may
reach (named bodies, environment rows, clone fanout, compiler Work) and how
many definitions a module may hold. Each of these three defects needs extra
arithmetic in a phase that is already near those bounds. Fitting them would
require restructuring the calculator's phase split, which is outside a seeded
fault.

**What the harness-level models show for these three faults.** The models in
[`oracle/faults.mjs`](oracle/faults.mjs) ran as the implementation on the
same seed-2112 vectors and on the seven fault probes (reports
`harness-FAULT-gen2112.json` and `harness-FAULT-fault-probes.json` in
`calc12-comparisons-20261015T205424Z`). Every disagreement below is
attributed only to the fault itself, and bounded shrinking reduces it:

| Fault model | Disagreeing input | Model output | Oracle | Shrunk to |
|---|---|---|---|---|
| integer-division | `7 / 2` (probe) | `3` | `7/2` | `1 / 2` |
| integer-division | `11 / (2 + 8.68)`, `5 / 4 - 6 / 2`, `4 / 8 - 2 / 6` (generated) | `1`, `-2`, `0` | `275/267`, `-7/4`, `1/6` | `1 / 2`, `1 / 2`, `1 / 3` |
| I64-wrapping | `9223372036854775807 + 1` (probe) | `-9223372036854775808` | `9223372036854775808` | unchanged |
| I64-wrapping | `2^64 + 1` (generated) | `1` | `18446744073709551617` | `2^64` |
| binary-float-decimal-parser | `0.1` (probe) | `3602879701896397/36028797018963968` | `1/10` | unchanged |
| binary-float-decimal-parser | `11 / (2 + 8.68)`, `2.3 - (0.5 + 2)` (generated) | binary fractions | `275/267`, `-1/5` | `8.68`, `2.3` |

This is harness-level evidence only: it shows that the comparison and the
attribution reject these defects when an implementation exhibits them. A real
mutant binary for these three faults remains open.

## Invalid-evidence controls

Receipt: `calc12-evidence-controls-20261015T205438Z/receipt.json`.

- **Subject.** The 44329dc release binary plus the eleven embedded fixtures.
- **Unrelated Wasm.** A real Core-Bootstrap `1 2 +` module that the same
  binary emitted.
- **Result.** `all_ok: true`.

| Control | Decision | Refusal codes |
|---|---|---|
| proof-for-wrapping-I64 | refused | `wrong-numeric-model` |
| proof-for-handwritten-reference-only | refused | `subject-not-actual-implementation` |
| proof-with-unmet-zero-divisor-precondition | refused | `unmet-zero-divisor-precondition` |
| unrelated-Wasm | refused | `unrelated-wasm` |
| calibration: test claimed universal | refused | `tests-claimed-universal` |
| calibration: bound proof shape | not-refused-structurally (not acceptance) | none |
| calibration: bound test observation (cites the seed-2112 comparison) | admissible-test-observation | none |

Each invalid control is refused with exactly its intended code and no other.

## Non-claims

- **Finite inputs only.** Agreement on finitely many inputs (38 cases-file
  expectations, 34 canonical vectors on two binaries, and 13 generated
  inputs) does not establish parser correctness or any universal arithmetic
  theorem (CALC-EVIDENCE-01).
- **No proof.** No proof is made, checked or accepted.
  `not-refused-structurally` is not acceptance (CALC-EVIDENCE-02 proof
  reports are out of scope).
- **Seeded faults are partial on the real binary.** Four of seven faults are
  rejected as real mutant binaries; integer-division, I64-wrapping and
  binary-float-decimal-parser are rejected only as harness-level models,
  because their mutant binaries could not run (see above).
- **Not proven exhaustive.** The seeded faults show that this harness detects
  these seven defects on these probes. They do not show that it detects every
  defect.
- **Budgets not modelled.** The oracle has no budget model. `ResourceLimit`
  results are reported, not judged.
- **No promotion.** No canonical case state or evidence changes.
  `tasks.md` L14 stays open.
