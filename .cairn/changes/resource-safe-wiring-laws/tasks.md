# Arrow-inspired, resource-safe wiring library and law suite tasks

## Design (this change)

- [x] [serial] Re-verify the #11 baseline citations at `ffcce53` (origin/main `44329dc` plus format/pin) and record their drift. Record design-time observations O11-1 to O11-8 with the prebuilt `noble-44329dc-release` binary. These observations are explicitly not evidence. r[WIRE-EVID-01]
- [x] [serial] Specify the `wire@1` library sketch: complete stack/effect signatures, derivations from existing words, the entry of pure Programs without `arr`, routing versus fanout, and explicit move-only/`Data`/`Capture` rules. The sketch links under the `44329dc` checker with one shared effect slot. r[WIRE-API-01]
- [x] [serial] Specify the observation relations and the law matrix L1-L14, the classification of the Haskell Arrow laws, evaluation order, effect-union and quota limits, and the separation of behavioral equivalence from identity. r[WIRE-LAW-01]
- [x] [serial] Write the worked examples: pure pipeline, resource-preserving frame (design-only), sum branch, ordered effects, and rejected resource fanout. r[WIRE-API-01] r[WIRE-LAW-01]
- [x] [serial] Specify the WIRE-CASE-01 to 24 executable-law test plan with independent expectations, negative cases and cleanup responsibilities. Add the proof plan mapped to existing Lean rules, with new model, implementation and correspondence obligations. r[WIRE-EVID-01]
- [x] [serial] Sketch the optional known-graph profile and analyze compose-tree flattening against reflection, proof identity, ordering and quota. r[WIRE-GRAPH-01]
- [x] [serial] Package `wire@1.seq-laws` with the #7 bundle schema, including a worked consumer and refusals. Keep `wire@1.frame-laws` unpackaged. Run `cairn validate` and the proposal, design and tasks gates; these are document checks only. r[WIRE-LAW-01] r[WIRE-EVID-01]

## Future implementation and evidence gates (not undertaken by this design)

- [x] [serial] Decide the library's home, which needs a module-publication decision, and add effect-union signature syntax or explicit adaptation helpers. Investigate the preparation refusals of effectful `split` and of `split`/`fanout` in long sessions (O11-4, O11-5). Make eligibility diagnostics name the library word. r[WIRE-API-01]
  Done (2026-11-20, commits `b71a1adcd4` and the design record "Implementation gates (W1,
  executed 2026-11-20)"). Home: `crates/noble-contracts/src/fixtures/wire.noble`, byte-for-byte
  the O11-4 sketch (2,933 bytes, SHA-256 `96a5a269…ceed9db28`), published as
  `WIRE_LIBRARY_SOURCE` and enforced by the exact-bytes link refusal shared with `result@1`,
  per OWNER DECISION 2026-11-20 (pinned fixture, result@1 precedent). Effect union: the
  explicit K-EFFECT-05 adaptation helpers were chosen over `e|f` signature syntax, with
  reasons, and the single-slot consequence is measured: `"L" [ fx@1.say ] [ 1 + ]
  wire@1.fanout` is `type-reject`, not a preparation refusal. Eligibility refusals name the
  applied library word: `kernel rejected a non-capturable value; word wire@1.fanout via dup;
  required S Data; actual S …`.
  O11-4/O11-5 reproduced and explained: both refusals are the session-cumulative identity
  budget (`DEFINITION_LIMIT` = 256 new named specializations; `noble-wasm`
  `source/admission/identity/records.rs`), refused whole as `independent source compilation
  refused`. Measured boundaries: 254 identities consumed → `split` (4 new) refused while
  `project_first` (1 new) runs; 252 consumed → effectful `split` (5 new) refused, 251 → runs
  with both `test.emit` requests traced. No default budget changed.
  Evidence: `cargo test -p noble-contracts --test declared wire::` — 4 passed
  (`wire::library_registers_only_its_exact_source_bytes`,
  `wire::library_refuses_any_other_declaration_bytes`,
  `wire::library_words_link_at_their_published_signatures`,
  `wire::fanout_at_a_resource_names_the_library_word`); `cargo test -p noble-cli --test smoke
  wire::` — 2 passed (`wire::runs_in_a_fresh_declared_session`,
  `wire::split_refusal_is_the_session_cumulative_budget`).
- [x] [serial] Prove `Wire.*` theorems for L1-L8 and L10-L14 in the pure MC1 fragment. This includes the general frame lemma and whitelisting `List.nil_append`/`List.append_nil`. Package `wire@1.seq-laws` only after #7 bundle support exists. r[WIRE-LAW-01] r[WIRE-EVID-01]
  Progress (2026-11-09): the Lean half is proved in the pure MC1 model, outside the source-proof
  vocabulary. `proofs/mc1/NobleContracts/Wire.lean` inlines the `wire@1` bodies (route (a)) and
  proves L1-L8 and L10-L14 as `Exec` equivalences, plus the general frame lemma `exec_frame`
  (`Exec p s t → Exec p (r ++ s) (r ++ t)`). `ContractGate.lean` pins the 23 theorems under the
  strict axioms, 90 in all. L5, L6 and L11-L13 are stated on the stack shapes their signatures
  require, and L7 assumes every result of `p` has a top value, which `Program<S+A,S+C>`
  guarantees. Still open: whitelisting `List.nil_append`/`List.append_nil` in the reviewed
  source-proof vocabulary, and packaging `wire@1.seq-laws`, which waits for #7 bundle support.
  Closed (2026-11-22, owner decision 52): the whitelist landed with reusable-contract-calls'
  NamedV3 task, which pins `List.append_assoc`, `List.nil_append` and `List.append_nil` as exact
  reviewed rule identities in `ContractGate.lean` (accepted 2026-11-20). Packaging
  `wire@1.seq-laws` moves into reusable-contract-calls' law-bundle task, which owns bundle
  support; this task is ticked with the proofs and the frame lemma it states.
- [ ] [serial] Develop trace, ownership and failure models: PO-04, PO-05, PO-06, V-MODEL-03/04 and V-RESOURCE-01..03. Develop resource-admitting execution before claiming any T, O or F entry or packaging the frame bundle. r[WIRE-LAW-01] r[WIRE-EVID-01]

  Progress (2026-11-23): the T/O/F models are developed in
  `proofs/foundation/NobleFoundation/WiringLaws.lean` at commit
  `0a5e30c861b8bf047b2d4b5bfbf0d1d74cc55bf1` over `OwnershipModel` (the
  wiring words are instruction sequences in that language, so every
  ownership result is an `OwnershipModel.owners_step` instance). T:
  `step_trace_any`, `step_trace`, `effect_sound`, `trace_within_bound`,
  `trace_prefix`, `step_append_code`, `reach_append_code`,
  `execFrom_append`; the derived-word runs `first_execFrom`,
  `second_execFrom`, `split_execFrom`, `fanout_execFrom`,
  `left_inl_execFrom`, `left_inr_execFrom`, `right_inr_execFrom`,
  `right_inl_execFrom`, `choose_inl_execFrom`, `choose_inr_execFrom`,
  `join_inl_execFrom`, `join_inr_execFrom`, `ifte_true_execFrom`,
  `ifte_false_execFrom`, `quote_run_execFrom`, `compose_run_execFrom`.
  Per-law T statements: `then_left_id`, `then_right_id`, `then_assoc`,
  `apply_quote`, `apply_compose`, `under_fuse`, `pair_unpair_exec`,
  `unpair_pair_exec`, `swap_swap_exec`, `first_fuse`,
  `split_first_second`, `fanout_dup_pair_split`, `left_beta_inl`,
  `left_beta_inr`, `branch_true`, `branch_false`,
  `behavioral_not_identity`. L9 counterexamples:
  `exchange_changes_trace` (same result, traces `1,2` versus `2,1`) and
  `exchange_changes_failure_point` (permit-then-deny host; the failed
  request and cleanup point differ); `halted_reach_eq` and
  `halted_trace_final` show no later program starts after an abort. O:
  `under_holds_resource_in_frame`, `owners_track_multiplicity`,
  `fanout_at_resource_untypable`, `project_first_at_resource_untypable`,
  `quote_at_resource_untypable`, `pair_with_resource_not_copy`,
  `sum_with_resource_not_copy`, `list_with_resource_not_copy`,
  `program_interface_resource_is_copyable`. F: failure classes `denied`,
  `failed`, `trapped`, `cancelled`, `exhausted`; `typed_not_stuck`,
  `underflow_stuck`, `wrong_shape_stuck`, `result_error_is_normal`; host
  boundary `retireOne_live`, `retireOne_retired`,
  `retireOne_idempotent`, `retireOne_identity`, `retire_live`,
  `retire_retired`, `retire_single_idempotent`, `retire_next`,
  `retired_never_reacquired`, `retire_then_acquire_keeps_retired`,
  `normal_cleanup_is_guest`, `abnormal_cleanup_is_host`.

  Obligation mapping: PO-04/V-MODEL-04 through `effect_sound`,
  `trace_within_bound` and `trace_prefix`; PO-05/V-RESOURCE-01 through the
  `Own` invariant, `owners_track_multiplicity` and
  `under_holds_resource_in_frame`; PO-06/V-RESOURCE-02 through
  `pair_with_resource_not_copy`, `sum_with_resource_not_copy`,
  `list_with_resource_not_copy`, `program_interface_resource_is_copyable`,
  `fanout_at_resource_untypable`, `project_first_at_resource_untypable` and
  `quote_at_resource_untypable`; V-MODEL-03 through `typed_not_stuck`,
  `underflow_stuck`, `wrong_shape_stuck`, `result_error_is_normal`,
  `no_waiting_status` and `untyped_loop_diverges`;
  V-RESOURCE-03 through `normal_cleanup_is_guest`,
  `abnormal_cleanup_is_host` and the separate host-boundary lemmas.

  Resource-admitting execution is **not** developed, so the box stays
  unchecked and no T/O/F law-matrix entry or `wire@1.frame-laws` package is
  claimed. The selected backend admits no resource type:
  `crates/noble-cli/src/core/runtime/abi.json` has no resource value tag or
  cell layout and says "No resource types are admitted by this backend";
  `crates/noble-wasm/src/component/lower/operations.rs` refuses non-`Data`
  operands at `dup`/`drop`; `crates/noble-cli/src/core/worker_value.rs`
  refuses non-`Data` worker ingress; `crates/noble-kernel/src/types/data.rs`
  excludes `Resource`/`LiveRef` recursively from `Data`; and
  `crates/noble-kernel/src/contracts.rs` refuses resource-bearing nominals
  in live slots. A minimal implementation would need an ABI tag and
  representation, lowering for construction/move/acquire/release and
  invocation transfers, worker ingress/egress, host retirement across an
  invocation boundary, and a test adapter: a cross-crate runtime feature,
  not a bounded slice. The design's "Resource-admitting execution
  (2026-11-23)" subsection records the same gap.

  Evidence, run on the committed clean tree
  `0a5e30c861b8bf047b2d4b5bfbf0d1d74cc55bf1`:
  `verification/ci/lean-proofs.sh <output>` printed
  `m3-build=0`, `m3-gate=0`, `foundation-build=0`,
  `foundation-check=0`, `overall=0`. The checker receipt
  `foundation/foundation-check.json` has SHA-256
  `f14add3700d84bad054a6ae39e2533f0753fa2ed4a06e284d7b8472c94e4c48c`
  and records `clean_source: true`, 2,806 theorems, 388 private
  theorems, the allow-list `propext`, `Classical.choice`, `Quot.sound`,
  and `implementation_theorem: false`. `summary.txt` has SHA-256
  `6d2ba8f00dfc3d34ea5274843c3adc6df384f36481106d34e41d6714d35875e3`.
  The Rust refusal sites cited above are read from the pinned sources;
  this slice edited no Rust file and ran no Rust test.
- [ ] [serial] At sync, register WIRE-CASE-01 to 24 as conformance designs and add a `WIRE` route to `tools/check-specs.mjs` `ROUTES`. Then execute with independent expectations and retain receipts without promoting proof status. r[WIRE-EVID-01]
- [ ] [serial] Implement the optional known-graph analysis and validate any flattening optimization by artifact correspondence (#10, PO-17) under the declared quota policy. Consider archive only after implementation, evidence and separately reviewed gates. r[WIRE-GRAPH-01]
