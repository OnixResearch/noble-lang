## Phase 1: Reproduce (issue #131)

- [x] [serial] Add the issue's negative test `fabricated_lean_proof_must_not_promote_procedure` and the forged-`tested` control `unexecuted_test_claim_must_not_mark_procedure_tested`, and run them against base `605c1d55` with no other change. Evidence: `cargo test -p noble-contracts --test editor --offline --locked -- procedures::admission::fabricated_lean_proof_must_not_promote_procedure procedures::admission::unexecuted_test_claim_must_not_mark_procedure_tested` reported `0 passed; 2 failed`; each `admit` returned `Ok` for the fabricated record. r[LPL-03]

## Phase 2: Design (this change)

- [x] [serial] Record D1–D9 in `design.md` (claims versus sealed receipts, the owner-supplied verifier at construction or load, the receipt binding and the verified outcome, once-per-claim verification, discarded incoming receipts, reload re-establishment and the `NPLIB002` store revision, rollback and inspection, the export, and the unchanged D14 NO-GO) and the MODIFIED LPL-03 delta with two new scenarios; pass the pinned Cairn `proposal`, `design` and `tasks` gates and `validate`. r[LPL-03]
- [x] [serial] Sync the MODIFIED LPL-03 into `.cairn/specs/learned-procedure-library/spec.md` with the pinned Cairn and regenerate `specs/LEARNED-PROCEDURE-LIBRARY.md`. The sync preview was blocked by `delta_merge.duplicate_accepted_requirements_section` because the spec repeated `## Purpose` and `## Requirements` after the generated purpose block, so those two duplicate headings were removed first; otherwise the canonical diff only adds the delta text. Evidence: `cairn sync procedure-evidence-acceptance --execute` reported `mutated: true`, `bun tools/cairn-specs.mjs --write-views` changed only that view, and `cairn validate` reported `valid: true` with no issues. r[LPL-03]

## Phase 3: Implementation

- [x] [serial] Add `procedures/acceptance.rs`: `EvidenceVerifier`, `VerificationSubject`, `Attestation`, the sealed `VerifiedEvidence` receipt with read-only accessors, minting with binding checks, receipt validation, admission acceptance and load confirmation. In `procedures.rs`, add `ProcedureLibrary::with_verifier`, the private `ProcedureRecord::verified` field with `verified_evidence()`, claim checks before acceptance, receipt merge and canonical order, receipt-only `tested`, `promoted` and `demote`, and the rollback receipt-toolchain check. r[LPL-03]
- [x] [serial] In `procedures/persistence.rs`, persist receipts in the record state (covered by the state digest and closure), add the claim digest, move the store magic to `NPLIB002`, add `load_with_verifier` with per-receipt re-establishment and no minting or repair, and export `verified_evidence` under manifest `schema_revision` 1 with a claims-versus-receipts non-claim. r[LPL-03]
- [x] [serial] Classify the `EvidenceVerifier::verify` dynamic dispatch as a reviewed local extension point in `policy/architecture.ncl` (`dynamic_dispatch.local_traits`, with reason and reopen condition, following the `CandidateRanker` precedent; no waiver or budget). Re-export `policy/architecture.json` with the pinned Nickel 1.17.0, which reproduces byte for byte, and regenerate its manifest with the pinned octet-standards. r[LPL-03]

## Phase 4: Tests

- [x] [serial] Add `tests/editor/procedures/admission/acceptance/{mod,unverified,isolation,reload}.rs` and the owner fixture ledger `support/verifier.rs`, covering the reproducer and control, forged-status refusals, the owner-verified path and receipt bindings, idempotent re-admission and once-per-claim verification, transplanted receipts, the verified outcome over the claimed one, malformed attestations, status filters and inspection without verifier calls, demotion, two-restart reload, reload refusals, a lenient-verifier store, the revision-1 magic and rollback toolchains. r[LPL-03]
- [x] [serial] Correct `evidence_classes_policy_and_dependency_edges_are_not_laundered`, which asserted that a fabricated `lean`/`proved` record is admitted: the claim alone now refuses `forged-status`, and the same record promotes only in a library whose owner's verifier checked that exact claim. Route the criteria (with new unverified and dissenting-owner cases), schema and generated-retrieval fixtures through the owner ledger, and teach the reload-negative parser the receipt layout. r[LPL-03]

## Phase 5: Verification (this change)

- [x] [serial] Run the focused and full checks. Evidence: `cargo test -p noble-contracts -- procedures::` (`--test editor`) passed 56 with 2 ignored historical one-shots; the GREEN reproducer and control pass. Workspace `cargo clippy --all-targets --all-features -D warnings` and `cargo fmt --all -- --check` passed. The full-package, Octet, Cairn and document results are recorded in the `verification/procedure-library/decision.md` erratum. r[LPL-03]

## Phase 6: Integration (separate integrator, not this change)

- [ ] [serial] Source-inventory renewal, reviewed-file re-pin and Octet capacity accounting for the touched Rust sources, then the repository flake gate run. This change leaves all of them to the integrator. r[LPL-03]
