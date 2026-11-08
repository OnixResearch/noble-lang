## Phase 1: Design (this change; document-only)

- [x] [serial] Audit the existing WIT worlds and the actual `bounded_region.rs` host behavior at `44329dc` (registration, borrowed read checks, release, destructor, post-return/abort cleanup, report schema, fuel/memory limits), recording findings F1–F11 with line citations. r[EPROTO-SCOPE-01] r[EPROTO-OBS-01]
- [x] [serial] Select `RegionProtocol-v1`; specify the transition table, result schema, resource identity, host assumptions A1–A6 and the five-way separation of permission, proof, applicability, ownership and cleanup authority. r[EPROTO-SCOPE-01] r[EPROTO-TRACE-01]
- [x] [serial] Specify the request/response/commit trace model, the normal-return postconditions for success and ordinary error, prefix safety P1–P6, and host-owned abnormal cleanup without a totality claim. r[EPROTO-TRACE-01]
- [x] [serial] Specify the order-preserving composition rule with a worked safe composition and rejected out-of-order, omitted/double release, wrong-identity and unmodeled-response cases. r[EPROTO-COMPOSE-01]
- [x] [serial] Write the EPROTO-CASE-01…11 unexecuted designs, the proof plan T1–T6, the monitor responsibilities and the first-profile recommendation; validate the change documents (not runtime or proof evidence). r[EPROTO-OBS-01]

## Future implementation and evidence gates (not undertaken by this change)

- [x] [serial] Introduce a new bounded-region report schema with separate release kinds, distinct trap/quota/outside-model classes and an ordered event trace; keep `noble-bounded-region/v1` receipts byte-identical. r[EPROTO-OBS-01]
  Done (2026-11-08): `noble component read-region --report v2 …` emits `noble-bounded-region/v2`;
  without the flag, or with `--report v1`, the adapter emits v1 as before. Beside the v1
  observations, v2 carries:
  - `releases`: `guest`, `destructor` and `host_retirement` counts (F1). A destructor drop is
    traced as `region.drop`; v1 still spells it `region.release`.
  - `cleanup.retirement_reason`: why the host retired the owner (F9). It is `omitted-release` after
    a normal return or a rejection, and otherwise the abnormal exit class.
  - `exit_class`: the v1 classes (`normal`, `rejected`, `trap`, `quota-exhausted`,
    `adapter-invalid`), plus `outside-model` for a result outside the schema. That is an error other
    than `bounds-reject` or `invalid-handle`, or an `ok` of another length than requested. The
    interruption control below adds `cancelled`.
  - `cancellation`: `after_reads` and whether the host cancelled. It read `"unsupported"` until the
    interruption control existed.
  - Refusal classes: each refused read's `host_class` in the trace, and per-class `refusals` counts
    that survive truncation (F8). The classes are `foreign-representation`, `no-owner`, `identity`,
    `bounds`, `scope-exhausted`, `admission`, `native-access` and `completion`. The guest still
    sees only `invalid-handle` or `bounds-reject`.
  - `lost_owners` (F7). For the adapter's table T4 proves completion always returns the owner, so
    this stays 0 unless that invariant breaks.

  The v1 and v2 traces are ordered (F6). The unit tests in `bounded_region.rs` cover the three
  release kinds and their retirement reasons, scope exhaustion against an identity refusal,
  outside-model results, and v1's exact field order and event encoding.

  Run once, not retained: the CLI built from this change compiled the host-locality fixture. Its v1
  reports for `000102 (1,2)` and `(2,2)` are identical, field for field, to cases 0 and 2 of
  `verification/host-locality/evidence-1bf39a52.json`.
- [x] [serial] Build the Lean `RegionProtocol` model, prove T2/T3 for the selected subject, and relate the `ownership::Table` transitions through the existing M5 lane (T4); keep T5/T6 open until separately discharged. r[EPROTO-TRACE-01] r[EPROTO-COMPOSE-01]
  Progress (2026-10-30): T1–T3 are proved in `proofs/foundation/NobleFoundation/RegionProtocol.lean`.
  - T1: the transition table as `step`, with host assumptions A1–A6 explicit.
  - T2: the subject's normal-return postconditions (`subject_normal_return`,
    `subject_rejects_exactly_out_of_bounds`), and prefix safety for every program and host. P1 is
    `exactly_one_release_commit`; P2–P5 are the monitor's rules with `run_is_accepted`; P6 is
    `stop_discards_the_rest`.
  - T3: `seq` over identical states, with `counters_do_not_commute` and `no_set_union_rule`.

  Done (2026-11-08): T4 is proved in `proofs/m5/RegionTable.lean`, on the pinned M5 extraction of
  the table methods the adapter calls (`Table::new`, `register`, `begin`, `native_access`,
  `complete`, `release`). Under the host the table induces, the adapter's table calls refine the
  model for every request, execution and run (`calls_refine`, `exec_refines`, `run_refines`).
  - `open_borrow_returns` discharges A4 for the table, so the model's `lost` state is unreachable.
  - The scope counter is the model's `granted`, with the model's scope limit as the ceiling.
  - P1 holds for the table's own accounting (`table_retires_exactly_once`).

  The lane is `verification/region-table/check.mjs`, with the receipt
  `verification/region-table/receipt.json` from the clean commit `0985ee06`. T5 (the Rust host
  functions follow the call sequence) and T6 (lowering) stay open as stated.
- [x] [serial] Implement a host interruption control before claiming any cancellation behavior; until then EPROTO-CASE-07 stays unsupported. r[EPROTO-OBS-01]
  Done (2026-11-08): with `--report v2 --cancel-after-reads N`, the host cancels the invocation
  after answering the N-th read (F3, RA-CLEAN-02).
  - Access is revoked at once. Every later guest call (read, release or destructor drop) traps with
    an interrupt before it reaches the protocol, so the trace records no event for it.
  - A guest that only computes is interrupted at its next function entry or loop header, through a
    zero Wasmtime epoch deadline. Epoch interruption is compiled in only with the flag, and the
    epoch never advances, so an interrupt is always the host's cancellation.
  - The exit class is `cancelled`, and the host retires the owner for reason `cancelled`. The v2
    `cancellation` field records `after_reads` and whether the host cancelled. v1 refuses the flag.

  Unit tests cover revocation after exactly N answered reads, the interrupt's classification and
  the flag rules. `--fuel N` (EPROTO-CASE-06) is added beside it. The control is a test knob: no
  asynchronous cancellation API exists.
- [x] [serial] Register and execute EPROTO-CASE-01…11 with independently constructed hostile components, retaining trace prefixes, reports and component bytes; record monitor verdicts as finite execution evidence only. r[EPROTO-SCOPE-01] r[EPROTO-OBS-01]
  Done (2026-11-08): `verification/effect-protocol/execution.mjs` executed all eleven cases from the
  clean commit `3ef3f43a`. The receipt is `verification/effect-protocol/execution-3ef3f43a.json`.
  - Inputs: the compiled subject, and ten hostile core modules written in the lane against the
    world's Canonical ABI and wrapped with the selected wasm-tools, so none derives from compiler
    output.
  - Retained: every module's WAT, bytes and hashes, and every scenario's full v2 report and trace.
  - The monitor: `monitor.mjs` replays each trace against the transition table. Ten mutation
    controls of the real case 01 trace check that it reports each violation. Its verdicts are
    finite execution evidence only.

  Results by case:
  - 01–02: success and ordinary errors as designed.
  - 03: 64 commits; the 65th read is refused as `scope-exhausted`.
  - 04–05: traps, after the commit and before any use; the host retires the owner.
  - 06: the smallest reading fuel (74) is exhausted after the commit and before the release.
  - 07: cancellation between two reads, and of a computing guest that otherwise exhausts its quota.
  - 08: the monitor reports `omitted-release`.
  - 10: out-of-order source is refused at check time.
  - 11: guest-returned unmodeled results are classified `outside-model`.

  Departures from the design's expectations:
  - Case 09: Wasmtime's handle table refuses the consumed handle (`unknown handle index`) before
    the second call reaches the host. It reports an untyped error, not a trap, so the adapter
    classifies the exit as `adapter-invalid`. There is one release commit and no read after the
    release, as designed.
  - Case 03's `native-access` refusal is not exercised. `Host::read` passes only the borrow `begin`
    has just opened, which T4 proves is authorized (`open_borrow_returns`).
  - Case 11's host-produced responses are covered only by monitor controls at the trace level. The
    audited host cannot produce them (A3), and no modified host is built.
