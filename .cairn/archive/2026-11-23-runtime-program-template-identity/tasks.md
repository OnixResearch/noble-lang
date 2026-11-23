## Phase 1: Draft and independent normative decision

- [x] [serial] Propose the existing P-ID-03 delta and draft negative scenario vectors: fixed literal/checked computed constant versus varying root input, duplicated/recomposed same binding versus equal-valued independent bindings, lexical owner versus VM owner, missing/ambiguous compiler provenance. Preserve P-RECIPE-04's literal recipe and ID-05's unexecuted 2→3 relation; do not sync or promote canonical cases. r[P-ID-03] `P-RECIPE-04`
- [x] [serial] Record the owner's approval in this isolated draft of the anonymous D domain, fixed-versus-varying operand rule, source-binding alias equivalence and semantic lexical recursive owner. This decision does not sync canonical P-ID-03, implement returned-target admission or establish proof/source-bound acceptance. P-ID-04's portable encoding remains unselected; no implementation may use a builder D or asserted string as a target D. r[P-ID-03] `P-ID-04`

## Phase 2: Future source-bound implementation and acceptance (open)

- [x] [serial] Extend the committed bounded selected I64 builder source→compiled instruction/table-site→actual Program operand lineage and strict experimental returned-target D/P to independently verified logical source-binding aliases and semantic lexical recursive owners where supported. The strict subset already distinguishes fixed literals/input-independent computed constants from varying root inputs; unsupported shapes remain opaque, not assigned an identity. Do not treat spans/NodeIds as canonical D or assume compiler helpers outside the trusted current control. r[P-ID-03] `P-ID-01`
  Progress (2026-11-20, Cairn campaign): an implementation of tasks 1, 2 and 4 (verified logical binding classes, lexical recursive owner, v4 experimental identities binding a template projection, target compiler metadata, with tests) was written on branch `cairn/template-identity` but withdrawn before integration. Its compiler-side projection in `crates/noble-wasm/src/source/origin.rs` lies inside the M4 wasm extraction lane, and the pinned Charon/Aeneas toolchain and the lane's external model cannot represent it: string-spelled keys (`String` push, compare, `format!` erasure), `slice::sort_by`, derived `Debug` over strings and pointer-address cell identity. An extraction-compatible redesign (structured rather than string keys, no sorting or formatting in extracted code) or a separately reviewed extension of the wasm lane's external model is required before these tasks can land; the three boxes stay open.
  Progress (2026-11-22, Cairn batch 6): the withdrawn implementation of tasks 1, 2 and 4 is
  re-applied (`cairn/template-identity` commits `d8d44d01`, `8301bb76` and `0a96aca1`) with its
  compiler-side projection redesigned for the extraction lane: the template is a set of
  structural rows (`OriginBinding` classes naming earlier class, static-body and fixed-recipe rows,
  interned by flat equality), revisited cells are recognised with `Rc::ptr_eq`, loops carry flags
  instead of early returns, and no String, `format!`, sorting or iterator adapter remains in
  `noble-wasm`. The host renders the canonical spelling (`q(r0)`, `c(…,…)`, …) from the rows and
  sorts by it, so the worker JSON, the returned-graph check and the v4 identities are as withdrawn.
  Charon and Aeneas translate the whole crate with the lane's arguments and the generated external
  model needs no new axiom or type. The three boxes are ticked only with the batch-6 extraction
  ladder (M4 discovery, M3-Wasm lane) and gates.
  Evidence (MEASURED, batch 6; implementation `d3ee4b70` re-applied from the withdrawn `d8d44d01`,
  redesigned in `777ab009`, shaped for the pinned Lean backend in `6858b0ef`, lint-cleared in
  `08b28e5a`/`b980c38f`): `noble-wasm` derives a bounded `OriginTemplate` from the checked selected
  source as structural rows: ordered distinct root captures; logical source-binding classes
  (`OriginBinding`: named owner, static body row, quoted root input, fixed recipe row, quoted
  Program class, composition of two classes) interned by flat equality with VM occurrence counts;
  content-keyed static-body and fixed-recipe tables; and a classified recursive owner.
  `live_slot_returned_dag` independently re-derives captures, classes and owner from its own
  selected-source replay and requires equality of the host-rendered canonical spellings.
  Same-binding duplication is one class with two VM occurrences (`q(r0)` occurrences 2);
  independent equal-valued root bindings stay two classes (`q(r1)`, `q(r0)`); reused versus
  re-executed Program bindings keep one class and the same D/P. A reachable reference to the
  enclosing named definition classifies as `SelfNamed` and mints no identity. The batch-6 M4
  discovery (`70cb4380`) regenerated the Wasm subject (`6173fbed`) with no new external-model
  axiom or type; the lock was renewed from the `8b601beb` discovery (`34f0ce97`) and checked
  (`M4-IMPLEMENTATION-GATE passed: sha256:85cf4e02…`, `daabe467`), and the M3-Wasm lane passed at
  `c64275ec` (`M3-EXTRACTION-GATE passed: sha256:3a5a39da…`,
  `verification/m3-wasm/implementation-20261122-c64275ec.json`). Tests: `cargo test -p noble-cli
  --bin noble returned_dag` (`returned_target_binds_verified_logical_alias_classes`), `cargo test
  -p noble-wasm --test live_slot_artifact`
  (`selected_target_projection_classifies_fixed_computed_and_root_bindings`,
  `selected_template_projection_separates_logical_bindings_from_vm_sharing`), `cargo test -p
  noble-wasm --lib origin` (`template_projection_requires_a_classified_recursive_owner`).
- [x] [serial] Extend the committed epoch-qualified returned-graph/operand and saved-owner check beyond strict experimental returned-target D/P to the approved identity-bearing coverage, verifying site/operand/owner/alias mappings against accepted source and captures; refuse identity-bearing targets whenever required mapping is absent, forged, truncated, stale or ambiguous. Verify same-binding duplication versus independent equal-valued quote graph with changed VM sharing/code-owner/pin; the current strict subset is not general returned-target admission. r[P-ID-03] `P-RECIPE-04`
  Evidence (MEASURED, batch 6, same commits and extraction ladder): identity-bearing admission
  refuses a target whose compiler projection is absent, truncated, forged (capture position,
  binding class or occurrence count), duplicated, stale across selections, or requires an
  unclassified lexical recursive owner; the retained owner, operand and site checks remain.
  Same-binding duplication versus independent equal-valued quote graphs is exercised with changed
  VM sharing (`shared-program` one cell versus `repeat-root` two cells: same D/P, occurrence counts
  1 versus 2) and with a changed code owner/pin (`renamed-alias`: distinct installed static donor
  and manifest, same D/P). Tests: `cargo test -p noble-cli --bin noble returned_dag`
  (`returned_target_refuses_absent_forged_truncated_and_ambiguous_template_metadata`), `cargo test
  -p noble-cli --test live_slot_selected_origin` (full lane), `cargo test -p noble-wasm --test
  live_slot_artifact` (`selected_template_projection_refuses_absent_forged_truncated_and_stale_metadata`).
  Still experimental local D/P only; not general returned-target admission.
- [x] [serial] Admit only the bounded two-root-I64, two-quote, two-compose, three-add returned target with a host-and-worker independently recomputed source-bound composite manifest: first-generation verified native quote/compose code donor, selected static add entry owner, exact loaded Wasm SHA/code spans, retained complete graph and ordered captures, experimental target D/P and checked source sites. Exercise rewritten table-entry/binary, missing core attestation, attempted donor discard while a saved target holds it, released saved owner, source-site substitution, opaque shape and asserted-manifest refusals before claiming a narrow proof-free dynamic publication; no caller-hash or builder-proof shortcut. Selected offline origin tests passed 3/3 and the actual selected binary SHA-256 `3562fd93f50e4f319853c3eeef33ded90c4df82ce6928f4c6ae359643708fda9` published and froze exact captured alias targets at epochs 1→2 in `/tmp/noble-lslot-resource-composite-admit-0vnFpX/LSLOT01-selected-origin-control.summary.json`; proof-required remains refused, canonical LSLOT case states remain unchanged. r[P-ID-03] `P-ID-07` `H-LSLOT-01`
- [x] [serial] Extend target compiler metadata and host selection to exact returned P/D/captures and installed Wasm, with tests for fixed2→3 D-different, checked computed2→3 D-different, verified root-input2→3 same-D/different-P, logical alias/lexical owner distinctions and all negative provenance cases. No positive LSLOT-05/08 outcome before actual target/proof/replay evidence. r[P-ID-03] `P-ID-07` `VC-LSLOT-01`
  Evidence (MEASURED, batch 6, same commits and extraction ladder): the compiler
  `SelectedOriginMetadata` carries the exact returned template projection per output and the host
  emits it as `templates`; host selection re-derives it and refuses disagreement. The experimental
  v4 target descriptor binds the verified-absent lexical recursive owner, and the composite
  manifest binds `template_sha256`, `identity.lexical_recursive_owner`, `identity.template_classes`
  and `source.template_captures/template_classes` beside the installed static and native donors
  (module id, artifact SHA-256, code span, entries), graph and recipe digests. The worker
  (`slot-engine.mjs`) recomputes the same projection and manifest independently. Tests: `cargo test
  -p noble-cli --test live_slot_selected_origin` asserts fixed/computed 2→3 D-different,
  root-input 2→3 same-D/different-P, logical alias classes and the manifest projection fields;
  `cargo test -p noble-wasm --test live_slot_artifact` asserts the projection matrix, alias/owner
  distinctions and every negative provenance case. No LSLOT-05/08 case state or evidence was
  promoted.
- [x] [serial] Add an actual-target Lean consumer with explicit statement/source/artifact premises; separately test ID-05 capture2→3 and LSLOT-05/08 proof-required versus replay-only adversarial boundaries without inheriting builder proof or real-effect authority. r[P-ID-03] `V-MODEL-01` `VC-LSLOT-01` `EV-LSLOT-01`
  Progress 2026-11-22 (integrator decision 43): the proof-required half is implemented for live
  slots. The statement premise is the slot authority's exact MC2 contract, the source premise is
  the target's closed subject rendered from its verified template and captured values, and the
  artifact premise is the target's composite code-donor manifest digest; the consumer of `noble
  build --require-proof` rechecks the proof and the core binds it to that subject. A builder or
  caller proof does not transfer (old-version and generic-wrapper records are refused), and the
  capture boundary holds: the valid proof for capture 2 is refused for the capture-1 target by the
  core's correspondence check. Still open: LSLOT-08's replay-only boundary and an ID-05
  capture 2→3 receipt of its own.
  Done 2026-11-23 (Cairn push B): the actual-target Lean consumer is the proof-required half
  (decision 43; LSLOT-05 promoted), and LSLOT-08's replay-only boundary runs from two actual
  baselines (decision 45; LSLOT-08 promoted); both from
  `verification/lslot-cases/acceptance-2026-11-23-pb.json` (sha256 `455597be…`, clean source
  `0fffc7d8`). Exact replay eligibility is neither behavioral proof nor real-effect authority.
  ID-05's capture 2→3 has its own record in
  `verification/live-slot-lanes/runs/20261123-0fffc7d8/receipt.json` (sha256 `98313669…`): one
  definition template, two ProgramValueIds for captures 2 and 3, and fixed-literal builders with
  different templates. ID-05 is a ProgramsData-Draft case and stays absent/not-run.
- [x] [serial] After independent source-bound review, decide any canonical sync, generated views/ledger/receipt, evidence/status promotion and archive under the existing gates; leave all nine LSLOT states and historical evidence untouched until then. r[P-ID-03] `EV-LSLOT-01`
  Progress 2026-11-23 (owner decisions 50 and 55): the integrator closeout review
  [`review-integrator-2026-11-22-closeout.md`](review-integrator-2026-11-22-closeout.md) accepts
  the sync with notes; the owner did not veto it. The delta's stale status paragraph was rewritten
  (N-1), `P-ID-03` was synced in push B with regenerated views and ledger, and the LSLOT-05/08
  states were promoted only from the push-B LSLOT gate receipt. The archive follows with the next
  ladder, because `crates/` tests and the live-slot lanes receipt tool cite this change's path.
  Done 2026-11-23: decided and executed as above. The sync landed in `942aaef3` (check-specs
  `--refresh-ledger --self-test --report` passed, `cairn validate` valid), LSLOT-05/08 were
  promoted in `d0417282` from `verification/lslot-cases/acceptance-2026-11-23-pb.json`, ID-05
  stays a ProgramsData-Draft case with its own lane record, and the change is archived with the
  proposal, design and tasks gates passing.

LSLOT-02's separate atomic physical epoch-1 A+B and A+B+C genesis is an approved implementation choice, not a P-ID-03 requirement or a completed task here. The integrated selected CLI exercise includes both complete maps, an old-root pin through epoch 2 and worker-side later-member quota refusal without partial installation; see `/tmp/noble-lslot-approved-integrated-final-20261006/receipt.json` (SHA-256 `b217d63192e42293398da7592ddc6024fdd971787d142ab3d5b9aeaf6716ce38`). Canonical LSLOT-02 case execution/evidence and all nine LSLOT implementation/execution/proof/trust states remain absent/not-run/open/unassessed in the existing `explicit-live-references` change under `K-LSLOT-02`, `H-LSLOT-01` and `EV-LSLOT-01`; neither sequential publication nor rewriting the frozen fixture can close them. The unsynced P-ID-03 draft and selected captured-target Wasm/Lean proof/replay remain open.

Superseded 2026-11-23 (Cairn push B): the paragraph above records the state before the LSLOT gate. All nine LSLOT cases are now implemented/passed with proof open from `verification/lslot-cases/acceptance-2026-11-23-pb.json`, `P-ID-03` is synced, and the selected captured-target evidence (LSLOT-05) and replay-only boundary (LSLOT-08) are executed; a proof that the derivation is correct and general returned-target admission remain open.
