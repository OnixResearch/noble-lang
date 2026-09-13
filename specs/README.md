# Canonical specification family

Revision: 0.1.0-draft.5

**Implement from [the Cairn specifications](../.cairn/README.md), not from the archived previews.** This is one canonical working family with explicit open contracts. It is not a stable language release.

The twelve normative documents now live under `.cairn/specs/`. Their old paths contain generated compatibility views, not separate authority. This directory retains the scenario designs, proof ledger, roadmap, status, and supporting documents.

The user confirmed that no prior implementation exists. Historical references to repository recovery and prior implementation work no longer describe current project state.

## Core and safety

| Document | Identifier | Role |
|---|---|---|
| [Language](SPEC-0001.md) | SPEC-0001 | Integrated language, safety, verification, and component commitments |
| [Bootstrap scope](CORE-BOOTSTRAP.md) | SPEC-B001 | First finite checker and Wasm acceptance target |
| [Backend experiments](BACKEND-EXPERIMENTS.md) | SPEC-BE001 | Required M3 comparison; no representation selected |
| [Safety](SAFETY.md) | SPEC-S001 | Mandatory safety invariants and claim boundaries |
| [WIT/WASI](WIT-WASI.md) | SPEC-W001 | Full target component profile |
| [Resource adapters](RESOURCE-ADAPTERS.md) | SPEC-R001 | First bounded synchronous resource adapter contract |
| [Developer experience](DEVELOPER-EXPERIENCE.md) | SPEC-DX001 | Scoped language ergonomics, libraries, module boundaries, protocol types, and development tools |
| [Exact calculator](CALCULATOR.md) | SPEC-CALC001 | First reference application: exact arithmetic, lexical functions, and bounded AI-authoring evaluation |

## Verification and evidence

| Document | Identifier | Role |
|---|---|---|
| [Verification](VERIFICATION.md) | SPEC-V001 | Reference model and proof obligations |
| [Toolchain](VERIFICATION-TOOLCHAIN.md) | IMPL-V001 | Whole-project Aeneas-first Rust, mandatory kernel refinement, and explicit external boundaries |
| [Program contracts](PROGRAM-CONTRACTS.md) | SPEC-V002 | Typed declarations, first-class evidence companions, composition, and verification tooling |
| [Evidence](EVIDENCE.md) | SPEC-EV001 | Fixture schema, independent status, and coverage rules |
| [Obligations](verification/obligations.json) | PO/SO ledger | Newly transcribed, all open |

## Decisions and work

1. [Decisions](DECISIONS.md) record selected changes and open questions.
2. [Roadmap](ROADMAP.md) gives an acyclic implementation order.
3. [Source map](SOURCES.md) records ancestry and missing historical material.
4. [Review resolution](REVIEW-RESOLUTION.md) links each finding to its disposition.
5. [Status](STATUS.json) records absence of implementation separately from spec and evidence status.

## Developer-experience amendment to draft.5

Select the bounded contracts in SPEC-DX001 and their unexecuted scenarios. M2 adds stack diagnostics; holes and guest declarations do not expand Core-Bootstrap. All implementation and proof status remains open.

The second group selects local names, capability-aware modules, property testing, resource protocol types, and compiler-backed documentation. Syntax and later-profile gates remain open. Property and documentation checks accompany M2/M4 without adding kernel mechanisms.

## Calculator amendment to draft.5

Select an exact programmable calculator as the first AI-authoring reference application. Its library uses arbitrary-precision integers and normalized rationals. Decimal input is exact, and division does not truncate. Noble's wrapping `I64` operators and bootstrap scope remain unchanged.

[CALCULATOR.md](CALCULATOR.md) defines expression precedence, numeric errors, lexical definitions, state preservation, budgets, and independent benchmark acceptance. MA1 through MA3 follow M4 and explicit library/tool gates. All [calculator scenarios](conformance/calculator-cases.json) remain unexecuted. Approximation and symbolic mathematics remain later extensions.

## Worker-contract amendment to draft.5

Tighten runtime-selected interfaces, explicit effect widening, recipe round trips, package dependencies, bounded admission, async ownership, and failure observations. These contracts add no agent-specific kernel syntax and do not expand Core-Bootstrap.

[Worker conformance](WORKER-CONFORMANCE.md) defines a generated pure worker plus an independently authorized shell action. Its [scenarios](conformance/worker-cases.json) include cancellation races, hostile packages, and exhausted budgets. MW1/MW2 retain separate implementation gates. All runtime and proof results remain absent/open.

## Octet contract amendment to draft.5

[OCTET-ADOPTION.md](OCTET-ADOPTION.md) maps inspected Octet contracts to Noble's existing owners. The amendment adds protected witnesses and observation-based receipts, nominal constructor invariants, explicit lifecycle coverage, and resolved effect admission. Rust architecture policy supplements the existing full deny-all lint gate.

[Adoption scenarios](conformance/octet-adoption-cases.json) retain unexecuted status. Octet source-shape checks and fixture-scoped analysis do not replace Charon → Aeneas → Lean or establish Noble runtime correctness. No new kernel syntax, toolchain pin, or implementation dependency is selected.

## Changes in draft.5

1. Select `Contracts-Draft`: typed contracts and first-class evidence companions without changing ordinary `Program<S,T,e>` execution.
2. Define checked composition, runtime family instantiation, and applicability guards. No runtime proof search is required.
3. Specify verification, explanation, and proof-required build operations with distinct claim outcomes and independent admission policy.
4. Add MC1/MC2, PO-19 through PO-21, and [contract scenarios](conformance/contract-cases.json). All implementation and proof work remains open.
5. Keep concrete contract grammar, companion representation, and portable encoding as explicit implementation-entry gates.

## Changes in draft.4

1. Require Charon → Aeneas → Lean for the entire semantic kernel and target the route across all Noble-owned production Rust.
2. Move resource-table decisions to the same route. Keep physical effects and synchronization behind explicit boundary contracts.
3. Replace the mandatory Verus lane with reviewed non-kernel exceptions. No exception is currently selected.
4. Require complete source inventories, extraction/proof coverage, and CI rejection of missing or unsupported required functions.
5. Keep implementation, pins, and proofs open. This revision does not establish Aeneas compatibility or a verified Noble kernel.

## Changes in draft.3

1. Require a WasmGC/managed-linear-memory comparison without selecting either backend.
2. Add native-boundary review, scoped Miri checks, and explicit decoding stages. Zerocopy remains optional.
3. Clarify branch joins, Rust domain types, resource cleanup, contract snapshots, and mandatory admission checks.
4. Add [adaptation scenarios](conformance/adaptation-cases.json), all unexecuted. [Source provenance](SOURCES.md) records the study and adoption boundary.

## Authority rule

Requirements in this family apply only to their declared subsets. A mismatch between two current documents is a defect, not permission to choose the easier rule.

The core owns language semantics. Profile documents add explicit boundary contracts. They cannot weaken core safety or turn a partial implementation into full-profile conformance.

The new revision preserves inherited requirement IDs. Reports must include the document revision, not only the ID. DEC-N001 consolidates decisions previously spread across the core, safety, verification, and WIT registers.

Syndicate/Synit with Preserves remains the selected concurrency direction. Its full normative profile is still open and does not block the core. Choreography and durability remain later, optional work.

## Validation

From the project root:

```sh
bun tools/cairn-specs.mjs --self-test
bun tools/check-specs.mjs --self-test
bun tools/cairn.mjs validate --root .
```

The validators check Cairn structure, compatibility views, document links, requirement preservation, fixture references, status fields, and roadmap dependencies. They do not parse or run Noble source.

[The Cairn workflow](../.cairn/README.md#edit-a-specification) explains how to update canonical documents and refresh the compatibility views.
