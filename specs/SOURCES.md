# Source map and revision provenance

Revision: 0.1.0-draft.5

The extracted handoff and ZIP remain unchanged. They are historical sources, not current implementation instructions.

## Observed integration chain

```text
SPEC-0001 baseline
  -> verification/safety core preview
  -> WIT/WASI core preview
  -> canonical SPEC-0001 revision 0.1.0-draft.2
  -> external-study adaptations, canonical revision 0.1.0-draft.3
  -> Aeneas-first implementation policy, canonical revision 0.1.0-draft.4
  -> first-class program contracts, canonical revision 0.1.0-draft.5
```

The WIT preview already includes the safety and verification additions. The current merge did not add those requirements twice.

| Current file | Preserved source |
|---|---|
| [SPEC-0001.md](SPEC-0001.md) | [WIT core preview](../noble-project-handoff-2026-09-12/reference/working-wit-wasi/SPEC-0001-wit-wasi-preview.md) |
| [SAFETY.md](SAFETY.md) | [Safety contract](../noble-project-handoff-2026-09-12/reference/working-safety/SAFETY.md) |
| [WIT-WASI.md](WIT-WASI.md) | [WIT/WASI profile](../noble-project-handoff-2026-09-12/reference/working-wit-wasi/WIT-WASI.md) |
| [VERIFICATION.md](VERIFICATION.md) | [Safety verification preview](../noble-project-handoff-2026-09-12/reference/working-safety/VERIFICATION-safety-preview.md) |

The [toolchain document](VERIFICATION-TOOLCHAIN.md) and [program contracts](PROGRAM-CONTRACTS.md) derive from their same-named baseline files.

The baseline source set includes the [RFC](../noble-project-handoff-2026-09-12/reference/baseline/rfc-0001-minimal-programs-as-data.md), [core specification](../noble-project-handoff-2026-09-12/reference/baseline/SPEC-0001-baseline.md), and [core decisions](../noble-project-handoff-2026-09-12/reference/baseline/SPEC-0001-decisions.md).

The [snapshot manifest](../noble-project-handoff-2026-09-12/CHECKSUMS.sha256) records exact source fingerprints. The ZIP SHA-256 is:

```text
62e0febf8f7f1d0dd73fcc711aa2d248edb2beac3a2cdc6d9dafea3b8a3bb20d
```

## Newly authored material

The bootstrap scope, bounded resource contract, consolidated decisions, roadmap, evidence schema, requirement index, and core/identity/resource scenarios are new material.

The safety and WIT scenarios derive from the archived descriptions, with corrected expectations and explicit evidence status. The PO/SO ledger is newly transcribed from the preserved theorem tables.

The toolchain template is new and unselected. No file is presented as a recovered historical fixture, patch, or lock file.

## External-study adaptations in draft.3

The user requested specification updates after the [external design study](../review/EXTERNAL-DESIGN-STUDY.md). That report remains the pre-adoption recommendation record. ND-16 through ND-20 state the selected adaptations.

| Source | Inspection identity | Adaptation |
|---|---|---|
| [Zena](https://zena-lang.dev/) | Source commit `a74a6939833b2f5fa8107c3e0c7f8ee3032457d0` | Compare backend techniques; preserve Noble stack, recipe, and ownership contracts |
| [Zerocopy](https://github.com/google/zerocopy) | Source commit `f2c99f928beb0356efe8c0765d177dad5167d899` | Adopt boundary-review discipline; dependency selection remains open |
| [A Few Good Ideas in Programming Languages](https://prydt.xyz/blog/a-few-good-ideas-in-pl/) | Unversioned page inspected for the study | Improve branch diagnostics and contract snapshots; defer broader syntax |

The new backend contract and adaptation scenarios are original Noble specifications, not copied implementation code. No new upstream suite or Noble experiment was executed for this revision. Source inspection does not establish upstream soundness, toolchain compatibility, or performance.

## Aeneas-first direction in draft.4

The user requested the AeneasVerif toolchain across Noble, especially the kernel. ND-21 through ND-25 record the selected interpretation: Rust implementation, Charon/Aeneas extraction, and Lean refinement proofs.

| Inspected source | Observed scope | Noble consequence |
|---|---|---|
| [AeneasVerif projects](https://aeneasverif.github.io/projects/) | Charon extracts Rust MIR to LLBC; Aeneas produces functional models and recommends Lean | Select the Rust → Charon → Aeneas → Lean route |
| [Aeneas README](https://github.com/AeneasVerif/aeneas#targeted-subset-and-current-limitations) | Safe Rust subset; unsafe and concurrency support remain development work | Keep the kernel safe and sequential; require explicit external boundaries |
| Same README, installation and Lean instructions | Aeneas selects a Charon revision and Lean/backend configuration | Select compatible immutable pins through actual extraction, not independent floating versions |

These are live-page observations during this revision, not immutable compatibility evidence. No upstream implementation was imported. No Aeneas, Charon, or Lean run occurred. Eurydice and Scylla remain unselected because their C translation roles do not supply Noble's Wasm backend.

This decision supersedes the draft.3 checker-only preference and mandatory Verus resource-table assignment. The retained Verus requirement IDs now apply only to approved non-kernel exceptions. Source-to-Wasm preservation, external effects, and synchronization remain separate obligations.

## First-class contract direction in draft.5

The user approved specification changes for first-class formal verification after the design discussion. ND-26 through ND-30 select typed contracts, evidence companions, compositional proof rules, verification tooling, and the MC1/MC2 workstream.

SPEC-V002 retains its inherited logical requirements and replaces its metadata-only starting scope. New API contracts and [contract scenarios](conformance/contract-cases.json) are original Noble design material. PO-19 through PO-21 add statement-export, companion-rule, and applicability obligations. No external implementation was imported or executed for this revision.

Concrete declaration grammar, representation, and transport remain explicit entry gates. Their absence does not establish implemented first-class support. The historical handoff and earlier reviews remain unchanged.

## Exact-calculator amendment to draft.5

The user approved specification updates after the AI-authoring and mathematical-calculator discussion. ND-41 through ND-45 select exact rational division, arbitrary-precision integers, exact decimal input, and a bounded reference-application benchmark.

[CALCULATOR.md](CALCULATOR.md) and its [scenarios](conformance/calculator-cases.json) are original Noble design material. They preserve wrapping `I64` semantics and the bootstrap boundary. This working-draft amendment does not claim a stable release or executed implementation.

| Inspected source | Observed scope | Adoption boundary |
|---|---|---|
| [Mathematics in Lean, algebraic structures](https://leanprover-community.github.io/mathematics_in_lean/C02_Basics.html#proving-identities-in-algebraic-structures) | Separate mathematical types and generic algebraic laws | Distinguish exact arithmetic models from machine arithmetic and implementation proofs |
| [Wolfram Language, Rational](https://reference.wolfram.com/language/ref/Rational.html) | Exact integer ratios with normalized positive denominators | Select normalized rational results, not Wolfram syntax or its entire numeric system |
| [Julia, Rational Numbers](https://docs.julialang.org/en/v1/manual/complex-and-rational-numbers/#Rational-Numbers) | Explicit rational construction and conversion to floating point | Separate exact and approximate computation; do not adopt Julia's infinite rational values |

These are unversioned documentation observations, not pinned implementation dependencies or formal evidence. No upstream source was copied. No Lean, Wolfram, Julia, Noble calculator, or AI benchmark was executed. The calculator's exact decimal literals, zero-divisor errors, and integer-power convention are explicit Noble application decisions.

## Worker-contract amendment to draft.5

The user approved kernel and boundary clarifications after the agent-substrate discussion. ND-46 through ND-50 record the selected contracts. [Worker conformance](WORKER-CONFORMANCE.md) and its scenarios are original Noble design material.

The amendment reuses existing language, safety, module, and resource boundaries. No external source, implementation, or agent framework was imported. It does not claim full checker rules, canonical package bytes, native-async interoperability, or executed worker support. The historical handoff remains unchanged.

## Octet contract amendment to draft.5

The user approved adaptation of Octet's boundary and evidence contracts. The inspected documentation was unchanged at Octet revision `cf04e894e53eb0947230118a086ef6066ddba38c`. [OCTET-ADOPTION.md](OCTET-ADOPTION.md) contains immutable source links and the requirement-owner map.

ND-51 through ND-55 record original Noble adaptations of those contracts. No Octet code was copied, linked, or executed for this amendment. The source revision identifies design evidence, not a tested Noble toolchain pin. The existing Aeneas-first policy and all historical snapshots remain unchanged.

## Missing historical material

| Historical reference | Current treatment |
|---|---|
| Original core conformance package | Absent; new bootstrap fixtures replace the dependency, not its history |
| Verification `DECISIONS.md` | Absent; DEC-N001 explicitly consolidates current decisions |
| Verification `SOURCES.md` and external R1–R10 records | Absent; inherited labels are unverified historical citations, not pinned tool evidence |
| Old amendment patches and manifests | Absent; preserved files and current requirement-preservation checks establish the local source chain |
| Full concurrency/choreography packages | Absent; summaries are informative only and do not block core implementation |

Within inherited verification prose, `B1` denotes the preserved core specification. External `R1` through `R10` labels do not name current normative dependencies. Their unavailable records cannot establish tool compatibility or discharge assumptions.

A compatible toolchain must be selected and executed under IMPL-V001 regardless of those historical claims.

## Concurrency and choreography identifiers

The historical choreography summary already uses `SPEC-CH001`, `SPEC-CH002`, and `SPEC-CH003`. New work must not silently reuse these identifiers for different documents.

No new full concurrency or choreography specification is claimed here. Future documents must publish an identifier, revision, scope, and explicit supersession relationship.

## Authority change from the review

The user confirmed that no implementation exists and that Noble will start from scratch. This supersedes the handoff's suggestion to locate prior implementation work.

The original [review](../REVIEW.md) remains a record of the earlier evidence. [REVIEW-RESOLUTION.md](REVIEW-RESOLUTION.md) records its current disposition.
