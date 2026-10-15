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

- [ ] [serial] Decide the library's home, which needs a module-publication decision, and add effect-union signature syntax or explicit adaptation helpers. Investigate the preparation refusals of effectful `split` and of `split`/`fanout` in long sessions (O11-4, O11-5). Make eligibility diagnostics name the library word. r[WIRE-API-01]
- [ ] [serial] Prove `Wire.*` theorems for L1-L8 and L10-L14 in the pure MC1 fragment. This includes the general frame lemma and whitelisting `List.nil_append`/`List.append_nil`. Package `wire@1.seq-laws` only after #7 bundle support exists. r[WIRE-LAW-01] r[WIRE-EVID-01]
- [ ] [serial] Develop trace, ownership and failure models: PO-04, PO-05, PO-06, V-MODEL-03/04 and V-RESOURCE-01..03. Develop resource-admitting execution before claiming any T, O or F entry or packaging the frame bundle. r[WIRE-LAW-01] r[WIRE-EVID-01]
- [ ] [serial] At sync, register WIRE-CASE-01 to 24 as conformance designs and add a `WIRE` route to `tools/check-specs.mjs` `ROUTES`. Then execute with independent expectations and retain receipts without promoting proof status. r[WIRE-EVID-01]
- [ ] [serial] Implement the optional known-graph analysis and validate any flattening optimization by artifact correspondence (#10, PO-17) under the declared quota policy. Consider archive only after implementation, evidence and separately reviewed gates. r[WIRE-GRAPH-01]
