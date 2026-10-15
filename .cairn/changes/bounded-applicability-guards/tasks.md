## Phase 1: Design (this change; document-only)

- [x] [serial] Re-verify the issue #8 baseline at `44329dc` (spec VC-LOGIC-02/VC-USE-01/VC-ADMIT-05/§8.3, `guard.rs` recognizer and wrappers, `op_invoke` compiled path, MC2 Lean guard theorems) and record that the cited paths have no drift from `0ea10d3`. (`VC-USE-01`) r[GUARD-BIND-01] r[GUARD-CORR-01]
- [x] [serial] Select applicability witnesses over refined input views; specify the grammar, formation/introduction/elimination/erasure rules, the no-conjunction decision, and composition through ordered stack interfaces. r[GUARD-BIND-01]
- [x] [serial] Specify exact guard binding and the C1–C6 correspondence statement, including refusal, undefined predicates, traps, exhaustion, effects and the consume-on-reject schema. r[GUARD-CORR-01]
- [x] [serial] Choose the single bounded `NonEmptyList` v2 extension with exact wrapper texts, and the fact-survival table separating `I64` wrapping from integer bounds. r[GUARD-LIST-01] r[GUARD-FACT-01]
- [x] [serial] Write the static and runtime examples, the GUARD-CASE-01…12 unexecuted designs and the evidence ledger; validate the change documents (document validation is not runtime or proof evidence). r[GUARD-CORR-01]

## Future implementation and evidence gates (not undertaken by this change)

- [ ] [serial] Prove C1 recognizer-link theorems for the three v1 templates and `NonEmptyList` over the reviewed MC1 `Holds`, and an extracted-Rust correspondence for `templates_of`; record them as scoped PO-21 evidence only. r[GUARD-CORR-01]
- [ ] [serial] Implement the versioned v2 template set (`NonEmptyList` recognizer, wrappers, `List A` slot injection for `op_invoke`, registry/digest version) with v1 bytes and gate unchanged; prove C2–C5 for `NonEmptyList` in a v2 MC2 gate. r[GUARD-LIST-01]
- [ ] [serial] Register and execute GUARD-CASE-01…12 against frozen executables, retaining stacks, identities, counters and refusal stages; keep exhaustion/trap distinct from rejection. r[GUARD-BIND-01] r[GUARD-CORR-01]
- [ ] [serial] Coordinate C6 (Wasm lowering/loading of wrappers, PO-17/PO-18) through #10. Static-witness companion rules wait for #7; resource-bearing applicability waits for #3. r[GUARD-FACT-01]
