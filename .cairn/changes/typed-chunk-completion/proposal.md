## Why

Issue #73 needs a fixed-chunk baseline for program completion before any learned
ranking is considered. Noble already has two separate admission boundaries: the
kernel checks an untrusted finite arena, and source preparation resolves and
rechecks source recipes. It does not yet have a host-owned library of exact,
reusable typed chunks or a deterministic one-hole completion request. Without a
bounded design, a completion service could mistake a matching signature for
behavioral equivalence, treat an editor suffix as preserved, or hide expanded
work behind a chunk boundary.

r[TCHUNK-01] r[TCHUNK-02] r[TCHUNK-03] r[TCHUNK-04]

## What Changes

- **ADDED** `TCHUNK-01`: a versioned pure-I64/Bool chunk envelope and immutable
  library, retaining exact recipes, captures, resolved dependencies,
  instantiations and source/semantic revisions. The profile is acyclic, has
  empty effects, and is admitted through normal source preparation and kernel
  acceptance; a digest or index is never authority.
- **ADDED** `TCHUNK-02`: a host-owned completion request and response plus a
  deterministic one-hole enumerator. It solves the complete ordered stack
  equation, including the surrounding tail, and emits an explicit source and
  candidate which are checked against the original request.
- **ADDED** `TCHUNK-03`: fixed budgets, shared-budget accounting, distinct
  invalid/unsupported/exhausted/no-result outcomes, mutation and freshness
  controls, and measurement of enumeration, checking, latency, expansion and
  retained bytes.
- **ADDED** `TCHUNK-04`: executable design fixtures and acceptance mapping for
  primitive and fixed chunks, two-chunk composition, nested quotations,
  wrapping-I64 boundaries, operand order, Boolean results, fresh
  instantiation, stack tails, equal-signature/different-behavior chunks and
  malformed or forged proposals.

This is a design-only change. It adds no Rust implementation, does not modify
canonical specs, and does not authorize search in ordinary `quote`, `compose`
or `run`. Learned ranking (#74), learned chunk discovery, runtime macros,
optimizer behavior and proof authority remain outside this change.

## Impact

- Rust subjects are design references only:
  `crates/noble-kernel/src/untrusted.rs`,
  `crates/noble-kernel/src/acceptance/mod.rs`,
  `crates/noble-contracts/src/source.rs`, and
  `crates/noble-contracts/src/source/editor.rs`.
- The future API is an opt-in host service. Existing Programs, editor trees,
  ordinary source admission and kernel limits are unchanged.
- The affected delta is the new change-local capability specification; no
  `.cairn/specs/` canonical file or generated view is changed.
- #74 is deliberately not implemented or designed as a ranking extension.

## Non-goals

No implementation code, learned ranking, learned boundary discovery, parser or
IR replacement, optimizer, runtime macro system, behavioral-equivalence proof,
substitution into an existing Program, universal completion claim, or production
adoption.
