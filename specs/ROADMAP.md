# Implementation roadmap

Revision: 0.1.0-draft.5

**M5, M6, M7, M8 and DXM1 are complete only for their separately bounded
component, local-service, finite-choreography and opt-in declared-module
profiles; broader language, platform and refinement obligations remain open.**
M1, M2, MC1 and the bounded M3 representation experiment are implemented. M4
connects accepted source to compiled managed-linear-memory Wasm and persistent
sessions, with retained CORE/DX execution, extraction, regression and quality
evidence. MC2 adds first-class certified companions, exact admission and
applicability checks, composition/family operations and proof-required builds.
Its [completion record](../verification/mc2/evidence.json) retains all 15
canonical cases and declared variants, separate scoped proof lanes, current
extraction and complete quality gates. Neither bounded execution nor
successful extraction establishes a general or verified backend.

M5 delivers the pinned `Component-Sync-Bootstrap` world, independent typed
component interoperability, move-only resource ownership and native-pin
retirement, and protected host authorization with observation-backed receipts.
Its [completion record](../verification/m5/evidence.json) binds all 24 selected
cases and 84 variants, 44 native tests, seven strict actual-Rust resource roots,
52 refusal controls, prior-milestone regressions and complete quality collection.
The separately retained final document/Cairn Nix receipt is required for closeout.
This M5 evidence is not full Component-Draft, WASI, native async or universal component,
authority-system or physical-release refinement.

M6 completes the selected `Component-Async-Bootstrap` profile under its
[completion record](../verification/m6/evidence.json). The
[native acceptance receipt](../verification/m6/acceptance.json) passes WI-11,
WI-12, WI-16 and local WORKER-08: 38 variants, 62 controls and 13 native
kernel tests. The independent source-bound extraction/check, full prior
regressions, unchanged deny-all/architecture gate and all thirteen
[Nix checks](../verification/m6/nix-checks.json) pass; native execution,
qualified local correspondence and milestone acceptance remain separate.

The separate active
[WI-03 exact-u64 change](../.cairn/changes/wit-exact-u64-adapter/proposal.md)
adds a finite caller-selected checked `echo: u64 -> u64` adapter without
weakening the default binding-stage refusal of implicit `u64` to Noble `I64`.
Its [original-source receipt](../verification/wi03-final/acceptance.json)
records a compiled Noble component and independent typed Wasmtime calls:
representable `0`, `42` and `i64::MAX` return exactly, both high-bit values
trap without results, and a negative guest `I64` cannot publish `u64`.
The internal `$enter` prologue runs before the ingress guard; the guard is
source-ordered before signed source-body use, not measured guest-body
nonentry. Checked mode is public caller opt-in, not authenticated host
authority. WI-03 proof, full Octet/Clippy and signed+sandbox Nix assurance
remain open; the change stays active and is not full Component-Draft or WASI.

[roadmap.json](roadmap.json) records dependency edges. Milestone status is separate from test and proof results.

## Foundation

| Milestone | Depends on | Exit criterion |
|---|---|---|
| M0 — Canonical draft | None | Current family, review dispositions, scenario schema, and document checks |
| M1 — Workspace | M0 | Aeneas-first crate boundaries, source inventory, pinned tools, extraction CI entry point, and quality gates |
| M2 — Checker/proof feasibility | M1 | Candidate schema, finite rules, actual extraction, one nontrivial refinement, and failing incomplete-coverage checks |
| M3 — Wasm feasibility | M1 | SPEC-BE001 representation comparison, a working candidate, nonconstant builders, explicit limits, and retained recipes |
| M4 — End-to-end core | M2, M3 | Bootstrap cases execute on Wasm with negative checks and scoped evidence |

M2 and M3 are parallel workstreams in the dependency graph. They do not require multiple agents. Neither waits for concurrency or choreography.

## First-class contract workstream

| Milestone | Depends on | Exit criterion |
|---|---|---|
| MC1 — Contract frontend and rules | M2 | Versioned grammar/IR and companion design, exact Lean obligation export, increment proof, composition/family rules, and CLI result contract |
| MC2 — First-class certified programs | MC1, M4 | Wasm companion inputs/results/aggregates, runtime capture instantiation, applicability guards, proof-required builds, and hostile-evidence rejection |

[PROGRAM-CONTRACTS.md](PROGRAM-CONTRACTS.md) defines VC-GATE-01 through VC-GATE-03. MC1 and MC2 are required project deliverables, but applications can omit behavioral proofs. Ordinary M4 delivery does not depend on MC1. Pure contract work does not wait for M5 resources, native async, or concurrency.

MC1 must discharge the named fragment of PO-15/19 for statement export and proof rules, including actual Rust correspondence. A frontend that proves a different handwritten program cannot pass. MC2 must discharge the applicable PO-16/20/21 obligations for admission, companion operations, and applicability checks. It must retain its exact backend/build assumptions. Executed Wasm examples do not establish backend verification.

MC2 acceptance executes all fifteen selected MC2 [contract scenarios](conformance/contract-cases.json) and their declared variants against the actual implementation, including eighteen independent-checker/core controls. Later optional intrinsic cases have separate states and do not inherit MC2 acceptance. Changed captures, forged certification, missing composition implications, false preconditions, incomplete proofs and unrelated Wasm are rejected on their declared paths. The retained receipts distinguish strict semantic/source lemmas, renderer-layout obligations, closed native source equations and bounded runtime observations. Broad PO-16/20/21 and general backend/host correctness remain open.

### Optional intrinsic Noble-source proof workstream (finite source-bound acceptance)

The separate active [Intrinsic Noble proofs Cairn change](../.cairn/changes/intrinsic-noble-proofs/proposal.md) selects proof declarations **inside opt-in `.noble` versioned modules** after MC1/MC2 and finite Declared-Modules-v1 support. `contract 1`/`proof 1` declarations are not executable words. The predicative proof universe quantifies over independently checked pure `PureTyCode` codes and eligible total values; dependent `Pi` and `Eq` are logical only, not runtime-dependent `Program` types. Selected real `.noble` source witnesses establish a polymorphic identity proof `∀ A:Type0, x:El(A), Eq A x x` and a compositional proof of the **actual** `[ 1 + ]` definition against its independently derived typed MC1 partial-correctness claim, including checked `pc_sequence` bridge/join and `PC`→exported-claim predicate/typing correspondence. The compiled erasure comparison is finite to the selected off/on modules: independently compiled Wasm and executable DefinitionId/recipe/quote/run observations match despite changed source hashes. MC2 evidence, owner-frozen law, applicability, artifact and host authorization remain separate; a general erasure theorem is open.

Lean is the **first independent exact-claim checker target for user-authored Noble proof terms**, not raw user Lean pasted into a module. Existing VT-ROLE-01/VT-SCOPE-01 requires pinned Charon → Aeneas → Lean for future **formal refinement of the actual Rust implementation** of module parser/resolver, source proof checker, typed Noble→Lean lowerer and admission; reuse that policy without adding a new dependency. Aeneas does not parse Noble source or replace the strict independent application-proof consumer. The immutable [source-bound acceptance receipt](../verification/intrinsic-proofs/acceptance.json) records six passed real-source cases (CONTRACT-17/18/19/21/22/25), exact reviewed CLI/kernel-peer build and raw commands, including a real pinned Lean consumer process fault and bounded MC2 erasure observations. CONTRACT-24 is an independent **assurance review** labeling checked Lean terms, valid-Lean/broken-translation counterexamples and unproved Aeneas/Rust claims, not a translation or refinement proof. CONTRACT-20/23 remain unexecuted and blocked on VC-OWNER-01/CONTRACT-16: no host MC1 release admission is asserted. Source-to-claim correctness, source translation, actual Rust refinement, general erasure/backend correspondence and host authority remain open; Bend2 remains comparative design, not a proof compiler. This optional workstream adds no prerequisite edge to already completed milestones.

The additive selected `contract 2` / `proof 2 ... for` named-call case CONTRACT-26 is **implemented/passed for its finite source-bound selection, proof open**. The immutable [named-v2 acceptance receipt](../verification/intrinsic-named-v2/acceptance.json) (SHA-256 `8ab64cf2856164dc3ed7f6126361ffa256593f2867dc2b150e93bc3fde5db4c0`) records 15 canonical variants, 97 commands, zero failures/integrity failures, independently authenticated source/graph and pinned strict Lean check of the generated typed wrapping `x+2` claim. `Definitions@5.step [ 1 + ]` imported into `Subject@3.twice [ d.step d.step ]` uses two distinct fresh specializations of one original lexical definition with authenticated occurrence/source/owner/typed Inst and complete graph reachability. The theorem concerns the `twice` definition BODY with preserved universal tail, empty selected parameters and full MC1 `Holds₂`; the separately submitted one-call root is independently authenticated. The gate separately observed compiled off/on proof erasure, I64 41→43 and signed wrapping boundary, without a general erasure/backend theorem. The separate external legacy regression (SHA-256 `4e2d4f62de72af1ac9c5bb8dd335ec41d0eed023044272af20b0a168b5791190`) passed the prior six cases with 154 commands under the same reviewed CLI; historical receipts stay byte-immutable. The synthetic fixed-slot Lean fixture alone remains insufficient. Source translation, actual Rust refinement, owner-frozen policy, artifact correspondence and host authority remain open; CONTRACT-16/20/23 are not promoted.

## Exact-calculator application workstream

[CALCULATOR.md](CALCULATOR.md) defines the first AI-authoring reference application. Its expression syntax and exact arithmetic do not expand bootstrap or change Noble's wrapping `I64` operators.

| Milestone | Depends on | Exit criterion |
|---|---|---|
| MA1 — Exact numeric library | M4 and explicit library gates | Arbitrary-precision integers, normalized rationals, exact decimal parsing, checked `I64` conversions, and resource limits |
| MA2 — Calculator application | MA1 and expression/session gates | Actual Noble/Wasm evaluator, parser, lexical nonrecursive functions, diagnostics, and unchanged state after failure |
| MA3 — AI-authoring evaluation | MA2 and authoring-tool gates | Independent change tasks, held-out acceptance, negative controls, recorded costs, and visible failures |

The `entry_gates` in [roadmap.json](roadmap.json) are open contracts, not completed dependencies. MA1 requires declarations/modules, iteration or recursion, text processing, numeric error schemas, and deterministic budget accounting. MA2 additionally requires command grammar for definitions and concrete application interfaces.

MA3 compares only supported authoring routes. Local-name syntax, editor holes, and structured tool schemas retain their own gates. An unsupported route cannot count as successful evidence or disappear from the report.

M1 through M4 do not depend on this application. Bootstrap arithmetic and composition provide an earlier, narrower precursor, not calculator acceptance. No estimate for MA1 through MA3 is credible before the required core and library experiments.

Optional calculator proof claims additionally require applicable MC1/MC2 support and exact subject/model correspondence. Ordinary calculator acceptance does not require application proofs. No theorem about wrapping `I64` automatically applies to the exact numeric library.

## Platform increments

| Milestone | Depends on | Exit criterion |
|---|---|---|
| M5 — Synchronous components | M4 | One pinned WIT world, independent peer, exact mappings, Aeneas-refined resource transitions, and SPEC-R001 cases |
| M6 — Native async | M5 | Pinned async ABI, direct-style execution, bounded ownership and retirement, complete lifecycle coverage, independent stream/future/cancellation cases, and actual-Rust refinement |
| M7 — Syndicate profile | M5 | Normative dataspace/facet model, bounded Preserves adapter, WIT boundary, and executed first service scenario |
| M8 — Optional higher layers | M7 | Choreography projection or durability contracts with separate acceptance evidence |

If M7 uses native async, that slice also depends on M6. No full concurrency claim can rely on unspecified async behavior.

### M8 selected choreography projection (bounded acceptance complete)

The archived [M8 Cairn change](../.cairn/archive/2026-09-26-m8-choreography-projection/proposal.md)
selects `Choreography-Service-M8` over completed M7: the versioned
`noble:choreography/service@1.0.0` **data descriptor**, one or two finite
publisher/subscriber rounds, exact deterministic global order and role-local
projections. The untrusted descriptor input is strict UTF-8 JSON bounded
to 512 bytes and depth three, with exact typed keys, no duplicate/unknown
keys or trailing non-whitespace content; no transport or general JSON
protocol is selected.
The descriptor is neither a new WIT world nor authority;
M7's `noble:syndicate@1.0.0` synchronous imports/exports remain unchanged.
One exclusive session on an empty M7 dataspace reserves two facets, one
concurrent assertion, two interests and four immediate/owed-removal events.
A trusted host pre-admits export identity, caller facet/role and exact pair
before guest invocation; each import and completion is separately monitored
under serialized pending-export state. A terminal compiled publish-and-trap
must show both imports, actual trap/retirement, typed add/remove and
subscriber absence, not publisher success. Pre-export refusal has zero state
or request effects; an in-flight refusal may legitimately retire the facet.
The [safety cases](conformance/safety-cases.json) S-CASE-18/19 and
[component cases](conformance/wit-wasi-cases.json) WI-19/20 are
`implemented/passed` for the selected canonical observations and all
38 hostile variants, including ten strict JSON boundaries and three
one-round capacity reservations. The
[M8 completion record](../verification/m8/evidence.json) binds the
[fresh compiled acceptance](../verification/m8/acceptance.json) to the
[source build](../verification/m8/build.json), with separately compiled
Noble participants, independent typed Wasmtime peer and seven-case,
16-variant fresh M7 regression. The
[whole-crate extraction](../verification/m8/extraction.json) retains only
three pure M7 dataspace Rust functions at compiler DefIDs 31/32/33 and nine
strict Lean theorem roots; all 265 inherited refusal controls pass. The
[17-command, 11-gate pre-promotion assurance](../verification/m8/assurance.json)
passes workspace Rust, Clippy, published Octet deny-all and complete Nix flake;
the separate final archived-source documents/Cairn Nix receipt is
`verification/m8/nix-checks.json`. On this host the selected Nix commands
used `--option builders ''` for local builds instead of the unusable remote
grpc/aspen daemon lock; signed substitutes, sandbox, pinned tools and gate
timeouts remained unchanged. The isolated user-store pilot hit a pinned
Valence source tarball HTTP 404 and was not accepted as a gate.

At M8 closeout, all 2,087 then-inventoried authored-production-body
refinements, including parser, projector, monitor, adapter, host and engine
correspondence, remained open. No user-defined protocols, distributed
transport, durability, fairness, full Syndicate, universal compiler/engine
claim or new WIT authority is selected.

### DXM1 selected declared modules (bounded acceptance complete)

The [archived native change](../.cairn/archive/2026-09-28-2026-09-26-declared-modules-v1/proposal.md)
and [completion record](../verification/declared-modules-v1/evidence.json)
select only finite `Declared-Modules-v1` source units. Opt-in `noble compile`
and framed `noble session` admit source opaque declarations, two-constructor
variants, immutable versioned session-local registry/import/export identities,
explicit typed `test.emit` adapter bindings and independently checked
managed-memory Wasm. Ordinary `noble run` has no such opt-in.
DX-03/08/09 pass 23 canonical rows (nine source/static, twelve zero-execution
link in off/on, two actual compiled denied-version-A after version-B display
rebind), within 39 actual source-bound case/supplemental rows and 27 planned
scenarios. The [22-command/14-gate pre-promotion assurance](../verification/declared-modules-v1/assurance.json)
retains Core/M4–M8 regressions, 32-unit source coverage, independent
630-source/268-refusal M4 extraction, all-workspace Rust, published Octet
deny-all and complete signed/sandbox Nix flake. A distinct final staged-source
documents/Cairn receipt is `verification/declared-modules-v1/nix-checks.json`.
Only the inherited three pure M7 dataspace functions have nine strict Lean
roots. All 2,595 authored production-body refinement obligations in the last
reviewed DXM1 inventory remain open; no nominal/module, host or engine proof
is claimed. General Result, local names, invariant-bearing types, portable
packages/cache, capability modules and MA/MW entry gates are unaffected.

### M7 selected local synchronous service (bounded acceptance complete)

The archived [M7 Cairn change](../.cairn/archive/2026-09-25-m7-syndicate-service/proposal.md)
selects a finite serialized dataspace with facet-owned assertions/interests,
bounded canonical Preserves text for `service(name:Text,ready:Bool)`, and the
versioned synchronous `noble:syndicate@1.0.0` WIT `service` world. The
`observe(name,ready)` Boolean denotes exact assertion membership, not
readiness. A compiled publisher publishes, an independently compiled
subscriber observes true and false readiness as distinct exact-pair
assertions, and the publisher's compiled publish-then-trap path retracts its
assertion for the surviving subscriber. The trusted independent Wasmtime
peer supplies dynamic typed Component Model linking/conversion and invokes
accounted production dataspace and host-policy decisions; it is not a
deployable general Syndicate runtime. This local synchronous slice does not
use or expand M6 native async.

The [M7 completion record](../verification/m7/evidence.json) binds all five
S-CASE-09/10/11/12/17 and two WI-14/18 cases and all 16 hostile variants to
fresh [compiled acceptance](../verification/m7/acceptance.json), including the
inspected shared-memory Wasm admission refusal and child/interest trap cleanup.
Its [independent whole-crate extraction](../verification/m7/extraction.json)
binds 548 source files, three compiler-ID-joined pure Rust dataspace functions
and nine strict Lean theorems; 37 M7 and 265 combined refusal controls pass.
The [sixteen-command assurance](../verification/m7/assurance.json) passes
earlier regressions, workspace Rust tests/Clippy, published Octet deny-all,
full Nix flake and Cairn/documents. The separate final archived-source
documents/Cairn Nix receipt is `verification/m7/nix-checks.json`. The 2,006
authored-body inventory refinements remain open. General Syndicate/Preserves,
fairness, transport, durability, general choreography beyond the separately
accepted bounded M8 slice, and universal compiler/engine correspondence remain
outside this historical M7 selection.

### M6 native-async implementation and closeout

The bounded M6 implementation addresses WI-ASYNC-01 through WI-ASYNC-05 and RA-ASYNC-01 through
RA-ASYNC-05, preserving WI-RES-04, the M5 protected-host contracts, and
DX-PROTOCOL-03. It is implemented, natively exercised and accepted for the
selected scope. [ND-56 through ND-58](DECISIONS.md#native-async-implementation-direction)
record the bounded lessons from the Bend2 comparison.

Suspension stays at the Component Model boundary and preserves sequential Noble
word order. Direct-style imports use host-owned task records, not an `IO` monad,
`async`/`await` syntax, a guest task constructor or a general spawn/channel
language. Pure computational parallelism is not an async-host ownership contract.

Implementation and retained acceptance scope:

1. **Selected native boundary.** [M6 pins](../verification/m6/pins.json) select
   `noble-test:async-boundary/bootstrap@1.0.0`, memory32/UTF-8 native async
   lowering, stackful lifting and `task.return`, Wasmtime/bindgen 40.0.2,
   `wasm-tools` 1.245.1 and WIT parser/component tooling 0.243.0. Mixed
   synchronous members retain their synchronous ABI. A separate hand-written
   peer component executes official
   `wasi:clocks/monotonic-clock@0.3.0-rc-2025-09-16#wait-for`; it is not Noble
   output or stable WASI 0.3 acceptance. Stable linkage and disabled
   async/stackful engine configurations are explicitly refused.
2. **Implemented ownership core.** The production task table reuses the M5
   resource and authority boundaries and covers `Pending`, `Ready`,
   `Delivered`, `Retiring` and `Retired` against fifteen event constructors.
   The exact 75-pair schema includes invalid and duplicate outcomes;
   payload/identity guards remain additional obligations. Namespace, owner
   context, generation and native-operation identities guard callbacks.
   Completion, result delivery, cancellation and retirement are distinct,
   serialized decisions with accounted input owners, result owners and pins.
3. **Selected admission and progress bounds.** Task, terminal-result, payload,
   parked-payload, pin, wakeup and retirement capacity is reserved before
   transferring owners or starting protected work. The cooperative driver
   uses 100,000 guest fuel, a 1,000-fuel yield quantum, a 4,194,304-byte
   linear-memory limit, eight live values and at most 32 retained external
   native jobs. Each live future/stream reserves 128 result bytes and 128 local
   plus 128 external parked bytes; the stream producer buffer is one byte and
   the consumer payload is bounded to 64 bytes. These are not whole-process
   heap bounds. Separate measured fuel, epoch and blocking-deadline probes
   establish their local interruption observations, not universal latency,
   fairness, eventual native completion or cleanup.
4. **Compiled Noble surface.** The [native gate](../verification/m6/gate.mjs)
   compiles and independently validates actual Noble components before the
   Rust peer runs WI-11's ordered imports and WI-12's future/stream terminal
   matrix. WI-16 rejects duplication, capture, generic drop and serialization
   of live values, including a Noble `Pair<Text,stream<u8>>`, with independently
   executed positive construction controls. Additional compiled worlds cover
   owned-resource return/domain errors and five-parameter calls. Live values
   remain non-`Data` and non-`Capture`; borrow-retaining async calls are refused.
5. **Ownership races and abnormal exit.** WORKER-08 and the local lifecycle,
   schema and progress controls exercise production decisions separately from
   the compiled Noble cases. Cancellation before completion, ready-result
   cancellation, delivery before cancellation, domain errors, stale/foreign
   callbacks, duplicate events, quota failures and late completion retain
   distinct observations. Stream closure is not producer completion or
   invocation cancellation. Abnormal exit revokes guest access in an isolated
   Store while host state and native pins survive until actual native stop
   and settlement. Wasmtime 40.0.2 provides no selected per-task cancellation
   API; Store destruction is not proof of retirement. This does not complete
   WORKER-01/09, a worker service or MW1/MW2.
6. **Authority, assurance and scoped closeout.** One-shot witness consumption
   and resource transfer remain separate obligations at the same admission
   boundary. Late authentic operation success cannot uncancel an invocation
   or restore guest ownership. The [independent fresh
   check](../verification/m6/extraction.json) passed the reviewed source-bound
   Charon → Aeneas → Lean lock: 14 strict actual-Rust roots, all 75 constructor
   pairs, 176 M6 and 228 total refusal controls across 526 bound source files.
   [Both formal archives](../verification/m6/formal-archives.json) passed full
   member-by-member roundtrip checks. Constructor coverage, correspondence and
   compiled declaration accounting are distinct from native runtime behavior.
   The [full prior-milestone regressions](../verification/m6/regressions.json)
   and [all thirteen Nix checks](../verification/m6/nix-checks.json) passed
   against their independently recorded source snapshots; the final
   documents/Cairn source projection excludes only its own generated receipt.
   The earlier-milestone regressions and unchanged deny-all/architecture
   gates remain separate; native acceptance alone does not replace them.

The [retained runtime archive](../verification/m6/runtime.tar.gz) and
[verified manifest](../verification/m6/runtime-manifest.json) preserve all
1,708 gate-retained members within 1,709 files. Deduplication preserves bytes
and permission modes, not execution-time inode identity.

At M6 completion the [source inventory](../verification/source-inventory.md)
covered 25 units with 1,939 open authored-body obligations; the historical M7
renewal covered 29 units with 2,006 open. At M8 closeout the then-current
renewal covered 30 units with 2,087 open; the last reviewed DXM1 renewal
checked 32 units with 2,595 still open. Historical milestone receipts
keep their historical inventories. Neither extraction nor this native execution
closes the broader frontend, kernel, compiler or backend proof obligations.

The executor is a mechanism, not authority or cleanup evidence. Bend2's
copyable result channels, channel-close behavior, and whole-loop halt are not
substitutes for Noble's admission/delivery/retirement protocol. M6 does not
close general async borrowing, all WASI/Component-Draft interfaces, worker
services, Syndicate, distributed retries, or universal backend/host refinement.

## Typed-worker conformance workstream

[WORKER-CONFORMANCE.md](WORKER-CONFORMANCE.md) connects kernel and host contracts
without new agent syntax. The M6 receipt retains WORKER-08's local task-owner
race slice; the broader actual worker/shell harness remains unimplemented and
unaccepted. That local evidence does not promote MW1 or MW2.

| Milestone | Depends on | Exit criterion |
|---|---|---|
| MW1 — Worker interfaces and admission | M4, M5 and explicit language/service gates | Generated worker admission, typed dispatch, round trips, hostile-input rejection, bounded execution, and authority-separated observations |
| MW2 — Worker cancellation slice | MW1, M6 | Actual worker/shell execution, both cancellation/delivery orders, late callbacks, and accounted native retirement |

MW1 requires a worker-interface checker, declared schemas/modules, an explicit preparation service, package encoding, and a bounded execution profile. The finite DXM1 session-local module selection alone does not close these broader entry gates or make MW1 a completed capability. MW2 additionally requires concrete native-async task interfaces. M2 establishes finite candidate limits in its own subset. M6 owns RA-ASYNC-02 through RA-ASYNC-05 regardless of the worker harness.

The MW1 case set is WORKER-02 through WORKER-07 and WORKER-09 through WORKER-12. Async-dependent rows in WORKER-09 remain MW2 obligations rather than false MW1 coverage. MW2 executes WORKER-01 and WORKER-08 plus those deferred rows. Reports retain a result for every applicable matrix entry.

Neither milestone blocks M1 through M4, MC1/MC2, or the calculator. These tests do not select worker scheduling, task leases, distributed retries, or durability. M6's completed local WORKER-08 slice did not establish M7 service acceptance, and neither the separately accepted bounded M7 service nor M8 choreography closes the broader worker milestones.

## M1 architecture and quality gates

All Noble-owned production Rust targets Charon → Aeneas → Lean, not just the checker. The semantic kernel uses safe sequential Rust within the confirmed extraction subset. It owns types, checker decisions, values, builders, recipes, identity, and ownership transitions. The shell owns source files, build orchestration, Wasmtime, and host effects. No generic `common` crate collects unrelated responsibilities.

1. Select compatible Rust, Charon, Aeneas, Lean/backend, and Nix inputs. Record immutable pins. Let Nix create `flake.lock`.
2. Pin a reviewed immutable `OnixResearch/octet` revision. Declare semantic core source scopes in `dylint.toml`.
3. Configure workspace-wide, all-target, all-compatible-feature checks. Keep explicit matrices for incompatible features or platforms.
4. Use the pinned Octet `octet-deny-all` pre-commit template with no disabled lints, warning budgets, or finding baselines.
5. Make `nix flake check` run the same gate. CI runs `nix flake check -L`.

M1 also activates VT-OCTET-01 through VT-OCTET-03. The architecture policy declares exact roles, providers, ports, targets, and feature coverage. Its Nickel source, exported policy, and freshness manifest form one reviewed input. Inventory/advisory output cannot close the gate. Missing compiler facts and unsupported required scopes remain blockers.

A strict lint profile supplements rather than replaces the full deny-all catalog. Function-address identity baselines do not permit existing findings. Source-token closure and fixture-scoped Charon analysis remain separate evidence. EV-BIND-01 and EV-TIER-01 prevent stale-input reuse and evidence-role promotion.

Required Rust checks include:

```sh
cargo test --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
pre-commit run octet-deny-all --all-files
```

The hook arguments are `--workspace -- --all-targets --all-features`. Narrow source suppressions require a reason and named owner. Broad suppressions are not acceptance evidence.

Before implementing reusable infrastructure, inspect published components for matching contracts. Dependencies must be pinned, not ambient sibling worktrees.

M1 establishes VT-SCOPE-01 through VT-SCOPE-05 source coverage and the VT-CI-05 extraction gate. It includes a minimal real extraction smoke test. Future components remain open inventory entries, not silently excluded crates. Aeneas compatibility is a workspace design constraint. Verus pins are unnecessary unless a reviewed non-kernel exception selects that lane.

M1 also establishes VT-NATIVE-01/02 boundary inventories, cited safety arguments, and scoped Miri checks. An empty inventory is explicit, not a claim that dependencies contain no unsafe code. Unsupported Miri configurations do not satisfy a required lane.

M2 uses domain-specific Rust types for identities and acceptance state. Wire
views do not enter its initial owned-data core. The separate
[ADAPT-04/05 decoder experiment](../verification/decoder-experiment/acceptance.json)
passed exact bounded owned-versus-safe-borrowed-byte-view cases, external
mutation/suspension refusals and scoped strict Miri; the
[ADAPT-07 design review](../verification/decoder-experiment/adapt07-review.json)
rejected native `IntoBytes` output as program identity. The optional zerocopy
crate/derive adapter remains unselected and not run. No canonical Noble binary
format or semantic acceptance follows; dependency adoption still needs a
concrete selected format and VT-NATIVE-03 extraction/ownership review.
The [ADAPT-13 native assurance review](../verification/decoder-experiment/adapt13-review-final.json)
classifies all four evidence-policy controls against the actual pinned,
Noble-local scoped Miri decoder receipt: a missing safety argument,
unsupported required Miri, or upstream-only Miri cannot satisfy the local
lane; even the cited argument and passing local checks establish only scoped
native evidence, not Wasm correspondence or all-target coverage. The
[ADAPT-14 snapshot review](../verification/decoder-experiment/adapt14-review-final.json)
evaluates initial `100`, amount `10`, final `110` under signed-I64 premises:
`after = before + amount` is true and `after = after + amount` is false.
This finite design counterexample is not a universal proof. ADAPT-16 remains
unrun: public Rust `ProgramValueId`, `BuildKey` and `AcceptedProgram` APIs do
not yet exist, so missing-symbol compile failures would be vacuous.

## M2 and M3 feasibility contracts

M2 completes the finite candidate and checking specification before checker acceptance claims. Extraction must use actual implementation functions, not rewritten generated Lean code. Its gate tests must reject a missing function, unsupported kernel body, unexplained external model, and unfinished required refinement.

The first theorem establishes a proof pattern, not whole-kernel verification. M2 maintains explicit open function contracts for the rest of the kernel. Later compiler, resource, runtime, and CLI increments extend the same source inventory and proof route.

M3 can begin from the declared bootstrap interfaces while M2 proceeds. Its experimental checker and backend remain explicitly unverified until their evidence connects at M4. Pure lowering, optimization, and emission code target Aeneas from the start. Wasm execution/reflection correspondence remains a separate theorem family even after Rust extraction succeeds.

[BACKEND-EXPERIMENTS.md](BACKEND-EXPERIMENTS.md) compares WasmGC with managed linear memory. Both need a recorded trial disposition, not two production implementations. A blocked candidate cannot supply invented performance measurements. At least one candidate must pass the M3 execution gates.

The [M3 comparison record](../verification/m3-wasm/evidence.json) covers both
candidates under optimization off/on, with 52 scenarios per configuration.
Managed linear memory is selected for M4's resource-free implementation because
its bounded arena is observable and requires no WasmGC feature. This is not a
speed or physical-memory victory: the selected layout retains an extra 64-KiB
page, and GC physical allocation/reclamation remains unknown. Actual pure-Rust
emitter extraction and strict bridge equations are separate from the executed
Wasm observations. PO-17/18 and SO-07 remain open.

The separately probed **current-source** `nominal::generic` loop repair passed
its eight scoped checks and pinned Charon 0.1.254 extraction (LLBC SHA-256
`2d280a6f2e4091f2ea0f9a991b6f60ff74287ec7f43c8ae1245cfc613e674404`).
Pinned Aeneas `505b6ca` exited 2 at a mixed `Ty`/`NominalShape` Clone + Debug
strongly connected component (`types/impls.rs:28-79,246-328`; `types.rs:26`)
**before Lean**. An off-tree, mirror-only SCC/iteration mitigation exposed at
least twelve additional borrow/context failures; no live SCC workaround was
selected. Thus this probe produced no current-source Lean root or accepted
refinement receipt; it does not change historical M3 proof evidence or close
the remaining proof obligations.

The component probe is nonblocking for resource-free M3. M5 owns complete boundary conversions and independent-peer evidence. M6 retains the async ownership gate.

A negative result is useful: unsupported extraction, an unsuitable ABI, or broken dynamic composition must remain visible. A negative result alone cannot close a milestone.

## M4 implementation and retained acceptance

M4 implements the selected resource-free Core-Bootstrap path, not a second M3
backend or a component runtime. Parsing/resolution and inference produce finite
untrusted candidates; independent kernel acceptance and backend rechecking
precede compiled Wasm execution. Supported values include wrapping `I64`,
Bool/Text/Unit, Pair/Sum/List, Programs and inert Syntax. Quote/compose/run,
reflection, stack controls and checked branches preserve ordered interfaces,
captures, exact recipes, resolved identities and conservative latent effects.
Persistent sessions retain actual compiled values and definitions. Preparation
refusals leave prior stack/namespace unchanged with no candidate-body host
requests; runtime traps and quota exhaustion terminate the session and retain
already observed request prefixes.

The retained `verification/m4/acceptance.json` passed all 18 CORE cases,
11 controls and the integrated DX-10/DX-12 workflows described below.
Final runtime acceptance used a read-only binary snapshot after Cargo tests,
avoiding races with mutable build targets. Developer-workflow execution remains
a mandatory M4 acceptance obligation, satisfied by this scoped runtime receipt,
not deferred to M5 or replaced by static document checks.
The MC1 36-case regression and the M3 four-configuration regression have passed
separately and do not substitute for M4 evidence.

Closeout passed fresh actual kernel/frontend/compiler extraction, both compiled
dependency/axiom audits, 27 refusal controls, independent checking against the
reviewed extraction lock, full deny-all/architecture and source-coverage gates,
and all 13 Nix checks, alongside integrated runtime execution. The
durable receipt authorities are `verification/m4/evidence.json`,
`verification/m4/acceptance.json`, `verification/m4/implementation.json` and
`verification/m4/extraction-lock.json`; raw runtime/workflow and assurance
artifacts belong in `verification/m4/runtime.tar.gz` and
`verification/m4/assurance.tar.gz`. Discovery alone is not a passing extraction
gate: independent check mode must re-extract and compare source/generated
identities, inventories, dependencies and axioms against the reviewed lock.
The reviewed receipt/inventory, not a fixed function/model count in prose,
defines coverage. The [acceptance evidence](../README.md#m4-acceptance-evidence)
distinguishes these completed observations from open refinement obligations.

The M4 receipt recorded 887 open authored-body obligations in 19 units.
MC2 recorded 1,319 open authored-body obligations in 21 units. The current
M5-renewed [source inventory](../verification/source-inventory.md) records
1,620 open authored-body obligations in 23 units.
M4 does not claim universal frontend/kernel/compiler/backend refinement,
PO-17/18 or SO-07 closure, MC2 companions, resources/components, physical GC
reclamation or isolated engine peaks. Final milestone promotion and Cairn
sync/archive follow the retained gates, not implementation alone.

## Developer-experience delivery

[SPEC-DX001](DEVELOPER-EXPERIENCE.md) defines the scoped acceptance gates. M2 requires DX-DIAG-01 and the three DX-01 negative diagnostic cases. Resource-shaped checker inputs do not require live resources.

Editor holes follow M2 and require a separate editor syntax contract. Guest domain types and Result library interfaces follow M4; they do not block bootstrap. Resource-bearing library execution also requires the ownership profile.

Identity tools require resolved recipes. Portable cache keys additionally require canonical encoding; proof reuse follows SPEC-V002 evidence admission. Resource-free test hosts can use the M2 environment. M5 supplies the bounded counter/resource host and its explicit ownership protocol; general guest library and resource-protocol interfaces remain separate work.

The separately accepted [DX-06 scripted host](../verification/dx06/acceptance.json)
passes six finite opt-in `test.clock` controls on frozen compiled Wasm:
authorized versioned substitution returns 42, while denial, missing or
incompatible mapping, script exhaustion and unexpected operation fail
explicitly. Latent `[test.clock]` reflects the declared effect; no real clock
or ambient-host fallback was called. This is test-only host evidence, not a
general adapter, authority or host/backend refinement theorem.

The active [Editor-Draft Cairn change](../.cairn/changes/editor-hole-transport/proposal.md)
now has a separately accepted corrected [DX-02 receipt](../verification/dx02/acceptance.json)
(SHA-256 `2a6e59c06c26b3120b9a242e990647d64d7b4ed455208398801183ac435d7b5f`,
source `sha256:624f95e6f6fccfb8bbcb2aaa9dd825dde866329c9fbbfe24b662803881366116`).
Four genuinely distinct analysis/admission variants preserve editor-hole constraints
but refuse direct, nested quotation and serialized admission before an accepted
program, worker or output directory. A separate hole-free typed source executes
as compiled Wasm `I64(3)`. The earlier falsely attributed DX-02 smoke is retained
only off-tree as `REJECTED-PROVENANCE`, never as a canonical receipt. The accepted
[ADAPT-06 receipt](../verification/adapt06/acceptance.json) (SHA-256
`a3be07a092cc48ee62d83f546996f16195195f53c23f1306f2ea1f0859909177`,
source `sha256:ed5cc5f8ec6e56196b9d84f8e8d756865446de3d57e4bb8cb7e212a1b51f2b65`)
distinguishes kernel acceptance under a forged erased-effect premise from
independent production preparation and request-only effect-bound refusals.
Neither finite gate proves a general source/kernel correspondence.

The [current-source replay](../verification/current-source-replay/acceptance.json)
(SHA-256 `20c407e546759402c804824c9b40037a8163e93e8cd22b416c10a56cb8330b1a`)
separately replays **DX-01, ADAPT-01, S-CASE-01/03/05/14** on the same frozen
kernel and host source inventory (`sha256:ad7369ac1a12ce456ab885233076bc0dfb5ec3873deb25ba17eb0c05a0a04291`).
Its freshly compiled CLI, independent typed Resource, effect and source/kernel
peers, and retained production resource Table passed 17 commands with no
failures or integrity failures. The six original case receipts and their
immutable source scopes remain separate; these appended current observations
do not discharge universal safety, backend, host or Lean refinement proofs.

After DX-02 and ADAPT-06 promotion, the independent
[renewed DX-06 receipt](../verification/dx06-current-source/acceptance.json)
(SHA-256 `cf0eb8225a53564478364fe3083b7baea674365586e490e886631ec7673b8357`,
source `sha256:b94a812c24df833df8023ed347eb82062df7486da858090fc1458bd0a5bfc694`)
replayed the same six compiled scripted-host controls and latent effect
reflection under the later frozen source. Its two retained commands, six
controls, zero failures and exact append-only DX-06 evidence projection preserve
the earlier receipt rather than renewing it in place. Subsequently the
[six-case renewal](../verification/current-source-replay-dx02/acceptance.json)
(SHA-256 `c5f6c5956fd5ee2de5fc20bf298c1eb33768f70b0078deb109feae3739e58cfe`,
source `sha256:036b507351a6a6dc3a1703961ef81a8fc1c105a2363e776cc94be970ffe6fb09`)
passed 13 newly recorded compiled CLI, independent typed Resource,
production source/kernel and resource Table commands on that chronology.
DX-01, ADAPT-01 and S-CASE-01/03/05/14 each append only a third evidence
object; the complete preceding case bytes and both earlier evidence objects
project back exactly. Separate frozen source revisions are intentional.

The additional pinned offline four-crate all-features Rust regression passed
250 tests with zero failed/ignored/filtered and zero compiler warnings in an
isolated external target (kernel 135, contracts 86, CLI 22, Wasm 7;
the external run is `/home/brittonr/.cache/noble-adapt06-tmp/rust-suite-20260930T124405`,
summary SHA-256 `f78572ae818907344cdb9f61931e62591c738b0de91aacd2179e2d3bdad3e9ae`).
Three `named_v2` CLI tests and MC2 controls contain environment-gated paths
that did not run without NOBLE_LEAN/BWRAP/PRLIMIT/SYSTEMD_RUN; this regression
does not add Lean or MC2 proof. The current sandboxed whole-workspace
boundary-controls gate did **not** pass: `compiler-architecture-ir.json` was
missing, no completed gate exit was retained and the wrapper-free Octet run
reported 44 deny findings/42 errors. The earlier unsandboxed 180-second cache
timeout was not gate acceptance. Historical coverage and source inventories
are not promoted by these finite tests; current Aeneas/Lean refinement and
general host/backend proof remain open.

### S-CASE-16 compiled artifact admission and subsequent renewals

The active [safety Wasm manifest admission Cairn change](../.cairn/changes/safety-wasm-manifest-admission/proposal.md)
has a separate finite [S-CASE-16 receipt](../verification/scase16/acceptance.json)
(SHA-256 `7534cfc6501b223e80e04ed0042fdf21fead5ffe246e4528b1358e8d605457d5`,
source tree `sha256:320bed079b5cd8564637164fe7dfdfc22b2124b6c14f15e9c6a46ec56a3805f2`).
It records 23 refusals, five actual compiled executions and three
unchanged-host start/data/element controls. A real source-compiled
`"audit" test.emit` artifact imports only `noble.test_emit`; an empty claimed
effect manifest refuses at admission before instantiation, guest requests or
protected work, even with candidate-supplied digest/trust hints. An
independently rebuilt, host-selected byte-exact source and effect policy
admits the positive artifact and records one authorized audit request.
Pure `1 2 +` compiles without effect-function imports, executes to `I64(3)`
and requests no host operation. These selected outcomes are not universal
artifact correspondence, runtime authority or Rust/Lean refinement proofs.

After this initial S-CASE-16 promotion, a *new* source-bound
[DX-06 renewal](../verification/scase16-current-source/dx06-acceptance.json)
(SHA-256 `4385bd6a4b91bc745c94c9ee4ca65ec709ca53f886d0f962eb5a495a138e30a8`,
source `sha256:ac328b1e8313d3a3bcc73a2175ed862ec5cdf3f4d6554e3931e6b5b8196a0b94`)
passed seven retained commands: the six compiled test.clock controls and
latent effect witness plus fresh real compiled test.emit/pure import,
forged-manifest and trusted admission contrasts. It appends only DX-06
evidence[2]. On that promoted source, the independent
[six-case renewal](../verification/scase16-current-source/acceptance.json)
(SHA-256 `487ca0f71ea9740f7f032537cd59f3197c4085735dea9ff9681237b82aa8e77b`,
source `sha256:4d77beea2c96438336f3eedf604ed679b570069f264c486ff24b763837103a9d`)
passed 18 retained commands: the same six diagnostic, branch, type, handle,
effect-bound and unbound-word cases through a fresh compiled CLI and
independent production peers, plus fresh effectful/pure conditional-import
admission controls. Each six-case record appends only evidence[3]. Both
postpromotion projectors recovered every prior complete case file byte at
their recorded pre-view-rendering source snapshots, retained S-CASE-16
evidence[0], and verified all older receipts by exact SHA. The later native
Cairn view regeneration changed `.cairn/specs/safety/spec.md` from the
pre-view runner-bound SHA `39d595c9...` to `f4c520c9...`; the old strict
source projection must not be presented as passing on the finalized view.
Neither old receipt was renamed, rewritten, or expanded to that later source.

The finalized native safety view is independently covered by **two further
immutable supplemental receipts**, without appending another canonical
evidence record: [final-view DX-06](../verification/scase16-final-source/dx06-acceptance.json)
(SHA-256 `bc36af63a8d80beb9e91156a6cae086d09c927a07786f2e276eca9c230249244`,
source `sha256:6a2f242c9a129aa63a4147341d041a1e16e69d2bf5650cd3b870e379d0145a9a`,
471 inventoried files, seven commands) followed by
[final-view six-case replay](../verification/scase16-final-source/acceptance.json)
(SHA-256 `194ee1cadf54d7e3def7f761758721397bdfb5dcc3f78d04523c639776895464`,
source `sha256:29c0210ed710a27f531c92bd04ee3a18c5e686697e7cd6a331cba116aadd70f4`,
472 inventoried files, eighteen commands). Both rerun the actual
effectful/pure conditional-import and admission contrasts using freshly
compiled binaries; the first also passes all six scripted-clock controls,
and the second reruns all six canonical diagnostic/safety paths through
compiled CLI and independent peers. Both report zero failures and integrity
failures, verify unchanged canonical case bytes and bind the rendered native
safety SHA `f4c520c996fe71a090d3d7d129b343ce9545b19ead6b32a06798057374d0208a`.
These supplemental release observations preserve earlier DX-06 evidence[2]
and six-case evidence[3] as separate historical source scopes; they do not
make those earlier case-linked receipts cover the final view.

The newly changed source also passed a fresh pinned offline four-crate
all-features regression: 250 tests, zero failed/ignored/filtered and no
compiler warnings across 32 runs, including eight compile-fail kernel
doctests, in the isolated external target
`/home/brittonr/.cache/noble-proof-tmp/scase16-rust-suite-20260930/target`.
Its first launch failed to locate `cc` because the selected GCC wrapper was
omitted from PATH; retrying with that pinned linker passed without a source
change. Environment-gated `named_v2` and MC2 paths do not become new Lean
or MC2 proof evidence. The already reported full boundary-controls/Octet
failure was not rerun or promoted; M3 Aeneas SCC and VC-OWNER-01/CONTRACT-16
owner-law prerequisites remain open.

The separate [S-CASE-02 bounded-region change](../.cairn/changes/safety-bounded-region-adapter/proposal.md)
uses a new versioned WIT resource and the production
`noble component read-region COMPONENT HOST_BUFFER_HEX OFFSET LENGTH` ingress,
not Core's 16-page `values.mjs` memory. The initial
[source-bound receipt](../verification/scase02/acceptance.json) (SHA-256
`23f2b73aff9ee4dfa0bbd7d613dba3e569caa316409a49abe9f7884eddcec0b1`)
and runner are preserved byte-exact as **historical flawed-source** evidence:
that source reused TableId/Context across Stores. A new
[corrected receipt](../verification/scase02/corrected-acceptance.json) (SHA-256
`3588522e32149a490ea0ec8b7c33abbcb229150b91e24a31d164a17b60a740d0`,
prepromotion source `sha256:7a7f157c6eb5ea77249ea3144c4458d7888c13f175f02872bb3902680438a1f0`)
retains thirteen new commands. The compiled guest's canonical read of
host-selected `000102` at `(3,1)` still returns `bounds-reject` before any
protected region read; `(1,2)` returns `0102`, a distinct host input returns
`bbcc`, and both release one owner. Two simultaneously live Hosts with equal
slot/generation now have distinct atomically allocated nonreused TableId and
Context, and the first Host's claim is refused in the second before read.
End-zero, overrun, signed-negative, checked-add overflow and direct
forged/context/kind/stale/retired owner controls are separate finite
observations. The result remains proof-open and does not retroactively extend
the S-CASE-16 final-view receipts to the changed source.

After the corrected S-CASE-02 source and native safety/resource views settled,
two **new**, separately frozen and promoted source-bound gates passed. The
[DX-06 receipt](../verification/scase02-current-source/dx06-acceptance.json)
(SHA-256 `d092350b77ae605dc182e1b46d60cf5dd3f2737e447b27b3fccd99f0c8bbfb0f`,
source `sha256:a631317358d0564dea0a1d17df834b17a85ce54bc3d40f2054ae96a572d7a2a3`,
491 inventoried source files, seven commands) adds DX-06 evidence[3]: all six
compiled scripted-clock controls, latent effect reflection and independently
compiled pure/effectful import plus forged/trusted admission contrasts. The
[six-case receipt](../verification/scase02-current-source/acceptance.json)
(SHA-256 `dd36ff5c04808c16461ab34a1c905a73e942e73a63e889fa1ee9647bbca72046`,
source `sha256:8984e2c1986dfffa3dec584ffb7ddbd342da4631fe91df582131662e81b0c552`,
492 inventoried files, eighteen commands) adds evidence[4] only for DX-01,
ADAPT-01 and S-CASE-01/03/05/14, replaying their exact compiled/independent
diagnostic, resource, effect and owner-Table observations plus compiled
pure/effectful admission contrasts. Each gate reports zero failures and
integrity failures, retains external raw output and reconstructs each entire
prepromotion case file byte-for-byte. Earlier receipts and case evidence remain
immutable historical source scopes; these new receipts do not relabel them.

The corrected source also passed a fresh pinned offline all-features four-crate
Rust regression: **253 passed, zero failed/ignored/filtered and zero compiler
warnings across 32 runs**, including eight kernel compile-fail doctests, with
external target `/home/brittonr/.cache/noble-proof-tmp/scase02-rust-suite-20260930/target`.
An initial isolated Cargo home lacked offline `addr2line`; the passing retry
used the populated user Cargo registry, selected pinned Rust/GCC linker and
external target/temp directories. Environment-gated named_v2 and MC2 paths
remain outside new Lean/MC2 proof evidence. The earlier whole-workspace
boundary-controls/Octet failure was not rerun or promoted; universal Rust,
Lean, backend, host, native memory and owner-law proofs remain open.

DX-07 local bindings remain deferred: the proposed `pop_box` lowering adds
runtime allocation and computation rather than stack-only binding semantics.
No DX-07 case was promoted. The live recursive `Ty`/`NominalShape` SCC remains
unchanged, and the current M3 Lean proof remains open.

M2 also requires bounded property-runner controls and compiler-backed static
documentation checks under DX-PROPERTY-01/02 and DX-DOC-01/02. M4 extends these
to Wasm composition, recipe, and execution observations. These remain **M4
acceptance obligations**, not deferred M5 work.

The retained [DX-10 harness](../verification/m4/property.mjs) run passed 100
seeded arithmetic/interface/exact-recipe/effect trials, 100 replay trials,
all eight hostile controls and 50-step bounded shrinking. Its 200 malformed
kernel cases are a separate checker observation, not Wasm property coverage.
The retained [DX-12 harness](../verification/m4/documentation.mjs) run passed
the two exact declared examples and all six hostile controls with explicit
resource-free test hosts. Coverage records unsupported, failed, timed-out and
unrun outcomes; a whole-command timeout does not establish guest entry.

The integrated [runtime gate](../verification/m4/gate.mjs) emits
`property-workflow.json` and `documentation-workflow.json` beside
`acceptance.json`, retained under `verification/m4/runtime.tar.gz`.
Document checks do not run these workflows, bounded
trials do not prove universal properties, and the two examples do not establish
that all documentation executes.

After M4, local bindings require a stack-only comparison plus lexical-scope, lowering, and capture-identity contracts. DXM1 separately completes a finite opt-in versioned local module and test.emit adapter slice with exact source-bound DX-03/08/09 evidence; general capability-aware modules still require their wider interfaces, portable dependency identity and explicit policy. Neither changes ordinary bootstrap grammar.

Resource protocol types require M5 and guest declaration/module interfaces. Their gate covers successful transitions, error ownership, invalid reuse, runtime validation, and ambiguous outcomes. Behavioral proof claims also require the corresponding host models; MC1 remains resource-free.

These slices retain separate acceptance evidence. They do not add general handlers, dependent types, macros, or another concurrency model.

### S-CASE-07 artifact correspondence admission

The [safety artifact correspondence Cairn change](../.cairn/changes/safety-artifact-correspondence/proposal.md)
has a separate finite [S-CASE-07 receipt](../verification/scase07/acceptance.json)
(SHA-256 `6bb9ee9bf3d9ed1077818426d29a1bb9895fe1733e1d8bbb3041e3d7fa2013d5`,
source tree `sha256:9bdba3deb1e1d9550404ee7fc6328449143fe4302fe7d8fb8e090b33de44dda5`)
on the existing Core-only `noble admit-artifact` route, without a production
code change. It records 21 commands, with seven `correspondence-reject` rows
(the canonical case, its hint-free twin, three supplemental variants and two
labeled non-canonical controls), two normal executions and three
structural-absence probes. A freshly built pinned offline CLI refuses a
pinned-validator-valid Core artifact compiled from `2 2 +`, whose correct empty
effect claim carries forged `source`, `digest`, `trusted_correspondence` and
`allowed` metadata, under host-selected `1 2 +` with `--opt off`. The refusal is
`admission`/`correspondence-reject` before instantiation, with zero guest
requests and protected operations, and is identical without the hints. The same
bytes are admitted only under their actual source (`I64(4)`), and the true
`1 2 +` artifact executes to `I64(3)`. Valid inert-append and
executable-mutation variants also refuse. `trusted_build` and
`translation_validation` are structural absences: the route has no such input,
and such manifest fields are `invalid-manifest`. The
[post-promotion check](../verification/scase07/postpromotion.mjs) binds
S-CASE-07 evidence[0] to this receipt. Correspondence means exact byte equality
with the host's own pinned compilation. PO-17/PO-18 backend and loader
refinement, cross-host provenance and general Wasm loading remain open.
Later-source DX-06/six-case replays and four-crate/doc checks are separate from
this receipt.

After S-CASE-07 promotion and its settled native safety view, **two new,
chronological source-bound gates** passed against a freshly compiled CLI. The
[DX-06 receipt](../verification/scase07-current-source/dx06-acceptance.json)
(SHA-256 `3b9e726f1d9204655b72289228c3321d158de336381130b66ef8228a26af7509`,
source `sha256:aa313c5701e203cd94189b750c4bd32110252df14e8221e9f8a3cef6557c1064`,
506 inventoried source files, seven commands) appends DX-06 evidence[4]:
six compiled scripted-clock controls, latent effect reflection and actual
compiled pure/effectful import plus forged/trusted admission contrasts. The
[six-case receipt](../verification/scase07-current-source/acceptance.json)
(SHA-256 `b20e176c5cbe4ca7af997771b58ad569e76baae783bb3c7a36f7fdef7292fbc1`,
source `sha256:211e3559b4867e70a72aa73fe9fbd6e67804edc75087c3d30992e331dcca7b38`,
507 inventoried files, eighteen commands) appends evidence[5] only for DX-01,
ADAPT-01 and S-CASE-01/03/05/14, replaying the exact compiled/independent
diagnostic, resource, effect and owner-Table controls and compiled
pure/effectful admission contrasts. Both report zero failures and integrity
failures. Their postpromotion checks reconstruct every earlier complete case
file byte-for-byte, retain the accepted S-CASE-07 evidence[0] and independently
bind its prepromotion case bytes; older receipts remain immutable and scoped
to their own source.

The frozen tree also passed a fresh pinned offline all-features four-crate
Rust regression: **253 passed, zero failed/ignored/filtered and zero compiler
warnings across 32 runs**, including eight kernel compile-fail doctests, with
external target `/home/brittonr/.cache/noble-proof-tmp/scase07-rust-suite-20260930/target`.
The passing command used the populated user Cargo registry, pinned selected
Rust/GCC linker and external target/temp directories. Environment-gated
named_v2 and MC2 paths do not establish new Lean/MC2 proofs. The earlier
whole-workspace boundary-controls/Octet failure was not rerun or promoted;
PO-17/PO-18, owner law, universal backend/loader correspondence and
cross-host provenance remain open.

### S-CASE-13 independent Wasmtime fuel and generated-heap quotas

The [runtime quota Cairn change](../.cairn/changes/safety-runtime-quotas/proposal.md)
and [corrected source-bound acceptance receipt](../verification/scase13/corrected-acceptance.json)
(SHA-256 `e8211d6d2b0c77ac540874a24dc5f8dedf3bd3773c60b1105975ad9b485fcf28`,
source `sha256:62358a638f39085fb62f3934507ef8a1278fd6df84c9aefed09b92eb29c9da1d`)
select the unchanged compiler's import-free, versioned quota world. The
production `noble component quota-core` Rust route requires the complete
WIT/world/export-source recipe, recompiles it with the pinned assembler and
byte-matches the candidate Core before admitting a private allocator signal.
The gate retains ten commands: a deliberately unmatched recipe is refused
before guest execution without any quota label, followed by six actual matched
guest variants through Wasmtime.
After successful instantiation, exactly one unit of guest-call fuel traps
`compute` as `OutOfFuel` with no returned value; ample fuel returns I64(3).
Separately, with ample fuel and the generated allocator's 64-byte quota,
56 text bytes plus their eight-byte result record succeed, 57 fail while
allocating the record, and 65 fail before a first copy. The same 65-byte
source succeeds at quota 76. Every instance retains its normal 16-page,
1,048,576-byte initial guest memory; 64 is a generated-heap limit measured
beyond the 65,536-byte heap baseline, never a 64-byte Wasmtime memory cap.
The production host classifies a **matched Noble** allocator refusal only when
its private generated quota diagnostic accompanies the unreachable trap, and
all six fresh import-free instances report zero callbacks and zero live heap
after generated post-return or explicit trap cleanup.

The exact [S-CASE-13](conformance/safety-cases.json) `execution` /
`specified-quota-failure` observations are implemented/passed with proof open
and explicit trust. The [corrected postpromotion check](../verification/scase13/corrected-postpromotion.mjs)
replays the case projection, retained raw commands, binary/source hashes
and historical runner/receipt bytes. The [first receipt](../verification/scase13/acceptance.json)
remains immutable historical evidence from before production enforced source
correspondence and does not admit arbitrary import-free Wasm's quota diagnostic.
This bounded execution is not universal
compiler/engine correspondence or a proof of language safety.

After corrected quota source/views and S-CASE-13 evidence[1] settled, **two
new chronological source-bound gates** passed with separately built
production CLIs. The [DX-06 receipt](../verification/scase13-current-source/dx06-acceptance.json)
(SHA-256 `4d0c779090282f633e6930ae5a045e88ff44d2f4cc22da2f906528d0a1c266c0`,
source `sha256:6f0be229caa1108b47ce6963aa606d099529367aa4bbe4af332627f6a3c1667f`,
527 inventoried files, seven commands) adds DX-06 evidence[5]: six exact
compiled scripted-clock controls, latent effect reflection and independently
compiled pure/effectful import with forged/trusted admission contrasts. The
[six-case receipt](../verification/scase13-current-source/acceptance.json)
(SHA-256 `65ea09a8cc7e3a0a05f7f990d4b5e8cff7977192c3ecf9cbee51f4d876ff8e37`,
source `sha256:c3279ccc4b91e257840cadb8037f87dec4f49bdd662d4165174c44029b00b670`,
528 inventoried files, eighteen commands) adds evidence[6] only to DX-01,
ADAPT-01 and S-CASE-01/03/05/14: the same exact compiled/independent
diagnostic, resource, effect and owner-Table controls plus compiled
pure/effectful admission. Both report zero failures and integrity failures,
retain external raw transcripts, reconstruct every prior whole case file
byte-for-byte and preserve the corrected quota correspondence prerequisite.
The first S-CASE-13 receipt remains historical flawed-source evidence; no
older source-bound receipt is relabeled as a claim on this newer tree. The
quota change's final Cairn task checkbox alone is deliberately excluded from
the renewed source inventory so it can close after this release; production,
WIT, native safety and all other quota change source are bound.

The changed source separately passed a fresh pinned offline all-features
four-crate Rust regression: **253 passed, zero failed/ignored/filtered and
zero compiler warnings across 32 runs**, including eight kernel compile-fail
doctests, under external target
`/home/brittonr/.cache/noble-proof-tmp/scase13-rust-suite-20260930/target`.
The passing command used the populated Cargo registry, pinned Rust/GCC linker
and external target/temp directories. Environment-gated named_v2/MC2 paths
do not establish new Lean/MC2 proofs. Owner-law, Octet, Aeneas and universal
compiler/engine correspondence remain open; the previously reported
whole-workspace boundary-controls failure was not rerun or promoted.

### S-CASE-04 recursive resource eligibility

The [distinct native Cairn change](../.cairn/changes/safety-recursive-resource-eligibility/proposal.md)
syncs only S-RES-01 and S-RES-04. Its
[source-bound acceptance](../verification/scase04/acceptance.json)
(SHA-256 `16ba34b6672486271eaef6bc22e7fd86c476374fd4f46acb745e66c2fc87f8be`,
source `sha256:328f05c640c6bf71082d1e5c241ee9890e91f4050582584465879f6adc584286`)
binds a real WIT `own<counter>` to `ResourceKind(1)`. An independent typed
source/export peer and the compiled production component CLI reject all
sixteen selected `dup`, generic `drop`, `quote` and `quote reflect` operations
for complete `Resource<R>`, `Pair<Text,Resource<R>>`,
`Sum<Resource<R>,I64>` and `List<Resource<R>>` values. Each diagnostic
names the exact Data-requiring word and **whole** resource-bearing type; no
component is published, guest invoked or protected operation performed.
The false-selected I64 sum is still ineligible because its unselected left
alternative bears the resource; a separate checked `case` allows the I64
payload's own Data operations. Real resource close, Pair destruction and
resource-free Data compile as independent positive components. The sum
and nonempty List positives establish source preparation, **not** successful
aggregate component lowering, WIT aggregate ABI, runtime branch selection
or execution. Unsupported WIT, unbound source and unrelated type errors
are distinct controls, not counted as eligibility.

The canonical `check/eligibility-reject` is an explicit classification of
the typed kernel refusal and literal CLI `component-check/error`, not a
fabricated CLI outcome. S-CASE-04 alone is implemented/passed, proof open,
trust explicit. The external raw receipt and its 27 selected command logs
bind source, historical receipts, WIT/input bytes and binary identities;
zero guest requests/protected operations are pre-execution deductions,
not measured host counters. This active change has no separate scoped
Octet/full signed+sandbox Nix assurance receipt, so archive and general
owner-law, resource, lowering and Rust-to-Lean proof remain open.

### S-CASE-06 host-authorized bounded filesystem read

The [separate native Cairn change](../.cairn/changes/safety-authorized-fs-read/proposal.md)
selects a **single-entry logical Directory**, not a general OS-directory
preopen: the invoker maps only the exact guest key `main.rs` to an independently
vetted, preopened regular file. The production `noble component read-fs` route
independently recompiles the invoker-selected complete WIT/world/export-source
recipe and byte-matches the component before file preopen or host READ grant.
The guest path cannot be joined to a native path or supply its own right.

The [final chronological source-bound receipt](../verification/scase06/final-acceptance.json)
(SHA-256 `6943c1a03163ebe5c7e98c1cdc4187cfa02a8c7007c57702e7f241d5679ab418`,
source `sha256:e67068ecefd7161de12dd73b484598294317f9d7c227c82c49af2781b7ae5ebf`)
retains nine commands against the actual compiled `"main.rs" fs.read` component,
production Wasmtime import and resource Table. With a live invocation-1 host
Directory and no READ, exactly one guest `fs.read` request is counted and
denied at authorization with zero protected reads. With READ independently
granted, **the same component** returns the exact bounded fixture bytes and
releases its owner. A compiled escape path, hostile right/handle/size controls,
and a mismatched source recipe are distinct refusals. The production report
classifies `authorization/denied` only from its retained Table READ-right
failure; a source-matched guest returning the error string `denied` cannot
manufacture that trusted host observation. The
[first receipt](../verification/scase06/acceptance.json) and
[intermediate corrected receipt](../verification/scase06/corrected-acceptance.json)
remain immutable historical source scopes, not claims about this later
host-side classification. The native
[S-CASE-06](conformance/safety-cases.json) alone is promoted to
implemented/passed, with proof open and trust explicit; the generated
[validation receipt](VALIDATION.json) is document validation, not runtime
assurance. Historical receipts retain their own source revisions.

This finite exact-file profile does not establish a general filesystem
sandbox, owner law, Octet source coverage, Aeneas refinement or universal
compiler/engine correspondence.

After S-CASE-06 promotion, **two new chronological source-bound gates** passed
against the truthful **ACTIVE** Cairn change; no archive is presumed. The
[DX-06 receipt](../verification/scase06-current-source/dx06-acceptance.json)
(SHA-256 `8c8ea0587ad033ba433fdfcaaff0a7a1cefd75b8d9e1c4dc54193a77625e22ff`,
source `sha256:a163cc3d46831b99500d2d5012053fb2dbaa373f76f50f238f692fb19769fc4e`,
548 inventoried files, seven commands) adds DX-06 evidence[6]: six exact
compiled scripted-clock controls, latent effect reflection and compiled
pure/effectful import with forged/trusted admission contrasts. The
[six-case receipt](../verification/scase06-current-source/acceptance.json)
(SHA-256 `aeaaba6afa2c2a7f4e18ea7704b4fc46b2c6ba029f42eb4779f1d71c15686da1`,
source `sha256:93a933d6820d4ef07f115f5983635928d75ff51e8c7d405bd80764a1b29bc406`,
549 inventoried files, eighteen commands) adds evidence[7] only for DX-01,
ADAPT-01 and S-CASE-01/03/05/14, replaying their exact compiled/independent
diagnostic, resource, effect and owner-Table paths plus the compiled
pure/effectful admission controls. Both report zero failures and integrity
failures, retain raw transcripts, reconstruct every prior complete case file
byte-for-byte and bind the final S-CASE-06 receipt as a distinct prerequisite.
Normative active-change files, native safety/resource/evidence views, WIT,
production crates and substantive Cairn task prose are source-bound; only
task checklist markers are normalized, exactly as in S-CASE-06's accepted
source projection. All previous receipts remain immutable historical scopes.

The frozen source separately passed fresh pinned offline all-features tests
of four production crates: **254 passed, zero failed/ignored/filtered and
zero compiler warnings across 32 runs**, including eight kernel compile-fail
doctests, under external target
`/home/brittonr/.cache/noble-proof-tmp/scase06-rust-suite-20260930/target`.
The passing command used the populated Cargo registry, pinned Rust/GCC linker
and external target/temp directories. Environment-gated named_v2/MC2 paths
are not new Lean/MC2 proofs. The active S-CASE-06 Cairn change is **not
archived**: Phase3 staged postpromotion and archive/final checklist remain
unmarked because no executed scoped Octet/full signed+sandbox Nix assurance
receipt exists. Selected Octet availability, source projection and native
document validation do not satisfy that missing assurance. The previously
reported whole-workspace boundary-controls failure was not rerun/promoted;
owner law and Aeneas refinement remain open.

### S-CASE-04 chronological replay on the two active Cairn changes

After the finite S-CASE-04 acceptance and native S-RES-01/S-RES-04 sync, the
new [DX-06 gate](../verification/scase04-current-source/dx06-acceptance.json)
(SHA-256 `cc8abef6995e4ec74dac74bab46789d55014199a8f4afa20c938946284928146`,
source `sha256:691ef44a36e43ad0cf650dab5de7811449f34725609ce3b414c2012eb3f4c14b`,
563 inventoried files, seven commands) appends DX-06 evidence[7]. Six
compiled scripted-clock controls, latent effect reflection and actual compiled
pure/effectful import plus forged/trusted admission contrasts passed with
zero failures or integrity failures. The subsequent
[six-case gate](../verification/scase04-current-source/acceptance.json)
(SHA-256 `850f455e59f41be52e2ae1b7996473825af80d00653517c720b0d4c0aafcd2c3`,
source `sha256:7da8019eae8427176d4eaa05d62e74b14d92e4ecd2856712037b685771ecdfc9`,
564 inventoried files, eighteen commands) appends evidence[8] **only** for
DX-01, ADAPT-01 and S-CASE-01/03/05/14. It repeated their selected
compiled/independent diagnostic, adaptation, effect, safety and resource-Table
paths with the same import/admission contrasts; zero failures or integrity
failures. The receipts retain raw output, binary identities, exact
prepromotion canonical case-file bytes and the prior S-CASE-04/S-CASE-06
receipts as separately bound historical prerequisites. A nested postpromotion
checker reverses the newest evidence byte-for-byte and reconstructs the
accepted S-CASE-04 prepromotion safety case without modifying historical
receipts or runners.

Both [S-CASE-04](../.cairn/changes/safety-recursive-resource-eligibility/proposal.md)
and [S-CASE-06](../.cairn/changes/safety-authorized-fs-read/proposal.md)
Cairn changes remain **ACTIVE**; the two full normative trees, including all
substantive task prose, native safety/resource/evidence views, WIT, production
source and frozen runner identities are bound. Only task checkbox completion
markers are normalized to the accepted source. Fresh pinned offline
all-features tests of noble-kernel, noble-contracts, noble-wasm and noble-cli
under external target
`/home/brittonr/.cache/noble-proof-tmp/scase04-rust-suite-20260930/target`
passed **254 tests across 32 runs, zero failed/ignored/filtered and zero
compiler warnings**, including eight compile-fail doctests. No separately
executed scoped Octet/full signed+sandbox Nix assurance or archive is claimed;
S-CASE-06 Phase3 staged postpromotion and final checklist remain
blocked/unmarked. The old whole-workspace boundary-control failure was not
rerun, and aggregate lowering, owner law, universal host/compiler behavior
and Rust-to-Lean refinement remain open.

### S-CASE-08 callback-returned owner ingress

The [active native Cairn change](../.cairn/changes/safety-callback-returned-owner/proposal.md)
selects the versioned `noble-test:callback-owner/bounded@1.0.0` WIT world
and production `noble component callback-owner` host route. Before creating
an owner or entering guest code, the host independently rebuilds the complete
invoker-selected WIT/world/export-source recipe and byte-matches the candidate
component. The compiled Noble guest `tokens.issue tokens.consume` reaches
an actual Wasmtime imported `issue() -> own<token>` result; host-selected
callback modes, never guest inputs, choose real retained table claims. A
retired generation of the reused slot, a separately live wrong-kind owner,
and a separately live foreign-context owner each fail `Table::validate` at
the **callback result ingress**, before `ResourceAny`/`Val::Resource` or any
trusted guest owner exists. The host retains and releases the pending table
owner once; no kernel `Table::transfer` is claimed. The same component in
authentic mode receives the WIT-owned token, invokes `consume`, returns 42,
and releases that retained table owner once. Shared nonwrapping identity
allocation prevents callback, region and filesystem Stores from sharing
TableId/Context namespaces, even when slots and generations match.

The [immutable prepromotion receipt](../verification/scase08/acceptance.json)
(SHA-256 `e4da8ac29888544ef6586b12582cc1abd2d3c1065bd5c7c10d70897179402925`,
source `sha256:4fc42fce61b7f78fc4711de3a195a61f90d91a2ca3df0e50964827f265e47cb0`)
records eleven selected-tool commands, four actual compiled-guest host
invocations, three native table/identity controls and a separately valid
byte-distinct component refused at admission before guest entry. All three
canonical hostile mutations report `callback/invalid-result`,
`trusted_values_created:0`, no protected operation, one guest request,
one pending-owner release and no remaining live owner/pin. Authentic reports
one trusted WIT owner and protected consume; mode-selective fixture releases
are separate from the pending owner. Only [S-CASE-08](conformance/safety-cases.json)
is promoted to implemented/passed with proof open and trust explicit. Older
S-CASE-02/04/06 receipts remain immutable historical source scopes; generated
document validation does not establish guest execution or proof. The callback
change remains **ACTIVE** and unarchived: full scoped Octet plus signed,
sandboxed Nix assurance and a staged postpromotion closeout receipt are
unavailable. General owner law, host/engine correspondence, native release
and Rust/Aeneas/Lean refinement remain open.

After S-CASE-08's finite acceptance, two **new chronological source-bound
gates** passed on the truthful three-**ACTIVE** Cairn change trees. The
[DX-06 receipt](../verification/scase08-current-source/dx06-acceptance.json)
(SHA-256 `8de91ac4459991125462ff509df64845e85c5b26cd7810cb791c937bdbeb5588`,
source `sha256:749a01408ce16aa56b556c9af8983d46298de570d7c1b95969712e081a72f45f`,
581 inventoried files, seven commands) adds DX-06 evidence[8]: six exact
compiled scripted-clock controls, latent effect reflection and actual compiled
pure/effectful import with forged/trusted admission contrasts. The subsequent
[six-case receipt](../verification/scase08-current-source/acceptance.json)
(SHA-256 `a95ce45b7c1bd2a6ca1613e0feaf6d634e65c65923e78fd480e50bf70033feee`,
source `sha256:2fb0fb4474582fee66a6fbe9799d815ce7bd12315fef94821dca4a3eab815d5d`,
582 inventoried files, eighteen commands) adds evidence[9] **only** for DX-01,
ADAPT-01 and S-CASE-01/03/05/14, retaining their exact selected
compiled/independent diagnostic, adaptation, effect, safety and resource-Table
paths and those admission contrasts. Both passed without failures or integrity
failures; they retain raw command transcripts, binary identities, prior
immutable receipts, and reconstruct every prior complete canonical case file
byte-for-byte. The nested postpromotion check additionally reconstructs
S-CASE-08's original absent/not-run prepromotion safety-case bytes.

Full substantive prose of all three active changes, synced native
safety/resource/evidence views, WIT and four production crates is source-bound;
only task checklist markers are normalized. Specifically, the S-CASE-08 runner
exhaustively reconstructs all **128** possible original marker assignments
on its seven current checklist lines and finds exactly one byte sequence
matching the immutable prepromotion task SHA-256, proving the original prose
is unchanged. Fresh pinned offline all-features tests of noble-kernel,
noble-contracts, noble-wasm and noble-cli under external target
`/home/brittonr/.cache/noble-proof-tmp/scase08-rust-suite-20260930/target`
passed **257 tests across 32 runs, zero failed/ignored/filtered and zero
compiler warnings**, including eight kernel compile-fail doctests. All three
Cairn changes remain active; full scoped Octet/signed+sandbox Nix assurance,
the S-CASE-08/S-CASE-06 final staged closeout and archive remain
blocked/unmarked. Document validation is not runtime assurance; universal
callback owner law, host/compiler behavior and Rust-to-Lean refinement remain
open. The previously reported whole-workspace boundary gate was not rerun.

### ADAPT-15 mandatory admission across build modes

The [immutable prepromotion receipt](../verification/adapt15/acceptance.json)
(SHA-256 `84dd9752dc305ee340f8cdfe93d8c93f25b9d4462ce2c5c95fd683e567217f27`,
source `sha256:5757b6e50044cf7dc4a7852751850e2c59cf3bd281ed99eb381bd1c9caeed3c2`)
binds eighteen commands and four actual **debug/release × absent/unrelated
verified proof** cells. The source-derived `"audit" test.emit` authentic
request is accepted and compiled in both actual Cargo build profiles; changing
only its request's advertised allowed-effect bound to empty is refused by the
production kernel with `Invalid/EffectInclusion(test.emit)` and by the Wasm
compiler in all four cells, before any negative guest, host request or
protected operation exists.
Both compiled CLIs independently verify an exact, separately frozen **pure
MC1 increment** proof. That proof is not attached to the effectful forged
candidate or consulted by the kernel acceptance API; adding it cannot alter
the rejection. The [ADAPT-15 case](conformance/adaptation-cases.json) alone
is implemented/passed with proof open and trust explicit. Original native
program-contract, safety, core-bootstrap, evidence and resource-adapter
specifications plus the three active Cairn changes remain source-bound and
unchanged; earlier receipts retain their original source revisions. This
finite admission result is not a universal checker, host, build-mode or
Rust-to-Lean refinement theorem, and full scoped Octet/signed+sandbox Nix
assurance and archive remain unclaimed.

After that acceptance, the [separately frozen DX-06 release gate](../verification/adapt15-release-source/dx06-acceptance.json)
(SHA-256 `d0bebd2159549ba0b8f025328a6c1c4972dff6976e9bff2997aa73dd2c56cc49`,
source `sha256:347dce24cc424c09247f9c59e560d813d32e384d8a4dc84d54509d872ecc0152`,
629 bound files, seven commands) rebuilt the production CLI against the
truthful three-ACTIVE source. It repeated six compiled scripted-clock controls
and the latent-effect witness, plus independently compiled pure/test.emit
import and forged/trusted admission contrasts. The subsequent
[six-case gate](../verification/adapt15-release-source/acceptance.json)
(SHA-256 `26453b3d895e5f56d6b10f95b2bf9b0ae8259e12c7ff3e4530f7bfd44660b7dd`,
source `sha256:f8c43bf745346b9dc354f5e55a61a45e3ea34cd74871cd3f9429465b19ecf925`,
630 bound files, eighteen commands) repeated the exact selected compiled
and independent DX-01, ADAPT-01, S-CASE-01/03/05/14 paths after the renewed
DX-06 observation. Both gates passed with zero failures or integrity failures.
The final source-frozen checker passed after appending one byte-reversible
observation to each selected canonical case; the new DX-06 and six-case
observations are each at evidence index **11**. It recursively reconstructs
the prior full case files and ADAPT-15/S-CASE-08 prepromotion preimages rather
than treating historical safety evidence as an exception.

Earlier finite gate results remain immutable historical observations:
[the first DX-06 receipt](../verification/adapt15-current-source/dx06-acceptance.json)
(SHA-256 `030e820d3e6c131c3cdade5cff592a8ec9d8b1db336260bef05a8e9e0c1c54be`)
passed its compiled controls, but its frozen postpromotion checker did not
reverse S-CASE-08's six-case safety observation before reconstructing the
callback's older preimage. The
[second DX-06](../verification/adapt15-final-source/dx06-acceptance.json)
(SHA-256 `14cd296c347995774d939874e259a9f4fc469eb84263a6b4aea32b61bfaffc28`)
and [second six-case](../verification/adapt15-final-source/acceptance.json)
(SHA-256 `6813c93fe5cbfb7f474f84ff9d3c451245013384f7024c2475d72fc0ecc540f7`)
compiled controls also passed, but the second frozen full checker compared
ADAPT-15's original safety source before reversing its own appended six-case
safety observation. Neither incomplete historical checker is the final
source-bound validation; their receipts and earlier evidence have not been
rewritten. The final gate binds both earlier receipt generations and corrects
the chronological source projections without changing accepted native,
production or producer receipt bytes.

Fresh pinned offline all-features tests of noble-kernel, noble-contracts,
noble-wasm and noble-cli in external target
`/home/brittonr/.cache/noble-proof-tmp/adapt15-rust-suite-20260930/target`
passed **257 tests across 32 runs**, with zero failed/ignored/filtered and
zero compiler warnings, including eight kernel compile-fail doctests.
All three safety Cairn changes remain ACTIVE; full scoped Octet and
signed+sandbox Nix assurance, Phase3 staged closeout and archive remain
blocked/unclaimed. Finite compiled controls and Cairn/document validation
do not establish a universal checker/host theorem, owner-law or Rust-to-Lean
refinement. The previously reported whole-workspace boundary failure was
not rerun.

### WI-03 checked-u64 current-source release

The [WI-03 original-source receipt](../verification/wi03-final/acceptance.json)
(SHA-256 `38460dc2de15b1e0cd9f5d9bb4636edba4f3c9b78502066f14f7fe083914109d`,
source `sha256:1443691b20888c4a8aaa20bb59dd7fb572c060457d5f40dc697898d0a26d1fa9`)
passed eleven compiled controls on 406 bound source files. Its **frozen
original postpromotion checker FAILED**, however: it required the native
WI-WIT-02/06 requirement suffix after Cairn-generated links to remain
byte-identical even though the newly authored WI-03 scenario was correctly
placed there. The historical runner, producer receipt, raw output and
failure were not rewritten or reported as a pass. The separate
[corrected current-source checker](../verification/wi03-current-source/postpromotion.mjs)
instead preserves the unchanged generated links byte-for-byte, proves the
exact authored requirement deltas and deterministic native/view rendering,
binds each changed production file to the immutable WI-03 producer source,
and reverses the canonical WI-03 row plus earlier chronological replays.

The new [DX-06 receipt](../verification/wi03-current-source/dx06-acceptance.json)
(SHA-256 `6d7fb9640d9804cda34e082aee33cabd8c52b77b8e5f46869c68230f5ed7380d`,
source `sha256:ac908a1dfb0f801a91fe40e79bde5dfc1a4e0d65b98603d3ea461029c210312b`,
671 files, seven commands, raw output
`/home/brittonr/.cache/noble-proof-tmp/wi03-current-dx06-20260930-ac908a-1`)
and subsequent [six-case receipt](../verification/wi03-current-source/acceptance.json)
(SHA-256 `c91596d589424209adec87b3f624ec5a253748af9e5222a573f28bf9428ea248`,
source `sha256:0d21aadeb6c29dda2c81ac37d518ffbbb6d2569a8b45b1a6aae114cb6649cc70`,
672 files, eighteen commands, raw output
`/home/brittonr/.cache/noble-proof-tmp/wi03-current-replay-20260930-0d21aa-1`)
each passed with zero command/integrity failures; the six selected cases are
DX-01, ADAPT-01 and S-CASE-01/03/05/14. Both source-frozen postpromotion
checks passed at their respective chronological checkpoints. Standalone
`--dx06` requires the pre-six-case canonical state and passed before the
six-case promotion; it is not a post-six final-state check. The full
postpromotion checker passed after the six-case promotion and first reverses
those six new observations in memory before rechecking DX-06. Each new
canonical observation is at evidence index **12**
(length 13), with the earlier ADAPT-15 evidence at index 11 unchanged.
Fresh pinned offline four-crate all-features tests passed **260 tests across
34 runs**, zero failed/ignored/filtered and zero compiler warnings, including
eight kernel compile-fail doctests, in external target
`/home/brittonr/.cache/noble-proof-tmp/wi03-rust-suite-20260930/target`.
The four relevant Cairn changes remain ACTIVE (eleven ACTIVE overall), and
WI-03's final Stage2 task remains unmarked. Proof, source-body nonentry,
full scoped Octet/Clippy and signed+sandbox Nix assurance, staged closeout
and archive are not claimed; previously reported boundary failures were not
rerun. These finite compiled observations do not prove universal checker,
host, compiler or Rust/Lean refinement.

The subsequently corrected WI-WIT-02 native requirement now names the actual
parser/resolver path: parsed `RawType::U64` resolves to distinct boundary
`Type::CheckedU64`, not a nonexistent resolved `Type::U64`. The change was
synced into the native spec and generated WIT-WASI view. This source drift
leaves the original WI-03 producer and the `wi03-current-source` DX-06/six-case
receipts as **immutable historical finite passes**, not current-source replay
or proof for the corrected native source. The first
[corrected-native DX-06 finite receipt](../verification/wi03-final-source/dx06-acceptance.json)
(SHA-256 `7df51fd4f663c6727cd2c6837282df286c16276833b242a67a0d8ad10bb4887f`,
source `sha256:801bf6a07c3f16efa3c147cdafcd880e629205a3973d1c100484325bb7e83607`,
677 files, seven compiled commands) also remains historical: its frozen
postpromotion checker **FAILED** because it tried to reverse the earlier
six-case evidence in the shared developer-experience file before removing
its later DX-06 observation. Its runner, raw output and canonical evidence
index 13 remain unchanged; its finite compiled PASS is not promoted to a
source-bound final checker PASS.

### WI-03 corrected-native final source release

The separately frozen
[DX-06 release receipt](../verification/wi03-release-source/dx06-acceptance.json)
(SHA-256 `8d49776c1789641f349a71ec65b006d62f2ebcbb1a1772e318d6a3c85f6b3bd6`,
source `sha256:77f368e93b594773266d33f98b3d70b239c4391fceb625580dbb38513157884c`,
682 bound files, seven commands, raw output
`/home/brittonr/.cache/noble-proof-tmp/wi03-release-dx06-20260930-77f368-1`)
passed its compiled controls and standalone chronological postpromotion
checkpoint with DX-06 evidence index **14** (length 15). The following
[six-case receipt](../verification/wi03-release-source/acceptance.json)
(SHA-256 `39995d3446f3a331f0c90b485ad5c5b44a881faa2b20d5dd6e1b44a3d0606f47`,
source `sha256:2b86ecf02a637ed53bbd101da3ef40dc964ed41c115fe0e3747590523148356b`,
683 files, eighteen commands, raw output
`/home/brittonr/.cache/noble-proof-tmp/wi03-release-replay-20260930-2b86ec-1`)
passed selected DX-01, ADAPT-01 and S-CASE-01/03/05/14 controls with each
new evidence at index **13** (length 14). Both gates report zero command or
integrity failures. The
[final full checker](../verification/wi03-release-source/postpromotion.mjs)
passed after six-case promotion, reversing the latest six observations in
memory before source-bound DX-06 recheck. Standalone `--dx06` is an earlier
pre-six checkpoint, not a post-six mode; it was not rerun unchanged after
the six-case append.

This release binds the corrected `RawType::U64`/`Type::CheckedU64` WI-WIT-02
authored delta, regenerated native/view and unchanged WI-WIT-06, exact
reviewed textual reversal to the immutable WI-03 producer proposal/design/
tasks/delta, the previously accepted 6d7/c915 receipts, and the first
7df51 historical finite receipt/runner, plus chronological case-file
preimages. No prior receipt, checker or evidence row was overwritten.
The previously observed offline four-crate suite passed **260 tests over
34 runs** with zero failed/ignored/filtered and zero compiler warnings.
All **378** checked four-crate source plus Cargo manifest/lock hashes and
the pinned Rust selection are identical across that observed run's source
receipt and the latest compiled release receipt (mapping SHA-256
`17ddf906769523113fc070e80e2088b0002933817de59857b422d5e5bea410bd`);
unchanged production was not retested merely for native prose correction.
Four relevant Cairn changes remain ACTIVE among eleven ACTIVE overall,
WI-03's last Stage2 task unmarked. Full scoped Octet/Clippy and
signed+sandbox Nix assurance, source-body nonentry, proof, staged closeout
and archive remain open/unclaimed; prior whole-workspace boundary failures
were not rerun. Selected compiled and document validation do not establish
general checker/host/compiler or Rust-to-Lean refinement.

### Diagnostic join and named-proof current-source replay

The subsequent scoped `noble-contracts` Diagnostic join and `noble-cli`
named-proof changes made the previous corrected-native release a historical
source scope. The new independent gate pins the exact previous and frozen
SHA-256 of **thirteen edited production source files plus one contracts test
file**, verifies every other file from that prior 683-file source map without
exception, and reverses the three canonical case files' evidence in exact
chronological order. The prior
[corrected-native DX-06](../verification/wi03-release-source/dx06-acceptance.json)
and [six-case](../verification/wi03-release-source/acceptance.json)
receipts, raw outputs, runner bytes and failed earlier checkers remain
immutable. No WI-03 case evidence was newly promoted.

The [new DX-06 receipt](../verification/diagnostic-join-source/dx06-acceptance.json)
(SHA-256 `288e78503d71c0a3c01280aed1a7c9f7bf38cc1e61d7d8eb53cb03b1321ab1a0`,
source `sha256:0d97f4cee772e1be2228dac84e056b75c007e9b8b4b227a78e7759249c2c6e3a`,
688 bound files, seven retained commands, raw output
`/home/brittonr/.cache/noble-proof-tmp/diagnostic-join-dx06-0d97f4-1`)
rebuilt the real CLI and repeated six compiled scripted `test.clock`
controls, latent effect reflection, and pure/effectful import and
forged/trusted admission contrasts. It appended DX-06 evidence index **15**
(length 16), then the standalone source-bound `--dx06` checker **passed
before** the six-case append. The
[new six-case receipt](../verification/diagnostic-join-source/acceptance.json)
(SHA-256 `eee52b5ee98db5de2b9a3cd24793f4ebaed3cbcab51b876e9852fc6a837672dc`,
source `sha256:e7476c64c8f702ef5890d72815a90da12ee60a2764e94086d1d71dfc561af887`,
689 bound files, eighteen retained commands, raw output
`/home/brittonr/.cache/noble-proof-tmp/diagnostic-join-replay-e7476c-1`)
rebuilt the production CLI and independent diagnostic, safety and handle
peers; the selected DX-01, ADAPT-01, S-CASE-01/03/05/14 each appended
evidence index **14** (length 15). The full source-bound checker runs after
the newer six-case append, first reversing that append in the overlapping
developer-experience file before checking DX-06 and earlier receipts. No
later standalone `--dx06` run is substituted for that chronological check.

The scoped owner separately reported **88 contracts tests**, CLI
smoke/core/build/bin checks, a real `verify-module` named-proof
`independent_recheck` and cross-version refusal, and a full pinned offline
workspace all-target, all-feature `cargo clippy ... -D warnings` PASS.
These are not the selected seven/eighteen-command receipt assertions or
formal proof. Four relevant Cairn changes remain ACTIVE among eleven ACTIVE
overall; WI-03's last Stage2 task remains unmarked. Scoped Octet, full
signed+sandbox Nix assurance, source-body nonentry, staged closeout and
archive remain open. Historical whole-workspace boundary failures were
not rerun; finite controls do not prove universal checker, host or
Rust-to-Lean refinement.

### Kernel overflow current-source replay (source-only)

The kernel dependency/schema work-accounting fix and the defensive bounded
generic-parameter arithmetic, together with the existing `README.md` budget
sentence, are an exact three-file source bridge from the immutable
diagnostic-join receipts. The new
[DX-06 receipt](../verification/kernel-overflow-source/dx06-acceptance.json)
(SHA-256 `b6d70e68c3ef53f847412960fff2f72107c82b60f5f8053f689a985726554e02`,
source `sha256:629f5c0848d62100dd0cc3bb973596e7dec1de552402cd80e956dc25ebd962fb`,
694 bound files, seven retained commands) repeated the six compiled
`test.clock` positive/negative controls and pure/effectful import and
admission contrasts. Its canonical DX-06 evidence index **16** (length 17)
passed standalone postpromotion before the six-case append.

The [six-case receipt](../verification/kernel-overflow-source/acceptance.json)
(SHA-256 `03a1f1301f6dba56b6ef72773b0bd6c30ed6ab676f15ec0525d7e0ea65c8aafd`,
source `sha256:96e8bd2c70aa65f2fa41715564dda770c5693f90c947a9c6b9c9feff7193043b`,
695 bound files, eighteen retained commands) repeated the compiled and
independent DX-01, ADAPT-01 and S-CASE-01/03/05/14 controls. Each appended
evidence index **15** (length 16); the real postpromotion checker passed
after restoring all three exact case-file preimages and rechecking the
prior DX-06 receipt. Both raw outputs remain in their separate external
`kernel-overflow-dx06-629f5c-1` and `kernel-overflow-replay-96e8bd-1`
directories. The earlier external DX-06 attempt (`33a6152a…`) passed
finite controls but **failed postpromotion**; it is retained only as
historical rejected evidence and is not referenced by a canonical case row.

This is source-only finite acceptance, not a selected Octet pass, full
signed+sandbox Nix assurance, proof, universal refinement or Cairn
closeout. The eleven Cairn changes remain ACTIVE. In particular these
controls do not exercise `generic_arm` or `generic_matcher`.

### Optional post-gate Bend/qcue design studies

These are **new separately gated studies**, not additional completion criteria
for already accepted M3, M6, MC2, M4's DX-10 or DXM1. The owner-law
[VC-OWNER-01](PROGRAM-CONTRACTS.md) challenge (CONTRACT-16) starts from an
owner-frozen expected pure MC2 theorem/interface/assumptions *before*
candidate edits, then independently binds the actual submitted
program/recipe, generated statement, checked proof and release artifact.
Proof-required policy rejects changes to any binding, while normal
well-typed execution remains proof-optional. No owner-approved immutable
pre-edit law or independent host selection exists yet, so CONTRACT-16 remains
open. The qcue-inspired optional
[DX-13 review](../verification/dx13/acceptance.json) passed only its
pinned-source methodology review: ten finite-model/negative/replay/preservation
controls and five exclusions. No actual Noble oracle or model comparison ran;
future bounded model agreement would be neither universal proof nor release
authority. Historical DX-10 is separate. This does not reopen MC2's
already executed exact-statement, proof/admission or artifact controls.

The selected finite Result-Library-Draft
[DX-04](conformance/developer-experience-cases.json) slice
[passed its own source-bound gate](../verification/result-library/acceptance.json):
versioned `result@1` generic Data variant and four ordinary rank-1 signed
combinators, eight canonical paths off/on, three static negatives, separate
Resource capture refusal and four independent hostile Env controls, including
Program/Syntax first-class payload positives. Selected-arm callbacks retain
conservative effects and complete owner accounting; resource-bearing payload
positives and first-class universals remain unimplemented. MC2 proof admission
and the VC-OWNER-01 owner law remain separate and OPEN. This is not an IO monad
or implicit propagation syntax.

Post-M3 [ADAPT-17](conformance/adaptation-cases.json) compares finite ADT
layout, ANF and call lowering on selected managed-linear-memory Wasm with the
same checked post-compilation workloads, configuration/limit pins, output,
ordered trace and observable recipe identities. It records allocation,
quota, trap and cleanup outcomes and separates guest logical charges from
retained/engine memory. Any future resource/component-positive trial must
add independent WIT ownership/ABI/retirement evidence. M3's completed
WasmGC-versus-managed-memory result is unchanged.

Only **after** the required branch/schema/interface gates, ADAPT-18 may
study explicitly independent finite resource-free pure fork/join with
precharged bounds, left-then-right results and deterministic abnormal-outcome
precedence. Unknown cost, shared owners, effects, native pins and inadequate
reservation reject before branch start. It selects neither guest spawn nor
GPU backend, changes no M6 sequential import behavior and cannot count
unsettled work as cleaned up. The independent study has no new milestone
edge or release deadline; each case needs its own execution evidence before
any measured-result claim.

## Octet contract delivery

[OCTET-ADOPTION.md](OCTET-ADOPTION.md) records the inspected revision and contract boundaries. Its reference revision is not a selected Noble toolchain pin.

M5 adds H-AUTH-01–04 and H-RECEIPT-01–02 to the implemented host/component subset. Denial creates no witness or protected operation. Successful admission consumes a one-shot witness, and success receipts require applicable observations. The API design accounts for resource ownership on every result branch.

M6 and MW2 require DX-PROTOCOL-03 for their complete declared lifecycle domains. The small OCTET-05 coverage matrix is a control fixture, not full async coverage. MW1 adds nominal constructor and resolved-effect controls under DX-TYPE-03/04 and DX-MODULE-04. Typed indirect calls retain their trusted effect bounds.

OCTET-08/09/10 supply M1 policy and evidence controls. OCTET-01 through OCTET-04 supply M5/MW1 protected-effect controls. OCTET-05 applies to M6/MW2 lifecycle gates. OCTET-06/07 apply after the existing MW1 language gates. Each scenario needs real execution evidence before its corresponding implementation claim.

These obligations preserve the existing milestone dependencies and the mandatory Charon → Aeneas → Lean route. They add no agent syntax, general refinement inference, or required Verus migration.

## Optional execution backends (unaccepted profiles)

The distinct [explicit live-reference issue #1 design](../.cairn/changes/explicit-live-references/proposal.md)
selects an opt-in `Live-Slot-Design` only: a checked generic caller borrows an
invocation-input `LiveRef` and dispatches from one pinned immutable registry
epoch per root. It neither redirects ordinary saved `[ n ]` nor amends guarded
`Live-Wasm-Draft` reload. [LSLOT-01..09](conformance/live-reference-cases.json)
are design cases, all absent/not-run with proof open and trust unassessed.
No implementation, milestone completion, acceptance or source-bound release
is claimed; independent review precedes any promotion.

The [MLIVE milestone](roadmap.json) follows M4 and DXM1 and is specified by
[the live namespace contract](../.cairn/specs/language/spec.md),
[the backend boundary](BACKEND-EXPERIMENTS.md) and its
[active change](../.cairn/changes/live-wasm-reload/proposal.md). It proposes
an opt-in persistent WebAssembly bytecode VM REPL/watch with atomic next-call
source reload and narrowly host-granted guest self-edit. A guarded resident
Node/V8 REPL and in-process Wasm byte emitter support Core-only atomic rebuild
for tested acyclic dependents under explicit host-selected file reload/direct
admission. Automatic watch is unavailable: a directory rename notification
does not authenticate the inode's bytes at rename time, and a writable mapping
or external hardlink can alter them without a selected-basename content event.
Use explicit `:reload`; automatic H-LIVE-01 and complete LIVE-05/watch
acceptance, including declared modules, effectful watch, full in-flight
source-change controls and code retirement, remain open.
Bounded guest self-edit refuses any current effective dependent rather than
rebuilding it. Indirect self-rebind/cycle refusal was exercised in scoped
live-child checks; general proof and canonical LIVE case acceptance remain
open. The guest may enqueue
a checked pure Program candidate during Wasm execution, but only host-side
post-return independent admission and a generation/definition-identity CAS
may publish it. Guest publication leaves the selected file unchanged; LIVE-09
and LIVE-10 remain unexecuted designs. The first target reuses selected
Node/V8, which may JIT and
does **not** establish strictly interpreted execution. A genuine interpreter
engine remains separately unselected. The [MCB milestone](roadmap.json) follows
M4 only and is specified by [the C11 AOT contract](C-BACKEND.md) and its
[active change](../.cairn/changes/c-backend/proposal.md); native execution
requires a pinned sandbox and separately checked source/C/ELF admission.
Full watch-case acceptance, declared-module snapshots, complete source-bound
LIVE-02 acceptance and code retirement remain open despite the guarded
Core-only rebuild.
Neither complete opt-in backend profile is accepted: all
[LIVE](conformance/live-wasm-cases.json) and
[CB](conformance/c-backend-cases.json) cases are absent/not-run and proofs
open. A guarded partial live REPL implementation is not LIVE acceptance.
Ordinary Core `run`/`session`, selected Wasm/Node-V8 and historical M4
evidence remain unchanged; these designs inherit no M4 execution claim.

## Estimates

Initial budgets for one engineer are 1–2 days for M1 and 3–5 days each for the M2/M3 feasibility experiments. These are investigation budgets, not completion promises.

M4 implementation is present; completion depends on its retained gates rather
than the initial feasibility estimates. Full checker metatheory, backend
correspondence, MC2 and the platform profiles do not have credible completion
dates yet.
