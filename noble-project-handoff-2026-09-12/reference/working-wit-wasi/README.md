# Noble WIT / WASI integration package

Revision: **0.1.0-draft.1**  
Date: **2026-09-12**

This package makes WIT/Component Model/WASI first-class at Noble's external platform boundary while preserving Noble's richer internal programs-as-data semantics.

## Files

- `WIT-WASI.md` — SPEC-W001, normative WIT/Component/WASI profile
- `WIT-WASI-DECISIONS.md` — DEC-W001 decisions and open work
- `SPEC-0001-wit-wasi-preview.md` — integrated SPEC-0001 safety/verification working preview
- `WIT-WASI-AMENDMENT.md` — merge/integration guidance
- `patches/SPEC-0001-wit-wasi.patch` — exact diff from the prior safety preview
- `conformance/wit-wasi-cases.json` — 15 expected component-profile scenarios, all `not-run`
- `VALIDATION.json` — package-level structural checks only

## Selected architecture

```text
Noble internals              external/platform boundary
------------------------     ---------------------------------
Program<S,T,e>          -->  WIT worlds/interfaces
Noble resources         -->  WIT own/borrow adapters
Noble effects           -->  versioned WIT operation identities
Noble execution         -->  Component Model components
host profile            -->  stable WASI 0.3 family
concurrency             -->  Syndicate/Synit WIT packages
protocol data           -->  Preserves
```

WIT is the standard ABI/interface contract, **not** the internal Noble type system. WASI is the preferred host-interface family, **not** Noble language semantics.

No implementation or proof conformance is claimed by this package.
