## Phase 1: Design selection (no implementation, measurement or allocator change)

- [x] [serial] Audit allocation sites, callers, cleanup paths and profile selection at 44329dc, record drift from 0ea10d3, and build the backend/profile × strategy compatibility matrix. r[ALLOC-CAP-01]
- [x] [serial] Define the minimal capability-specific contract: lifetime preconditions, selection granularity, provenance, layout and handles, reuse statements, and accounting. r[ALLOC-CAP-01] r[ALLOC-OBS-01]
- [x] [serial] Separate failure classes F1–F4 with atomicity and cleanup rules, and state isolation and observational-equivalence obligations. r[ALLOC-FAIL-01] r[ALLOC-ISO-01]
- [x] [serial] Write the W1–W8 benchmark plan, the proposed decision criteria D1–D4, the ALLOC-CASE-01..12 conformance designs and the recommendation. Run document validation and the proposal/design/tasks gates only. r[ALLOC-EV-01]

## Phase 2: Future implementation and evidence gates (open; not undertaken here)

- [ ] [serial] Execute W1–W8 baselines for the existing strategies A1–A7 with the recorded controls, and ratify or revise D1–D4 from measurements. r[ALLOC-EV-01] r[ALLOC-OBS-01]
- [x] [serial] Implement distinct F1–F4 classification and per-strategy accounting revisions in the runtimes and hosts, then execute ALLOC-CASE-02/09/10. r[ALLOC-FAIL-01]
- [ ] [serial] Register and execute ALLOC-CASE-01, 03–08, 11 and 12 against each strategy claimed compatible, including differential equivalence and cross-instance refusal. r[ALLOC-CAP-01] r[ALLOC-ISO-01]
- [ ] [serial] After #4 selects a native route, compare the system allocator with jemalloc inside that route with representation held constant. Never apply it to Wasm guest cells. r[ALLOC-ISO-01]

## Progress (2026-11-23)

OWNER DECISION 2026-11-20 (Decision 2): Phase 2 allocation W1–W8 baselines, F1–F4 quota
classification, and ALLOC-CASE execution are authorized inside `allocation-strategy-contract`
despite the proposal's original Phase-1-only out-of-scope line.

Phase 2 Box 2 (`F1–F4` failure classification and `ALLOC-CASE-02/09/10`) is complete; Boxes 1, 3,
and 4 remain open. The committed harness is `verification/allocation-baselines/`
(initial harness commit `9ce308ca`; review fixes commit `b2cb41cd`): `run.mjs` drives each
strategy's own host (A1/A3 shipped `session`/`live repl`, A4 `live slot`, A5/A6 `wasm-experiment`,
A7 the component runtime fragment, A2 the `calculator` session), records pinned Node/V8 and
`wasm-tools`, 22 source file hashes (including `fragments.mjs` and `linear-address.wat`), host
affinity and load with every sample, and writes a clean source-bound receipt plus raw JSONL.
`cases.mjs` gates ALLOC-CASE-02/09/10 and stays red on the four F1–F4 split assertions until
the runtime/host F1–F4 split lands.

Measured on clean commit `b2cb41cd` (pinned Node `24.13.0` / V8 `13.6.233.17-node.37`,
`wasm-tools 1.245.1`, `--samples 5 --warmups 1` clean verification run, `source_rebound.unchanged: true`):

- **W1–W8 baseline receipt** (`baselines/receipt.json` sha256 `7343c16ec3133a4fd420aa3843f803d98f1141f9425aba2038618d21fc5a75a3`;
  `samples.jsonl` sha256 `427da3cbbfad113ee6fccc6364caf850cea0eb7e568e0a74385bf1aba32bbb47`;
  `summary.txt` sha256 `4272fa688ab5f1d40a34c260bac946c013114412d7750a9873d284991ecb3ae8`):
  - **97 unique points** (106 summary rows including at-most-1 noisy retry, **618 raw samples**),
    **0 harness errors**, **0 `type-reject` on W1**, **0 W1 value mismatches**.
  - **W1:** `W1/{A1,A3}/{left,right,balanced}/{8,16,32,128}` all complete `normal` matching the
    `BigInt` oracle (`A1/128` median `18.6–24.8 ms/sub`; `A3/128` median `53.1–64.9 ms/sub`);
    `W1/{A1,A3}/{left,right,balanced}/144` all hit `runtime-exhausted` (`quota_reason: "recipe leaves"`).
    `W1/{A5,A6}/{left,right,balanced}/{8,32,255,256}` all complete `normal` matching the oracle
    (`A5/left/256` median `31.6 us`; `A6/left/256` median `30.3 us`) and refuse `257`.
    `W1/A4/left/{8,16}` execute and match the oracle (`A4/left/16` median `1.23 ms`).
  - **W2:** `W2/{A1,A3}/500` complete `normal` (`14.1 ms/sub` A1, `41.2 ms/sub` A3); `W2/{A1,A3}/10000`
    exhaust cleanly at submission 513 with `quota_reason: "allocation"` (`196,608` logical bytes,
    384 B/root, A1 never reclaims). `W2/{A4,A5,A6,A7}/10000` complete all 10,000 roots
    (`73.7 us` A4 invoke, `5.1 us` A5, `4.9 us` A6, `0.1 us` A7).
  - **W3:** `W3/A4/64/seed-{1..5}` release 64 distinct saved Program owners in five seeded random
    orders (`program-released`, `seed-1` median `83.5 us`).
  - **W4:** `W4/{A1,A3}/1000` define 500 versions and execute 256 invocations before hitting the
    256 specialization-entry cap (`exhausted`, 0 harness errors); `W4/A4/1000` completes 20
    install/49-invoke/discard cycles (`installed,executed,discarded`, median `100.2 us`).
  - **W5:** `W5/{A1,A3}/100` (`normal`), `W5/A4/100` (`executed,program-released`, 33 retained/released
    Program owners interleaved with 34 scratch invokes), `W5/{A5,A6}/100`, and `W5/A7/100` all complete cleanly.
  - **W6:** `W6/A1/F1` (`normal` at peak `7,680 B`, `runtime-exhausted` below), `W6/{A5,A6}/F1`
    (`distinct`, peak `1,440 B`, `at` status 0 / `below` status 1 reason 1), `W6/A7/F1` (`distinct`,
    64 B normal / 63 B trap), and `W6/host/F4` (`process-death`, `SIGTRAP` under `ulimit -v 262144`).
  - **W7:** `W7/A1/trap` (`trap`), `W7/A3/trap` (`trap`, status 6, `request_trace: ["test.emit:first","test.abort"]`
    under `--allow-effects test.emit,test.abort`), `W7/{A5,A6}/steps` (`exhausted`, `quota_reason: 6`),
    `W7/A7/range` (`trap`), and `W7/A4/steps` (`executed`).
  - **W4/A2/40 and W8/A2/40 (bounded debug-CLI finding):** running all 40 `calculatorLines()`
    against the unoptimized debug `noble` binary hit the 1,800 s session timeout (`SIGKILL`) after
    emitting 17 (`W4/A2/40`) and 10 (`W8/A2/40`) stdout lines because cold calculator lines compile
    hundreds of guest modules in debug Rust (`~1.08 s/miss` vs `~2.24 ms/hit`).
- **Failure-class case gate receipt** (`cases/receipt.json` sha256 `64398ca93fd002b6a3e1d5058a92fdd9acecb8383d00c20252f8457a66df72d7`;
  `checks.jsonl` sha256 `fa7be057bcaa657c395c33c76770402e76eb0dfa19760fcbd9f96ea447668061`):
  - **20/24 checks pass** (exit 1): all 9 `CASE-09/*` checks pass (mid-construction rollback with
    zero live partial cells and `engine_error: null` on `managed-linear-memory` and `wasm-gc`, plus
    A7 reservation trap), all 5 `CASE-10/*` checks pass (session kill/restart and stale saved-Program
    owner refusal at both `invoke` and `release-program`), and 6/10 `CASE-02/*` checks pass.
  - The **only 4 failing checks** are the pre-split F1–F4 classification assertions:
    `CASE-02/F1/distinct-class` (`failure_class` absent), `CASE-02/F2/strategy-exhaustion`
    (live-slot pool reports `quota_reason: 1` instead of `8`), `CASE-02/F3/memory-growth`
    (lowered-memory linear fragment reports `quota_reason: 1` instead of `9`), and
    `CASE-02/four-distinct-classes` (`observed: { F1: "unsplit:allocation", F2: "unsplit:quota_reason_1", F3: "unsplit:quota_reason_1", F4: "F4-engine-oom" }`).

- **F1–F4 split implementation & `ALLOC-CASE-02/09/10` pass (Box 2 complete):**
  - `crates/noble-wasm/runtime/live-accounting.wat`: checks pool/slot exhaustion (`slot > 4096 || live > 4096`) before `bytes > $allocation_limit`, emitting `quota(8)` (`F2-strategy-exhaustion`), `quota(1)` (`F1-logical-quota`) for logical byte quota, and `quota(9)` (`F3-memory-growth`) on `$storage_ensure` failure.
  - `crates/noble-wasm/runtime/accounting.wat`: preserves `quota(6)` (`$runtime_tick`) and `quota(1)` (`next > $cell_limit || bytes > $allocation_limit`), and emits `quota(9)` on `$storage_ensure` failure.
  - `crates/noble-cli/src/core/runtime/{abi.json,preamble.mjs,engine.mjs,slot-engine.mjs,protocol.mjs}`: maps `quota_reason` `"8"` (`"strategy exhaustion"`) and `"9"` (`"memory growth"`), adds `failure_class` mapping (`1..7 -> F1-logical-quota`, `8 -> F2-strategy-exhaustion`, `9 -> F3-memory-growth`, engine/host OOM -> `F4-engine-oom`, normal/non-OOM traps -> `null`), and emits `failure_class` across `CoreEngine`, `SlotEngine`, and `protocol.mjs`.
  - `verification/allocation-baselines/run.mjs`: updates `controls.accounting_revision` to `'F1-F4 split: reason 1 logical quota, 8 strategy exhaustion, 9 memory growth, F4 host/engine classification'`.
  - `verification/allocation-baselines/cases.mjs`: passes **24/24 checks** cleanly (`CASE-02/four-distinct-classes`: `F1-logical-quota`, `F2-strategy-exhaustion`, `F3-memory-growth`, `F4-engine-oom`; all 9 `CASE-09/*` and 5 `CASE-10/*` checks).

Remaining before Box 1 can be ticked: run the full 30-sample W1–W8 matrix (and resolve the
debug-CLI A2 40-line timeout or record an explicitly bounded A2 session), retain the receipt and
raw samples, and ratify or revise D1–D4 from those measurements. Boxes 3 and 4 remain open.
