# Noble Safety Package — 0.1.0-draft.1

This package makes Noble's general language-safety position normative.

## Contents

- `SAFETY.md` — SPEC-S001 safety contract
- `SAFETY-DECISIONS.md` — DEC-S001 decisions/open work
- `SAFETY-AMENDMENT.md` — integration guide
- `SPEC-0001-safety-preview.md` — integrated working preview, including selected `run`/`reflect` surface names
- `VERIFICATION-safety-preview.md` — verification preview with SO-01..SO-09
- `conformance/safety-cases.json` — 15 expected scenarios, all not-run
- `verification/safety-obligations.json` — 9 open proof obligations
- `patches/` — diffs against supplied integration/verification documents
- `VALIDATION.json` — structural package validation only

## Safety thesis

Noble has no guest `unsafe` escape hatch and no language-level undefined-behavior outcome. Accepted guest code remains within checked type/stack/effect/resource semantics; live authority is non-forgeable ordinary data; standard concurrency excludes direct shared mutable Noble memory; native unsafety is confined to explicit host/runtime/compiler boundaries; stable safety is release-gated by executed tests, mechanized metatheory, implementation correspondence, and a trust ledger.

## Status

This package **does not** claim the compiler/runtime already satisfies these requirements. Runtime scenarios are not-run and proof obligations are open.
