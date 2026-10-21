## Phase 1: Contract

- [x] [serial] Map the declared M4 acceptance contract and existing M2/MC1/M3 components (B-SCOPE-01). r[B-GATE-01]
- [x] [serial] Validate the M4 proposal, design, delta specification and full task checklist before implementation. r[B-GATE-01]

## Phase 2: Implementation

- [x] [parallel] Implement bounded normative lexing/parsing and immutable name resolution for expressions and nonrecursive definitions (B-SCOPE-01). r[B-GATE-01]
- [x] [parallel] Reuse/generalize inference, preserve rank-1 named uses and monomorphic first-class programs, and emit finite untrusted candidates with complete interfaces and effects (B-CHECK-01/03). r[B-GATE-01]
- [x] [serial] Independently validate definitions, environment contracts, hostile candidates and every executable specialization before emission (B-CHECK-02/05). r[B-GATE-01]
- [x] [parallel] Extend the selected managed-linear-memory backend to arbitrary accepted bodies and the entire bootstrap data/control/wrapping arithmetic vocabulary (B-SCOPE-01, B-WASM-01). r[B-GATE-01]
- [x] [parallel] Preserve runtime captures, compiled quote/compose/run, aggregates, ordered interfaces and exact inert reflection across optimization modes (B-WASM-01/02). r[B-GATE-01]
- [x] [parallel] Implement declared resource-free host requests with ordered effects and trap propagation, plus explicit preparation/runtime exhaustion (B-SCOPE-02, B-CHECK-07). r[B-GATE-01]
- [x] [serial] Expose documented CLI and persistent sessions retaining compiled values, namespace snapshots and transactional rejection (B-RESULT-01/02). r[B-GATE-01]

## Phase 3: Acceptance

- [x] [serial] Execute every declared CORE-01 through CORE-18 source or structured harness with every expected observation. r[B-GATE-01]
- [x] [serial] Exercise rank-1 freshness, first-class monomorphism, complete ordered joins, immutable rebinding, all bootstrap words, aggregate capture and exact reflection (B-CHECK-03, B-WASM-02). r[B-GATE-01]
- [x] [serial] Exercise lexical boundaries, hostile candidate/environment mutations, resource eligibility, state-preserving rejection, ordered host traps and exact/exceeded limits (B-CHECK-04/05/07, B-RESULT-02). r[B-GATE-01]
- [x] [serial] Execute the bounded Wasm property workflow, replay, independent result/recipe/effect checks, all shrinking and exhaustion controls, and the separate malformed-candidate lane (DX-PROPERTY-01/02). r[B-GATE-01]
- [x] [serial] Execute the two published DX-12 examples and every compiler/runtime/host/timeout documentation control without fallback execution or ambient credentials (DX-DOC-01/02). r[B-GATE-01]
- [x] [parallel] Extend actual-Rust Charon/Aeneas/Lean extraction, dependency/axiom audits and refusal gates for changed pure code; retain explicit external models and open obligations (B-IMPL-01/02). r[B-GATE-01]
- [x] [serial] Renew compiler-derived source inventories and run affected regressions plus all full quality gates, preserving MC1 and M3 coverage (B-IMPL-01). r[B-GATE-01]
- [x] [serial] Demonstrate actual CLI/session workflows and retain reproducible source/tool-bound raw evidence and receipts. r[B-GATE-01]
- [x] [serial] Update documentation, generated views and conformance/status/roadmap ledgers only after acceptance passes. r[B-GATE-01]

## Lifecycle closeout

After all tasks pass, inspect and execute specification sync and archive with the actual completion date, then validate the archived specifications and final milestone evidence. Archiving is not its own acceptance prerequisite. M4 completion does not close MC2, M5, universal frontend/kernel refinement or backend correctness obligations.
