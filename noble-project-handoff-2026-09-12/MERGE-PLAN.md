# Noble Canonical Spec Merge Plan

## Goal

Produce one reviewable specification revision that incorporates the selected directions without losing requirements from parallel workstreams.

## Do not do this

Do **not** take either `SPEC-0001-safety-preview.md` or `SPEC-0001-wit-wasi-preview.md` and declare it canonical as-is. Each preview starts from a related baseline but represents a different amendment path. Blind replacement can drop requirements from the other workstream.

Do not extend the old BEAM-first concurrency package unchanged. The default concurrency decision has moved to Syndicate/Synit + Preserves.

## Recommended merge order

### 1. Freeze the baseline revision

Use the project-backed SPEC-0001 + verification extension as the historical base and record exact fingerprints.

### 2. Apply selected surface decisions

Normalize the **new revision** to:

```text
call  -> run
reify -> reflect
comments -> #
```

Retain migration notes for old fixtures/docs rather than rewriting historical source files in place.

### 3. Integrate safety requirements

Merge SPEC-S001/DEC-S001 requirements into the new revision or reference them normatively. Preserve especially:

- no guest unsafe escape;
- no language-level UB;
- hostile boundary validation;
- move-only/nonforgeable authority;
- standard concurrency isolation intent;
- mechanized stable-safety release gates.

### 4. Integrate WIT/WASI profile

Merge SPEC-W001/DEC-W001 as the standard external interface/host profile:

- Component Model standard artifact;
- WIT standard external interface language;
- WASI preferred standard host family;
- resource mapping and conservative effects;
- native async boundary;
- build-identity/versioning rules;
- component conformance/release gates.

### 5. Write the new standard concurrency specification

Create a fresh spec (suggested family: `SPEC-C002`) based on Syndicate/Synit + Preserves rather than editing the old BEAM-first package in place.

It should define at minimum:

- typed dataspaces/protocol identity;
- assertions/interests/messages;
- facets/scoped lifetime;
- actor/facet failure and assertion retraction;
- no direct shared mutable Noble memory;
- capability/authority rules;
- Preserves schema/wire boundary and resource exclusions;
- WIT packages for component crossings;
- cancellation, quotas/backpressure, cleanup, and failure semantics;
- formal concurrency safety obligations.

Use BEAM/OTP as an implementation/supervision reference within this profile where useful.

### 6. Reconcile choreography

Update choreography projection so roles/conversations target the standard Syndicate concurrency layer. Preserve explicit global protocol semantics, but avoid a second unrelated transport/runtime model.

### 7. Re-run traceability

Update every requirement cross-reference, decision register, conformance case, proof obligation, and release gate against the new integrated revision.

### 8. Publish an explicit status ledger

Every requirement/case/obligation must be one of:

```text
implemented + executed
implemented + not-run
specified + unsupported
proof open
proof accepted
assumption / trusted boundary
```

Do not convert document validation into compiler/runtime conformance.

## Proposed canonical revision output

A reasonable target set is:

```text
SPEC-0001-next.md          integrated language/kernel/program model
SPEC-S001.md               safety contract
SPEC-W001.md               WIT/WASI/component profile
SPEC-C002.md               Syndicate/Preserves concurrency profile
SPEC-CH002.md              choreography over standard concurrency
SPEC-V001-next.md          integrated verification/trust model
DECISIONS-next.md          unified decision register
conformance/...            unified fixtures
verification/...           unified obligation ledger
```

The exact numbering may change; the key requirement is that every document names the revision/profile it depends on.
