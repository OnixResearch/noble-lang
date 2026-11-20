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
  wire_library::` — 2 passed (`wire_library::runs_in_a_fresh_declared_session`,
  `wire_library::split_refusal_is_the_session_cumulative_budget`).
- [ ] [serial] Prove `Wire.*` theorems for L1-L8 and L10-L14 in the pure MC1 fragment. This includes the general frame lemma and whitelisting `List.nil_append`/`List.append_nil`. Package `wire@1.seq-laws` only after #7 bundle support exists. r[WIRE-LAW-01] r[WIRE-EVID-01]
  Progress (2026-11-09): the Lean half is proved in the pure MC1 model, outside the source-proof
  vocabulary. `proofs/mc1/NobleContracts/Wire.lean` inlines the `wire@1` bodies (route (a)) and
  proves L1-L8 and L10-L14 as `Exec` equivalences, plus the general frame lemma `exec_frame`
  (`Exec p s t → Exec p (r ++ s) (r ++ t)`). `ContractGate.lean` pins the 23 theorems under the
  strict axioms, 90 in all. L5, L6 and L11-L13 are stated on the stack shapes their signatures
  require, and L7 assumes every result of `p` has a top value, which `Program<S+A,S+C>`
  guarantees. Still open: whitelisting `List.nil_append`/`List.append_nil` in the reviewed
  source-proof vocabulary, and packaging `wire@1.seq-laws`, which waits for #7 bundle support.
- [ ] [serial] Develop trace, ownership and failure models: PO-04, PO-05, PO-06, V-MODEL-03/04 and V-RESOURCE-01..03. Develop resource-admitting execution before claiming any T, O or F entry or packaging the frame bundle. r[WIRE-LAW-01] r[WIRE-EVID-01]
- [ ] [serial] At sync, register WIRE-CASE-01 to 24 as conformance designs and add a `WIRE` route to `tools/check-specs.mjs` `ROUTES`. Then execute with independent expectations and retain receipts without promoting proof status. r[WIRE-EVID-01]
- [ ] [serial] Implement the optional known-graph analysis and validate any flattening optimization by artifact correspondence (#10, PO-17) under the declared quota policy. Consider archive only after implementation, evidence and separately reviewed gates. r[WIRE-GRAPH-01]
