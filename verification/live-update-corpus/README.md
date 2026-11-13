# Bounded live-update corpus (Noble)

This directory implements Noble's part of issue #119. It exports bounded observations from the existing live-slot fixture lanes; it does not add a runtime or change the meaning of those lanes.

## Scope and evidence model

- `schema.json` defines the versioned corpus envelope. Each scenario names its canonical cases and profile dimensions, lane and CLI identities, retained raw reports with SHA-256 provenance, the exact edit-request digests and ordering, selected identities, normal/abnormal effect prefixes, independent expectations, and per-runtime outcomes.
- `profiles/noble.json` freezes the Noble profile from `.cairn/changes/live-reference-lifetimes/design.md` and the live-slot quota bound. It distinguishes declarations from observations and lists unsupported dimensions. The Relax profile is deliberately absent: the `makepad-relax` column is `not-run` because compiler-lab #188/#189/#190 are unavailable.
- `corpus.mjs` is the expectation table and deterministic raw-report normalizer. Expected values come from cited canonical statements or declared fixture arithmetic, never from lane output. `export.mjs` pins the CLI, runtime sources, fixture lanes, test runners, profile inputs and exporter implementation. It retains the raw lane output and raw session files alongside the normalized scenarios.
- `controls.mjs` mutates copies of evidence to check semantic, raw-byte, event-binding, profile, expectation and canonical-state refusal paths. `controls.test.mjs` replays the suite against the committed evidence directory.

The seven result classes are `contract-violation`, `intentional-semantic-difference`, `supported-outcome`, `unexpected-refusal`, `runtime-failure`, `unsupported` and `not-run`. Noble and the external runtime are classified under their own profiles; the absent Relax run cannot establish a difference or correspondence. Any unsupported dimension and its reason remain visible.

Timing is a bounded wall-time observation tied to the scenario, source revision, outcome and admission classification. It is not a benchmark or performance claim. The issue-119 item-10 report records observations from the exported lanes, including that the saved-Program capacity lane has no baseline and that its human-readable summary prevents machine-readable normalization of the asserted capacity boundary.

This evidence does **not** promote any canonical `LSLOT-01..09` implementation/execution/proof state, does not attach evidence to those canonical cases, and does not claim model-to-implementation correspondence. The existing lanes remain authoritative for their own assertions.

## Export and verify

Build the CLI with the pinned toolchain (`cargo build -p noble-cli --offline --locked`), then export from a clean committed source tree to a new directory outside the repository, with a private (not world-writable) temporary directory:

```sh
TMPDIR=<private temporary directory> \
  /nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node \
  verification/live-update-corpus/export.mjs \
  <absolute path of the built noble CLI> \
  <new directory outside the repository>
```

The export directory must not already exist. The exporter invokes the existing nested-root, coherent-map, relocation-witness, typed-case, ownership, package-retention, module-root-retention and program-capacity lanes. The exporter's final JSON summary reports scenario classes, control results and self-verification failures; a successful export has zero failures and all controls passing.

After placing a clean-commit export under `evidence/`, run:

```sh
TMPDIR=<private temporary directory> \
  /nix/store/7py3q3rn0qbcyzwqk6mdbyd8r1m9x54r-bun-1.4.4/bin/bun \
  test verification/live-update-corpus/
```

`corpus.mjs` re-derives each normalized observation and effect prefix from the retained raw files, checks their hashes and source pins, and rejects changed profiles, expectations, event bindings or canonical-case states. Receipts are produced from a clean source commit; adding the resulting receipt/evidence is a later commit and does not change the source revision named inside it.
