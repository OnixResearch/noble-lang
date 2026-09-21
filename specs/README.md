# Canonical specification family

Revision: 0.1.0-draft.5

**Implement from [the Cairn specifications](../.cairn/README.md), not from the archived previews.** This is one canonical working family with explicit open contracts. It is not a stable language release.

The twelve normative documents now live under `.cairn/specs/`. Their old paths contain generated compatibility views, not separate authority. This directory retains the scenario designs, proof ledger, roadmap, status, and supporting documents.

The repository now contains the bounded checker, experimental MC1 contract frontend,
M3 compiled-program representation experiment, and M4 source-to-Wasm
Core-Bootstrap implementation. M4 uses the selected managed-linear-memory backend:
source is resolved and inferred, independently accepted by the kernel, rechecked
for lowering, then executed as compiled Wasm. Persistent sessions retain compiled
Programs, captures, stack and immutable resolved bindings rather than replaying
source. Static refusals preserve prior state and make no candidate-body host
requests; runtime traps retain already observed request prefixes.

The implemented data/control scope includes wrapping `I64`, Bool/Text/Unit,
Pair/Sum/List, quotation/composition/execution, inert exact reflection and
bootstrap branch/stack operations. Only explicit resource-free test hosts are
available; resources, imports, recursion and runtime proof companions remain out
of scope. See the [CLI usage and evidence boundary](../README.md#running-core-bootstrap-source).
[Status](STATUS.json) and the [roadmap](roadmap.json) distinguish implemented
behavior from retained acceptance and open language/proof obligations. M4's
retained runtime acceptance includes the integrated developer workflows;
the [acceptance evidence](../README.md#m4-acceptance-evidence) separately records
the passed extraction, regression and quality gates. M5 is the next primary milestone.
Amendment sections below record design adoption,
not automatic completion.

## Core and safety

| Document | Identifier | Role |
|---|---|---|
| [Language](SPEC-0001.md) | SPEC-0001 | Integrated language, safety, verification, and component commitments |
| [Bootstrap scope](CORE-BOOTSTRAP.md) | SPEC-B001 | First finite checker and Wasm acceptance target |
| [Backend experiments](BACKEND-EXPERIMENTS.md) | SPEC-BE001 | M3 comparison; managed linear memory selected for the M4 direction |
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
| [Obligations](verification/obligations.json) | PO/SO ledger | Per-obligation acceptance, open scope, and evidence |

## Decisions and work

1. [Decisions](DECISIONS.md) record selected changes and open questions.
2. [Roadmap](ROADMAP.md) gives an acyclic implementation order.
3. [Source map](SOURCES.md) records ancestry and missing historical material.
4. [Review resolution](REVIEW-RESOLUTION.md) links each finding to its disposition.
5. [Status](STATUS.json) records implementation, execution, proof, and trust status separately.

## Developer-experience amendment to draft.5

Select the bounded contracts in SPEC-DX001. M2 adds stack diagnostics; holes and
guest declarations do not expand Core-Bootstrap. Implementation and execution
status is scenario-specific, not a consequence of adopting this amendment.

The second group selects local names, capability-aware modules, property testing,
resource protocol types, and compiler-backed documentation. Syntax and
later-profile gates remain open. Property and documentation checks accompany
M2/M4 without adding kernel mechanisms.

M4's retained runtime acceptance passed [DX-10's property harness](../verification/m4/property.mjs)
and [DX-12's executable-documentation harness](../verification/m4/documentation.mjs).
DX-10 passed 100 seeded Wasm composition/interface/recipe/effect trials,
100 replay trials and eight hostile controls with 50-step bounded shrinking;
its 200 malformed-kernel cases are a separate checker lane, not Wasm property
coverage. DX-12 passed the two declared examples and six hostile controls with
explicit resource-free hosts;
unrun, unsupported, failed and timed-out examples remain visible. A
whole-command timeout is not proof of guest entry. The same integrated run passed
all 18 CORE cases and 11 controls using a read-only binary snapshot after Cargo
tests, avoiding races with mutable build targets.

The gate emits scoped `property-workflow.json` and `documentation-workflow.json`
receipts alongside `acceptance.json`. The runtime evidence is retained at
`verification/m4/acceptance.json` and `verification/m4/runtime.tar.gz`, with
`verification/m4/evidence.json` recording the complete retained boundary.
The separate actual-source extraction authorities are the reviewed
`verification/m4/extraction-lock.json`, `verification/m4/implementation.json`
and `verification/m4/assurance.tar.gz`. Discovery produces a review candidate;
independent check mode re-extracts and compares source/generated identities,
inventories, dependencies and axioms against that reviewed lock. Neither
discovery nor runtime acceptance substitutes for that check, and no universal
refinement or all-documentation execution claim follows.

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
4. Add MC1/MC2, PO-19 through PO-21, and [contract scenarios](conformance/contract-cases.json). MC1 now has a bounded experimental implementation; MC2 and universal contract correctness remain open.
5. Keep concrete contract grammar, companion representation, and portable encoding as explicit implementation-entry gates.

## Changes in draft.4

1. Require Charon → Aeneas → Lean for the entire semantic kernel and target the route across all Noble-owned production Rust.
2. Move resource-table decisions to the same route. Keep physical effects and synchronization behind explicit boundary contracts.
3. Replace the mandatory Verus lane with reviewed non-kernel exceptions. No exception is currently selected.
4. Require complete source inventories, extraction/proof coverage, and CI rejection of missing or unsupported required functions.
5. Keep implementation, pins, and proofs separate from specification adoption. This revision alone does not establish Aeneas compatibility or a verified Noble kernel.

## Changes in draft.3

1. Require a WasmGC/managed-linear-memory comparison without selecting either backend.
2. Add native-boundary review, scoped Miri checks, and explicit decoding stages. Zerocopy remains optional.
3. Clarify branch joins, Rust domain types, resource cleanup, contract snapshots, and mandatory admission checks.
4. Add [adaptation scenario designs](conformance/adaptation-cases.json). Their ledger now records the scoped M3 execution of ADAPT-08/09/10/12; the remaining designs are unexecuted. [Source provenance](SOURCES.md) records the study and adoption boundary.

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
