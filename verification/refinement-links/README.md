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

- **Root set.** Every theorem declared by the reviewed module scope of that subject in `roots.json`
  (`roots.json` v2 carries one scope per subject; both subjects are reviewed, so no subject is
  unnamed). Naming roots by hand would let the lane claim coverage of a theorem it never looked at.
  `links.mjs` exits 2 when the closure declares a scope `roots.json` does not review, so a run
  cannot widen its own root set and its coverage numbers unreviewed. Lean-derived shapes
  (`X.eq_3`, `X.ofNat_ctorIdx`, `X.match_*`, `X.noConfusion*`, `X.sizeOf_spec`, `X.casesOn*`) are
  machinery, not roots, exactly like `X._proof_*`.
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

| Subject | Generated functions | Linked (in-tree) | Linked (out of tree) | Unlinked (in-tree) | Rust out of tree | Roots | Closure |
|---|---|---|---|---|---|---|
| MC1 `proofs/mc1/NobleContractImpl/Funs.lean` | 1023 | 6 | 3 | 674 | 343 | 10 | available |
| M3 `proofs/m3/NobleKernel/Funs.lean` | 922 | 199 | 59 | 104 | 619 | 13 | available |

Totals: MC1 1023 functions = 9 linked + 1014 unlinked; M3 922 functions = 258 linked + 664 unlinked.
Drift against the reviewed record: MC1 1023 `unchanged`, M3 922 `unchanged`, both 0 failing
(`summary.json`, `drift-mc1.json`, `drift-m3.json` in the receipts).

MC1's declared root set is the 10 theorems of `NobleContractImpl.Projection` and
`NobleContractImpl.Statement`. Nine MC1 functions are linked, each mentioned by six of those roots:

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

Across all ten MC1 roots the closure holds 12290 mentions: 5337 definitions, 2914 theorems, 1970
constructors, 570 inductive types, 221 recursors, 1278 axioms. The other four roots are about
admission evidence and mention no extracted function. The remaining 1014 extracted functions are
named by no declared root: measured coverage, not a defect claim, because the MC1 refinement
statements are about admission and rendering.

M3's declared root set is the 13 theorems of `NobleM2.Refinement`, and every one of them mentions
the same 258 functions, so the M3 link set is the closure of a single root family with no
intersection to take. Across the 13 roots the closure holds 14677 mentions: 9906 definitions, 2262
constructors, 1469 theorems, 702 inductive types, 338 recursors. 664 extracted functions are named
by no declared root.

## The M3 admission repair (issue #84)

The M3 subject was `closure-unavailable` at the previous measurement: `NobleM2/Embed.lean` built a
kernel `Env` value missing six fields the current `NobleKernel` requires (`nominals`,
`callerModule`, `declaredModules`, `definitionOwners`, `boundAdapters`, `resourceKinds`). The
repair follows the new admission surface with an explicitly **empty admission fragment**
(`emptyAdmission`), whose docstring states that nothing is claimed about nominals, owners or
adapters, instead of defaulting those fields silently into each literal.

One field is deliberately *not* an admission premise. `resourceKinds` is projected straight from the
reference environment, because the kernel admits a `resource` type only when the environment
declares it. `native_decide` first reported the eligibility theorem false, and an `#eval` of both
sides showed why: the reference returned `.eligibility (.resource fixtureResource)` while the
kernel returned `.invalidType`. Recording `resourceKinds` as an admission premise would have made
the docstring claim something the failing fixture disproves.

The projection also now covers `ContractType`, `EvidenceType`, `CertifiedType` and `NominalType`
(nominal keeps its identity and an opaque unit shape), and the reference `Constraint` admits the
three kernel-only rejection reasons `invalidType`, `invalidContract` and `privateDefinition`,
documented as reasons the reference checker never emits. `lake build NobleM2.Refinement` completes
successfully (1684 jobs, `lake-build-m3.log`) with only style lints.

`before-repair/` keeps the earlier failing evidence (`m3.json` with 0 linked functions,
`m3-build-error.txt`, `m3-closure-error.txt`) beside the repaired receipts.

## Controls (each one measured, `receipts/2026-10-31/controls.json`)

| Control | Result |
|---|---|
| Formatting and comment only inside a linked Rust body | 1023 `unchanged`, 0 failing |
| One identifier token changed in that body | 1022 `unchanged`, 1 `rust-changed`, failing |
| Generated declaration text edited, Rust untouched | 1 `generated-changed`, failing |
| A generated declaration deleted from the module | 1022 `unchanged`, 1 `removed`, in-tree coverage 6 -> 5, failing |
| Root renamed in the closure | 1023 `unchanged`, 1 `unreviewed-root` and 1 `removed-root`, failing |
| Unreviewed Rust change, then its digest recorded | 1 `rust-changed` first, then 1023 `unchanged` and exactly 1 reviewed digest differing |

Controls run on a hardlinked scratch tree with `NOBLE_REFINEMENT_REPO`; Charon records absolute
checkout paths, so `links.mjs --rust-root` maps them into the scratch tree and a control can never
edit the real checkout. The harness deletes the scratch copy of the manifest before writing it:
`cp -al` shares inodes, so writing through a shared file would corrupt the committed lane (an
earlier harness revision did exactly that, which is why the reviewed baseline is now rebuilt from
the two measured manifests and never merged from a stale file).

Every control result above was measured again after the M3 repair, against the merged baseline of
both subjects (1945 links: MC1 1023, M3 922).

## Running

```sh
verification/refinement-links/run.sh /tmp/lane-out
```

Outputs `closure-<subject>.json`, `links-<subject>.json`, `drift-<subject>.json` and `summary.txt`
with one exit status per stage. A closure that fails to build fails the run and its subject's
manifest is still measured, with every link `closure-unavailable`.

The reviewed baseline (`reviewed.json`) records one entry per subject, keyed by link and root name,
and `bootstrap-reviewed.mjs` replaces exactly the re-measured subject's records. `drift.mjs`
compares only the records whose `subject` matches the manifest it was given; without that scoping,
the other subject's links read as `removed` and the gate fails on correct work. `roots.json` v2
likewise carries one reviewed module scope per subject, and `links.mjs` exits 2 on any mismatch.
