## Context

This is a design-only change for issue #10. Nothing below is implemented, executed or proved by this change. It extends and coordinates the active `safety-artifact-correspondence` (S-CASE-07) and `intrinsic-noble-proofs` changes. It does not reopen their accepted fragments or duplicate the S-CASE-07 gate.

**Baseline re-verified at `44329dc`.** The issue cites `0ea10d3`; `git diff 0ea10d3 44329dc` is empty for the cited specs, active-change files, `crates/noble-contracts/src/companion/`, `crates/noble-cli/src/build*`, `crates/noble-cli/src/core/artifact.rs` and `specs/verification/obligations.json`.

Drift exists elsewhere:
- `44329dc` adds the experimental calculator, including a calculator Wasm profile and `crates/noble-wasm/runtime/calculator*.wat`. It also changes `crates/noble-cli/src/core/runtime/engine.mjs`, `crates/noble-cli/src/core/worker.rs`, `crates/noble-wasm/src/source/emit/imports.rs` and `README.md`. Every line number cited below for those files is at `44329dc`.
- `crates/noble-wasm/runtime/` now holds 21 `.wat` helpers, while `crates/noble-cli/src/core/runtime/config.json:68-80` still lists 11 as `runtime_assets`.
- README live-slot lines move by +46.

The README has no MC1/MC2 section. Its verification section (`README.md:548`) and Mathlib comparison (`README.md:77-122`) are the relevant text.

### Two routes that are never joined

| Route | Code | Admits | Binds | Lacks |
|---|---|---|---|---|
| **A. `noble admit-artifact WASM --effects CLAIMS_JSON [--source] [--allow-effects] [--opt off\|on]`** | `crates/noble-cli/src/core/artifact.rs:95-181`; worker `admit` in `crates/noble-cli/src/core/runtime/engine.mjs:797-899` | Candidate Wasm whose **final bytes** equal the host's pinned rebuild of host-selected source (`engine.mjs:879-880`), before instantiation | Report: `wasm_sha256`, `source_sha256`, `wat_sha256`, `optimized`, imports, `correspondence:'byte-exact'` (`engine.mjs:887-897`). The S-CASE-07 receipt separately records CLI, Node, wasm-tools and wasm-opt hashes | Any proof, claim, owner law or policy input. Candidate `source`/`digest`/`trusted_correspondence`/`allowed` are parsed and discarded. Tool hashes are computed but **not compared to pins**. Single-expression source only, with no dependency closure (`artifact.rs:140-149`) |
| **B. `noble build SOURCE --contract C [--proof P] [--require-proof] [--out DIR]`** | `crates/noble-cli/src/build.rs:120-216`; `bind_artifact` in `crates/noble-contracts/src/companion/release.rs:49-97` | Release when independently rechecked Lean evidence proves the regenerated statement and `bind_artifact` matches SOURCE's canonical op events and interface | Report `noble-mc2-build/v1`: core subject identity and 64-bit statement digests, evidence class, Lean path/version string. `artifact_correspondence` records **sizes only** (`build/report.rs:41-78`) | Wasm/WAT hashes, tool/helper identities, optimization mode (assembly is always `optimized:false`), and a hash check when re-reading `module-1.wasm` (`crates/noble-cli/src/core/worker/assembly.rs:36`). The guard wrapper is a separate `wrapper.noble` text, not an admitted artifact |

As a result, no path composes "accepted evidence → exact subject" with "exact final bytes → instantiation". Obligations PO-17 (lowering) and PO-18 (loading) are `open` with **no** evidence. PO-16, PO-19, PO-20 and PO-21 are `open` with bounded evidence (`specs/verification/obligations.json`).

## Current-profile coverage and trust matrix

Legend:
- **Check**: implemented and exercised by a named gate.
- **Bind**: identity recorded in a retained receipt.
- **Trust**: assumed component.
- **Open**: theorem obligation.

| # | Chain edge | Existing checks (gate/receipt) | Exact identity bound | Trusted | Unsupported here | Open obligations |
|---|---|---|---|---|---|---|
| 1 | Owner-selected law → contract | VC-OWNER-01 is spec text only. Build takes an invoker `--contract`; policy revision is constant 1 (`build/report.rs:9`). CONTRACT-16/20/23 absent/not-run | none (no owner-law revision in any report) | invoker selects the contract file | owner-frozen law records | VC-OWNER-01 release gate |
| 2 | Source/recipe + dependencies | Route A: single expression. Route B: single quotation root, no definitions (`bind_artifact`). named-v2: authenticated `Definitions@5`/`Subject@3` graph (CONTRACT-26 receipt) | Route A `source_sha256`; Route B canonical op events (64-bit fold index + retained events) | Rust frontend/kernel acceptance | dependency closure on Core routes; declared modules | PO-19 beyond MC1 matrix |
| 3 | Logical claim | Consumer regenerates the statement from the retained preparation (`export_lean`) | full statement retained in-core. Reports: FNV-1a 64-bit `claim_statement_digest` (`build.rs` fold) | MC1 elaborator | non-MC1 claims | PO-19 |
| 4 | Checked proof/rules | Sandboxed isolated Lean consumer; strict axioms; MC2 finite rule replay (MC2 gates) | Lean path + version/commit string (`workflow/verification.rs`). No Lean binary or library tree hash in build reports. Intrinsic receipts hash frozen Lean | Lean kernel; `CheckObservation` authenticity (§8.3) | tactics, producer theorems | PO-15, PO-16, PO-20 |
| 5 | Checked lowering | `noble-wasm` re-runs kernel acceptance before emission (`crates/noble-wasm/src/source/admission.rs:33-43`) | none (no lowering certificate) | `noble-wasm` compiler | general Core lowering proof; Route A and B | **PO-17**, SO-07 (no evidence) |
| 6 | Emitted WAT/Wasm + optimization | Route A final-byte equality; M3 determinism check (`tools/m3-wasm.mjs:308-335`); CONTRACT-22/26 off/on byte equality across proof variants | Route A `wat_sha256`/`wasm_sha256` per report; S-CASE-07/16 receipts hash wasm-tools/wasm-opt. Intrinsic receipts lack a wasm-opt identity | wasm-tools; binaryen wasm-opt 125 `-O2` (`config.json:17-28`) | opt-on in build/live/slot/calculator | PO-17; optimizer preservation |
| 7 | Loaded bytes | Route A instantiates the same in-memory compared buffer. Slot path: SHA-256 cross-check before `new Module` (`slot-engine.mjs:265-268`) | Route A buffer = compared bytes. Route B: none (temp re-read without hash) | Node/V8 (pinned path/flags, `worker.rs:504-525`) | build-path bytes | **PO-18** (no evidence) |
| 8 | Runtime helpers / host adapters | Helpers are `include_str!` into the WAT (`crates/noble-wasm/src/source/emit/imports.rs:122-270`), so they are covered by byte equality only against the same CLI build | helper file hashes only in gate receipts at their own revisions; `runtime_assets` list stale (11 of 21) | helper semantics; JS host adapters | changed helper sets | PO-14, PO-17 |
| 9 | Actual invocation | Fresh engine; start-section rejection; import allowlist; instantiation does not run the body (`engine.mjs:810-811,904-928`). MC2 companion re-observation + `bind` (`companions/session/binding.rs:132-160`). Guard wrapper is tested only | MC2 subject identity; S-CASE-07 zero-counter refusals | worker non-malice; OS | host authorization is separate (S-HOST-03) | PO-21 (wrapper Wasm), PO-18 |

**Artifact identity is not semantic preservation.** Rows 6–7 can authenticate *which bytes* run relative to a trusted compiler. Only rows 5, 8 and 9 obligations (PO-17, PO-18, PO-14, PO-21) could establish *what those bytes mean*, and none is discharged.

## Decisions

### Decision: Select one extension: a proof-bound artifact admission envelope for the MC2 pure quotation profile

**Choice:** Join Route A and Route B for exactly the profile MC2 already accepts:
- a resource-free Core-Bootstrap program;
- one closed inert quotation root with no definitions or imports;
- an MC1 partial-correctness contract;
- `LeanExact` evidence;
- optionally one #8 v1 guard template wrapper.

The new admission envelope `noble-proof-artifact-admission/v1` (PAA-1) is checked by an independent consumer that:
1. selects the owner law/contract, source and optimization mode itself;
2. rechecks the proof;
3. binds the subject;
4. rebuilds and compares the **final subject and wrapper Wasm bytes**;
5. instantiates exactly the compared in-memory buffers in a fresh engine.

Before protected execution, any mismatch refuses.

**Rationale:** The #7 generalized named-call profile has no accepted design yet, and it would need dependency-closure identity and per-callee bindings on top of a joined envelope. PAA-1 is the shared prerequisite that every later profile (#7 named calls, #8 `NonEmptyList`, #9 protocol subjects) must extend rather than re-invent. It reuses implemented pieces (Route A comparison, Route B evidence/`bind_artifact`, MC2 wrappers) and fixes the cross-cutting gap without a new backend or proof.

### Decision: Strengthened trusted build, not translation validation, for PAA-1

**Choice:** PAA-1 authenticates correspondence by a **strengthened trusted-build receipt (TB+)**: the consumer rebuilds with tools whose hashes are compared against policy pins, and requires exact equality of the final bytes. A bounded translation validator is **not** selected for this slice. No PAA-1 report may use the words "translation validation" or "semantic preservation".

| Criterion | TB+ (selected) | Bounded translation validation (rejected for PAA-1) |
|---|---|---|
| What it authenticates | the executed bytes are the pinned compiler's output for the consumer-selected source, options, helpers and tools | that emitted code relates to the source recipe under a reviewed Wasm and helper semantics |
| Prerequisites available | Route A rebuild/compare, Route B evidence, policy pins (`policy/tool-selection.json`) | no reviewed Wasm/helper semantics (PO-17 has no evidence). The only structural checker (`live_slot_origin.rs`, closed opcode set) treats helpers as trusted |
| Residual trust | `noble-wasm`, helpers, wasm-tools, wasm-opt, V8 (explicit) | the same helpers and engine plus the validator itself, while appearing stronger than it is |
| Failure mode it catches | substituted, stale, mutated or appended bytes; wrong source/options/helpers/tools | miscompilation, *only if* the semantics were reviewed |
| Cost | low, deterministic, reproducible | needs a decoder, a helper model and per-construct relations |

**Rationale:** A structural opcode matcher without helper semantics would relabel trust as validation. TB+ states its trust boundary honestly (VC-ADMIT-03 allows an "explicitly trusted build … with its real trust boundary reported"). Translation validation remains the designated route for PO-17 in a later profile. The lowering theorem obligation O1 stays open either way.

### Decision: Consumer authenticates; producer assertions are discarded

**Choice:** Every envelope field is either recomputed by the consumer or refused. Producer-supplied digests, source claims, tool identity claims, `trusted_*` flags and optimization claims are parsed (where the format allows) and discarded, exactly as Route A does today.

## Exact admission envelope (PAA-1)

| Field | Content | Authenticated by consumer as | Producer may assert? |
|---|---|---|---|
| `profile` | `MC2-pure-quotation@1`: resource-free Core-Bootstrap, one closed quotation root, empty dependency closure, no resources, components, live slots or calculator | refusal of any `def`/import/resource/module form (`unsupported-profile`) | no |
| `owner_law` | owner-frozen law id, revision, SHA-256 of the exact expected statement (VC-OWNER-01) | equality with the consumer-held frozen record before any proof check | no |
| `source` | SHA-256 of consumer-selected SOURCE bytes; canonical op-event list and its SHA-256; interface | recomputed from the bytes the consumer read; `bind_artifact` events equality | no (candidate `source`/`digest` discarded) |
| `semantic` | semantic revision, MC1 IR revision, candidate format, kernel acceptance version | constants of the pinned CLI | no |
| `claim` | SHA-256 of the full regenerated statement bytes (not the 64-bit fold), claim kind, assumptions, `requires`/`ensures` roots | regenerated by `export_lean` from the consumer's preparation; equal to the owner-law statement | no |
| `proof` | SHA-256 of evidence bytes; class `LeanExact`; Lean binary SHA-256 + version/commit; contract-library source-tree SHA-256; transitive axioms; policy revision | isolated recheck; Lean binary and library tree hashes compared to policy pins (new) | evidence bytes only |
| `guard` | none, or (template-set version, template, literal, wrapper source SHA-256, wrapper Wasm SHA-256) | recomputed from the registry entry's recognized template (#8 GUARD-BIND-01); wrapper rebuilt and byte-compared | no |
| `toolchain` | SHA-256 of the noble CLI binary; SHA-256 of each `crates/noble-wasm/runtime/*.wat` actually embedded (full directory listing, not the stale list); SHA-256 + version of node, wasm-tools and wasm-opt; V8 flags | each hash compared to an envelope-pinned value from `policy/tool-selection.json` (new: mismatch → `tool-pin-reject`) | no |
| `build` | `opt ∈ {off, on}` chosen by the consumer; exact optimizer flags | consumer selection; flags from the pinned config | no (candidate opt claim discarded) |
| `artifact` | subject WAT SHA-256; subject final Wasm SHA-256 + length; wrapper final Wasm SHA-256 + length | consumer rebuild, then `bytes.equals` per artifact | candidate bytes only |
| `load` | SHA-256 of the exact in-memory buffers passed to `new WebAssembly.Module`; fresh engine id | computed over the compared buffer, which is never re-read from disk | no |
| `authorization` | **not part of the envelope identity**: host effect policy and current grants | checked separately at invocation (S-HOST-03) | no |

**Order** (all steps before any `new Instance`):

1. envelope/profile validation;
2. owner-law statement equality;
3. proof recheck;
4. subject binding;
5. tool/helper pin checks;
6. rebuild;
7. subject and wrapper byte equality;
8. start-section/import reflection;
9. host authorization;
10. instantiate the compared buffers.

**Refusal codes** are distinct: `invalid-envelope`, `unsupported-profile`, `law-mismatch`, `proof-inconclusive`, `subject-mismatch`, `tool-pin-reject`, `correspondence-reject`, `guard-mismatch`, `stale-context`, `authorization-denied`. Every refusal reports zero guest requests and zero protected operations.

**Unchanged behavior:**
- Ordinary `noble run`/`session` and proof-free `build` remain available and unchanged (VC-SCOPE-01). PAA-1 is opt-in and labels anything else as ordinary execution.
- Route A without proof inputs keeps its S-CASE-07 meaning.

**Cache invalidation.**
- Accepted envelopes may be cached only under a key equal to the complete envelope (all fields above except `authorization`). Changing any field, including policy revision, helper hash, tool hash, opt mode or law revision, forces full revalidation (VC-ADMIT-04, VC-VALUE-03).
- Executable code may not be reused across admissions. Each admission uses a fresh worker process, so the V8 in-process cache (`config.json:35`, "repeated-identical-bytes-may-use-v8-process-cache") cannot carry code between admissions.
- Lean rechecks use no producer caches (as `workflow/rules.rs` already requires).
- The Mathlib `.olean` cache is outside PAA-1.

**Optimization mode.**
- `opt` is a consumer selection bound into the envelope. Candidates built under the other mode refuse (`correspondence-reject`, as S-CASE-07's option-mismatch control already shows).
- Off and on produce distinct receipts.
- wasm-opt correctness is a residual trust. Its identity is pinned and hashed.

**Erasure.**
- Proof declarations and evidence never enter executable bytes. PAA-1 requires that the subject Wasm built from the proof-bearing source equal the Wasm built from its proof-erased source under the same envelope; this is an admission precondition, extending CONTRACT-22/26 to the admission path.
- The guard wrapper is **not** erasable (VC-ADMIT-05). When the contract precondition is not `true`, the envelope must carry a guard, and only the wrapper may be invoked as a certified call. Projecting the subject alone is an ordinary, uncertified run.

## Positive and negative control plan

Every control must:
- run against a frozen `0500` copy of the CLI (as the MC2 gates do), with a source tree hash and a policy-pinned tool hash set;
- retain raw stdout/stderr and every input, candidate and rebuilt byte string;
- emit an immutable receipt that records its own runner hash.

**Negative candidates are produced by an independent preparation script.** That script derives mutations with `wasm-tools print/parse` or byte edits, never through the admission code path, and it is hashed separately. Positive artifacts come from a separate fresh build. The design-local case IDs below are `absent / not-run / open / unassessed`.

| Case | Prepared input | Expected |
|---|---|---|
| ACORR-CASE-01 positive | `[ 1 + ]`, increment law, `LeanExact` proof, opt off and on, no guard | admitted; `I64(x+1)` executed from the compared buffer; full envelope receipt per mode |
| ACORR-CASE-02 positive guarded | `[ 1 + ]`, conditional-monotonicity law, `LtI64Max` wrapper | admitted; `MAX-1 ↦ inr MAX`, `MAX ↦ inl 1`, body not started |
| ACORR-CASE-03 correct proof, wrong body | valid increment proof; candidate and SOURCE `[ 2 + ]` (same interface) | `subject-mismatch` (claim regenerated for `[ 2 + ]` ≠ law), zero execution; also candidate `[ 2 + ]` bytes with SOURCE `[ 1 + ]` → `correspondence-reject` |
| ACORR-CASE-04 stale dependency/version | SOURCE containing a `def`/import (out of profile); envelope pins an older helper-set hash or CLI hash | `unsupported-profile`; `tool-pin-reject`, before rebuild |
| ACORR-CASE-05 weakened claim | proof of `ensures (eq (out y) (out y))`, or `requires false`, against the frozen law | `law-mismatch` even though Lean accepts the weaker theorem |
| ACORR-CASE-06 code/recipe divergence | existing S-CASE-07 targeted push mutation, plus a recipe-atom-only mutation | `correspondence-reject` |
| ACORR-CASE-07 changed guard | wrapper for `EqI64Literal 6` with contract `k=5`; `NeI64Min` wrapper substituted for `LtI64Max`; wrapper omitted for a non-`true` precondition | `guard-mismatch` |
| ACORR-CASE-08 altered helper/import | candidate built by an independently rebuilt CLI whose `operations.wat` add helper is altered; candidate with an extra or renamed import | `correspondence-reject` / import-reflection refusal; consumer with an altered helper tree → `tool-pin-reject` |
| ACORR-CASE-09 post-check replacement | replace the candidate file on disk and the build temp `module-1.wasm` after comparison | executed bytes are still the compared buffer (`load` hash equals `artifact` hash). Any path that re-reads from disk must refuse on hash mismatch (current build re-read is a known gap) |
| ACORR-CASE-10 producer forgery | candidate metadata with matching digest, `trusted_correspondence:true`, a claimed tool hash and `opt:on` | identical outcome with and without the metadata |
| ACORR-CASE-11 erasure | proof-bearing vs proof-erased source, off/on | identical subject bytes required; a proof-only edit changes no executable byte |
| ACORR-CASE-12 cache | admit; change only the policy revision, or only the opt mode; re-admit in the same host | full revalidation, no cached acceptance; fresh worker per admission |

**Retained existing controls:**
- S-CASE-07: wrong-program artifact, inert append, targeted mutation, no-source, opt mismatch.
- CONTRACT-13: unrelated Wasm via `build --require-proof`.
- CONTRACT-22/26: off/on erasure byte equality.

PAA-1 neither replays nor relabels them. New coverage is exactly ACORR-CASE-01…12.

## Proof obligations (separate from hashes, tests and extraction)

| Id | Statement | Relation | Discharged by hashes/tests? |
|---|---|---|---|
| O1 lowering | For every accepted `MC2-pure-quotation@1` subject, Wasm execution of `noble-wasm` output (with reviewed helper semantics) refines Noble `Exec`, including recipe/reflection observations | PO-17 slice | **No** |
| O2 optimizer | `wasm-opt -O2` with the pinned flags preserves O1's relation | residual trust, or a per-artifact validator later | No |
| O3 loading | instantiation of the compared buffer under the import ABI binds the exported Program to the checked code and recipe | PO-18 slice | No |
| O4 guard | #8 C1–C6 for the selected templates, including C6 (wrapper Wasm) | PO-21 | No |
| O5 erasure | `compile(src) = compile(erase(src))` by construction of the executable IR; ordinary observations are unchanged | PO-20 / VC-INTRINSIC-04 (finite byte equality exists; theorem open) | No |
| O6 envelope soundness | the consumer's Rust decision procedure (`bind_artifact`, admission, pin checks) refines its specification: acceptance ⇒ statement, subject events and bytes equal the consumer's own | PO-16 (Aeneas route) | No; extraction success alone is not refinement |
| O7 identity-to-semantics | O6 ∧ O1 ∧ O2 ∧ O3 ⇒ the executed artifact satisfies the proved claim | composition theorem | No |

Matching hashes establish only O6's *premises* for one execution. Passing ACORR cases are finite observations, and extraction success is not refinement (VC-GATE-03).

**Bounded O1/O3/O7 follow-on (#52):** the [one-call linking pilot](../../../verification/linking/README.md)
separates source/helper-model composition from real lowering and final-byte
loading. Its actual named callee is embedded in one compiled module, outside
PAA-1's definitions/imports exclusion. Exact subject/dependency/artifact receipts
and finite execution controls do not discharge O1, O3 or O7. PAA-1 and the
completed #10 design are unchanged; no PO-17/18 status is promoted.

## Residual assumptions and unsupported cases

**Assumed** (reported in every PAA-1 receipt):
- the `noble-wasm` compiler and embedded helpers;
- wasm-tools; wasm-opt;
- Node/V8 and the OS loader;
- integrity of the pinned CLI binary (hash-pinned, not proved);
- Lean kernel and isolated-checker soundness;
- `CheckObservation` authenticity;
- a non-malicious worker process (same trust family as the consumer);
- the policy pin file itself;
- absence of cross-host provenance (no signatures or remote attestation).

**Unsupported:**
- definitions, imports and declared modules (→ #7 profile with dependency-closure identity);
- resources and components (→ #9, #3);
- live slots and reload (→ #1);
- calculator profile;
- async;
- alternate backends (→ #4, which needs fresh correspondence evidence and inherits no Wasm claim);
- allocation-strategy changes (→ #5).

**Not claimed:** a verified compiler, Wasm engine, host or malicious-worker resistance, or "hashes prove semantics".

## Cross-issue coordination

| Issue | Shared gap | Owned here | Owned there |
|---|---|---|---|
| #6 proof automation/diagnostics | Refusal codes must be actionable | PAA-1 refusal vocabulary | Diagnostics UI; producers stay untrusted |
| #7 named calls | Dependency closure and per-callee identity | Envelope extension point `source.dependency_closure` (empty in PAA-1) | Named-call profile and its bindings |
| #8 applicability guards | Wrapper bytes and C6 | `guard` field, ACORR-CASE-02/07 | GUARD-* binding, templates |
| #9 effect protocol | Compiled component correspondence (T6) | Out of PAA-1 (components unsupported) | `RegionProtocol-v1` |
| #3 ownership | Resource-bearing subjects | Excluded | Ownership/lifetime preservation |
| #1 live references | Live publication | Excluded; live-slot SHA cross-check noted in the matrix | Live-version publication |
| #15 correctness foundation | Phase 3 semantics → lowering → helpers → Wasm | O1–O3, O7 stated as obligations; matrix rows 5–8 | Mathematical foundation and comparator |
| #4 / #5 | Backends and allocation | Excluded | Their own evidence |

## Risks / Trade-offs

- TB+ ties admission to an exact pinned toolchain and CLI, so legitimate toolchain upgrades require new pins and new receipts.
- Pinning the full helper directory exposes the stale 11-entry `runtime_assets` list. That list must be corrected before PAA-1 can be implemented. Correcting it is a separate change (it affects M3 harness checks).
- Hashing frozen Lean and the library tree in build reports changes the `noble-mc2-build/v1` schema. A new schema version is needed, and existing receipts stay unchanged.
- The selected profile is narrow (single quotation). Its value is the joined envelope, not coverage.

## Acceptance mapping

| Issue #10 acceptance checkbox | Section |
|---|---|
| Current-profile coverage/trust matrix linked to existing gates, receipts and open proof obligations | "Two routes that are never joined"; "Current-profile coverage and trust matrix" |
| One selected extension with an exact admission envelope and a justified trusted-build versus translation-validation decision | "Decision: Select one extension…"; "Decision: Strengthened trusted build, not translation validation"; "Exact admission envelope (PAA-1)" |
| Reproducible positive and independently constructed negative-control plan for the exact artifact that is instantiated, with immutable source/tool/proof receipts | "Positive and negative control plan" (ACORR-CASE-01…12, retained controls) |
| Proof obligations for semantic preservation and guard/erasure correspondence kept separate from hashes, finite tests and extraction success | "Proof obligations (separate from hashes, tests and extraction)" |
| Explicit residual assumptions and unsupported cases, without promoting universal compiler, Wasm engine, host or malicious-worker correctness | "Residual assumptions and unsupported cases" |
