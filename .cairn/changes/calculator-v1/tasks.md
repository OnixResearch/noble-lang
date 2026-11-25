# Calculator-v1 active change tasks

## Selected contract and compiler boundary

- [x] [serial] Record an active, unselected calculator-only application delta without editing canonical CALC requirements/cases or inherited M1–M4 evidence. Preserve the distinction between guest results, host limits, backend refusal and native failure. r[CALC-GATE-01] r[CALC-LIMIT-01]
- [x] [serial] Implement calculator-only independent source/backend structural admission with separate 512 named-body and 896 environment-row guards, one cumulative preparation work ledger capped at 2,000,000,000 issued units, selected 16-to-64-page managed memory / 72,362 cells (monotonic within a submission; reclaimed only by the boundary compaction below) with unchanged per-invocation allocation ceiling, and immutable signature observations while ordinary/cursor profiles remain unchanged. Exercise scoped hostile structural inputs and real selected Wasm sessions; do not claim proof or full CALC case acceptance from those checks. r[CALC-BOUND-01] r[CALC-LIMIT-01]
- [x] [serial] Add calculator-only boundary compaction: the runtime `collect` export runs before `begin`. It is order-preserving and rooted only in operand slots, slides live dynamic Text, zeroes the cell tail, certificate table and observation globals, and moves nothing while a reachable Program runs compiled code. It fails closed on malformed roots, edges or Text order. The engine refuses calculator `park` and `project` and reports `collection {cells_before, cells_after}`. Keep every cap and the shared/pinned runtime assets byte-identical. Exercise a garbage-churn regression that exhausts without compaction and preserves and executes live aggregates, Text and capture Programs with it. Include stale-handle, idempotence, compiled-Program, exact 72,362-cell boundary, poisoned, malformed and non-monotone dynamic-Text negative cases, and an end-to-end CLI line. This claims no CALC case and no proof. r[CALC-BOUND-01] r[CALC-LIMIT-01]
- [x] [serial] Reuse dead calculator compiled-code table slots: after each normal submission the runtime's read-only `compiled_programs_reachable` export (the `collect` closure) reports whether a Program reachable from the operand stack runs compiled code; the engine reports it, and when it is false the host prepares the next module with `Compiler::prepare_quiescent`, which restarts only that preparation's function allocation at the fixed base 5 (commit publishes it; a refused preparation changes nothing). Keep TABLE_LIMIT and every cap, and keep ordinary/live/cursor/declared output byte-identical. Exercise a regression where repeated dead quotation bursts exhaust the 16,384-entry table without reuse and keep fitting for over twice as many modules with it, while a live compiled Program, including one inside an aggregate, keeps its slots reserved and still runs. This claims no CALC case and no proof. r[CALC-BOUND-01] r[CALC-LIMIT-01]
- [x] [serial] Reuse exactly equal checked calculator modules (issue #16 option 1B): record each calculator Program-free preparation at a canonical compiler epoch (generation-0 flag, `text_end` and the content-addressed static literal table) and reuse it only for the same namespace recipe, exact source bytes and exact input stack types through `Compiler::reuse_program_free`, which refuses any other epoch; bind the submit generation per instance instead of baking it into calculator WAT; let each worker reuse the assembled Wasm and V8 Module of byte-identical WAT; bound the host (96 MiB accounted) and worker (64 MiB WAT+Wasm) caches. For the cumulative per-preparation Work ledger above, a reused module issues no new Work and carries the Work its recorded preparation issued; it publishes exactly the recorded signature keys. Keep every cap, keep ordinary/live/cursor/declared output byte-identical and execute every guest module. Exercise lockstep fresh-versus-reuse arenas (identical WAT, Work, table spans, execution and stacks), refusal of non-canonical compiler states and stale generation bindings, cache key/bound unit tests, an exact-WAT worker test and a CLI trace comparison with reuse off and on. On the committed session (#17), neither cache is part of the commit or the restore: an entry recorded by a module of a failed, restored line stays exact at its full key and is reused only when a later line reaches exactly its canonical epoch; restore removes the undone worker artifacts but keeps compiled Modules. Exercise this with `calculator::tests::failed_line_module_cache_entries_hit_only_at_their_exact_epoch` (cache-on session after a failed line versus a cache-off session: identical stacks, compiler, flags and arena; namespace content identical) and the Node `calculator restore keeps exact-WAT modules and removes the undone artifacts` test. Observations (no case state): with reuse on and off, a debug build of the merged tree `0b8c7f9` ran `1+2` twice with byte-identical WAT, Wasm, execution reports and issued Work per module (308 s vs 1,088 s; the second line fully reused); the integrated CALC-01/02/03/06/07/09 CLI sweep on release `eccbc02` (receipt `verification/calculator-v1/receipt-eccbc02-release.json`) passed 34/34 runnable vectors with outputs and spans identical to the `44329dc` receipt. r[CALC-GATE-01] r[CALC-LIMIT-01]

## Complete guest application and observed behavior

- [ ] [serial] Finish pure Noble exact-number, parser, evaluator, lexical session and default/finite exact display paths through checked typed guest phases in one candidate compiler/worker per line. Validate the complete public command/diagnostic contract, not merely a source module or a convenient arithmetic vector. r[CALC-SCOPE-01] r[CALC-GATE-01]
  - Progress (2026-11-20): the complete public command/diagnostic contract is written into
    `design.md` ("Public command and diagnostic contract") and the CALC-STATE-02 delta under owner
    decision 15: `let NAME = EXPR` echoes the value, `fn NAME(P, ...) = EXPR` is silent, whitespace
    between tokens is insignificant, a refusal prints exactly
    `Category [start,end) provenance=unavailable`, and exit status is 0/1/2. The existing CLI is
    checked against it in `crates/noble-cli/src/calculator.rs` (`report`, `parse_options`,
    `diagnostic`), and three unit tests pin it on `cb5e9f71`:
    `calculator::tests::line_outcomes_map_to_the_public_streams_and_exit_status`,
    `calculator::tests::options_select_the_documented_budgets_and_refuse_a_bad_command` and
    `calculator::tests::diagnostics_print_the_public_line_and_refuse_invented_spans`. CALC-09's
    six `later_features` are observable: `verification/calculator-v1/cli-mapping.json` records one
    probe per feature and `vectors.mjs` generates `CALC-09/later-features/01` to `06` (the
    generated set is now 40 vectors). A release binary of `cb5e9f71` (sha256 `b4b2931a…2d1736`)
    refused all six explicitly with exit status 1 and no stdout line: `2^0.5`
    `NonIntegerExponent [1,2)`, `x + 1` `UnknownName [0,1)`, `sin(1)` `UnknownName [0,3)`,
    `1 m` `TrailingInput [2,3)`, `sqrt(2)` `UnknownName [0,4)` and `1 + 2i`
    `TrailingInput [5,6)`; the raw probe I/O is retained outside the repository. This task stays
    open: the six new vectors have no current-source acceptance receipt yet, so the whole contract
    is not receipt-bound. Review R4: the CLI writes values and diagnostics with `writeln!` on
    locked handles and reports a write failure as `noble calculator: terminal output failed: …` with
    exit 2 instead of panicking with 101. `calculator::tests::terminal_write_failures_are_reported_instead_of_panicking`
    pins the failing-writer path, and a release binary of `f45a6a3a…4ffa` with a genuinely closed
    stdout read end exited 2 with `Broken pipe (os error 32)` after 117 s on `1+2`. A descriptor
    that is already closed (`>&-`) is ignored by the standard library, so it prints nothing and
    leaves the exit status at the line outcomes; the design records that.
- [x] [serial] Prove the seven guest budget categories with other budgets adequate, and verify independent native/transport/host failures never masquerade as `ResourceLimit`; exercise a failed candidate after an accepted line and verify the prior state survives. Restore evidence: `calculator::tests::failed_lines_of_every_class_restore_the_committed_session` restores the committed arena and host state byte for byte after a real `$` line (SyntaxError) and after failures of each class (guest diagnostic, guest `ResourceLimit`, native quota, host bound, Rust and worker infrastructure failure) injected after real worker modules ran, and later lines match a reference session and a replay-reconstructed one; the ignored `calc_state_02_failed_lines_leave_definitions_byte_identical` does the same after real `let`/`fn` lines, with real `let x = 1 / 0` and `2 ^ 5000` failures. The v2 budget plan and 2026-11-20 execution passed independent fresh-context review (`verification/calculator-v1/budgets/review-2026-11-23.md`); canonical CALC-08 case promotion remains separate under the CALC case-gate task below. r[CALC-LIMIT-01] r[CALC-STATE-02]
  - Partial evidence (#20), release snapshot of 44329dc (sha256 e1fb70ba…8127), plan and observations in `verification/calculator-v1/budgets/` (README, summary.json). All seven categories were executed: N−1 refuses with its category named, N succeeds, and a follow-up line sees the prior state. The brackets were InputBytes 8/9, NumericDigits 2/3, IntermediateIntegerBits 25/26, SyntaxDepth 2/3, CallDepth 1/2, EvaluationWork 38,052/38,053 and OutputSize 6/7. In the OutputSize before-commit control, a refused `let` kept `x = 7` and left `y` unbound. None of these printed a `ResourceLimit`: TransportLimit, a forced timeout, a memory-capped scope, and the 44329dc-only HostLimit HistoryBytes/ReplayWork (#17 replaces replay with a committed session and restore). A lowered runtime step quota is a runtime status (library harness). The killed-worker control did not produce a native failure on 44329dc, because each line replays into a fresh candidate worker. This task stays open: the failed-candidate-after-accepted-line case on the integrated tree and the CALC case gates are not done.
  Progress (2026-11-10): the brackets were re-run on this tree. A release build of `53c89e0d`
  (sha256 479b4c97…df84) ran the same plan in 22 sessions, 1,103 s at concurrency 4. All seven
  categories kept their boundaries: N succeeded, N−1 refused with its category named, and the
  follow-up line still saw the prior state; `input-bytes` also refused the setup line at N−2. The
  failed-candidate-after-accepted-line case ran in the OutputSize session: a refused `let y` left
  `x = 7`, and the follow-up `y` was `UnknownName`. None of the native, transport, timeout or
  memory-cap controls printed a `ResourceLimit`. Four planned control rows no longer match and were
  not rewritten: the two `44329dc`-only host-replay preflights cannot fire after #17 replaced replay
  with a committed session, and a killed worker and the 2 GiB cap now surface as a restore refusal
  and a closed worker instead of silent recovery and a kill. The new observations are in
  `budgets/README.md` and `budgets/summary.json`. This task stays open: the CALC case gates are a
  separate step, and the four control rows need a reviewed plan revision before the matrix can be
  called complete.
  Progress (2026-11-20): the plan revision is written. `budgets/plan.json` is
  `noble-calculator-budget-plan/v2` with a `revision` record under owner decision 17:
  `control/history-bytes` and `control/replay-work` are retired because the committed-session route
  removed the host replay preflight they asserted, and `control/worker-killed` and
  `control/memory-cap` now expect the selected failure labels (`calculator restore failed` and
  `engine worker closed its reply stream`, exit status 2, no `ResourceLimit`) instead of the
  `44329dc` silent recovery and external kill. The lane records retired rows without running them
  and refuses a retired row that still carries an expectation. The lane also sets
  `-p OOMPolicy=continue` on its memory-capped scopes so a cgroup OOM kill cannot race a
  user-manager scope stop; two one-row reruns at `c5f6d8ea` (binary sha256 `f45a6a3a…4ffa`,
  receipts `057e1c98…` and `bfc25c2b…`) reproduced the memory-cap observation exactly. The revised
  plan then ran: a release binary of `cb5e9f71` (sha256
  `b4b2931a…2d1736`) ran 20 sessions at concurrency 2 in 1,660 s and all 20 matched; all seven
  categories kept their boundaries (N−1 refused with its category named, N accepted, the
  follow-up line saw the prior state), and the four controls printed no `ResourceLimit`. The
  compact observations are `budgets/summary-2026-11-20.json` (the `53c89e0d`, 2026-11-10
  `summary.json` is kept), and the raw receipt (sha256 `b1d66e0d…62034`) is outside the
  repository. Erratum: the plan's `revision.reason` says the two changed rows expected "silent
  recovery"; they did not. v1 `control/worker-killed` expected exactly stdout `7` and no stderr,
  with the next line failing, and v1 `control/memory-cap` expected no output; `44329dc` recovered
  silently (exit 0) and was killed by the scope (SIGTERM), which is why those rows did not match.
  `plan.json` keeps that sentence because its hash is bound to the receipt; the correction is here
  and in `budgets/README.md`. This task stays open: the plan revision still needs its independent
  review under decision 1, and the CALC case gates are a separate step.
  Review (2026-11-23): `OWNER DECISION 2026-11-20:` (Decisions 1, 17, and 28; independent
  fresh-context review with owner veto retained) recorded in
  [the budget review record](../../../verification/calculator-v1/budgets/review-2026-11-23.md)
  (verdict ACCEPT) verified `budgets/plan.json` v2 (`3f2d1bf3…`) against
  `budgets/summary-2026-11-20.json` (`dd8f5ee8…`, 20/20 non-retired sessions matched at `cb5e9f71`,
  2 retired rows, `resource_limit_lines: 0` on all 4 active controls), the seven guest budget charge
  sites and `38,053` `EvaluationWork` boundary, the four drifted control rows under decision 17, the
  `revision.reason` v1-expectation erratum, and source stability across `cb5e9f71..e5e2ac5d` (zero
  budget/restore/fixture changes; raw receipts `b1d66e0d…`, `057e1c98…`, `bfc25c2b…` remain
  external). Canonical `CALC-08` case promotion stays separate under the CALC case-gate task below.
- [ ] [serial] Exercise applicable full CALC-01 through CALC-13 inputs, expected fields, hostile controls and independent-oracle requirements before proposing any canonical case state or implementation-status change. Leave unexercised scenarios absent/not-run with no invented evidence. The CLI runtime vectors of CALC-01/02/03/06/07/09 run through the [acceptance runner](../../../verification/calculator-v1/README.md), whose receipts are implementation observations only. Its [44329dc release receipt](../../../verification/calculator-v1/receipt-44329dc-release.json) and [integrated eccbc02 release receipt](../../../verification/calculator-v1/receipt-eccbc02-release.json) each observed 34/34 CLI vectors passing (CALC-07's two import/resource variants are unsupported on the CLI); no case state changes from them. r[CALC-GATE-01]
  - Progress (2026-11-23): added the clean-tree, source-bound case gate ([gate.mjs](../../../verification/calculator-v1/cases/gate.mjs), `sha256:6a0cb481d8a2fa874b6f5b1c2e18c6826e41a378e4ac5de91a39ef4a0557de9a`; [README](../../../verification/calculator-v1/cases/README.md)) and executed lanes `cli`, `state-host`, `oracle`, `numeric-fast`, and `cli-unit` on a debug `--no-default-features` build at clean commit `89dd2742e76a41aabfc2b0be4e56a7ff0044178a` (`source_revision: sha256:947ba979bac2edc8bdaa3854cf87e8bf7e9a7f0a478e274c56d087cffc387d26`), retained at [the 2026-11-23 acceptance receipt](../../../verification/calculator-v1/cases/runs/20261123-89dd2742/acceptance.json) (`sha256:2bff9100b0f0d369514290d38059375273fefad55fc1ce4ae01cffe898ac7f49`). Observed: `cli` 40/40 vectors + 2/2 negative controls, `state-host` (`calc02` + `calc03` 4/4 multi-line `state_unchanged`, 873-module trace, `CoreEngine` `hostFunctions = []` / `host_requests_total = 0`, `engine-calculator.test.mjs` 4/4 `skipped 0`), `oracle` 38/38 cases + 41/41 comparable receipts, `numeric-fast` 4/4, `cli-unit` 7/7. Promoted in `specs/conformance/calculator-cases.json`: `CALC-01`, `CALC-02`, `CALC-03`, `CALC-06`, `CALC-09` (`implemented`/`passed`, citing `OWNER DECISION 2026-11-20:` Decision 26 reading and caveat), and `CALC-07` (`partial`/`passed`, citing `OWNER DECISION 2026-11-20:` Decision 17 for the two `unsupported-on-cli` variants). Kept `absent`/`not-run`: `CALC-04` and `CALC-05` (`numeric-slow` `#[ignore]` `calc_04_constructor_and_decoder_vectors` exceeded 240 s in debug and `--release` builds are out of bounds here; `numeric-fast` 3/3 passed as partial observation), `CALC-08` (20-session debug `budgets/run.mjs` exceeds debug bounds), `CALC-10`/`CALC-11` (`MA3` per Decision 17), `CALC-12` (release mutant boundary and open `acorr` O3/O6 & `MA-12/13`), and `CALC-13` (open entry gates and release-build boundary). This task stays open until `CALC-04/05/08/12/13` close.

## Native lifecycle and closeout

- [ ] [serial] Run the pinned `tools/cairn.mjs` proposal/design/tasks gates and native validation, then perform any deliberately approved sync/archive separately. Issue #22 ran these gates with pinned Cairn `15f00875562025e7ea7e0d1f4af24d1a2e2ac06f`: proposal PASS and design PASS; the tasks gate and `validate --root .` first FAILED because the compiled-code table-reuse task lacked a requirement reference, then PASSED (tasks) and returned valid (validate) after it gained r[CALC-BOUND-01] r[CALC-LIMIT-01]. Logs and their hashes are recorded in the main-gates receipt, `verification/main-gates/receipt.json`. Follow-up verification at pinned Cairn `15f00875562025e7ea7e0d1f4af24d1a2e2ac06f` on the updated `calculator-v1` tree: `gate proposal calculator-v1`, `gate design calculator-v1`, `gate tasks calculator-v1`, `validate --root .`, `bun tools/check-specs.mjs --self-test`, `bun tools/cairn-specs.mjs --self-test`, and `bun tools/cairn.mjs --self-test` re-run clean on the final committed tree; `cairn sync` and `cairn archive` remain deliberately deferred. r[CALC-GATE-01]
