# File-by-file review inventory

Return to [the review](../REVIEW.md).

All paths in this inventory are relative to `../noble-project-handoff-2026-09-12/`.

Review method: complete text reads for unique documents and JSON descriptions. Complete diffs cover inherited text in the three integration previews. The checksum manifest covers file integrity only.

## Handoff documents — 8 files

### Architecture and authority

| File | Review result |
|---|---|
| `README.md` | Correctly warns that no complete canonical specification exists. Its preview-history description needs R6. |
| `PROJECT-HANDOFF.md` | Clear architectural summary and honest implementation caveat. Preserve staging, authority, and proof boundaries. |
| `CURRENT-DECISIONS.md` | Useful selected-direction register. It cannot replace a revisioned normative family. |
| `SOURCE-OF-TRUTH.md` | Separates baseline, drafts, and history. Conversation precedence is not self-contained evidence for a new engineer. |

### Work and evidence

| File | Review result |
|---|---|
| `MERGE-PLAN.md` | Needs corrected preview ancestry, a noncircular milestone order, and independent status dimensions. See R4/R6/R9. |
| `NEXT-WORK.md` | Good core acceptance intent. P0 needs absent source. Checker and extraction risks need earlier experiments. |
| `STATUS.json` | Explicitly says implementation is unknown. Tool-role booleans must not become implementation claims. |
| `CHECKSUMS.sha256` | All 34 listed files passed. The manifest does not authenticate its producer or prove implementation correctness. |

## Baseline sources — 8 files

### Language and design

| File | Review result |
|---|---|
| `reference/baseline/rfc-0001-minimal-programs-as-data.md` | Preserves rationale and initial scope. Provisional syntax is historical, not current surface authority. |
| `reference/baseline/SPEC-0001-baseline.md` | Substantial kernel, resource, recipe, identity, and execution contracts. Checker algorithm, encoding, ABI, and recovery remain open. |
| `reference/baseline/SPEC-0001-decisions.md` | Distinguishes adopted constraints from open decisions. Its O-07 remains operationally incomplete despite later architectural selection. |
| `reference/baseline/REFERENCES-BEND-AMBIENT.md` | Useful adaptation boundaries. Does not justify a second evaluator, speculative effects, copied identity formats, or live-resource replay. Twelve candidate scenarios remain unexecuted. |

### Verification

| File | Review result |
|---|---|
| `reference/baseline/VERIFICATION.md` | Strong finite-prefix, ownership, acceptance, and trust distinctions. Eighteen PO obligations are listed. Supporting ledger and source register are absent. |
| `reference/baseline/VERIFICATION-TOOLCHAIN.md` | Concrete verification policy, including hostile wrappers and tool pinning. No compatible toolchain or extraction result is supplied. |
| `reference/baseline/PROGRAM-CONTRACTS.md` | Sound separation of partial correctness, prefix safety, totality, identity, and admission. Evidence format and implementation remain deferred. |
| `reference/baseline/VERIFICATION-AMENDMENT.md` | Additive integration intent is clear. Referenced patches, baseline fingerprints, and the verification decision addendum are not included here. |

## Safety workstream — 9 files

### Contract and integration

| File | Review result |
|---|---|
| `reference/working-safety/README.md` | Honest no-runtime/no-proof status. Lists a patch directory absent from this archive. |
| `reference/working-safety/SAFETY.md` | 37 numbered requirements. Strong no-UB, hostile-input, prefix-effect, ownership, and scoped-concurrency requirements. |
| `reference/working-safety/SAFETY-DECISIONS.md` | Selected safety direction includes Syndicate already. Open work is explicit. |
| `reference/working-safety/SAFETY-AMENDMENT.md` | Requires preservation of current comments and concurrency. Referenced patches are absent. |
| `reference/working-safety/SPEC-0001-safety-preview.md` | Reviewed against the complete baseline. Retains verification additions. Comment rules and one `call` example remain stale. |

### Scenarios and proof obligations

| File | Review result |
|---|---|
| `reference/working-safety/VERIFICATION-safety-preview.md` | Complete diff adds SO-01 through SO-09 and V-SAFE-01 through V-SAFE-04. Does not establish those obligations. |
| `reference/working-safety/VALIDATION.json` | Historical structural-only report. Its 37/15/9 counts agree with the reviewed material. |
| `reference/working-safety/conformance/safety-cases.json` | Fifteen prose scenarios, package status `expected-not-run`. R2 identifies an invalid acceptance alternative. R8 covers missing traceability. |
| `reference/working-safety/verification/safety-obligations.json` | Nine explicitly open obligations. No proof artifacts or implementation correspondence evidence. |

## WIT/WASI workstream — 7 files

### Contract and integration

| File | Review result |
|---|---|
| `reference/working-wit-wasi/README.md` | Correctly describes the preview as safety/verification-integrated. Referenced patch is absent. |
| `reference/working-wit-wasi/WIT-WASI.md` | Forty numbered requirements. External/internal separation is clear. Type mapping, resource lifetimes, exact pins, and correspondence remain open. |
| `reference/working-wit-wasi/WIT-WASI-DECISIONS.md` | WD-14 conflicts with versioned operation identity. See R1. |
| `reference/working-wit-wasi/WIT-WASI-AMENDMENT.md` | Explicitly bases this work on the safety/verification preview. Current first-party pages corroborate its WASI 0.3 release direction. |
| `reference/working-wit-wasi/SPEC-0001-wit-wasi-preview.md` | Complete diff preserves safety additions and adds the component profile. It is the most integrated core preview here, not a publication-ready canonical spec. |

### Scenario evidence

| File | Review result |
|---|---|
| `reference/working-wit-wasi/VALIDATION.json` | Structural-only claims do not establish runtime conformance. Resolved references do not imply complete requirement coverage. |
| `reference/working-wit-wasi/conformance/wit-wasi-cases.json` | Fifteen explicitly `not-run` scenarios. Nineteen distinct references resolve against forty profile requirements. Concrete harness inputs and observations remain absent. |

## Historical material — 3 files

| File | Review result |
|---|---|
| `reference/historical/BEAM-CONCURRENCY-SUMMARY.txt` | Summary only. Its cited 66 scenarios and 12 obligations are not present. Retain bounded communication, ownership, restart-incarnation, and ambiguous-outcome lessons without restoring BEAM-first semantics. |
| `reference/historical/CHOREOGRAPHY-SUMMARY.txt` | Summary only. Its cited 68 scenarios, 16 obligations, and 13 boundaries are not present. Recover the full package before detailed projection review. |
| `reference/historical/LUNATIC-FLAWLESS-COMPARISON.txt` | Informative runtime/durability comparison. Includes superseded resource-capture reasoning. External project implementation and licensing claims were not re-audited. |

The historical scenario counts are not added to the 30 JSON scenarios actually included in this archive. The 12 Bend/Ambient candidates are separate informative table entries.
