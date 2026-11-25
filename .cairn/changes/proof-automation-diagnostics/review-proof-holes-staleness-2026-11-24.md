# Review record: editor-only proof-hole transport and retained-evidence staleness (`PAUTO-CASE-11`, `PAUTO-CASE-15`)

- **Change and task:** [`.cairn/changes/proof-automation-diagnostics`](tasks.md), task 3 ("Add editor-only proof-hole transport that is rejected on every admission path, plus stale-status invalidation that retains old evidence. Execute PAUTO-CASE-11 and 15."). This record covers task 3 (`PAUTO-CASE-11` and `PAUTO-CASE-15`) only. Task 2 (`PAUTO-CASE-01`, `07`–`09`, `12`–`14`, `16`), task 5 (approach comparison, external SMT/tactic decision, conformance JSON registration, and requirement sync), and `V-INTRINSIC-01` reconciliation remain outside it.
- **Reviewed commits:**
  - Base implementation and policy re-pins at `e5e2ac5d888298aba9f19c28f2d8079ffd415e6f`:
    - `a237f334fbd0c5a362ca62f8335d7ac28bb2104b` (`feat(contracts,cli): add editor-only proof-hole transport and retained-evidence staleness`)
    - `64ba8a207b47738401bf29c1d62fb58152a567f5` (merge commit including `04c77899677d0a981f92f7ac762d2053b97d0f49`, `docs(cairn): record proof-hole and retained-evidence progress note`)
    - `d8df1055012083983d413e869c520c5b9a5ecf64` (`chore(policy): re-pin bound sources for Cairn push C`)
    - `61d02b612a73ef3a1c9eeeb5ff8956e64e67297f` (`fix(contracts): avoid Prop-vs-Bool Aeneas loop codegen in valid_hole_label`)
    - `e5e2ac5d888298aba9f19c28f2d8079ffd415e6f` (`chore(policy): re-pin logical.rs after valid_hole_label Aeneas fix`)
  - Gate and retained-receipt commits on `cairn/proof-automation-diagnostics`:
    - `a92b2a1be37a56f11d604f568e34f280c7f40013` (`test(pauto): add PAUTO-CASE-11 and PAUTO-CASE-15 holes-gate lane`)
    - `3a7ed88ff95a1f168782aac3764111dde4eb7b2f` (`fix(pauto): resolve --out to absolute path in holes-gate`)
    - `9c75094a0bb60314e48f2c9f92acdff405263620` (`fix(pauto): match verify-module resolve/proof-check premise diagnostics in holes-gate`)
    - `b4791ff32536cd2d9693c84b2b82ce6eb502e1a1` (`fix(pauto): match acceptance stage/code for shadowed premise binder in holes-gate` — live gate `tested_commit`)
    - `92602aaa808c13111380b75cbcf8858610ea50ef` (`test(pauto): retain PAUTO-CASE-11 and PAUTO-CASE-15 receipt for b4791ff3`)
- **Review process (fresh-context independent review):** Performed by five independent fresh-context agents that did not implement the work — two fresh-context reviewers, two fresh-context empirical challengers (`PAUTO-CASE-11` and `PAUTO-CASE-15`), and one fresh-context forensic integrity auditor — rather than an implementer self-review.
  - `OWNER DECISION 2026-11-20`: Independent review: wherever a task demands "independent review", "separately reviewed gates", "a reviewed conformance change" or review before sync/registration/archive, a fresh-context reviewer agent that did not implement the work performs the review. Its review record is retained in the repository (e.g. `verification/<lane>/review-<date>.md` or the change's `design.md` "Review" section) with the reviewed commit, findings and verdict. The owner keeps a veto. Slice agents do NOT review their own work; the integrator schedules the review.
  - `OWNER DECISION 2026-11-20`: `sync_route = integrator-review-owner-veto` — The integrator records reviews before remaining syncs, registrations and archives; the owner keeps a veto. Reconcile both rewrites of `V-INTRINSIC-01` before either sync. Sync and publication still require the applicable evidence and gates.
- **Date:** 2026-11-24
- **Verdicts:**
  - Fresh-context Reviewer 1: **APPROVE** (LOW risk; 0 Critical, 0 Major, 0 Integrity Violations)
  - Fresh-context Reviewer 2: **APPROVE** (LOW risk; 0 Critical, 0 Major, 0 Integrity Violations)
  - Fresh-context Challenger 1 (`PAUTO-CASE-11` & `holes-gate.mjs`): **APPROVE**
  - Fresh-context Challenger 2 (`PAUTO-CASE-15`): **APPROVE**
  - Fresh-context Forensic Integrity Auditor: **CLEAN**
  - **Owner veto:** Applies before requirement sync, conformance registration, archive, or push.
- **Labels:** `MEASURED` means the result was observed in an executed command or retained artifact listed under "Verification and evidence". `INFERENCE` means reasoning from inspected source that was not executed in that exact configuration.

## Reviewed files and retained receipt digests (at `92602aaa808c13111380b75cbcf8858610ea50ef`)

### Implementation, policy, and source files inspected

| File | sha256 |
|---|---|
| [`crates/noble-contracts/src/source/declared/state.rs`](../../../crates/noble-contracts/src/source/declared/state.rs) | `adec4fc96463f0540000dee1fd5a37d9455b63fb6fe81343bc89563a312391aa` |
| [`crates/noble-contracts/src/source/declared/state/register/logical.rs`](../../../crates/noble-contracts/src/source/declared/state/register/logical.rs) | `5e385025f7504ade6add1b116ed810919547505d0423761f2ace4c46b934386c` |
| [`crates/noble-contracts/src/intrinsic.rs`](../../../crates/noble-contracts/src/intrinsic.rs) | `f4066d73d3230c39ec8b39f4feb918d88beba116652371c8222bbaff0564a810` |
| [`crates/noble-cli/src/core/editor.rs`](../../../crates/noble-cli/src/core/editor.rs) | `45b62cef92505d8540aec9f6a864b95e822823f8dd006503b7514f056025a299` |
| [`crates/noble-cli/src/workflow.rs`](../../../crates/noble-cli/src/workflow.rs) | `8e75a584a7402f2504c4e654562b489f14c935ea1b9b3d8356818ac76339460c` |
| [`crates/noble-cli/src/workflow/evidence.rs`](../../../crates/noble-cli/src/workflow/evidence.rs) | `73766d8092471717c783806830acb6ca6103849668fdbdf7613abddd605dc3ea` |
| [`crates/noble-cli/src/workflow/intrinsic.rs`](../../../crates/noble-cli/src/workflow/intrinsic.rs) | `642d624d3aab5f69f17ff4e8b7434e265d875fea595bb5e7421a72e93738a499` |
| [`crates/noble-cli/src/workflow/obligations.rs`](../../../crates/noble-cli/src/workflow/obligations.rs) | `6b6263a6250dc67bdb2d099a7405791ef1087c40e8f71456ca1c808f595d3a3f` |
| [`crates/noble-cli/src/workflow/diagnostics.rs`](../../../crates/noble-cli/src/workflow/diagnostics.rs) | `ff2c080712bf982d464c7dca6f590e4ff078956c33b749298b78a736c58bc553` |
| [`crates/noble-cli/tests/editor.rs`](../../../crates/noble-cli/tests/editor.rs) | `cc010a5ee8c9ecd4bd38525c98b59bc3d39aad54bf3e6dcdebc6e54babd5ad08` |
| [`crates/noble-cli/tests/named_v2.rs`](../../../crates/noble-cli/tests/named_v2.rs) | `44850fb841c4906f4da8429ffb27d77ab3912ddc7a1cd99d8f90bebcec1e2081` |
| [`crates/noble-cli/tests/named_v2/obligations.rs`](../../../crates/noble-cli/tests/named_v2/obligations.rs) | `8538d49faa96a332b4eb0248b7d2aecb3c465ea1906a2e4a6585f9f259eb862c` |
| [`policy/tool-selection.json`](../../../policy/tool-selection.json) | `aa7fba1b4fc2a2e0402c72c44d017aa351901b9ab045c80f6279fdedd0c42e7a` |
| [`policy/tool-selection-files.ncl`](../../../policy/tool-selection-files.ncl) | `7a9f70d5a24be8c3ddd6b0a62794687224abbb7bd579016a81fc1089c0588031` |

### Gate, documentation, and retained receipt artifacts

| File | sha256 |
|---|---|
| [`verification/proof-automation-cases/holes-gate.mjs`](../../../verification/proof-automation-cases/holes-gate.mjs) | `d4cb7ab9b34f61603ad37a909d2c0ae2196e17031af84b485229aca6d7355339` |
| [`verification/proof-automation-cases/README.md`](../../../verification/proof-automation-cases/README.md) | `b78f8e99b62d6cc6a92cfcf112be2a83c483ba8f12439918feca357c62abfe68` |
| [`verification/proof-automation-cases/runs-holes/20261124-b4791ff3/acceptance.json`](../../../verification/proof-automation-cases/runs-holes/20261124-b4791ff3/acceptance.json) | `4139179f2b604eec3d1a0f7afde6e02351c7b55d74cf2a7bcebef2428afbab54` |
| [`verification/proof-automation-cases/runs-holes/20261124-b4791ff3/evidence/v1-accepted/evidence.json`](../../../verification/proof-automation-cases/runs-holes/20261124-b4791ff3/evidence/v1-accepted/evidence.json) | `d7426e8648dcc926186a300feea2661be02c29e615b9eebc96b7e2d8bba6a9d4` |
| [`verification/proof-automation-cases/runs-holes/20261124-b4791ff3/evidence/v1-accepted/report.json`](../../../verification/proof-automation-cases/runs-holes/20261124-b4791ff3/evidence/v1-accepted/report.json) | `4e2dfd314efffc065239cda84cdcd459dcf6d4a10278cd1539c104cc4e923ced` |
| [`verification/proof-automation-cases/runs-holes/20261124-b4791ff3/evidence/v1-accepted/source.noble`](../../../verification/proof-automation-cases/runs-holes/20261124-b4791ff3/evidence/v1-accepted/source.noble) | `014f813c88f67232bb483d0c1192bd57885c4b49ec01f840cab72ffbfa74c7ed` |
| [`verification/proof-automation-cases/runs-holes/20261124-b4791ff3/evidence/old-rev/evidence.json`](../../../verification/proof-automation-cases/runs-holes/20261124-b4791ff3/evidence/old-rev/evidence.json) | `29326a8cf26a9e4dc817fd9e679173db1c822fdf11efc37ba1a2bb40716ad318` |
| [`verification/proof-automation-cases/runs-holes/20261124-b4791ff3/evidence/forged/evidence.json`](../../../verification/proof-automation-cases/runs-holes/20261124-b4791ff3/evidence/forged/evidence.json) | `d7426e8648dcc926186a300feea2661be02c29e615b9eebc96b7e2d8bba6a9d4` |
| [`verification/proof-automation-cases/runs-holes/20261124-b4791ff3/evidence/forged/source.noble`](../../../verification/proof-automation-cases/runs-holes/20261124-b4791ff3/evidence/forged/source.noble) | `2c889fe87ac8b390c5f03f6d4f5cbdf6b42b3b12b1bbc73cc7ce9f120e3a0507` |

Retained receipt metadata (`acceptance.json`):
- `schema`: `"noble-proof-automation-holes-acceptance/v1"`, `result`: `"completed"`, `verdict`: `"all-passed"`, `dirty`: `false`, `receipt_ready_for_retention`: `true`, `hygiene_violations`: `0`, `failures`: `[]`.
- `tested_commit`: `"b4791ff32536cd2d9693c84b2b82ce6eb502e1a1"`.
- `source_revision`: `"sha256:ef816430528dabb1c9ffe3fe5ecc1e247837edf22584dd2a12cebd9497cea395"` (verified over all 609 `SCOPE` files at both `b4791ff3` and `92602aaa`).
- `promotable`: `["PAUTO-CASE-11", "PAUTO-CASE-15"]`, `unmet`: `[]`, `failed`: `[]`, `not_executed`: `[]`.
- `binary` (`supplied_sha256` / `executed_sha256` / `executed_sha256_after`): `"747a7f740a7a0179b71296ec256dd32995c06cda73c0d5af8b644d81b2ce5d19"`.
- Every recorded `raw_outputs` file (21 files under `raw/`) and `fixtures` file (10 files under `sources/`) matched its recorded SHA-256 digest on disk (`MEASURED`).

## Structured Findings Table (Decision 1 / Decision 60)

| ID | Source | Severity | Location | Finding | Disposition |
|---|---|---|---|---|---|
| `REV2-01` | Reviewer 2 | Info | [`holes-gate.mjs:588-623`](../../../verification/proof-automation-cases/holes-gate.mjs), [`logical.rs:276, 1245-1466`](../../../crates/noble-contracts/src/source/declared/state/register/logical.rs), [`intrinsic.rs:2757-2765`](../../../crates/noble-contracts/src/intrinsic.rs) | `Resolve` vs `Acceptance` premise-refusal checks in `holes-gate.mjs` (`undeclared-premise-binder` and `undeclared-premise-missing` at `Stage::Resolve` / `intrinsic-resolve-invalid` / `rejected-source`; `undeclared-premise-shadowed` with prior `proof 1 h` passing `Resolve` and hitting `check_batch` at `Stage::Acceptance` / `intrinsic-proof-refused` / `rejected-term` / `"Acceptance: use does not name a proof premise"`) faithfully match unmodified `noble-contracts` and `noble-cli` without hiding bugs or weakening [`design.md:359`](design.md). | Accepted — accurate conformance coverage of both refusal paths. |
| `REV2-02` | Reviewer 2 / Challenger 2 | Minor (Pre-existing) | [`logical.rs:1458`](../../../crates/noble-contracts/src/source/declared/state/register/logical.rs) | `resolve_reference` checks every unqualified `(use NAME)` against `prior` (`proof` declarations) without tracking `intro` premise binders, so even `(intro (p (Eq I64 x x)) (use p))` without a prior `proof p` is refused fail-closed at `Stage::Resolve`. Pre-existing at `e5e2ac5d` (zero `crates/` edits in `e5e2ac5d..HEAD`) and outside `holes-gate.mjs` scope. | Noted (pre-existing baseline behavior; fails closed; non-blocking). |
| `REV2-03` | Reviewer 2 | Minor | [`holes-gate.mjs:534-568`](../../../verification/proof-automation-cases/holes-gate.mjs), [`evidence.rs:669-710`](../../../crates/noble-cli/src/workflow/evidence.rs) | `PAUTO-CASE-11` (`old-rev`) mutates `proofs[0].model_revision` on single-module `v1.noble` (`dependencies: []`) rather than a multi-module `use` dependency (`dependencies[i].model_revision`). `evidence::compare_dependencies` implements recursive dependency comparison identically, and Challenger 1 also probed live cross-module subject staleness (`depmath@1` `[ 1 + ] -> [ 2 + ]`) and injected `dependencies` entries (`dependencies.length` mismatch, exit `4`). | Accepted as minor coverage note; non-blocking. |
| `REV2-04` | Reviewer 2 | Minor | [`tasks.md:22, 39`](tasks.md) | All technical evidence for Task 3 is complete; finalization retains this review record, ticks Task 3, and updates the stale line-39 note so already-landed bound-file re-pins (`d8df1055`, `e5e2ac5d`) are no longer listed as open while keeping Track 1 M4 renewal / `specs/VALIDATION.json` and Task 5 registration/sync separate. | Resolved in the finalization commit. |
| `R1-01` | Reviewer 1 | Minor | [`holes-gate.mjs:534-567`](../../../verification/proof-automation-cases/holes-gate.mjs), [`evidence.rs:669-709`](../../../crates/noble-cli/src/workflow/evidence.rs) | Same observation as `REV2-03`: single-module `proofs[0].model_revision` mutation exercises `intrinsic-stale-proof` exit `4` and `goal-view` `stale` chip end-to-end; recursive `dependencies` check confirmed in `evidence.rs:669-709`. | Accepted (non-blocking). |
| `R1-02` | Reviewer 1 | Minor | [`holes-gate.mjs:517, 557`](../../../verification/proof-automation-cases/holes-gate.mjs), [`intrinsic.rs:1797-1804`](../../../crates/noble-cli/src/workflow/intrinsic.rs) | `case11-verify-stale` and `case11-revision-stale-verify` do not pass `{ stripSandbox: true }`, whereas `goal-view` calls do. Source inspection confirms `verify-module --evidence` returns `Err(evidence::stale_failure(...))` before Lean is invoked, and both outputs report `independent_recheck: false`; Challenger 1 independently confirmed exit `4` (`intrinsic-stale-proof`) with all `NOBLE_*` sandbox variables stripped. | Accepted (non-blocking). |
| `R1-03` | Reviewer 1 | Informational | [`intrinsic.rs:1075-1122`](../../../crates/noble-cli/src/workflow/intrinsic.rs) | Pre-existing at `e5e2ac5d`: `verify-module --emit EXISTING_DIR` invokes `evidence::write` after `execute()` completes, so `case11-emit-existing` runs Lean before failing with `emit-destination` (exit `2`), leaving retained `evidence.json` byte-identical. | Informational only (`crates/` untouched). |
| `CH1-01` | Challenger 1 | Note (Non-bypass) | [`evidence.rs:738-787`](../../../crates/noble-cli/src/workflow/evidence.rs), [`evidence.rs:546`](../../../crates/noble-cli/src/workflow/evidence.rs) | Two non-bypass observations: (1) `evidence::compare` checks per-proof `model_revision`/`checker_revision` rather than the top-level `revisions` summary object, so tampering only `revisions.checker` still leaves `goal-view` at `chip: "incomplete"` and runs full Lean recheck in `verify-module`; (2) `evidence::read_bounded` uses `std::fs::metadata`, so symlinks to matching-digest files are followed while symlinks to wrong-digest files fail `evidence-invalid` and dangling symlinks fail `evidence-io`. | Accepted (neither weakens security or acceptance). |
| `CH2-01` | Challenger 2 | Note (Ordering / Wording) | [`obligations.rs:360`](../../../crates/noble-cli/src/workflow/obligations.rs) | With sibling holes (`h1`, `h2`), `transport.holes` and `sites[0].sites` list holes in document order (`h1`, `h2`) while `sites[0].holes` lists `["h2", "h1"]` due to LIFO `Vec::pop()` in `proof_hole_sites`; `goal-view --proof-holes --module prior_hole.noble` refuses with `proof-hole-transport` (exit `2`) whereas `goal-view`/`obligations`/`verify-module` refuse with `intrinsic-contract-invalid` (exit `2`, `"editor-only"`). | Accepted (both refuse cleanly; non-blocking). |
| `AUD-01` | Forensic Auditor | Info | [`holes-gate.mjs`](../../../verification/proof-automation-cases/holes-gate.mjs), [`runs-holes/20261124-b4791ff3`](../../../verification/proof-automation-cases/runs-holes/20261124-b4791ff3/acceptance.json) | Anti-cheat static inspection, 609-file cryptographic `SCOPE` binding, 21 raw output + 10 fixture + `evidence.json` SHA-256 audit, live pinned Lean 4.35.0 consumer tracing (`bwrap` + `NamedV2Consumer.lean`, positive and negative controls), isolation, `noble-maintainers` authorship, and public-repo hygiene grep all passed with zero integrity violations (**CLEAN**). | Cleared. |

No Critical or Major findings. Zero integrity violations detected.

## Independent Review, Empirical Challenge, and Forensic Audit Summary

### 1. Retained-evidence emission and staleness (`PAUTO-CASE-11`)

- **Real Lean emission and non-clobbering (`MEASURED`):** `verify-module sources/v1.noble --emit evidence/v1-accepted --timeout-ms 600000` executes pinned Lean 4.35.0 (`NamedV2Consumer.lean` inside `bwrap`), producing `outcome: "proved"`, `stage: "independent-recheck"`, `independent_recheck: true`, `strict_lean_axioms: ["propext", "Quot.sound", "Classical.choice"]`, and `evidence.json` (`sha256:d7426e8648dcc926186a300feea2661be02c29e615b9eebc96b7e2d8bba6a9d4`). Re-emitting to an existing directory refuses with exit `2` (`emit-destination`) and leaves `evidence.json` byte-identical.
- **Read-only `goal-view --evidence` projection (`MEASURED`):** With all `NOBLE_*` sandbox variables stripped, matching `goal-view --evidence` reports `schema: "noble-goal-view/v1"`, `acceptance: false`, `outcome: "not-run"`, `category: "explanation-only"`, `checker_result: null`, `chip: "incomplete"`, `matches: true`, never grants `checked`, and leaves `evidence.json` unmodified. Matching `verify-module --evidence` still performs full `independent_recheck: true` against Lean.
- **Pre-Lean staleness and forgery refusal (`MEASURED`):** Changing the subject `[ 1 + ] -> [ 2 + ]` (`v2.noble`), mutating `proofs[0].model_revision` (`old-rev`), mutating `proofs[0].checker_revision` or `statement_digest`, injecting `dependencies`, or changing a proof-free dependency module all mark `goal-view --evidence` `stale` and cause `verify-module --evidence` (even with all `NOBLE_*` sandbox variables stripped) to refuse before Lean with exit `4` (`outcome: "unsupported"`, `category: "stale-dependency"`, `diagnostic.code: "intrinsic-stale-proof"`), leaving `evidence.json` byte-identical. Tampered `outcome`, `independent_recheck: false`, `schema`, empty `proofs: []`, truncated/non-JSON, forged `source.noble`, and symlinked wrong-source files all fail closed with exit `2` (`evidence-invalid`, `forged-identity`).

### 2. Editor-only proof holes and undeclared premise boundaries (`PAUTO-CASE-15`)

- **Isolation of `ModuleSession::proof_editor` (`MEASURED`):** `ModuleSession::{new,new_signed_self,new_declared_text,new_calculator}` all set `proof_editor: false`; only `ModuleSession::new_proof_editor` (`state.rs:1735`) sets `proof_editor: true`, called exclusively by `goal-view --proof-holes` (`obligations.rs:1796`) and its unit test helper (`obligations.rs:2164`).
- **Transport boundaries (`MEASURED`):** Valid `noble-proof-editor/v1` transport (`goal-view --proof-holes`, including multi-hole sibling/cross-proof cases and the 64-hole boundary) reports exit `3`, `outcome: "unknown"`, `category: "incomplete-hole"`, `chip: "incomplete"` (remaining `incomplete` even with matching `--evidence`). Over-limit (65 holes), 0-hole documents, duplicate/invalid labels, even-length arrays, comment-spoofed holes, extra un-transported holes, and misplaced holes in `requires`, `ensures`, pure propositions, headers, or `def` bodies all fail closed with exit `2` (`proof-hole-transport`).
- **Admission and ordinary-command refusal (`MEASURED`):** Raw `(editor-hole ...)` in `.noble` source and `--module` prior-file smuggling refuse with exit `2` across `verify-module` (`intrinsic-contract-invalid`, `"Check: a proof hole is editor-only and never .noble source"`, `runtime_prover_calls: 0`), `obligations`, `goal-view`, `run`, `session --framed --declared-modules` (`guest_requests: 0`, `prior_namespace: "unchanged"`), and `editor`.
- **Undeclared premise `(use h)` (`MEASURED`):** `undeclared-premise-binder.noble` and `undeclared-premise-missing.noble` refuse at `Stage::Resolve` (`intrinsic-resolve-invalid`, `rejected-source`, exit `2`); `undeclared-premise-shadowed.noble` (with a prior `proof 1 h` shadowed by `(intro (h I64) (use h))`) refuses at `Stage::Acceptance` (`intrinsic-proof-refused`, `rejected-term`, `"Acceptance: use does not name a proof premise"`, exit `2`), matching `logical.rs:276, 1245-1466` and `intrinsic.rs:2757-2765`.

### 3. Forensic integrity, isolation, and public-repo hygiene

- **Anti-cheat and reproducibility (`MEASURED`):** Reviewer 1 independently re-executed `holes-gate.mjs` from scratch on `92602aaa`, reproducing `verdict: "all-passed"`, `promotable: ["PAUTO-CASE-11", "PAUTO-CASE-15"]`, `source_revision`, `evidence.json` (`Buffer.compare === 0`), and all 21 raw stdout files bit-for-bit against [`runs-holes/20261124-b4791ff3/`](../../../verification/proof-automation-cases/runs-holes/20261124-b4791ff3/acceptance.json).
- **Policy and file isolation (`MEASURED`):** `e5e2ac5d..92602aaa` modified only [`verification/proof-automation-cases/holes-gate.mjs`](../../../verification/proof-automation-cases/holes-gate.mjs), [`verification/proof-automation-cases/README.md`](../../../verification/proof-automation-cases/README.md), and [`verification/proof-automation-cases/runs-holes/20261124-b4791ff3/`](../../../verification/proof-automation-cases/runs-holes/20261124-b4791ff3/acceptance.json), leaving `gate.mjs`, `runs/`, `crates/`, `policy/*`, `specs/*`, and `crates/noble-cli/src/core/runtime/config.json` untouched. Every commit uses `noble-maintainers <maintainers@onixresearch.org>`.

## Scope and Non-Claims

- **Covered here:** Task 3 (`PAUTO-CASE-11` and `PAUTO-CASE-15` live pinned-Lean-consumer gate, `noble-proof-editor/v1` transport isolation, and `noble-intrinsic-evidence/v1` staleness/forgery enforcement). Bound-file re-pins for `state.rs`, `workflow/intrinsic.rs`, and `logical.rs` already landed on the base branch at `d8df1055` and `e5e2ac5d`.
- **Not claimed here (remaining open):**
  - Task 2 (`PAUTO-CASE-01`, `07`–`09`, `12`–`14`, `16` sandboxed template and helper producers) stays `[ ]`.
  - Task 5 (approach-comparison criteria evaluation, external SMT/tactic profile decision, `specs/conformance/*.json` case registration for `PAUTO-CASE-02..06`, `PAUTO-CASE-11`, and `PAUTO-CASE-15`, and requirement sync) stays `[ ]`.
  - `V-INTRINSIC-01` delta reconciliation and sync (Owner Decision 60) and change archive remain open.
  - Track 1 Push C M4 extraction renewal, `policy/source-inventory*`, and `specs/VALIDATION.json` remain isolated on the Push C track.
  - Under Owner Decisions 1 and 60, owner veto applies before requirement sync, conformance registration, archive, or push.

## Verification and evidence (`MEASURED` on `92602aaa`, prior to finalization docs commit)

- `node verification/proof-automation-cases/holes-gate.mjs --self-test` -> exit `0` (`holes-gate self-test: verdicts, proof-hole splice round-trips, hygiene controls, and Mismatch checks passed`).
- `node verification/proof-automation-cases/gate.mjs --self-test` -> exit `0` (`gate self-test: only fully observed cases are promotable; the unbounded-integer scan rejects Int claims`).
- `cairn validate --root . --json` -> exit `0` (`valid: true`, `changes: 14`, `change_issues: []`, `issues: []`, `findings: []`, `spec_issues: []`).
- `bun tools/check-specs.mjs` -> exit `0` (`documents: 16`, `requirements: 645`, `scenarios: 330`, `result: "passed"`).
- `cargo fmt --check` -> exit `0`; `cargo clippy --all-targets --all-features -- -D warnings` -> exit `0`; `cargo test -p noble-cli` -> exit `0` (all test targets passed; 3 pinned-Lean tests in `tests/named_v2.rs` are `#[ignore]`d and covered directly by the live Lean runs above).
- Public-repo hygiene scan over `git diff e5e2ac5d..92602aaa` -> 0 violations.
- Live pinned Lean 4.35.0 runs: [`runs-holes/20261124-b4791ff3/acceptance.json`](../../../verification/proof-automation-cases/runs-holes/20261124-b4791ff3/acceptance.json) (`sha256:4139179f2b604eec3d1a0f7afde6e02351c7b55d74cf2a7bcebef2428afbab54`), independent Reviewer 1 full `holes-gate.mjs` re-run, Challenger 1 (`PAUTO-CASE-11`), Challenger 2 (`PAUTO-CASE-15`), and Forensic Auditor positive/negative controls (`NamedV2Consumer.lean`).
