# Implementation roadmap

Revision: 0.1.0-draft.5

**Next: create the Rust/Nix workspace with the required quality gates.** Noble starts from scratch. No old repository or test run is assumed.

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

Before MC2 acceptance, execute the [contract scenarios](conformance/contract-cases.json) with actual implementations. Reject changed captures, forged certification, missing composition implications, false preconditions, incomplete proofs, and unrelated Wasm. No candidate body runs on a failed admission path.

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
| M6 — Native async | M5 | Complete async ownership contract, pinned ABI matrix, stream/future/cancellation cases |
| M7 — Syndicate profile | M5 | Normative dataspace/facet model, bounded Preserves adapter, WIT boundary, and executed first service scenario |
| M8 — Optional higher layers | M7 | Choreography projection or durability contracts with separate acceptance evidence |

If M7 uses native async, that slice also depends on M6. No full concurrency claim can rely on unspecified async behavior.

## Typed-worker conformance workstream

[WORKER-CONFORMANCE.md](WORKER-CONFORMANCE.md) connects kernel and host contracts without new agent syntax. Its cases remain unexecuted harness designs.

| Milestone | Depends on | Exit criterion |
|---|---|---|
| MW1 — Worker interfaces and admission | M4, M5 and explicit language/service gates | Generated worker admission, typed dispatch, round trips, hostile-input rejection, bounded execution, and authority-separated observations |
| MW2 — Worker cancellation slice | MW1, M6 | Actual worker/shell execution, both cancellation/delivery orders, late callbacks, and accounted native retirement |

MW1 requires a worker-interface checker, declared schemas/modules, an explicit preparation service, package encoding, and a bounded execution profile. These are open entry gates, not completed capabilities. MW2 additionally requires concrete native-async task interfaces. M2 establishes finite candidate limits in its own subset. M6 owns RA-ASYNC-02 through RA-ASYNC-05 regardless of the worker harness.

The MW1 case set is WORKER-02 through WORKER-07 and WORKER-09 through WORKER-12. Async-dependent rows in WORKER-09 remain MW2 obligations rather than false MW1 coverage. MW2 executes WORKER-01 and WORKER-08 plus those deferred rows. Reports retain a result for every applicable matrix entry.

Neither milestone blocks M1 through M4, MC1/MC2, or the calculator. These tests do not select worker scheduling, task leases, distributed retries, or durability. A completed single-worker slice does not close M7 or M8.

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

The component probe is nonblocking for resource-free M3. M5 owns complete boundary conversions and independent-peer evidence. M6 retains the async ownership gate.

A negative result is useful: unsupported extraction, an unsuitable ABI, or broken dynamic composition must remain visible. A negative result alone cannot close a milestone.

## Developer-experience delivery

[SPEC-DX001](DEVELOPER-EXPERIENCE.md) defines the scoped acceptance gates. M2 requires DX-DIAG-01 and the three DX-01 negative diagnostic cases. Resource-shaped checker inputs do not require live resources.

Editor holes follow M2 and require a separate editor syntax contract. Guest domain types and Result library interfaces follow M4; they do not block bootstrap. Resource-bearing library execution also requires the ownership profile.

Identity tools require resolved recipes. Portable cache keys additionally require canonical encoding; proof reuse follows SPEC-V002 evidence admission. Resource-free test hosts can use the M2 environment; live-resource substitution waits for M5.

M2 also requires bounded property-runner controls and compiler-backed static documentation checks under DX-PROPERTY-01/02 and DX-DOC-01/02. M4 extends these to Wasm composition, recipe, and execution observations. The DX-10 and DX-12 designs define required positive and negative harness controls; document checks do not execute them.

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

Re-estimate M4 after the experiments. Full checker metatheory, backend correspondence, and the platform profiles do not have credible completion dates yet. Estimate MC1 and MC2 after M2 establishes the Lean model and extraction costs.
