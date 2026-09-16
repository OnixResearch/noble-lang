# M1 command and control matrix

## Interpretation

The [runbook](m1-runbook.md) names the exact commands and reserved interfaces.
The [acceptance delta](../.cairn/changes/m1-workspace/specs/verification-toolchain/spec.md) remains normative.
This matrix does not mark a scenario, proof, or M1 implementation task complete by itself.

- **Observed** means a retained local run covers the stated narrow scope.
- **Historical** means earlier evidence exists, but it is not a current integrated control.
- **Planned** means the required command or control has not executed as a Noble acceptance check.

Each listed mutation requires a separate fixture or matrix cell.
Every listed field or configuration dimension needs its own mutated cell.
The future `m1-controls --family FAMILY --all-required` command must enumerate those cells explicitly.
A positive baseline must pass before its related mutated inputs can establish rejection coverage.

The selected host is `x86_64-linux`. The target is `x86_64-unknown-linux-gnu`, with the lane configuration in the runbook.
Every required unit, feature configuration, generated body, and dependency belongs to the appropriate inventory comparison.
Tests retain their compiler-bound roles. Test-only observations cannot satisfy production obligations.

## Toolchain — VT-M1-01

Current commands: `nix build .#checks.x86_64-linux.tool-selection`, `.#checks.x86_64-linux.tool-versions`, and `.#checks.x86_64-linux.tool-overrides`.
The available extraction and Miri procedures supply executed compatibility observations separately.
The aggregate `m1-controls --family toolchain --all-required` interface is reserved for task 5.2.

| Control | Positive observation or separate rejected mutations | Current state |
|---|---|---|
| TC-BASE | Actual kernel extraction, generated Lean compilation, named Miri probe, and ten backend Git sources | Observed within task 1.3; first-run rehearsal recorded separately |
| TC-PINS | Missing reviewed/observed pin; floating revision; wrong source NAR hash; missing source set; changed recipe/output | Observed in pure selection controls, not authenticated compiler observations |
| TC-BACKEND | Wrong Charon requirement; wrong Lean requirement; missing/duplicate/changed Lake dependency; added path field; noncanonical package name | Observed in pure selection controls |
| TC-CONFIG | Stale bound file; wrong host/target/word size; changed panic/overflow/features/arguments/limits; missing Lean archive hash | Observed in pure selection controls |
| TC-OVERRIDE | Each of 16 named environment overrides; manual argument; backend directory symlink; real same-byte Charon path override | Observed command rejections; the real Nix override also preserved the lock |

The retained selection suite contains 96 pure cases, including two positive cases.
It also contains 18 shell rejection cases and one real Nix override rejection.
These are distinct control scopes, not a count of successful compiler executions.
Selection success cannot promote itself to M1 acceptance.

## Core and shell — VT-M1-02

Positive command: `nix build .#checks.x86_64-linux.rust-tests`.
Reserved control command: `m1-controls --family boundary --all-required`. Tasks 2.1 and 2.2 own this family.

| Control | Positive observation or separate rejected mutations | Current state |
|---|---|---|
| BND-BASE | Positive budget decrements once; zero exhausts; shell subprocess smoke and invalid arguments | Observed Rust tests; not the full boundary matrix |
| BND-EFFECT | Kernel filesystem/network/process access; environment access; clock/randomness; hidden mutable state | All eight cells matched: five host-effect cases, atomic state, thread-local state, and the real-registry random call |
| BND-UNSAFE | Owned unsafe body; unsafe declaration or foreign boundary without the required ownership record | Unsafe block, unsafe function, and a public foreign `extern` declaration all matched the compiler prohibition. The unsafe/native ownership record obligation maps to NAT-SITE in task 3.3 |
| BND-DIRECTION | Inward host type; reverse shell dependency; relocated kernel body labeled as an adapter exception | Host return type and the acyclic reverse edge matched. The relocated-body exception obligation needs exception records and maps to SRC-EXCEPTION in task 2.4 |
| BND-TEST | Production inclusion of test source; shared production fact hidden by test projection; test witness promoted to production evidence | Noble-specific cases match: a production import of test source rejects before collection, and a no-std witness that holds only outside the test configuration rejects with `no-std-obligation`. Provider controls cover the remaining projection shapes |

The smallest valid workspace is the baseline. A rejected fixture must preserve its original kernel ownership.
A compiler error unrelated to the intended boundary does not establish that boundary's rejection.

The [boundary check](boundary-controls.md) contains two positive baselines and sixteen negative fixtures.
All sixteen match, including the separate forbidden randomness-dependency control, the real-registry random call, the foreign unsafe declaration, the test-source import, and the no-std test witness.
The thread-local fixture remains unchanged and passes. Its historical collector crash is retained.
The BND family is complete for task 2.2. The unsafe/native ownership record and relocated-body exception obligations stay with NAT-SITE and SRC-EXCEPTION.
The full `m1-controls` interface remains reserved.

## Source inventory — VT-M1-03

Reserved commands: `source-inventory collect`, `source-inventory check`, and `m1-controls --family source --all-required`.
Tasks 2.3 and 2.4 own this family. The collect and check commands are implemented as the `source-inventory` app and the `source-coverage` check; the aggregate `m1-controls` interface remains reserved for task 5.2.
Counts are reported separately: `extracted`, `modeled`, `excepted`, `open`, and `proved`, plus generated, structural, test, macro, dependency, tool, and future-component counts.

| Control | Positive observation or separate rejected mutations | Required result |
|---|---|---|
| SRC-BASE | Complete compiler-derived bodies and dependency closure, matched to reviewed classifications | Observed: `source-coverage` classifies 6 bodies, 4 generated items, and 6 structural subjects across 16 reviewed paths and 17 compiler items, with separate extracted/proved/modeled/excepted/open counts |
| SRC-BODY | Omitted production function; macro body; generated body; closure or method; unclassified current dependency | Controls observed: subject omission, macro origin, dependency edge, tool, and test-subject mutations |
| SRC-CONFIG | Omitted target; omitted feature configuration; absent library/binary/test unit; unsupported required compiler lane | Controls observed: separate target, feature, default-feature, unit-coverage, package, and source-scope mutations |
| SRC-BINDING | Stale source/mapping/dependency/contract; duplicate or ambiguous identity; incomplete observation/reference closure; unknown or over-limit required fact | Controls observed for stale subjects, package binding, and unit/coverage identity; reference-closure and over-limit rejection remain provider-owned |
| SRC-EXCEPTION | Kernel exception; relocated kernel body; non-kernel exception missing symbol/configuration/owner/diagnostic/contract/assumptions/evidence/reassessment | Controls observed: any kernel exception rejects, an incomplete record rejects, and an excepted body without a record rejects. No exception is currently approved, so the relocated-body path remains open work |

Tests, non-Rust tools, upstream compilers, generated artifacts, and future components require explicit classifications.
An open future Wasm, WIT, Verus, or byte-view component does not fail a narrow smoke claim.
An omitted current component does fail accounting. Document discovery and source-text counts cannot prove this inventory complete.

## Extraction — VT-M1-04

Current commands: the Charon, Aeneas, and Lean procedures in the runbook, and the retained tool run that executes `extract-kernel` against the offline proof root and then re-verifies the record.
Reserved command: `m1-controls --family extraction --all-required`. Tasks 3.1 and 3.2 own the integrated gate; task 4.4 executed the gate phase.
The extraction and scoped-Miri phases are tool runs: pure flake evaluation cannot reference the host-materialised store inputs, so the thirteen `checks.*` entries stay pure.

| Control | Positive observation or separate rejected mutations | Current state |
|---|---|---|
| EXT-BASE | Extract the actual production library and compile an unedited generated module into a fresh object | Observed: `extract-kernel` ran against the offline proof root built from `nix/offline-inputs.nix` from a fresh compiler-derived inventory; extraction and `verify` both exited 0 and the record binds the actual subject (`.pi/m1-extraction/run8`, `run8-verify.log`). The acceptance phase retains the run as a tool run |
| EXT-MISSING | Missing requested function; required function omitted from generated output; unrelated upstream fixture | Observed: a function absent from the compiler-derived inventory is rejected; a function absent from `lean/translation.json` is rejected as `function-not-in-generated-output`; and an unrelated fixture repository is rejected as `source-tree-mismatch` |
| EXT-BODY | Unsupported required body; unexplained opaque model; incomplete translation metadata | Established for coroutine-producing bodies: an `async` fixture makes Charon exit 101 with `Coroutine types are not supported yet`, and the app writes no record. The translation binding rejects a subject that is not a local generated entry. `dyn` dispatch, `dyn Fn`, capturing closures, recursion, and iterator chains are accepted and retained as a supported baseline |
| EXT-OUTPUT | Edited generated function; substituted handwritten implementation; stale object accepted instead of fresh compilation | Observed: editing the generated Lean file, deleting the object, changing the selection policy, changing the inventory identity, and changing the kernel source are each rejected with a named diagnostic |
| EXT-INPUT | Changed source/dependency/macro/feature/target/contract/arguments/limits; failed or timed-out translation/compilation | Observed: kernel source, workspace lock, kernel manifest, selection policy, inventory identity, target, and tool revisions are all bound and reject on drift. A changed feature set or argument list changes the selection policy, so it rejects as `selection-mismatch`. A failed translation writes no record |

The source collector runs before extraction. The classification comparison runs after extraction.
When you compare generated bytes, preserve the module basename.
A failed cache or tool bootstrap is an operational failure, not a successful semantic rejection.

## Proof boundary — VT-M1-04 and VT-M1-07

Reserved commands: `m1-acceptance` with an independently reviewed proof-required expectation and `m1-controls --family proof --all-required`.
Tasks 3.2 and 5.1 own this family. **All integrated proof-boundary rows remain planned.**

| Control | Positive observation or separate rejected mutations | Required result |
|---|---|---|
| PRF-SMOKE | Passed extraction with no refinement proof under a smoke-only expectation | Keep refinement open without changing the smoke evidence role |
| PRF-MISSING | Proof-required expectation with no proof; unresolved required obligation | Reject the requested proof claim |
| PRF-HOLE | Proof hole; unchecked axiom/model that discharges a required obligation; timeout; unknown result | Reject unresolved proof-required acceptance |
| PRF-SUBJECT | Theorem about a separate reference implementation; stale theorem/source/configuration binding | Reject the substituted proof subject |
| PRF-PROMOTION | Lint, architecture, Miri, generated Lean compilation, or receipt replay relabeled as refinement | Reject evidence-role promotion |

No Noble refinement proof exists. A rejection of missing proof does not create one.
M2 and later runtime milestones retain their separate obligations.

## Octet and architecture — VT-M1-05

Current commands: `nix flake check`, the published pre-commit hook, the direct `noble-octet-gate`, and `cargo-octet artifact verify`.
Reserved command: `m1-controls --family octet --all-required`. Tasks 4.1–4.4 own complete local and CI enforcement.

| Control | Positive observation or separate rejected mutations | Current state |
|---|---|---|
| OCT-BASE | Full pinned deny-all catalog, fresh Nickel policy, all required targets/features, complete architecture collection, valid replay | Observed for task 4.1: `octet-controls` proves one immutable revision across `flake.nix`, `flake.lock`, `.pre-commit-config.yaml`, and the reviewed selection; the published hook and the direct gate both ran over `--workspace --all-targets --all-features` with 0 findings and a valid bundle replay. The Nix `octet` check runs the same gate in every `flake check` |
| OCT-LINT | Lint finding; disabled lint; warning/finding budget; broad suppression; stale source allowance | Observed: `octet-controls` rejects any lint downgrade, any missing `unsafe_code = "forbid"`, and any waiver in the reviewed policy; a kernel filesystem call is rejected by the published gate (exit 2, `impure_call_in_core`), and a crate-level `#![allow(unused, dead_code)]` does not hide it. Octet's internal waiver *matching* semantics remain provider-owned |
| OCT-POLICY | Empty policy; stale export; stale manifest; inventory/advisory mode replacing gate mode | Observed: a stale JSON export fails the `policy` check comparison; an emptied policy is rejected by the published gate with named contract diagnostics (`invalid-effect-executor-scope`, `missing-capability-classification`); a required feature the packages do not define is rejected; dropping a required capability family blocks the gate (exit 1); and an advisory-mode policy is rejected by `octet-controls` as `architecture-mode-not-gate`. Advisory-mode execution was additionally rejected for a stale manifest, so "advisory exits 0" was not observed |
| OCT-FACTS | Missing compiler fact/unit; unsupported required configuration; forged test exemption; missing per-unit no-std evidence | Observed for task 4.2: the compiler coverage artifact reports `complete`, the roster covers `lib`, `lib+test`, `bin`, `bin+test`, and two `test+test` units, the Nickel export and freshness manifest are re-generated and compared by the `policy` check, and `octet-controls` asserts the declared roles, packages, sources, targets, features, capability families, outbound authority, and core no-std scope. Forged test exemption and per-unit no-std negatives remain provider-owned |
| OCT-REPLAY | Changed artifact or receipt; resealed inconsistent disposition; stale source/policy/configuration identity | Valid positive replay observed; Noble tampering controls remain planned |

Console output remains explicitly unprotected. No synthetic authorization witness can hide a missing protected-effect obligation.
Native binary/library test configurations retain production roles. Only independently declared, compiler-bound integration tests can receive the test role.

## Native assurance — VT-M1-06

Current command: `nix run .#native-assurance -- inventory|check`.
Reserved commands: `native-assurance inventory`, `native-assurance check`, and `m1-controls --family native --all-required`.
Tasks 3.3 and 3.4 own the capability; the `native-assurance` check is one of the thirteen scoped checks, task 4.4 executed the scoped-Miri tool run, and the aggregate `m1-controls` interface stays reserved for task 5.2.

| Control | Positive observation or separate rejected mutations | Current state |
|---|---|---|
| NAT-BASE | Accounted native/dependency scope, safety arguments, supported Miri configuration | Observed: `native-assurance inventory` and `check` pass over 6 units, 46 production items, 0 unsafe sites, 0 foreign items, 0 production dependencies, with safety arguments and target assumptions. The pinned Miri baseline passed 2 tests |
| NAT-SITE | Unrecorded owned unsafe site; absent safety argument; invalid native boundary; empty inventory without complete source accounting | Observed in synthetic controls: an owned-unsafe site without a record, a reviewed site the compiler does not observe, an absent safety argument, and an empty scope without the forbid/function-safety/foreign-item accounting basis all reject |
| NAT-DEPENDENCY | Missing dependency version/features/macros/target assumptions/verification boundary; generated-code omission | Partly observed: unclassified production dependency, missing fixture-dependency record, unrecorded fixture lock, unclassified compiler tool, unclassified foreign library, and missing target assumptions reject. Per-dependency feature/macro records remain open |
| NAT-MIRI | Unsupported required target/model; skipped test; invalid memory/provenance fixture; interpreter error/timeout mislabeled as pass | Observed: the pinned strict-provenance baseline passes and a provenance-violating fixture is rejected with the exact Miri diagnostic. `miri-unsupported-configuration` and `miri-configuration-mismatch` cover unsupported required configurations; a skipped-test control remains open. Task 4.4 executed the phase with the offline sysroot input from `nix/offline-inputs.nix` (`.pi/m1-native/toolrun/`) |
| NAT-BYTEVIEW | Initial core gains a byte-view dependency without VT-NATIVE-03 evidence | Observed: any production dependency edge outside the selected packages rejects as `production-dependency-mismatch` |

Invalid memory fixtures belong only in isolated test inputs. They cannot enter production crates or discharge production obligations.
The full native inventory must distinguish owned unsafe code, dependency internals, compiler tools, and foreign/native libraries.

## Independent acceptance — VT-M1-07

Reserved commands: `m1-acceptance` and `m1-controls --family evidence --all-required`.
Task 5.1 added the reviewed expectation `policy/m1-acceptance.ncl`, the pure comparator, and sixteen negative controls; task 5.2 ran the required matrix and retained the commands.

| Control | Positive observation or separate rejected mutations | Required result |
|---|---|---|
| EVD-BASE | Independent expected subject, source, claim, configuration, and complete required phases | Observed: `policy/m1-acceptance.ncl` binds subject, packages, target, pins, and fifteen required phases with per-phase evidence kinds and bindings. The full evaluation on tree `4420049e` accepts: `valid: true`, fifteen of fifteen phases passed, zero diagnostics (retained under `.pi/m1-acceptance/`) |
| EVD-BINDING | Changed subject/source/dependency/policy/tool/target/feature/command/input/assumption/result | Observed in synthetic controls: stale subject, package scope, configuration, tool pin, and per-phase binding each reject with a named diagnostic |
| EVD-PHASE | Missing phase; unexecuted/failed/unsupported/timed-out required phase; incomplete control matrix | Observed in synthetic controls: missing, failed, unsupported, timed-out, and not-run phases each reject; an unexpected extra phase also rejects |
| EVD-AUTHORITY | Producer chooses its own expected bindings; local digest presented as authentication; test evidence promoted to production | Observed: a producer-supplied expectation rejects as `producer-supplied-expectation`, and each forbidden evidence-role promotion rejects as `evidence-role-promotion` |
| EVD-LIFECYCLE | Planning/document gates or a partial smoke result used to sync/archive M1 | Observed: the expectation requires all fifteen phases, so a partial smoke result cannot accept; `acceptance-incomplete` blocks it |

The expectation must not be reconstructed from the receipt under evaluation.
Source archives, hashes, and replay remain useful evidence without becoming independently authenticated acceptance.
The final CI gate must run every required positive and negative matrix cell without silently omitting unavailable scopes.
