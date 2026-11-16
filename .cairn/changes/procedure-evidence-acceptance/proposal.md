## Why

Issue #131 reports, and the reproducer confirms, that learned-procedure
admission accepted unverified, caller-supplied proof claims as evidence. Before
this change, `ProcedureLibrary::admit` retained a `promoted` record when an
`EvidenceRecord` named a proof class (`lean`), the outcome `proved`, any
nonempty toolchain and claim strings, the recomputed subject identity and the
selected policy revision. Nothing checked that a proof or a test existed.
`validate_record` used `EvidenceClass::proves()` and the consumer's
`allowed_classes` list as if classification were verification. The same root
cause let an unexecuted `test`/`passed` string mark a record `tested`. The
archived design decision D14 stated the gap ("the library does not
independently run a proof checker on an arbitrary claim string"), but LPL-03
still let such a claim satisfy a demanded obligation. The existing test
`evidence_classes_policy_and_dependency_edges_are_not_laundered` expected that
fabricated promotion to be admitted.

On base `605c1d55` the issue's negative test
`fabricated_lean_proof_must_not_promote_procedure` and the forged-`tested`
control `unexecuted_test_claim_must_not_mark_procedure_tested` both failed:
admission returned `Ok` for each record.

## What Changes

- Every `EvidenceRecord` is a claim. The library retains it unchanged as
  visible, unverified provenance, and a claim never establishes `tested` or
  `promoted` by itself.
- The library owner supplies an `EvidenceVerifier` when constructing the
  library (`ProcedureLibrary::with_verifier`) or loading it
  (`ProcedureLibrary::load_with_verifier`). `ProcedureLibrary::new` and `load`
  have no verifier, so every claim stays unverified and neither status can be
  reached. The verifier sees only the exact immutable subject
  (`VerificationSubject`) and one claim, and it returns an `Attestation` or
  nothing.
- At admission the library asks the verifier once about each claim that it has
  not retained for that subject before. For an attestation it mints a sealed
  `VerifiedEvidence` receipt bound to the subject, the claim's obligation,
  class, policy revision and digest, the verifier and toolchain identity, the
  command and input digests, and the observed outcome. Callers can read
  receipts but cannot construct or edit them, and receipts on incoming records
  are discarded and re-derived.
- `tested`, `promoted` and `demote` use receipts only. A malformed attestation
  is refused: a proof outcome for a non-proof class, or a missing verifier,
  toolchain, command or input binding.
- Persistence stores receipts in each record's mutable state, so they are
  covered by the state digest and the generation closure. The store magic
  becomes `NPLIB002`, and claims-only revision-1 stores are refused. Load
  re-establishes every persisted receipt through the loading owner's verifier,
  and refuses `invalid` at `verified_evidence` when no verifier is supplied or
  its answer differs. It never mints a receipt or repairs state. Rollback also
  requires every receipt toolchain.
- The identity manifest (`schema_revision` 1) adds a per-entry
  `verified_evidence` array next to the unchanged `evidence` claims, plus a
  non-claim that separates them.
- LPL-03 is modified to define acceptance by the owner's verifier at
  admission, the receipt binding, the reload rule and the inspection rule.

## Impact

- **Files**: `crates/noble-contracts/src/source/procedures.rs`,
  `procedures/acceptance.rs` (new), `procedures/persistence.rs`; tests under
  `crates/noble-contracts/tests/editor/procedures/` (`admission.rs`,
  `admission/acceptance/{mod,unverified,isolation,reload}.rs` (new),
  `admission/guards/criteria.rs`,
  `metrics/schema.rs`, `reload-negative.rs`, `retrieval/generated{,/fixtures}.rs`,
  `support/{mod,verifier}.rs`); the LPL-03 delta and its canonical sync in
  `.cairn/specs/learned-procedure-library/spec.md` with the generated
  `specs/LEARNED-PROCEDURE-LIBRARY.md`; the reviewed `EvidenceVerifier`
  dispatch entry in `policy/architecture.ncl`, with its exported JSON and
  manifest; and `verification/procedure-library/{decision.md,obligations.json}`
  (erratum).
  The change adds no Cargo target, crate, dependency or binary.
- **Testing**: `noble-contracts` and `noble-cli` tests with all targets and
  features, workspace Clippy with `-D warnings`, `cargo fmt --check`, the Octet
  deny-all and architecture gate, the pinned Cairn `validate`, and the
  document checks.

## Non-claims

- The library ships no verifier. Nothing in this change checks a Lean, Verus
  or test artifact. The receipt records that the owner-supplied verifier
  accepted a claim, and the trust placed in that verifier remains the owner's
  assumption. The tests use a fixture ledger that stands for an owner's
  independently recorded checks; it is not a proof checker.
- No Rust-to-model refinement, universal encoder correspondence or new Lean
  theorem is claimed. D14's NO-GO for proof-demanding promotion without
  checked implementation-refinement evidence is unchanged.
- Earlier `promoted` and `tested` states in tests and receipts relied on
  claim-only evidence. Retained receipts are not rewritten. The erratum in
  `verification/procedure-library/decision.md` records the correction.
