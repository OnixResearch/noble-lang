## Phase 1: Design (this change; document-only)

- [x] [serial] Re-verify the issue #10 baseline and drift at `44329dc` (Route A `admit-artifact`, Route B `build --require-proof`/`bind_artifact`, the worker load paths, helper embedding, the optimizer and pins, the PO-14…21 statuses, and the S-CASE-07/CONTRACT-13/22/26 receipts), then build the nine-edge coverage/trust matrix. r[ACORR-TB-01]
- [x] [serial] Select the PAA-1 proof-bound admission envelope for the MC2 pure quotation profile; specify its exact fields, authentication source, ordering, refusal codes and separate host authorization. r[ACORR-ENV-01]
- [x] [serial] Compare a strengthened trusted build with bounded translation validation and choose the trusted build for PAA-1; specify cache invalidation, optimization mode, erasure and guard non-erasure. r[ACORR-TB-01] r[ACORR-CACHE-01]
- [x] [serial] Write the ACORR-CASE-01…12 control plan (independent negative preparation, frozen consumer, immutable receipts, retained existing controls), obligations O1–O7, residual assumptions, unsupported cases and the cross-issue ownership table; validate the change documents (not runtime or proof evidence). r[ACORR-CTRL-01] r[ACORR-TB-01]

## Future implementation and evidence gates (not undertaken by this change)

- [ ] [serial] Correct the stale `runtime_assets` helper list in a separate change, then add policy pins for the CLI, helpers, Lean binary and library, and a new build report schema that keeps `noble-mc2-build/v1` receipts unchanged. r[ACORR-ENV-01]
- [ ] [serial] Implement the opt-in PAA-1 consumer by joining Route A comparison with Route B evidence/subject binding, comparing subject and wrapper bytes, instantiating only the compared buffers, and refusing with distinct codes before instantiation. r[ACORR-ENV-01] r[ACORR-CACHE-01]
- [ ] [serial] Build the independent negative-preparation script, then execute ACORR-CASE-01…12 against a frozen consumer with immutable receipts. r[ACORR-CTRL-01]
- [ ] [serial] Pursue O1–O7 separately (PO-17/18 slices, optimizer, PO-21 C6, the PO-20 erasure theorem, PO-16 refinement); never close them from hashes, tests or extraction success. Extend the envelope for the #7 named-call profile only after its dependency-closure design is accepted. r[ACORR-TB-01]
