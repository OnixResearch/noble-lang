# Integrator review: LIVE round 2 (`cairn/live-r3`), 2026-11-21

**Reviewer:** the campaign integrator, under owner decision 28 (reviewer agents are unavailable, so
the integrator reviews branches it did not implement). This is not a fresh-context review. The
owner keeps a veto before any push.

**Reviewed:** branch `cairn/live-r3` (the `cairn/live-r2` integration of `cairn/live-seams` and
`cairn/abort-env` plus the LiveR2 fixes `a25f5528`, `2aa0b3ff`, `ccd27433`, merged with the batch-2
tree), against the round-1 review `review-2026-11-20.md` findings L1-L15 and A1-A9.

## Integrator changes on this branch (not reviewed by anyone else)

- `aa7a220f`, `46dbd28c`: extraction fixes (Aeneas join shape in `effect_universe`; the
  `test.abort` row built locally in contracts and Wasm again, so the kernel change is only the live
  test-host validation rule). Masks and rows are unchanged; kernel 240, contracts 471 and Wasm 127
  tests pass.
- `02c4679b`, `481435e1`, `b8addfff`, `24181a8e`, `7f24d9c5`, `db9336df`: gate fixes found by the
  first complete runs of the merged gate (it had never completed before):
  - LIVE-03's binding lane matched a diagnostic wording the registry never printed;
  - LIVE-02 `input.before` required a stack field that definition reports do not carry;
  - LIVE-04 called `.map` on the variant `Map`;
  - LIVE-09 required distinct module digests for two derived definitions whose modules are
    byte-identical, and a `signatures` field the pending report does not have (its own mapping
    disclaims a signature catalogue);
  - L13 (below) and the round-3 receipt name.
  None of these weakens an observation of CLI behaviour: each removed or replaced conjunct could
  never hold on any binary.
- `e8c83ddb`: owner decision 30 applied (LIVE-05's `missing-import` variant dropped as
  inapplicable and recorded in the case file's `amendments`; LIVE-05 joins the promoted set).

## Final run

One run of the merged gate on clean `e8c83ddb`: `result: passed`, LIVE-01, 02, 03, 04, 05, 06,
07 and 09 fully observed, 95 commands, 122 reports. Receipt
`verification/live-cases/acceptance-2026-11-21-r3.json` (SHA-256
`bac5b385c01d2520a13192f7a9e292393b524f6a6e9480e00fd3b61cc9bf20e7`); raw outputs and `bin/` are
retained outside the repository.

## Findings

| Finding | Disposition | Evidence |
|---|---|---|
| L1 stop class / old arena | Resolved | Every LIVE-04 variant has an exact expected stage, outcome and diagnostic (`gate.mjs` `expectedStops`); the worker's `:arena` witness (memory, table, globals, guest executions, host requests, protected operations, source generation, compilations) is compared across each refusal, allowing exactly the documented discarded preparation. The round-1 mutant M-A+M-B was not re-run. |
| L2 seam decides/reports | Resolved (option a) | `live.rs`: the frame bound (`MAX_LIVE_BINARY`) and `check_live_binary_binding` with discard are in every build; the seam only substitutes bytes. |
| L3 forged = different | Resolved | The seam presents an independent WAT digest (`NOBLE_LIVE_TEST_BINARY_REPLACE_WAT_SHA256`); the two variants refuse on `wasm_sha256` and `wat_sha256` respectively. |
| L4 literal counters | Resolved | Counters come from the cumulative `:arena` witness, not from refusal literals. |
| L5 candidate executions | Resolved | The worker's cumulative `guest_executions` is equal across the invocation, pending and committed receipts and advances by one on the next call. |
| L6 grant consumption | Resolved | `:arena` shows the grant selected after two refused candidates and `null` after the successful publication. |
| L7 kernel/byte/shadow | Resolved (narrowed) | Committed source digests equal the derived texts; the isolated shadow preflight instantiated the committed source/WAT/Wasm digests; type/effect is the checked `[I64] -> [I64]`, 0 effects. |
| L8 inputs | Resolved | The selected file is `evolve.noble`; both children are compared. |
| L9 process restarts | Resolved | Worker child PIDs are read from `/proc/<pid>/task/*/children` before and after the reload. |
| L10 LIVE-03 defect claim | Resolved | The host-binding-mismatch variant runs as written and refuses at admission with the session preserved. |
| L11 receipt overwrite | Resolved | Every `acceptance*.json` is excluded from the source manifest; `tools/check-specs.mjs` checks each cited `receipt_sha256` against its file (with a self-test). |
| L12 per-child env | Resolved | `commands` rows record each child's environment. |
| L13 vacuous absence | Resolved (`db9336df`) | The absence run's byte-seam scope is the submitted `20 addone`. |
| L14 path-dependent hashes | Resolved | Binaries are retained under the run's `bin/` (outside the repository); the receipt says hashes are confirmable against that copy. |
| L15 LIVE-10 record | Resolved | `wrong-selected-name` is listed as a named test, not as an observed CLI variant. |
| A1 substitute workload | Resolved | The in-flight lane runs the case's own source, flags and `[ addone ]` submission under the `in-flight-execution` seam; the plain-child row no longer claims the reload ran while an invocation was in flight. |
| A2 `owner_decision` in input | Resolved | Removed from LIVE-02's `input`; decision 21a stays in `tasks.md` and the evidence assumptions. |
| A3 raw outputs | Resolved by the final run | The final merged run keeps `raw/` and `bin/` outside the repository. |
| A4 `missing-import` | Resolved by owner decision 30 | The live profiles refuse `import missing@1 as m` lexically; the owner dropped the variant as inapplicable. It is recorded in the case file's dated `amendments` block and the change tasks. |
| A5 `protected_operations` | Resolved | The DX-LIVE-01 delta now says `test.emit` performs no protected operation and `protected_operations` stays 0, as the code does; the LIVE-05 evidence reports the measured 0 without calling `test.emit` a protected operation. |
| A6 execution-time authorization | Resolved | The worker admits `test_emit`/`test_abort` imports only for the selected `live_effects` mask (`engine.mjs`), and an ungranted session refuses both words at resolution. |
| A7 REPL harness | Resolved | The LIVE-05 evidence and the receipt's non-claims say the trap variant runs in `noble live repl` under the explicit host grant, not under the watched-session harness. |
| A8 receipt hygiene | Resolved | Every receipt file is excluded from the manifest, and the new evidence records use the cases' own `input.harness` names. |
| A9 two gate copies | Resolved | One merged gate; one run at one commit produces the receipt. |

## Verdict

**ACCEPT-WITH-NOTES (integrator).** LIVE-02, 03, 04, 05 and 09 are promoted from the merged
receipt; LIVE-01, 06 and 07 keep their reviewed `004d24c4` receipt and are observed again by the
same run. LIVE-08 and LIVE-10 are not promoted. Notes: the round-1 L1 mutant was not re-run; the
integrator wrote the gate fixes listed above and reviewed them itself, which the owner veto covers.
