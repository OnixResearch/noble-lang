# Noble Cairn specifications

**Start with [the language](specs/language/spec.md) and [the bootstrap scope](specs/core-bootstrap/spec.md).**

Revision: `0.1.0-draft.5`

This directory owns the thirteen normative draft specifications and their requirement IDs.
Accepted specifications describe selected contracts. They do not establish completed code, executed scenarios, or accepted proofs.

## Core and execution

| Cairn ID | Document | Contract |
|---|---|---|
| `language` | SPEC-0001 | [Language semantics](specs/language/spec.md) |
| `core-bootstrap` | SPEC-B001 | [First implementation subset](specs/core-bootstrap/spec.md) |
| `backend-experiments` | SPEC-BE001 | [Wasm representation comparison](specs/backend-experiments/spec.md) |
| `c-backend` | SPEC-BE002 | [Opt-in nonselected C11 backend design](specs/c-backend/spec.md) |
| `safety` | SPEC-S001 | [Safety invariants](specs/safety/spec.md) |

## Boundaries and applications

| Cairn ID | Document | Contract |
|---|---|---|
| `wit-wasi` | SPEC-W001 | [Component profile](specs/wit-wasi/spec.md) |
| `resource-adapters` | SPEC-R001 | [Resource ownership and adapters](specs/resource-adapters/spec.md) |
| `developer-experience` | SPEC-DX001 | [Language tools and libraries](specs/developer-experience/spec.md) |
| `calculator` | SPEC-CALC001 | [Exact calculator](specs/calculator/spec.md) |

## Verification and evidence

| Cairn ID | Document | Contract |
|---|---|---|
| `verification` | SPEC-V001 | [Reference model and proof obligations](specs/verification/spec.md) |
| `verification-toolchain` | IMPL-V001 | [Rust → Charon → Aeneas → Lean](specs/verification-toolchain/spec.md) |
| `program-contracts` | SPEC-V002 | [Typed contracts and evidence companions](specs/program-contracts/spec.md) |
| `evidence` | SPEC-EV001 | [Evidence and status rules](specs/evidence/spec.md) |

## Supporting records

[The family manifest](../specs/spec-family.json) maps old paths to native Cairn paths.
The normative Markdown files under `specs/` at the project root are generated compatibility views.
They preserve old links but do not provide separate authority.

[The roadmap](../specs/ROADMAP.md), [status](../specs/STATUS.json), and [decisions](../specs/DECISIONS.md) remain supporting records.
The [scenario designs](../specs/conformance) and [proof ledger](../specs/verification/obligations.json) carry independently recorded execution, proof, and evidence status.
The worker and Octet maps remain supporting documents, not additional normative specs.

Conversion began with empty `changes/` and `archive/` directories; it did not fabricate implementation tasks, completed changes, or archive evidence.
Subsequent implementation changes have their own lifecycle records. Active changes belong in `changes/<slug>/`, with explicit requirement deltas.
`changes/explicit-live-references/` remains active for future implementation after the separate issue #1 `Live-Slot-Design` contract was accepted into the canonical Cairn specifications. Its nine registered LSLOT scenario designs remain absent/not-run/open/unassessed with empty evidence; canonical design acceptance does not supply an opt-in slot runtime, accepted proof or LIVE case promotion.
`changes/live-wasm-reload/` is an active design with guarded partial runtime:
a resident Node/V8 REPL, in-process Wasm byte emission, checked explicit reload
and bounded cooperative Linux `noble live watch` on same-directory
`fsync`-then-atomic-rename saves. Its selected-byte and watch-sequence checks
refuse stale compiled candidates and unsupported partial/in-place changes; an
external writable mapping can still change bytes without a watch event, so
ACK-time observation is not continuing file immutability or complete
H-LIVE-01/LIVE-05 acceptance. Scoped real-child checks exercise transitive
Core rebuild, old Program and declared-module snapshots, counted Core code
retirement, an active old Wasm frame before a reload ACK, and candidate A
compiled before fsynced save B supersedes it without an A ACK. The production
declared REPL now has an exact host-stdin binding revocation; default and
feature actual V8 child suites and one direct CLI run observed a retained A
Program denied after B import and A host revocation, while B stayed allowed.
Those V8 child and direct CLI observations belong to the separate LIVE06 donor
source. The isolated combined successor `ac3f60c9820acd0103a017ca8aad8fe17c3a6041`
subsequently passed *bounded*, separately authorized Rust and direct Node 24/V8
checks on its own binary SHA-256
`4024a0c6a1cb5b132f15be7beccd0756a899d2aba017e55bf46a490c8804a63b`:
exact host revocation denied saved A without a protected operation, B remained
allowed, and selected returned-target D/P, alias, old/new source and epoch CAS
controls ran. This is not the donor's full suite, a current document-validation
result or canonical LIVE/LSLOT acceptance. `specs/VALIDATION.json` still records
only the older donor's inputs and must not be read as a pass for this checkout.
Bounded host-granted guest self-edit refuses any current effective dependent
rather than rebuilding it. Indirect self-rebind/cycle refusal is scoped, not
established by general proof or accepted LIVE case evidence. General
effectful watch, full in-flight source-change controls, interpreter execution,
complete source-bound LIVE-02 and canonical LIVE-06 acceptance remain open.
`changes/c-backend/` remains a specification-only active design. Their
canonical scenarios remain absent/not-run with proof open, their roadmap
acceptance milestones not-started, and neither complete profile changes
historical M4 or Declared-Modules-v1 claims.

## Inspect and validate

From the project root:

```sh
bun tools/cairn.mjs spec list --root .
bun tools/cairn.mjs spec show core-bootstrap --root .
bun tools/cairn.mjs validate --root .
bun tools/cairn-specs.mjs --self-test
bun test tools/cairn-specs.test.mjs tools/check-specs.test.mjs
bun tools/check-specs.mjs --self-test
```

The Cairn runner requires Bun, Nix, and SSH access to `git@github.com:OnixResearch/cairn`.
It selects Cairn revision `15f00875562025e7ea7e0d1f4af24d1a2e2ac06f` and the generated policy from that same immutable source.
It does not use an ambient sibling checkout or an unrelated installed binary.
The initial invocation can fetch and build Cairn.

Cairn checks requirement structure and scenario clauses.
The local checks cover compatibility drift, source links, requirement preservation, scenario references, status fields, and roadmap dependencies.
They reject review-only execution evidence outside review scenarios and require an explicit claim in each evidence record.
Proof obligations use the same evidence checks. Accepted and failed obligations require proof evidence with the matching obligation ID and result.
These checks do not execute Noble or discharge proofs.

## Edit a specification

1. Edit the applicable `.cairn/specs/<id>/spec.md` file outside its generated comment regions.
2. If scenario designs change, edit their JSON records under `specs/conformance/`.
3. Run `bun tools/cairn-specs.mjs --write-views` to refresh scenario clauses and compatibility views.
4. Run `bun tools/check-specs.mjs --refresh-ledger --self-test --report` to refresh the requirement ledger and document receipt.
5. Run `bun tools/cairn.mjs validate --root .` to validate the native specifications.

Each requirement uses `### Requirement: ID` and a matching `r[ID]` marker.
Imported requirements also retain their `**ID.**` labels.
New native requirements do not need those labels before regeneration.
The adapter adds missing compatibility labels and preserves the ordered native IDs.
The ledger uses the same fence-aware native parser. Fenced examples do not create requirements or historical preservation obligations.
Generated-region comments inside fenced examples remain literal text. Regeneration preserves those examples and encodes local link paths after rebasing them.
The adapter rejects missing markers, mismatched identities, duplicate IDs, and malformed generated regions before writing any specification or compatibility view.
Generated scenario regions link to exact case IDs and their complete `input` and `expected` fields.
They do not create new tests or close missing test designs.

[The migration record](MIGRATION.md) describes the conversion and the explicit normative-word changes.
