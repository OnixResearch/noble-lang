# Core Specification Amendment — WIT / Component Model / WASI

Document: AMEND-W001  
Revision: 0.1.0-draft.1  
Date: 2026-09-12  
Applies to: SPEC-0001 safety/verification working preview 0.1.0-draft.1  
Status: Proposed additive integration; Project sources have not been overwritten

## 1. Purpose

This amendment changes WIT/WASI from deferred backend details into the selected standard Noble platform boundary.

It adds:

- `Component-Draft` as a reporting/conformance subset;
- WIT as the standard external interface language;
- Component Model components as the standard portable deployment/interoperability artifact;
- direct compiler ingestion of WIT packages and worlds;
- lossless typed WIT import/export adapters;
- owned/borrowed WIT resource rules integrated with Noble ownership;
- conservative effects for imported WIT operations;
- WASI 0.3 as the preferred standard host family;
- Component Model native `async func`, `stream<T>`, and `future<T>` as the standard cross-component async ABI;
- explicit WIT/WASI version/build identity rules;
- an explicit Syndicate/Preserves/WIT layering contract;
- Component-boundary conformance requirements and a strengthened G-05 release gate.

## 2. Important boundary

The amendment deliberately does **not** make Noble WIT-shaped. `Program<S,T,e>`, stack polymorphism, effects, recipes, program identity, and proofs remain internal Noble concepts. WIT describes the external component contract.

## 3. Current ecosystem baseline

This revision was written against first-party documentation current on 2026-09-12. WASI 0.3 is the current stable family; 0.3.1 is the current shipped patch release as of this date. The Noble spec names the 0.3 family, while exact patch/package/toolchain versions are pinned by build/profile configuration rather than frozen into language semantics.

## 4. Integration files

- `WIT-WASI.md` — normative WIT/WASI profile (SPEC-W001)
- `WIT-WASI-DECISIONS.md` — decisions and open work (DEC-W001)
- `SPEC-0001-wit-wasi-preview.md` — integrated core working copy based on the safety/verification preview
- `patches/SPEC-0001-wit-wasi.patch` — exact diff from that preview
- `conformance/wit-wasi-cases.json` — expected/not-run component scenarios

## 5. Merge rule

For a newer branch, merge the requirements semantically instead of replacing later syntax, safety, verification, Syndicate, or tooling work wholesale. Preserve the selected `run`/`reflect` terminology and `#` comment direction.

## 6. Status

This package selects architecture and defines requirements. It reports no executed Noble Component Model implementation, no completed cross-language conformance run, and no proof that emitted components preserve Noble semantics.
