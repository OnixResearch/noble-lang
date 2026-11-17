# Bounded live-update corpus (Noble)

This directory implements Noble's part of issue #119. It exports bounded observations from the existing live-slot fixture lanes and compares them with compiler-lab's Makepad Relax lane over the same scenarios. It does not add a runtime or change the meaning of those lanes.

## Scope and evidence model

- `schema.json` defines the versioned corpus envelope. Each scenario names its canonical cases and profile dimensions, lane and CLI identities, retained raw reports with SHA-256 provenance, the exact edit-request digests and ordering, selected identities, normal/abnormal effect prefixes, independent expectations, and per-runtime outcomes. Two revisions share the schema, and neither accepts the other's `makepad-relax` column:
  - `noble-live-update-corpus/v1` is the Noble-side **export**. Its `makepad-relax` column is `not-run` with a reason and no profile. compiler-lab's `validate relax-live` consumes exactly these bytes.
  - `noble-live-update-corpus/v2` is the **comparison**: the same Noble scenarios with the imported `makepad-relax` column (the frozen Relax profile identity, one of the seven result classes, each anticipated difference with its observed relation and basis, and one record binding per Relax admission setting), plus a `comparison` block that binds the export, the compiler-lab revision, the pinned Relax host, the retained compiler-lab files and the replay.
  - The v2 text also corrects two defects of the v1 text, which never matched the exported data or the validator: the undeclared source `tree`, and a minimum of one request per variant, which configure-only typed probes (`type-stack-*`, `nesting-*`) do not send.
- `profiles/noble.json` freezes the Noble profile from `.cairn/changes/live-reference-lifetimes/design.md` and the live-slot quota bound. It distinguishes declarations from observations and lists unsupported dimensions. It was re-frozen on 2026-11-16 before the comparison it governs; the only change from the 2026-11-13 profile is the removal of the `makepad-relax-runtime` gap, because the Relax column is now measured under its own profile.
- The Relax profile is frozen in compiler-lab (`src/validation/relax_live/profile.json`, id `compiler-lab-relax-live-profile/relax-5441582-v1`). compiler-lab is private, so the corpus binds the profile's id, path, SHA-256 and commit, not its text.
- `corpus.mjs` is the expectation table and deterministic raw-report normalizer. Expected values come from cited canonical statements or declared fixture arithmetic, never from lane output. `export.mjs` pins the CLI, runtime sources, fixture lanes, test runners, profile inputs and the lane's own code, and retains the raw lane output and raw session files alongside the normalized scenarios.
- `comparison.mjs` derives the v2 corpus from the export and the retained compiler-lab material and verifies every binding. `import-relax.mjs` produces a comparison directory.
- `controls.mjs` mutates copies of the evidence to check semantic, raw-byte, event-binding, profile, expectation and canonical-state refusal paths (`runControls`, export) and the Relax-column bindings (`runComparisonControls`, comparison). `controls.test.mjs` replays both suites against the committed evidence.

The seven result classes are `contract-violation`, `intentional-semantic-difference`, `supported-outcome`, `unexpected-refusal`, `runtime-failure`, `unsupported` and `not-run`. Each runtime is classified under its own profile. The Noble cell is Noble's own-contract class. A Relax setting records `relax_contract`, its class under the Relax profile, and `result_class`, its class relative to Noble: a Relax outcome that its own profile admits and that differs from Noble on an anticipated dimension is an `intentional-semantic-difference`, a defect of neither runtime. Unsupported dimensions and their reasons stay visible on both sides.

Timing is a bounded wall-time observation, not a benchmark or performance claim. Noble timings are bound to the case, source revision, class and admission outcomes; each Relax timing is bound to its case, admission setting, Relax revision, host and profile digests, edit binding, class and per-edit admission outcome.

This evidence does **not** promote any canonical `LSLOT-01..09` implementation/execution/proof state, does not attach evidence to those canonical cases, and does not claim model-to-implementation correspondence. The existing lanes remain authoritative for their own assertions.

## Evidence layout

- `comparison/` is the current evidence: `corpus.json` and `receipt.json` (revision v2), `export/` (the v1 export the Relax lane consumed: `corpus.json`, `profile.json`, `raw/`, `receipt.json`) and `relax/` (the retained compiler-lab run and replay).
- `relax/` keeps the compiler-lab report, request, controls, Relax host identity, pins, one record per scenario and admission setting, and the raw live streams; and the replay's report, request and controls. Local paths are replaced by `<compiler-lab-run>`, `<compiler-lab-replay>`, `<noble-export>`, `<compiler-lab-checkout>` and `<home>`, and fixture source lines that rendered compiler diagnostics quote after a numbered gutter are replaced by `<fixture source line withheld>`. Each retained file names the digest of its retained bytes, the digest of the bytes compiler-lab wrote, and how many of each replacement it received (`comparison.retained`). The Rust fixture sources, reference executables, command records and the profile text stay in compiler-lab; `comparison.withheld` lists them with reasons, and the records bind every fixture version by SHA-256.
- `evidence/` is the first export (2026-11-13, source `d51e7e4c`). It is retained as produced and superseded: its pins name sources that have changed since, so it verifies only at its own revision.

compiler-lab derives and replay-verifies the Relax observations (its own evidence controls and `--replay`). Noble's verifier checks that the retained evidence agrees with itself and with the export: retained digests, the derivation of every column, the consumed export and Noble profile, the frozen Relax profile and host, each record's class, temporal-admission summary, recomputed edit binding, Noble anchor and timing binding, a reproduced replay of every setting, the compiler-lab controls, and the absence of local paths, quoted fixture source and unlisted files. It does not re-derive Relax observations from the raw streams.

## Produce and verify

Build the CLI with the pinned toolchain (`cargo build -p noble-cli --bin noble --offline --locked`, default features), commit the source, then export from the clean tree to a new directory outside the repository, with a private (not world-writable) temporary directory:

```sh
TMPDIR=<private temporary directory> \
  /nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node \
  verification/live-update-corpus/export.mjs \
  <absolute path of the built noble CLI> \
  <new export directory outside the repository>
```

The export directory must not already exist. The exporter invokes the existing nested-root, coherent-map, relocation-witness, typed-case, ownership, package-retention, module-root-retention and program-capacity lanes. Its final JSON summary reports scenario classes, control results and self-verification failures; a successful export has zero failures and all controls passing.

In a clean compiler-lab checkout at the revision named in `comparison.mjs`, build the pinned host (`nix build .#relax-host`) and the CLI, then run the lane on that export and replay it (inside the compiler-lab development shell):

```sh
compiler-lab validate relax-live --host <pinned relax host> \
  --corpus <new export directory>/corpus.json --timeout-ms 120000 --out <new run directory>
compiler-lab validate relax-live --host <pinned relax host> \
  --corpus <new export directory>/corpus.json --timeout-ms 120000 \
  --replay <new run directory> --out <new replay directory>
```

Import the run from the same clean Noble commit and copy the result to `comparison/`:

```sh
TMPDIR=<private temporary directory> \
  /nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node \
  verification/live-update-corpus/import-relax.mjs \
  <new export directory> <new run directory> <new replay directory> \
  <compiler-lab checkout> <new comparison directory outside the repository>
```

The importer refuses an export from another commit, a compiler-lab checkout at another revision or with another Relax profile, a run that consumed other bytes, a replay of another run, and any retained file that still names a local path or the producing host or quotes fixture source. Then verify:

```sh
TMPDIR=<private temporary directory> \
  /nix/store/7py3q3rn0qbcyzwqk6mdbyd8r1m9x54r-bun-1.4.4/bin/bun \
  test verification/live-update-corpus/
```

`corpus.mjs` re-derives each normalized Noble observation and effect prefix from the retained raw files, checks their hashes and source pins, and rejects changed profiles, expectations, event bindings or canonical-case states. `comparison.mjs` re-derives the v2 corpus and checks every Relax binding. Receipts are produced from a clean source commit; adding the resulting evidence is a later commit and does not change the source revision named inside it.
