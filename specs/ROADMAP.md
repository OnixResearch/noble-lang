# Implementation roadmap

Revision: 0.1.0-draft.5

**M5, M6 and M7 are complete for their separately bounded component and
local-service profiles; M8 remains open.**
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

MC2 acceptance executes all [contract scenarios](conformance/contract-cases.json) and their declared variants against the actual implementation, including eighteen independent-checker/core controls. Changed captures, forged certification, missing composition implications, false preconditions, incomplete proofs and unrelated Wasm are rejected on their declared paths. The retained receipts distinguish strict semantic/source lemmas, renderer-layout obligations, closed native source equations and bounded runtime observations. Broad PO-16/20/21 and general backend/host correctness remain open.

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
fairness, transport, durability, M8 and universal compiler/engine
correspondence remain outside this accepted selection.

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
covered 25 units with 1,939 open authored-body obligations; the current M7
renewal covers 29 units with 2,006 still open. Historical milestone receipts
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

MW1 requires a worker-interface checker, declared schemas/modules, an explicit preparation service, package encoding, and a bounded execution profile. These are open entry gates, not completed capabilities. MW2 additionally requires concrete native-async task interfaces. M2 establishes finite candidate limits in its own subset. M6 owns RA-ASYNC-02 through RA-ASYNC-05 regardless of the worker harness.

The MW1 case set is WORKER-02 through WORKER-07 and WORKER-09 through WORKER-12. Async-dependent rows in WORKER-09 remain MW2 obligations rather than false MW1 coverage. MW2 executes WORKER-01 and WORKER-08 plus those deferred rows. Reports retain a result for every applicable matrix entry.

Neither milestone blocks M1 through M4, MC1/MC2, or the calculator. These tests do not select worker scheduling, task leases, distributed retries, or durability. M6's completed local WORKER-08 slice did not establish the separate M7 service acceptance, and the completed bounded M7 service does not close M8 or the broader worker milestones.

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

M2 uses domain-specific Rust types for identities and acceptance state. Wire views do not enter its initial owned-data core. Zerocopy adoption waits for a concrete format, a paired decoder experiment, and VT-NATIVE-03 review.

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

After M4, local bindings require a stack-only comparison plus lexical-scope, lowering, and capture-identity contracts. Capability-aware modules require explicit linking, operation contracts, and dependency identity rules. Neither changes bootstrap grammar.

Resource protocol types require M5 and guest declaration/module interfaces. Their gate covers successful transitions, error ownership, invalid reuse, runtime validation, and ambiguous outcomes. Behavioral proof claims also require the corresponding host models; MC1 remains resource-free.

These slices retain separate acceptance evidence. They do not add general handlers, dependent types, macros, or another concurrency model.

## Octet contract delivery

[OCTET-ADOPTION.md](OCTET-ADOPTION.md) records the inspected revision and contract boundaries. Its reference revision is not a selected Noble toolchain pin.

M5 adds H-AUTH-01–04 and H-RECEIPT-01–02 to the implemented host/component subset. Denial creates no witness or protected operation. Successful admission consumes a one-shot witness, and success receipts require applicable observations. The API design accounts for resource ownership on every result branch.

M6 and MW2 require DX-PROTOCOL-03 for their complete declared lifecycle domains. The small OCTET-05 coverage matrix is a control fixture, not full async coverage. MW1 adds nominal constructor and resolved-effect controls under DX-TYPE-03/04 and DX-MODULE-04. Typed indirect calls retain their trusted effect bounds.

OCTET-08/09/10 supply M1 policy and evidence controls. OCTET-01 through OCTET-04 supply M5/MW1 protected-effect controls. OCTET-05 applies to M6/MW2 lifecycle gates. OCTET-06/07 apply after the existing MW1 language gates. Each scenario needs real execution evidence before its corresponding implementation claim.

These obligations preserve the existing milestone dependencies and the mandatory Charon → Aeneas → Lean route. They add no agent syntax, general refinement inference, or required Verus migration.

## Estimates

Initial budgets for one engineer are 1–2 days for M1 and 3–5 days each for the M2/M3 feasibility experiments. These are investigation budgets, not completion promises.

M4 implementation is present; completion depends on its retained gates rather
than the initial feasibility estimates. Full checker metatheory, backend
correspondence, MC2 and the platform profiles do not have credible completion
dates yet.
