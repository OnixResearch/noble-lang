## Phase 1: Design (this change; document-only)

- [x] [serial] Re-verify the issue #8 baseline at `44329dc` (spec VC-LOGIC-02/VC-USE-01/VC-ADMIT-05/§8.3, `guard.rs` recognizer and wrappers, `op_invoke` compiled path, MC2 Lean guard theorems) and record that the cited paths have no drift from `0ea10d3`. (`VC-USE-01`) r[GUARD-BIND-01] r[GUARD-CORR-01]
- [x] [serial] Select applicability witnesses over refined input views; specify the grammar, formation/introduction/elimination/erasure rules, the no-conjunction decision, and composition through ordered stack interfaces. r[GUARD-BIND-01]
- [x] [serial] Specify exact guard binding and the C1–C6 correspondence statement, including refusal, undefined predicates, traps, exhaustion, effects and the consume-on-reject schema. r[GUARD-CORR-01]
- [x] [serial] Choose the single bounded `NonEmptyList` v2 extension with exact wrapper texts, and the fact-survival table separating `I64` wrapping from integer bounds. r[GUARD-LIST-01] r[GUARD-FACT-01]
- [x] [serial] Write the static and runtime examples, the GUARD-CASE-01…12 unexecuted designs and the evidence ledger; validate the change documents (document validation is not runtime or proof evidence). r[GUARD-CORR-01]

## Future implementation and evidence gates (not undertaken by this change)

- [ ] [serial] Prove C1 recognizer-link theorems for the three v1 templates and `NonEmptyList` over the reviewed MC1 `Holds`, and an extracted-Rust correspondence for `templates_of`; record them as scoped PO-21 evidence only. r[GUARD-CORR-01]
  Progress (2026-11-09): C1 is proved at model level for the three v1 templates.
  `proofs/mc2/NobleCertificates/GuardLink.lean` proves `Holds e [.i64 x] [] params ↔
  GuardPrecondition t x` for each of the five spellings `recognize` accepts: `(lt x0 i64::MAX)`,
  `(eq x0 k)` and `(eq k x0)`, and `(not (eq x0 i64::MIN))` and its swap. `recognized_link`
  covers every v1 template, and a control shows the refused `(le x0 i64::MAX)` holds at
  `i64::MAX`, where `ltI64Max`'s precondition fails. `MC2Gate.lean` pins the seven theorems
  under the strict axioms, 42 in all. Still open: the extracted correspondence for
  `templates_of` (v1) and `guard_v2::templates_of`, which needs a proved link from the
  `Prepared` arena to MC1
  terms. C1 for `NonEmptyList` is now proved at model level in `GuardsV2.nonEmptyList_link` and
  `nonEmptyList_link_of_type` (2026-11-20); its extracted link is the same open obligation.
- [x] [serial] Implement the versioned v2 template set (`NonEmptyList` recognizer, wrappers, `List A` slot injection for `op_invoke`, registry/digest version) with v1 bytes and gate unchanged; prove C2–C5 for `NonEmptyList` in a v2 MC2 gate. r[GUARD-LIST-01]
  Progress (2026-11-20): Lean model. `proofs/mc2/NobleCertificates/GuardsV2.lean` adds a separate
  `GuardTemplateV2` set (the three frozen v1 templates plus `nonEmptyList`), the pure
  `Type0Code` element predicate with exclusion controls, `HasInput`, `GuardPreconditionV2`,
  `guardRejectsV2`, the fixed guard/wrapper programs and both fixed renderer texts. It proves
  C1 (`nonEmptyList_link`, `nonEmptyList_link_of_type`), C2
  (`guardProgramV2_nonEmptyList_exec_iff`, `guardProgramV2_exec_iff`), C3
  (`nonEmptyList_sound`, `nonEmptyList_exact`, `guard_sound_v2`), C4
  (`wrapperProgramV2_exec_iff`, `wrapperProgramV2_rejects`, `wrapperProgramV2_accepts`,
  `guard_correspondence_v2`) and C5's refinement half (`sourceWrapperProgramV2_refines`,
  `liveWrapperProgramV2_refines`). `lake env lean MC2Gate.lean` (unchanged file) still reports
  `"guard_templates":3,"required_theorems":42,"result":"passed"`; `lake env lean
  MC2V2Gate.lean` reports `"guard_templates_v2":4,"guard_templates_v1":3,"type0_codes":7`
  and `"required_theorems":21,"result":"passed"` with `strict_axioms` exactly
  `["propext","Classical.choice","Quot.sound"]`. The `parse(renderer)` half of C5 and the C1
  link for `guard_v2::templates_of` are the extracted-Rust legs (task 1).
  Rust.
  `crates/noble-contracts/src/companion/guard_v2.rs` is the versioned implementation: the same
  four-template set under its own digest domain (`GUARD_SET_VERSION_V2 = 2`), a whole-root
  recognizer for exactly `Not(IsNil(Input 0))` on a one-element `[List A]` interface whose
  element is a pure `Type0` code, and the exact wrapper texts; `guard.rs` and `GuardTemplate`
  are byte-identical and `MC2Gate` is untouched. `ContractEntry` retains the recognised v2 set
  separately in `guards_v2`, so a v1 entry and a v2 entry can never be confused.
  `op_invoke` enters the v2 path only when the evidence's retained v2 set contains
  `NonEmptyList`; it takes the argument from a live session slot (never a CLI literal), requires
  that slot's type to equal the retained `[List A]` interface and the
  `[list, companion, Program]` operand layout, then runs the fixed live text on
  `[list, Program]`. The companion operand cell is removed by the recorded `swap drop` prelude
  (`invocation_prelude` in the report) because the guest exposes no cell-copy injection; the
  wrapper text itself is byte-identical to the design. Tests (observed): `cargo test -p
  noble-contracts --test companion guardv2` → 7 passed (recognition, nested `List (List I64)`,
  eight hostile whole-root/interface shapes plus two ill-typed refusals, frozen v1 bytes and
  their v2 lifts, the two fixed v2 texts, versions and digest domain separation, stable codes);
  `cargo test -p noble-cli --test smoke guardv2` → 4 passed, executing on the real runtime the
  source text (`nil` → `Sum left 1` with the body not run, `7 nil cons` → `Sum right 99`), the
  live text, the prelude-plus-live-text composition (empty → `left 1`, nonempty → `right 99`,
  prelude leaves `[list, Program]`), and the frozen v1 texts (`MAX` → reject, `MAX-1` → accept,
  `eq 5` accept/reject). Still open from this task: an execution of the companion-level
  `op_invoke` path against a frozen executable with an admitted list-contract proof (GUARD-CASE
  lane, task 3).
  Reworked 2026-11-22 (owner decision 32, delegated: "do what is most correct/best long term").
  The `[list, companion]`-only layout and its `swap drop` prelude consumed the session and could
  not observe GUARD-CASE-03's tail clause, while the design names "the existing op_invoke route"
  with an argument injection taken from a session slot. `invoke_list` now copies the selected
  slot's list into a Program cell (`dup quote`, carried to the top past the cells above the slot;
  `quote` captures only data, K-PROG-07), parks the whole frame as the v1 path does, injects the
  copy and the retained subject into a fresh frame, runs the unchanged live text after the
  `[ run ] dip` prelude, restores the frame (tail and companion preserved, identity re-checked)
  and drops the copy on every path; a slot more than 32 cells below the top is refused at
  `stack-depth`. Reports carry `invocation_prelude` and `invocation_argument_copy`. Observed:
  `cargo test -p noble-cli --test smoke guardv2` -> 5 passed, including the new prelude
  composition and the copy past a cell above the slot.
- [ ] [serial] Register and execute GUARD-CASE-01…12 against frozen executables, retaining stacks, identities, counters and refusal stages; keep exhaustion/trap distinct from rejection. r[GUARD-BIND-01] r[GUARD-CORR-01]
  Progress (2026-11-22): registered in `specs/conformance/guard-cases.json` (profile
  `Bounded-Applicability-Design`, all twelve absent/not-run/open/unassessed, citing the canonical
  VC-LOGIC-02, VC-USE-01, VC-ADMIT-05 and VC-COMPOSE-01). A separate file, not
  `contract-cases.json` as the design suggested, because the MC2 gate treats any other case in
  that file as unscoped. Case designs follow owner decisions 32-34: GUARD-CASE-03 keeps its tail
  clause; GUARD-CASE-11 uses a harness-level fault model; GUARD-CASE-02's substituted binding runs
  as a core control (`controls/guard.rs`); its registered refusal is `correspondence-mismatch`,
  the code `release::correspond` returns for evidence presented under another contract, where
  the design had guessed `MismatchedContext`/`StaleContext`. Three designs were made executable:
  GUARD-CASE-07's changed predicate uses
  `ensures true` on both contracts (the design's unchanged `head` postcondition is unprovable
  under `requires true`, so that contract could never be admitted); GUARD-CASE-10 records every
  guarded composition refusing at the implication (`rules/derive.rs::composition` composes only
  two `IncrementBy` premises, so no composite with a guard can form) and keeps the existing
  control that a derived increment composite carries no guard; GUARD-CASE-08's family variant
  presents a builder with a changed captured constant.
  Progress (2026-11-22, before the first gate run): the design's empty `[]` arguments in
  GUARD-CASE-03 and -09 were registered as a bare `nil`, whose submission leaves the element type
  open, so its slot is refused at injection typing (`stack-type`, GUARD-CASE-12's refusal); they
  are now spelled `nil dup 0 swap cons drop`, an empty `List I64` (the nested argument likewise).
  GUARD-CASE-11 is implemented under decision 33 and amended under decision 33a (the case file's
  `amendments` block): the test-only `live-test-barrier` feature build's one-shot
  `NOBLE_LIVE_TEST_GUARD_FAULT` selection, keyed by the SHA-256 of the guard's checked source,
  either lowers that one execution's step cap or raises a Wasm trap at the top of its guest
  frame. `noble companions` reports an abnormal guard exit as an `invoke` line with outcome
  `guard-runtime`, the engine's classification, quota reason, trap text and charged work, ends
  the session (`terminated-after-runtime-failure`) and refuses every later operation
  (`session-terminated`): the poisoned engine's parked frame is never restored. A step-cap sweep
  showed a cap of 1 exhausting in reflection/type-observation work with no execution step and
  caps of 2 to 32 exhausting in execution work, where the body's start is not observable, so the
  case uses cap 1.
- [ ] [serial] Coordinate C6 (Wasm lowering/loading of wrappers, PO-17/PO-18) through #10. Static-witness companion rules wait for #7; resource-bearing applicability waits for #3. r[GUARD-FACT-01]
