# CALC-13 implementation-entry-gate review against 44329dc

**Subject:** `44329dc`, the WIP guest-backed `noble calculator`.

**Procedure:** the `review` procedure of
[CALC-13](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/conformance/calculator-cases.json#L167-L173).
It walks the eight required gates and the five substitute variants
([CALC-GATE-01](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/.cairn/specs/calculator/spec.md?plain=1#L580-L607),
CALC-SCOPE-01/02, CALC-BOUND-01).

**This is a review record, not a promotion.** CALC-13 stays
`implementation: absent`, `execution: not-run`, `proof: open`,
`trust: unassessed`, `evidence: []`. No other case changes either.

**Method:**

- **Source reading at 44329dc.** Rust line numbers are 44329dc's; this branch's
  base `ffcce53` only reformatted Rust files.
- **Runtime observations.** These come from the CALC-12 oracle lane on this
  branch ([results](calc-12-results.md)) and from the #19 acceptance runner,
  both run on the 44329dc release binary.

Unless a link says otherwise, every link below is a permalink at
`44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2`.

Status vocabulary:

- **evidence, unpromoted:** executed or recorded evidence exists for that
  scope, but it is not CALC-13 evidence.
- **partial:** code exists, or only non-source-bound observations, while the
  gate's contract is still open.
- **open:** neither a contract nor evidence exists.

## Verdict

At 44329dc the calculator is **not** a complete calculator under CALC-GATE-01.

- **Open gates.** Five of the eight required gates are open or only partly
  evidenced at the contract level: exact-numeric-library interfaces,
  iteration, budget accounting, definition-command grammar, and M4 renewal on
  the current source. The tree is therefore itself an instance of the
  `open-required-library-gate` substitute.
- **Substitutes.** The other four substitutes are each distinguishable from
  the implementation by source evidence, as recorded below.
- **Expected fields.** On CALC-13's own expected fields:
  - `bootstrap_expanded: false` is supported.
  - `calculator_blocks_M4: false` is supported.
  - `new_kernel_mechanisms_required: false` is **in tension** with
    calculator-only kernel behavior rows. This is recorded as an open question
    below, not decided here.

## Required gates

| Gate | Status | Evidence at 44329dc | Open |
|---|---|---|---|
| M4 | evidence, unpromoted (for M4 itself) | [roadmap.json L113-L144](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/roadmap.json#L113-L144): M4 `completed` / `passed`, completion record [verification/m4/evidence.json](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/verification/m4/evidence.json#L1-L10). M1–M4 do not depend on the calculator ([ROADMAP.md L100](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/ROADMAP.md?plain=1#L100)). | The M4 evidence predates the calculator WIP (2026-09-21). The current-source boundary gate is recorded `NOT PASSED` ([STATUS.json L24](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/STATUS.json#L24)). No M4 regression run on 44329dc exists, so renewal is **open**. |
| exact-numeric-library | partial | Guest types: `calc_big@1.BigInt` = sign plus LSB-first `List<Bool>` ([calculator-bigint.noble L1-L3](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-contracts/src/fixtures/calculator-bigint.noble#L1-L3)), `num@1.Rat` ([calculator-numeric.noble L1-L32](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-contracts/src/fixtures/calculator-numeric.noble#L1-L32)), and the exact decimal cursor (`calculator-decimal.noble`). `Cargo.lock` has no bignum crate. On the 44329dc release binary the oracle lane agrees on all 34 canonical CALC-01/02/03/06/07/09 vectors and on 13 of 14 seed-2112 generated inputs; the 14th (`2^64 + 1`) stops with `ResourceLimit EvaluationWork` under the default budgets, which the oracle cannot judge ([results](calc-12-results.md)). | CALC-SCOPE-02 requires a separately specified library; none exists. CALC-NUM-06 "library interface gate" ([spec L188-L193](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/.cairn/specs/calculator/spec.md?plain=1#L188-L193)) is open: `numeric_library_interfaces: open` ([STATUS.json L984](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/STATUS.json#L984)). MA1 `not-started` ([roadmap.json L219-L244](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/roadmap.json#L219-L244)). CALC-04 and CALC-05 are not run (#20). |
| declarations-and-modules | partial | Declared-Modules-v1 (DXM1) is passed for its scope ([STATUS.json L238-L255](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/STATUS.json#L238-L255)). The calculator registers 11 versioned modules in its own `ModuleSession::new_calculator` ([calculator.rs L273-L305](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L273-L305); [state.rs L217-L220](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-contracts/src/source/declared/state.rs#L217-L220)). | DXM1 excludes general types, Result and local bindings ([roadmap.json L544-L583](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/roadmap.json#L544-L583)). The calculator relies on cross-module nominal signatures ([README L214-L216](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/README.md?plain=1#L214-L216)) that no DXM1 receipt covers. The [calculator-errors README L18-L23](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/verification/calculator-errors/README.md?plain=1#L18-L23) still calls that an upstream gate. |
| iteration-or-recursion | partial; no gate contract | A calculator-only builtin `iterate` (ID 27): source resolution in [resolution.rs L213-L224](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-contracts/src/source/resolution.rs#L213-L224), kernel scheme in [contracts.rs L575-L639](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-kernel/src/contracts.rs#L575-L639), runtime in [calculator.wat L1-L14](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-wasm/runtime/calculator.wat#L1-L14). Recursion is refused, and CALC-STATE-01 functions are nonrecursive. | No requirement ID, ledger case or design section covers `iterate`. The gate name appears only in GATE-01 and the roadmap. **Open.** |
| text-processing | partial | The calculator lexer reads bytes with `text.byte` ([calculator-lexical.noble L1-L14](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-contracts/src/fixtures/calculator-lexical.noble#L1-L14)). It also uses the calculator-only `text.from-ascii` and `text.length`. | Text-Byte-Cursor-v1 is an unpromoted change, and TEXT-01 is absent/not-run with `calculator_accepted: false` ([text-byte-cursor-cases.json L1-L44](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/conformance/text-byte-cursor-cases.json#L1-L44)). Calculator-v1 is a disjoint profile, so TEXT-01 would not cover builtins 30 and 31 even if run. |
| error-schemas | partial | `calc_errors@1` declares the seven limit categories and the domain errors ([calc-errors.noble L1-L65](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-contracts/src/fixtures/calc-errors.noble#L1-L65)). `result@1` and the result library DX-04 are passed and scoped ([STATUS.json L256-L271](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/STATUS.json#L256-L271)). | The calculator-errors gate is an observation that is not source-bound, with no ledger case ([README L25-L34](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/verification/calculator-errors/README.md?plain=1#L25-L34)). The CALC-NUM-06 `Result` schema interface gate is open. The host duplicates the diagnostic arm layout in Rust ([calculator.rs L4948-L5024](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L4948-L5024)). |
| budget-accounting | partial; contract open | EvaluationWork has a unit and accounting model ([design.md L33-L52](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/.cairn/changes/calculator-v1/design.md?plain=1#L33-L52)). The seven slots and defaults are in [calculator.rs L21-L25](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L21-L25). | **Six non-Work categories have no documented unit at 44329dc:** InputBytes, NumericDigits, IntermediateIntegerBits, SyntaxDepth, CallDepth, OutputSize. CALC-08 records `accounting_contract: implementation-entry-gate-open`, STATUS records `budget_accounting: open` ([L985](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/STATUS.json#L985)), and calculator-v1 task L13 is unchecked. See [Budget units](#budget-units-for-the-six-non-work-categories). |
| definition-command-grammar | **open** at the contract level; implemented in the guest | The guest `calc_defs@1` recognizes `let` and `fn` ([calculator-definitions.noble L424-L428, L479-L493](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-contracts/src/fixtures/calculator-definitions.noble#L424-L493)). The host only branches on the guest opcode ([calculator.rs L622-L624](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L622-L624)). The README documents `let NAME = EXPR` / `fn NAME(P, ...)=EXPR` ([README L177-L191](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/README.md?plain=1#L177-L191)). | The spec still calls the spelling a CLI design gate ([spec L318](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/.cairn/specs/calculator/spec.md?plain=1#L318)). STATUS records `definition_command_grammar: open` ([L983](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/STATUS.json#L983)), and MA2 lists it as an entry gate. The undocumented observable behavior the gate must also settle is listed in [Definition-command grammar](#definition-command-grammar). |

### Budget units for the six non-Work categories

At 44329dc only EvaluationWork has units. The other six are open in
design.md, the spec and the README.

Issue #20 has written units for all seven categories into
`.cairn/changes/calculator-v1/design.md` on branch
`test/issue-20-calc-budgets`, commit `cf088de` (owner
`noble-calc-budgets-20`). At the time of this review that commit is
**committed but unreviewed and not on `main`**, so the gate stays open at
44329dc.

The #20 worker's earlier summary of its reading of the guest code is below.
It is cited only as status; this review did not check it against `cf088de`
or run it:

| Category | Draft unit |
|---|---|
| InputBytes | line byte length, debited once at preparse (slot 0); per `cf088de` it counts the newline of unframed input |
| NumericDigits | cumulative decimal digits per line, decimal point excluded (slot 1) |
| IntermediateIntegerBits | admission-only check, not consumed: every measured width (literal estimate, operator width sum plus 2, gcd and finish widths, display width) is compared with slot 2 |
| SyntaxDepth | parenthesis nesting depth (slot 3) |
| CallDepth | one per entered user-function call, restored on return (slot 4) |
| OutputSize | display characters (slot 6) |

**What closes this gate:** the #20 design text, reviewed against the guest
code, plus CALC-08 receipts with each limit exceeded independently. Both are
#20 deliverables.

### Definition-command grammar

The guest implements one concrete spelling. The CLI design gate still has to
record these observable behaviors:

- **Value echo.** A successful `let NAME = EXPR` returns `Outcome::Value` and
  prints its value. A successful `fn` prints nothing
  ([calculator.rs run loop L114-L131](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L114-L131)).
  The spec does not say this. The #19 mapping first assumed `let` was silent,
  then corrected that in #19 commit `1e16953`. The CALC-12 oracle follows
  CALC-STATE-02's "accepted value".
- **Header recognition.** Header recognition is token-based in the guest, with
  `=` as lexer token 14.
- **Function names.** `fn` names share the single namespace with variables.
- **Replay history.** Only state-changing lines (`let`/`fn`) enter replay
  history, bounded by the HistoryCommands, HistoryBytes and ReplayWork host
  limits.

## Substitute variants (each must be rejected as a complete calculator)

| Variant | Finding at 44329dc | Status |
|---|---|---|
| `host-only-calculator` | Parsing, name resolution, exact arithmetic and display run in the guest. Every phase is a source-checked, compiled and executed Noble module (`execute` path: [calculator.rs L4543-L4606](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L4543-L4606)). The result value is guest Text ([decode_result L4634-L4652](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L4634-L4652)). No Rust bignum or rational crate is linked. The seeded-fault lane is runtime corroboration: editing a guest fixture alone (reversed subtraction, left-associated power, gcd always one) changes the binary's printed result (`-7`, `64`, `2/4`; [results](calc-12-results.md)), which a host-computed result could not do. The truncating-division, wrapped-sum and binary-literal guest mutants could not be compared, because the backend refused their phases. | rejected for 44329dc (source plus seeded-fault observation) |
| `I64-only-demo` | Values are unbounded sign-magnitude bit lists. `9223372036854775807 + 1` prints `9223372036854775808` on the 44329dc release binary (CALC-01/04, oracle agreement in [results](calc-12-results.md)). The generated `2^64 + 1` stops with `ResourceLimit EvaluationWork` under the default budgets, an explicit limit rather than a wrapped value. Core I64 semantics are unchanged. | rejected for the observed vectors; CALC-04 and CALC-05 are not run |
| `ambient-file-access` | The guest gets no host functions (`engine.hostFunctions = calculator ? [] : …`, [engine-host.mjs L43](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/core/runtime/engine-host.mjs#L43)). Undeclared module imports are refused ([engine.mjs L913](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/core/runtime/engine.mjs#L913)), and calculator injection is limited to Text, I64 and Bool ([L1150](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/core/runtime/engine.mjs#L1150)). The Node worker runs with `env_clear()` ([worker.rs L506](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/core/worker.rs#L506)). | rejected at the guest boundary. The worker *process* still has the host's file and process capabilities: it writes a temp dir and spawns the WAT assembler. No OS sandbox was found [INFERENCE]. That is shell authority, not guest authority. |
| `application-parser-as-Noble-backend` | The calculator parser and evaluator (`expr@1` shunting-yard plan and evaluation frames) are Noble modules compiled by the selected backend (`noble-wasm`). They interpret calculator expressions, and CALC-SCOPE-01 says such an interpreter "does not satisfy Noble's compiled-program backend obligations". The backend obligation therefore stays with M4/noble-wasm, which this calculator *uses* rather than replaces. Glue sources are host-authored, but only from fixed literals and guest word names; user bytes enter only as a Text input frame. | rejected as a backend claim; nothing at 44329dc claims otherwise |
| `open-required-library-gate` | Five required gates are open or only partly evidenced at the contract level (table above). MA1 is `not-started`, and ROADMAP calls these entry gates "open contracts, not completed dependencies" ([ROADMAP.md L96](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/ROADMAP.md?plain=1#L96)). | **applies to 44329dc**, so the tree is not a complete calculator |

## Host shell boundary

design.md claims a "pure typed guest, narrow terminal shell"
([L9-L13](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/.cairn/changes/calculator-v1/design.md?plain=1#L9-L13)).
The host's actual responsibilities at 44329dc are listed below. Lines refer to
`crates/noble-cli/src/calculator.rs` unless noted.

1. **Terminal and options.**
   - Framing and the 65,536-byte input frame (`TransportLimit InputFrameBytes`, exit 2).
   - Seven budget flags with defaults `[1024, 256, 4096, 32, 16, 4000000, 4096]`.
   - stdout for values; stderr for guest diagnostics, host limits and native or infrastructure failures.
   - Exit status: 1 if any line failed, 2 on a fatal error.
   - Code: [L82-L174](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L82-L174).
2. **Orchestration.** One line is a sequence of hundreds of small, separately
   checked and compiled guest submissions in **one** candidate
   `ModuleSession`, compiler and worker: lexer, header, budget injection,
   shunting-yard parser, evaluation frames, normalization and display
   ([`submit_line` L448-L4285](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L448-L4285)).
   - The host chooses the next phase from guest-certified kinds, opcodes,
     cursors and widths. For example, binary operator kind 1 to 5 selects
     `num.op_begin_*` or `op_divisor_*`.
   - It enforces host progress bounds derived from the budget slots
     (`budget[5] + 3·len + 1` evaluator steps, `len + 1` parser steps). These
     fail as errors, never as `ResourceLimit`.
3. **Replay.** Every line starts from a fresh candidate (`Self::new()`: 11
   modules, a new compiler, a new worker) and replays the accepted `let`/`fn`
   lines in order.
   - It commits only on `Value` or `Defined` with the guest stack exactly
     `[calc_env@1.State]`; otherwise the candidate is discarded.
   - Replay is bounded by the host limits HistoryCommands (64), HistoryBytes
     (65,536) and ReplayWork (`bytes + 128·count ≤ 65,536`), each labelled
     `provenance=host-replay-preflight`.
   - A failed replay is fatal (exit 2).
   - Code: [L361-L431](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L361-L431).
4. **Body admission.** `fn` bodies are admitted without evaluation through
   guest `calc_defs@1.begin_session_*` steps
   ([L4291-L4499](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L4291-L4499)).
   - The host reads each body token's guest-reported kind and width, then
     dispatches the name, call or ordinary check. The checks themselves
     (parameter match, global search, self-recursion, arity) are guest words.
   - The scheduling bound `work + 16·len + 64`
     ([L4313-L4314](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L4313-L4314))
     ends in an infrastructure failure, not a guest diagnostic.
5. **Rendering.** The host maps guest diagnostic variant arms to category
   strings and appends `provenance=unavailable`
   ([L4948-L4983](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L4948-L4983)).
   It also re-checks output length against `budget[6]` and 65,536
   ([L4267-L4284](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L4267-L4284)).

**Assessment against design.md.**

- **Consistent:**
  - The host never parses source bytes, resolves a name or computes a rational.
  - Display mode is a guest decision; the host passes one Bool.
  - All phases of a line share one candidate.
  - The guest gets no filesystem, network, clock or process imports.
  - Native failures are printed as `NativeFailure` or `InfrastructureFailure`,
    never converted to guest results.
- **Gray zones, which a narrower shell contract would have to state
  explicitly:**
  - The host **inspects public guest outputs** (kinds, cursors, widths, Bools,
    variant arms) to schedule phases. design.md allows "schedule a
    guest-certified action kind", but the shell is a large phase interpreter
    (about 4,000 lines), not a thin loop.
  - The **error-schema arm layout is duplicated in Rust**.
  - **Budget-derived progress bounds and the output-length re-check** are
    host policy alongside the guest's own accounting.
  - The **seeded `failure-mutates-session` fault** is a one-line change to
    `submit` ([seeded faults](seeded-faults/README.md)). Transactional state
    correctness therefore rests on the host shell, not on the guest. This
    branch's oracle lane catches that fault.

## Expected fields of CALC-13

| Field | Observation |
|---|---|
| `outcome: reject-each-as-complete-calculator` | Each variant is assessed in the table above. `open-required-library-gate` applies to 44329dc. |
| `bootstrap_expanded: false` | Supported. Calculator builtins resolve only in the Calculator-v1 profile, which is disjoint from Core-Bootstrap and the other profiles ([engine-host.mjs L8-L13](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/core/runtime/engine-host.mjs#L8-L13)). |
| `calculator_blocks_M4: false` | Supported ([ROADMAP.md L100](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/ROADMAP.md?plain=1#L100); validator control `calculator-blocks-bootstrap`). |
| `new_kernel_mechanisms_required: false` | **Open question.** The kernel adds calculator-only `Behavior` rows (iterate, compare, div-rem, text-from-ascii, text-length; [contracts.rs L84-L97](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-kernel/src/contracts.rs#L84-L97)), and the runtime adds calculator-only ops (`calculator.wat`). Whether profile-scoped builtin rows count as "new kernel mechanisms" under CALC-SCOPE-02 ([spec L45](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/.cairn/specs/calculator/spec.md?plain=1#L45)) is not decided by this review. |

## Documentation discrepancies found during review

These are recorded, not fixed here:

- **Environment rows.** design.md L23 and the delta spec say 768
  environment rows; `CALCULATOR_ENVIRONMENT_ROW_LIMIT` is 896
  ([clone_budget.rs L10](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-kernel/src/contracts/clone_budget.rs#L10)),
  as is the README.
- **Module count.** design.md L31 says "ten separately checked calculator guest
  modules"; the shell registers eleven, including `result@1`.
- **Memory reclamation.** The calculator-v1 delta spec says no memory
  reclamation is implied, while design.md, tasks.md and the README describe
  `collect` compaction, table-slot reuse and signature-pool restart.
- **Signature gate.** The calculator-errors README still calls typed nominal
  source signatures an upstream gate that the README says now exists.
- **`let` value echo.** It is undocumented (see the definition-command grammar
  section above).

## What would change this review

- The library, iteration and budget-accounting gates are closed with
  contracts and receipts on the reviewed source.
- The definition-command grammar is selected in the spec, including the
  `let` echo.
- M4 regression evidence exists on the calculator source.
- The `new_kernel_mechanisms_required` question is decided.
- CALC-01 through CALC-09 receipts exist on the same source, with the CALC-12
  oracle and seeded faults rerun on that source.

Promotion of any case still requires the separate review gate.
