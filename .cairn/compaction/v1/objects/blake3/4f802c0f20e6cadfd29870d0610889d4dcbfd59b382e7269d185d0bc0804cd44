# M2 checker feasibility design

## Context

The repository has twelve accepted draft specifications, working document validation, a pinned Charon → Aeneas → Lean toolchain, and one extracted smoke function (`noble_kernel::consume_budget`).
No type checker, candidate format, reference model, or refinement proof exists.
[M2 in the roadmap](../../../specs/ROADMAP.md) requires candidate schema, finite rules, actual extraction, one nontrivial refinement, and failing incomplete-coverage checks.
The language specification fixes the declarative judgment `Gamma ; Constraints |- body : S -- T ! e` and its representative rules (EMPTY, SEQUENCE, QUOTATION) in `.cairn/specs/language/spec.md` section 8.1.
The verification specification fixes the acceptance theorem shape (V-CHECK-03) and the obligation routes for the checker (PO-09, PO-10, PO-11).
The bootstrap specification fixes the finite checker boundary and its limits (B-CHECK-01 through B-CHECK-07).

## Goals and Non-goals

M2 delivers the named finite checking fragment, the actual acceptance representation, one extracted nontrivial validation function, the Lean reference model, the first fragment refinement theorems, and the gates that reject incomplete coverage.
M2 does not deliver inference, generalization, recursion, resources in the live sense, evaluation, Wasm, or whole-kernel verification.

## Decisions

### Decision: Check a named finite fragment, not the whole language

**Choice:** The M2 fragment supports exactly the three bootstrap node forms (literal, resolved invocation, quotation) with finite stack-tail constraints, the bootstrap word contracts, and the five-way outcome domain. Its exact contents are enumerated below and every omission is listed.

**Rationale:** B-SCOPE-01 fixes this subset; V-MODEL-01 permits a smaller mechanized bootstrap subset only with enumerated omissions; V-GATE-05 requires narrow results to retain their labels, so the fragment is named everywhere ("M2 fragment v0").

**Fragment v0 contents**

Types: `Unit`, `Bool`, `I64`, `Text`, `Pair<a,b>`, `Sum<a,b>`, `List<a>`, `Program<S,T,e>`, inert `Syntax`, and the opaque `Resource<k>` constructor for negative eligibility fixtures only (no resource operation is admitted).
Stacks: finite ordered lists of types; schemes quantify over stack variables, value-type variables, and effect variables; all quantifiers are rank-1 and the variable set of every scheme is finite.
Nodes: literal (`I64`/`Bool`/`Text`/`Unit`), invocation (`DefId` + total instantiation substitution), quotation (a finite body sequence; the body is checked even when unused).
Environment contracts (v0): `dup`, `drop` (both requiring `Data`), `swap`, `dip`, `+`, `-`, `*` (wrapping `I64`), `quote` (requiring `Data`), `compose`, `run`, `reflect`, `unit`, `pair`, `unpair`, `inl`, `inr`, `case`, `if`, `nil`, `cons`, `list.case`, and the supplied resource-free `test.emit` carrying its concrete effect identity.
Effects: finite sets of host-operation identities; union is associative, commutative, and idempotent; construction and inspection produce none of their operands' latent effects; invocation propagates the instantiated bound.
Eligibility: `Data` is a recursive predicate; `Resource<k>` is non-`Data` in every position, including inside `Pair`, `Sum`, and `List` payloads; `quote` and the duplication/discard words require it, and a `Sum<Resource<k>,I64>` remains non-capturable.
Limits (B-CHECK-07): input bytes, decoded nodes, nesting depth, dependency traversal, type and stack size, checking work, memory (bounded through node/type/work limits), and diagnostic output; each limit has a boundary and an exhausted control.
Outcomes: `accepted`, `invalid`, `unsupported`, `exhausted`, `internal-failure`; only acceptance yields a checked program value; rejection issues no candidate-body host request and leaves session state unchanged (B-RESULT-01, B-RESULT-02).

Enumerated omissions (V-MODEL-01): inference and unification (candidates are supplied), generalization and recursive definitions, module imports, preparation services, live resources and adapters, evaluation semantics and recipes, Wasm lowering and execution, concurrency, contracts, and the calculator.

### Decision: Acceptance checks instantiated interfaces, not inference

**Choice:** A candidate carries each invocation's substitution witness; the checker verifies that the substitution is total and bounded, that applying it to the environment scheme yields exactly the node's claimed interface, and that sequence joins and effect/eligibility constraints hold on the instantiated interfaces. Freshness is enforced by requiring each node's interface to be exactly its own instantiation (no generalization of an instantiated interface is admitted).

**Rationale:** K-CHECK-06 puts instantiations and witnesses in the candidate; B-CHECK-03 requires a declarative derivation per node with fresh instantiation per named use; B-CHECK-02 binds exact environment identities. This keeps the acceptance rules finite and decidable while the unproved inference algorithm stays a separate, later target (K-CHECK-02 follow-up work, V-CHECK-02).

### Decision: Domain-specific Rust types at every boundary

**Choice:** `noble-kernel` separates untrusted candidate data, decoded candidates, checked interfaces, and accepted programs with distinct types; constructors and decoders never produce an accepted program; byte views live only in the shell-side decode adapter (B-IMPL-03). The kernel stays `no_std` + `alloc`, dependency-free, and inside the confirmed extraction subset (owned data, simple control flow, no I/O).

**Rationale:** B-IMPL-03 and VT-SCOPE-02; the M1 boundary controls already reject host types and effects in kernel code.

### Decision: Reference model with the required theorem shape

**Choice:** `proofs/m2/` defines the Lean reference model: the fragment's types, schemes, eligibility, effect sets, candidate/request data, the declarative judgment `Derives` with EMPTY/SEQUENCE/NODE/QUOTATION rules, and a reference decision function `check`.
The M2 theorem set is: (a) `check` soundness — acceptance implies well-formedness and a derivation (V-CHECK-03 shape, PO-09 for the fragment); (b) `check` termination on the finite fragment and positive acceptance coverage — a structural measure plus derivable-input acceptance (PO-10 for the fragment); (c) refinement — the extracted Rust functions compute the same outcome as `check` on the fragment, so acceptance of the extracted checker implies a derivation (PO-11 for the fragment).

**Rationale:** V-CHECK-03 fixes the statement shape; V-CHECK-04/05 require outcome distinction, termination, and positive coverage; PO-11 is the Aeneas-lean route for the actual checker; SOUNDNESS alone is insufficient for a usable-checker claim (V-CHECK-05).

### Decision: Refinement binds generated functions to the contract

**Choice:** The refinement theorem is stated about the Aeneas-generated functions in the extraction record (same record bindings as M1: source blake3, inventory identity, selection, tool revisions), not about a rewritten Lean copy. A proof-required claim without a building theorem target, with a proof hole, or about a substituted subject is rejected by the gate.

**Rationale:** B-IMPL-02 and VT-AENEAS-04; M1 established the binding machinery and its rejection controls; the gates extend that record to the M2 functions.

### Decision: Extend the gates with separate statuses and refusals

**Choice:** The reviewed source classification lists every M2 kernel function with separate extracted, proved, modeled, excepted, and open status.
The gates reject: a missing function (inventory/coverage comparison), an unsupported kernel body (extraction refusal on a checker-function fixture), an unexplained external model, an unfinished required refinement (proof-required claim), and an unexecuted required coverage case.

**Rationale:** VT-SCOPE-05, VT-CI-05, ROADMAP M2 exit criteria; M1 built the compiler-derived comparison and extraction refusal controls that this extends.

### Decision: Bounded property and documentation controls accompany M2

**Choice:** A bounded property harness generates well-typed candidates, passes each through independent acceptance, records subject, property, generator/shrinker revisions, seed, bounds, environment, and outcomes, shrinks with a bound while preserving the failure predicate, and keeps malformed-candidate fuzzing in a separate lane (DX-PROPERTY-01/02).
Static documentation examples bind source, expected observations, revision, build context, and host configuration; they run the actual checker, keep failures and unsupported cases visible, and distinguish illustrative text from executed evidence (DX-DOC-01/02).
Diagnostics identify the failing word or join, required and actual stacks, and the violated effect or eligibility constraint, and report unavailable provenance instead of inventing it (B-DIAG-01, DX-DIAG-01); the three DX-01 negative cases are covered as controls.

**Rationale:** ROADMAP requires these controls for M2; none of them may substitute for the refinement theorem.

### Decision: Evidence and claim-ledger updates stay honest

**Choice:** `specs/STATUS.json` component fields and the `proof_implementation_exists` gate flag change only when the corresponding evidence exists; `specs/verification/obligations.json` records for PO-09, PO-10, and PO-11 accept only the exact fragment claims with bound evidence fields (claim, subject, revision, kind `lean-kernel` / `aeneas-lean`, result, source revision, configuration, assumptions).
All other obligations remain open.

**Rationale:** EV-BIND-01, EV-TIER-01, V-CLAIM-01, V-EVIDENCE-04; the validator rejects an `accepted` obligation while the greenfield flag is false, so the flag and the records move together.

### Decision: Comply with the full deny-all catalog in the checker's style

**Choice:** The checker, its environment, and its tests conform to every cataloged Tiger-style rule with no waiver and no budget: modules of at most 300 lines, short functions split at phase boundaries, no self-recursion (explicit bounded work stacks), no caller-owned mutation in pure functions (state transitions are returned), collection growth with explicit capacity or a local bound, fixed-width integers at public boundaries, no trait-less imports (fully qualified paths), no `unwrap`, `expect`, or `panic` in checked code, exhaustive enum matches, and compliant naming (acronyms, boolean predicates, no path-word repetition).

**Rationale:** M1 established the deny-all catalog without disabled lints, warning budgets, or finding baselines (VT-M1-05, VT-OCTET-01..03). The first extraction probe measured 293 catalog findings across the initial fragment modules and one incomplete architecture shard, so the feasible checker shape is the iterative, value-passing one the catalog demands — which also matches the Aeneas extraction subset (no mutable-argument generics, bounded loops, explicit state). The style pass is a redesign of the internals, not a scope reduction: every rule and limit control stays.

## Requirement Traceability

The [delta](specs/verification-toolchain/spec.md) contains every new task-linked requirement (VT-M2-01 through VT-M2-04).
The following map preserves every requirement assigned to M2 in `specs/roadmap.json`:

| Requirement | Where the change satisfies it |
|---|---|
| B-CHECK-06 | Fragment branch rules and join controls (tasks 2.2, 2.5) |
| B-CHECK-07 | Limits, outcomes, boundary and exhausted controls (task 2.4) |
| B-DIAG-01 | Diagnostic records and DC-01..03 controls (tasks 2.3, 5.3) |
| B-IMPL-02 | Extraction of the actual representation and validation function (tasks 3.1, 3.2) |
| B-IMPL-03 | Domain-specific types and decode boundaries (task 2.1) |
| VT-AENEAS-01 | Checker code inside the confirmed extraction subset (tasks 2.1, 3.1) |
| VT-AENEAS-04 | Refinement stated about the generated functions (task 4.4) |
| VT-SCOPE-05 | Per-function extracted/proved/modeled/excepted/open reporting (tasks 3.1, 6.1) |
| VT-CI-05 | Gate refusals and regeneration on change (tasks 3.3, 4.5, 6.1) |

Developer-experience controls assigned to M2: DX-DIAG-01 (tasks 2.3, 5.3), DX-PROPERTY-01/02 (task 5.1), DX-DOC-01/02 (task 5.2).

## Planned Artifacts

- `crates/noble-kernel/src/` modules: `types.rs`, `scheme.rs`, `candidate.rs`, `check.rs`, `diag.rs` (exact split decided during task 2.1).
- `crates/noble-kernel/tests/` fragment tests and `crates/noble-cli/` acceptance harness.
- `nix/`: extraction subject extension and the M2 gate wiring; `policy/source-inventory.ncl` and its checked export updated.
- `proofs/m2/`: Lake root, reference model modules, refinement targets, proof README.
- `verification/`: fragment specification, extraction record update, coverage record, evidence records, scope review update.
- `.pi/m2-*` retained evidence roots outside the worktree.

## Verification Plan

### Planning-package checks

Run the existing Bun conversion checks, regression tests, and document-validator self-tests.
Run the proposal, design, and tasks gates for `m2-checker-feasibility` with the explicit worktree root, then Cairn validation.

### Implementation checks

The workspace will expose, in addition to the M1 checks:

- `check-count` positive acceptance corpus: every accepted fixture yields a derivation record; every rejected fixture yields its named outcome and diagnostic.
- `limits`: boundary and exhausted controls for every declared limit.
- `eligibility`: `dup`, `drop`, and `quote` reject resource-bearing values; `Sum<Resource<k>,I64>` joins reject; resource payload elimination types without capture.
- `effects`: hidden `test.emit` in a quotation body rejects against an empty allowed bound; `run`/`case`/`if` propagate unions; construction stays empty.
- `extraction-m2`: the entry point regenerates the M2 functions; the record binds source, inventory, selection, and tool revisions.
- `refinement-m2`: the reference model builds; the soundness, termination/coverage, and extracted-refinement targets build under the pinned toolchain with no proof holes.
- Refusal controls: missing function, unsupported body, unexplained model, unfinished refinement, unexecuted required coverage case, and a hand-written substitute theorem each reject.
- Property and documentation controls per their decisions above.

### Acceptance sequence

Baseline and package gates; fragment and schema with controls; kernel implementation with tests; extraction and binding; reference model and refinement; gate integration and refusals; full control matrix; evidence records and scope review; then sync, archive, commit, push, and integration.

## Risks / Trade-offs

- Aeneas subset limits on the chosen data-structure encodings (owned vectors, sorted sets, recursion shapes) can force representation changes; the extraction probe in task 3.1 decides these before the proof is attempted.
- The reference model can grow beyond the fragment; every decision keeps the fragment label and the omission list explicit.
- Proof effort for completeness (PO-10) may exceed feasibility; soundness and termination are the required minimum, completeness is attempted and reported honestly if it stays partial.
- Lean backend build time repeats the M1 offline-input workflow; the M2 proof root reuses the same pinned backend and records its own source binding.
- The greenfield flags in `specs/STATUS.json` are shared state; they change only with the fragment evidence, and every other obligation stays open.

## Rollout and Recovery

Implementation proceeds in task order: fragment and schema, kernel representation, checker with controls, extraction binding, reference model, refinement, gates, property and documentation controls, then acceptance.
A failed probe leaves the change active with its exact diagnostic and configuration; no gate is weakened to make progress.
