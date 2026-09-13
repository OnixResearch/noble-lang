# Review resolution

Revision: 0.1.0-draft.5

The user confirmed a greenfield implementation. [The original review](../REVIEW.md) remains historical evidence. This file records the current disposition, not completed implementation work.

## Foundation findings

| Finding | Disposition | Current contract or work |
|---|---|---|
| R1 — WIT identity | Corrected in draft | P-ID-08 and WI-ID-04/05 distinguish semantic dependencies from adapter-only builds. Six relational identity scenarios. |
| R2 — Effect denial oracle | Corrected in draft | S-CASE-05 requires static rejection with zero requests. S-CASE-16 covers forged manifests. Authority denial remains separate. |
| R3 — Missing repository | Resolved by user clarification | STATUS records a fresh implementation. M1 creates the workspace. Missing historical documents are explicitly non-prerequisite sources. |
| R4 — Work order | Corrected in draft | M2 checker/proof and M3 Wasm feasibility start after M1. Neither waits for concurrency or choreography. |
| R5 — Borrow/async contract | Bounded subset specified; broader work remains open | SPEC-R001 defines synchronous owner-threaded borrows and retirement. Borrowed exports and async borrow lifetimes remain unsupported. |

## Documentation findings

| Finding | Disposition | Current contract or work |
|---|---|---|
| R6 — Preview ancestry | Corrected | SOURCES records the actual additive chain. The document validator checks inherited requirement retention. |
| R7 — Surface drift | Corrected | K-SYN-04/05 define comments and old-name handling. The list example uses `run`. Lexer and migration cases are explicit. |
| R8 — Traceability | Evidence routes and scenario records added | Every numbered requirement has a derived index entry. Missing concrete scenarios remain `test-design-open`, not passed coverage. |
| R9 — Mixed statuses | Corrected | SPEC-EV001 separates implementation, execution, proof, and trust. Validator self-tests include implemented code with a failed test and an open proof. |

## External-study follow-up

Draft.3 adopts ND-16 through ND-20 from the [study](../review/EXTERNAL-DESIGN-STUDY.md): backend comparison, native-boundary review, checked decoding, branch diagnostics, and contract/ownership safeguards.

These are specification changes and new test designs. WasmGC and zerocopy remain experiment candidates. The study's upstream execution and performance limitations still apply.

## Aeneas-first follow-up

Draft.4 adopts ND-21 through ND-25: whole-project Aeneas-first Rust, mandatory kernel refinement, deterministic resource transitions, narrow external exceptions, and complete verification coverage. This supersedes the earlier mandatory Verus resource-table assignment. Tool execution and implementation evidence remain absent.

## First-class verification follow-up

Draft.5 adopts ND-26 through ND-30. SPEC-V002 now requires typed contract inputs, first-class companions, checked composition, runtime family instantiation, and explicit applicability checks. MC1/MC2 and PO-19 through PO-21 track implementation and proof work separately. The new contract scenarios remain unexecuted.

## Not closed by this revision

1. A complete checker solver, formal candidate encoding, or mechanized soundness proof.
2. A working Wasm calling convention, runtime, resource table, or verified backend.
3. Complete WIT mapping, borrowed exports, async scheduling, or tested toolchain pins.
4. Full Syndicate/Preserves, choreography, or durability contracts.
5. Executed Noble conformance tests or stable release claims.

The new fixtures are test designs. The document validator runs only structural checks and its own positive/negative regression tests.
