# Refinement links for the Charon -> Aeneas route (issue #81)

Per-function Rust<->Lean refinement links with a revision-bound drift check. Every extracted
function in a generated module records its Rust identity, its generated Lean identity and the
handwritten theorems whose transitive constant closure mentions it; the drift check fails when any
part of a reviewed link changes until that link is reviewed again.

**A passing drift check is not a refinement proof.** It does not discharge extraction trust
(VT-AENEAS-06): it says the reviewed triples still hold, not that the theorems mean what the Rust
means.

## Files

| File | Role |
|---|---|
| `links.mjs` | generated manifest: Rust identity and token digest, generated identity and text digest, declared roots, linkage class |
| `closure.lean` | reads each root's statement and its transitive generated-constant mentions from the compiled Lean environment; imports no subject module by `import`, so one file describes both subjects |
| `roots.json` | declares the root-set *module scope* only; theorem names themselves come from the environment |
| `reviewed.json` | the reviewed baseline per link: digests, theorem set, coverage, and the revision it was reviewed at |
| `drift.mjs` | compares the manifest against `reviewed.json` and classifies every link |
| `run.sh` | the whole lane: both closures, both manifests, drift; `env -i` with the policy-selected Lean and Node |
| `bootstrap-reviewed.mjs` | one-shot: writes `reviewed.json` from a measured manifest. Never part of the gate |

## Decisions, made once and reviewed here

- **Root set.** Every theorem declared by the chosen modules (`NOBLE_CLOSURE_MODULES`). Naming roots
  by hand would let the lane claim coverage of a theorem it never looked at, and would need an edit
  whenever a theorem is added. Which modules count is the owner's choice; that list is visible in
  `roots.json` and in the manifest's `rootSet`.
- **Rust body digest is token-based.** Comments are dropped, literals kept verbatim, whitespace runs
  collapsed, then sha256. A formatting-only or comment-only Rust edit does **not** fire
  `rust-changed`; any token change does. A MIR/LLBC digest would bind formatting to the pinned
  Charon and hide source edits behind it.
- **Generated digest is declaration text** from a declaration's rust-path header to the next header,
  so an Aeneas revision bump with identical Rust fires `generated-changed`.
- **Mentions are classified by declaration kind.** A root mentioning an inductive type, constructor or
  recursor is a type-side mention, never a lost function link. A root mentioning a generated `defn`
  the translator recorded no source span for is an `unsourcedDefinition`: a coverage gap of the
  inventory (no Rust identity exists), reported and counted, never silently counted as covered.
- **`closure-unavailable` is its own class.** A subject whose refinement lane does not compile cannot
  answer "which theorem mentions this". Reporting it as `unlinked` would claim nobody mentions those
  functions; reporting it as clean would be worse.
- **Coverage is measured, not gated.** A 1023-function extraction does not become a lie because a
  function has no root yet, and a gate that only goes green once everything is linked would be
  either never green or bumped unread. What fails is drift *away* from the reviewed record in either
  direction, an unavailable closure, and a reviewed link that vanished from the manifest.

## Classes

| Class | Meaning | Clears when |
|---|---|---|
| `unchanged` | digests, theorem set and coverage equal the reviewed record | nothing to do |
| `rust-changed` | Rust token digest differs | the change is reviewed and the link's digest recorded again |
| `generated-changed` | same Rust tokens, different generated declaration text (tool drift) | the generated change is reviewed |
| `proof-changed` | a root statement, its mention set, or the root set differs | the proof change is reviewed |
| `unreviewed` / `unreviewed-root` | no reviewed record | reviewed once |
| `removed` | a reviewed link is absent from the current manifest | the declaration is restored or its removal is reviewed |
| `orphaned` | a root mentions a generated definition the extraction no longer produces | the proof or the extraction is fixed |
| `closure-unavailable` | the subject's environment does not compile | the subject's lane compiles |
| `external-source` | the translator traced the Rust to the standard library or an external crate | nothing: there is no body here to digest |

## Measured at this revision (see `receipts/2026-10-31/`)

| Subject | Generated functions | Linked (in-tree) | Unlinked (in-tree) | Rust out of tree | Closure |
|---|---|---|---|---|
| MC1 `proofs/mc1/NobleContractImpl/Funs.lean` | 1023 | 6 | 674 | 343 | available |
| M3 `proofs/m3/NobleKernel/Funs.lean` | 922 | 0 | 303 (all `closure-unavailable`) | 619 | **unavailable**: `NobleM2/Embed.lean` builds a kernel value missing six fields the current `NobleKernel` requires |

MC1's declared root set is 27 theorems of `NobleContractImpl.Projection` and
`NobleContractImpl.Statement` (the `meaning.eq_*` / `sourceMeaning.eq_*` equation lemmas among
them). Their closures mention 5408 generated definitions with no Rust identity, 7528 type-side
constants and 30 Lean-derived declarations (`X.match_1`, `X._proof_*`).

Nine MC1 functions are linked by the 27 declared roots, each mentioned by six roots:

| Linked function | Rust body |
|---|---|
| `companion.Budget.charge` | in-tree |
| `companion.admit.check_evidence_purity` | in-tree |
| `companion.admit.accept.eligible` | in-tree |
| `rendering.rejected` | in-tree |
| `rendering.types.stack` | in-tree |
| `rendering.types.value` | in-tree |
| `Prepared.impl.candidate` | outside this repository |
| `Prepared.impl.expressions` | outside this repository |
| `Prepared.impl.params` | outside this repository |

The remaining 1014 extracted functions are named by no declared root. That is measured coverage, not
a defect claim: the MC1 refinement statements are about admission and rendering, and the rest of the
extraction has no root yet.

## Controls (each one measured, `receipts/2026-10-31/controls.json`)

| Control | Result |
|---|---|
| Formatting and comment only inside a linked Rust body | 1023 `unchanged`, 0 failing |
| One identifier token changed in that body | 1022 `unchanged`, 1 `rust-changed`, failing |
| Generated declaration text edited, Rust untouched | 1 `generated-changed`, failing |
| A declaration made unparsable and vanishing from the manifest | 1 `removed`, failing |
| Root renamed in the closure | 1 `unreviewed-root`, failing |
| Unreviewed Rust change, then its digest recorded | 1 `rust-changed` first, then 1023 `unchanged` and exactly 1 reviewed digest differing |

Controls run on a hardlinked scratch tree with `NOBLE_REFINEMENT_REPO`; Charon records absolute
checkout paths, so `links.mjs --rust-root` maps them into the scratch tree and a control can never
edit the real checkout.

## Running

```sh
verification/refinement-links/run.sh /tmp/lane-out
```

Outputs `closure-<subject>.json`, `links-<subject>.json`, `drift-<subject>.json` and `summary.txt`
with one exit status per stage. A closure that fails to build fails the run and its subject's
manifest is still measured, with every link `closure-unavailable`.
