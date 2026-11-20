## Phase 1: Design (this change; document-only)

- [x] [serial] Re-verify the issue #10 baseline and drift at `44329dc` (Route A `admit-artifact`, Route B `build --require-proof`/`bind_artifact`, the worker load paths, helper embedding, the optimizer and pins, the PO-14…21 statuses, and the S-CASE-07/CONTRACT-13/22/26 receipts), then build the nine-edge coverage/trust matrix. r[ACORR-TB-01]
- [x] [serial] Select the PAA-1 proof-bound admission envelope for the MC2 pure quotation profile; specify its exact fields, authentication source, ordering, refusal codes and separate host authorization. r[ACORR-ENV-01]
- [x] [serial] Compare a strengthened trusted build with bounded translation validation and choose the trusted build for PAA-1; specify cache invalidation, optimization mode, erasure and guard non-erasure. r[ACORR-TB-01] r[ACORR-CACHE-01]
- [x] [serial] Write the ACORR-CASE-01…12 control plan (independent negative preparation, frozen consumer, immutable receipts, retained existing controls), obligations O1–O7, residual assumptions, unsupported cases and the cross-issue ownership table; validate the change documents (not runtime or proof evidence). r[ACORR-CTRL-01] r[ACORR-TB-01]

## Future implementation and evidence gates (not undertaken by this change)

- [x] [serial] Correct the stale `runtime_assets` helper list in a separate change, then add policy pins for the CLI, helpers, Lean binary and library, and a new build report schema that keeps `noble-mc2-build/v1` receipts unchanged. r[ACORR-ENV-01]
  Progress (2026-11-09): the stale list was corrected outside this change, but it drifted again:
  `runtime_assets` declared 26 of the 27 helpers, because `experiment-cleanup.wat` (embedded at
  `crates/noble-wasm/src/source/emit/imports/entry.rs`) was never added. Progress (2026-11-20,
  slice `acorr-env`): `runtime_assets` now declares all 27 helpers and `tools/m3-wasm-policy.mjs`
  owns the cross-check (`validateRuntimeAssets`) that `tools/m3-wasm.mjs` applies to the tree it
  snapshots for the M3 harness. `crates/noble-cli/tests/build/runtime_assets.rs` drives that same
  function against the real tree in `cargo test -p noble-cli --test build`; removing
  `experiment-cleanup.wat` from the declaration fails
  `the_declared_helper_list_equals_the_runtime_directory` (observed red, then green).
  `noble-mc2-build/v4` (`--report-schema v4 --pins FILE`) records the running CLI's SHA-256, each
  embedded runtime helper's SHA-256 with the helper-set digest, the selected Lean binary SHA-256
  and contract-library BLAKE3, the worker-verified node/wasm-tools/wasm-opt path, version and
  SHA-256, and the optimizer mode, and refuses the report unless every observed identity equals the
  selection (`build-pin-mismatch`, one named identity per refusal). `v1` keeps its exact field set
  and `v3` keeps its exact field set plus `selected_toolchain`; both were re-run on this tree.
  `policy/tool-selection.ncl` carries the pin set as `build_report` (CLI identity rule, helper root
  and count with `set_sha256`, Lean toolchain/rev/binary path and SHA-256, and the three tool
  outputs/versions/SHA-256); `policy/tool-selection.json` regeneration is the integrator's step.
  The assembly re-read at `crates/noble-cli/src/core/worker/assembly.rs` is now hash-checked
  against the worker's reported `wat_sha256`/`wasm_sha256` before the bytes can become evidence.
  Remaining for this task: nothing in the code path; the delivered-value check is
  `cargo test -p noble-cli --test build` plus the ignored lane
  `report_pins::v4_binds_every_identity_and_refuses_a_changed_one`.
- [x] [serial] Implement the opt-in PAA-1 consumer by joining Route A comparison with Route B evidence/subject binding, comparing subject and wrapper bytes, instantiating only the compared buffers, and refusing with distinct codes before instantiation. r[ACORR-ENV-01] r[ACORR-CACHE-01]
  Progress (2026-11-20, commits `f73e560b`, `22e2d166`, `61ff634a`): `noble admit-proof-artifact`
  implements the `noble-proof-artifact-admission/v1` envelope in the design's order: profile
  validation, owner-law (frozen record or consumer-selected MC1 contract) statement equality, the
  shared exact proof recheck and `bind_artifact` subject binding, policy-pin toolchain verification,
  the consumer rebuild in the selected optimizer mode, subject and certified-call byte equality,
  start-section/import reflection and host authorization, then instantiation of exactly the compared
  in-memory buffers in two fresh workers. The distinct codes are `invalid-envelope`,
  `unsupported-profile`, `law-mismatch`, `proof-inconclusive`, `subject-mismatch`,
  `tool-pin-reject`, `correspondence-reject`, `guard-mismatch`, `stale-context` and
  `authorization-denied`; every refusal reports zero guest requests and protected operations. The
  `--emit-pins` route observes the running toolchain and writes the
  `noble-build-report-pins/v1` document the release owner freezes; admission compares the CLI, all
  27 embedded helpers, the Lean binary and the Node, wasm-tools and wasm-opt identities to it. The
  optional v1 guard wrapper is bound only from the registry's recognized template and its certified
  call is compiled, byte-compared and executed from the compared buffer. The comparison/
  instantiation hook for ACORR-CASE-09 exists only in a `paa1-test-barrier` build. Observed
  end-to-end on 2026-11-20: one off-mode positive admission (`admitted`, `I64 42` from the compared
  buffer), one guarded `LtI64Max` admission (`inr 9223372036854775807`), one `EqI64Literal 5`
  admission, and one barrier admission whose replaced files left the executed result `42` with
  `load` equal to the compared hashes. Tests: `cargo test -p noble-cli --test build paa1` (5),
  `--test smoke paa1` (3), `--bin noble paa1` (2); clippy `-D warnings` clean; rustfmt clean.
- [ ] [serial] Build the independent negative-preparation script, then execute ACORR-CASE-01…12 against a frozen consumer with immutable receipts. r[ACORR-CTRL-01]
  Progress (2026-11-20, commits `f73e560b`, `7a123120`, `61ff634a`): the independent preparation
  (`verification/acorr/prepare.mjs`: sealed wrapper/call renderings, wasm-tools print/parse
  mutations, pin-document text edits) and `verification/acorr/gate.mjs` are committed. The gate
  freezes a 0500 CLI, records the clean source commit/tree, emits `noble-build-report-pins/v1`
  documents for the off, on and barrier builds, prepares every positive and negative candidate
  outside the admission path, and asserts all twelve cases; it reports `passed` only when every case
  executed. The conformance registration is prepared in
  `specs/conformance/artifact-correspondence-cases.json` with all twelve cases `not-run` (no state
  is promoted), the spec-family entry and the `ACORR` route. The full gate run was not completed in
  this session: measured at ~3–4 minutes per admission (one isolated Lean recheck each) and 38
  admissions, the run exceeded the remaining session time after one admitted case
  (`001-case01-off-41.json`, `admission: admitted`, compared buffer executed). No receipt is retained,
  so this task stays open. Re-run with
  `bun verification/acorr/gate.mjs NOBLE NEW_DIR --barrier-cli NOBLE_BARRIER` on a clean commit
  (`NOBLE` a `cargo build -p noble-cli --bin noble` artifact, `NOBLE_BARRIER` the same build with
  `--features paa1-test-barrier`); then promote only the cases the retained receipt records.
- [ ] [serial] Pursue O1–O7 separately (PO-17/18 slices, optimizer, PO-21 C6, the PO-20 erasure theorem, PO-16 refinement); never close them from hashes, tests or extraction success. Extend the envelope for the #7 named-call profile only after its dependency-closure design is accepted. r[ACORR-TB-01]
