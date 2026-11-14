## Why

NGRAPH-FORMAT-01 currently admits only exact binary32 bits and exactly representable decimals.
That strict default is valuable, but it leaves common decimal inputs such as `0.1` unavailable.
Issue #40 requires an explicit, independently specified way to request one round-to-nearest,
ties-to-even conversion without changing the existing exact path or adding implicit coercions.

## What Changes

- Add the opt-in file declaration `literal-modes noble-graph-literal-modes/v2` and the per-value
  `rne:<decimal>` marker, whose exact decimal rational is rounded directly once to binary32.
- Preserve existing exact decimal and exact-bits syntax and diagnostics; refuse RNE overflow and
  over-limit tokens explicitly, and print canonical binary32 bits.
- Define four explicit I64/F32 boundary adapters, including truncation, exactness, non-finite and
  range behavior, with neither implicit conversions nor saturating/wrapping casts.
- Specify an immutable CLI import receipt that binds the original spelling and bytes to the
  canonical exact-bit graph and supports verified source or canonical export.

## Impact

- **Files**: NGRAPH-FORMAT-01, the root README's literal-mode table and user-facing adapter/import
  documentation. This spec package records the normative contract only; Rust, Lean, corpus and
  CLI implementation evidence is tracked separately by issue #40.
- **Testing**: Pinned Cairn proposal/design/tasks gates and `validate`; document checks. This package
  does not claim that implementation tests or Lean proofs passed.
