# Bounded size-change closure evidence (issue #101)

The analysis placement decision and its reopen conditions are recorded in [`decision.md`](decision.md).

Two lanes give the opt-in size-change checker a model check and executed observations. The checker
is restricted to the signed declared-text profile (`--text-byte-cursor --signed-self-recursion`)
and is requested with `--size-change-evidence`.

The model is `proofs/m3/NobleM2/SizeChange*.lean`:

- `SizeChange`, `SizeChangeSites`, `SizeChangeClosure` — the earlier one-self-loop/shared-measure
  model and its constructive refusal facts;
- `SizeChangeChecker` — the argument classifier (`Arg`), the graphs it derives (`graphOf`) and the
  guarded decrease it proves (`graphOf_decrease_sound`);
- `SizeChangeGuarded` — the **guarded execution run**: a step is the chosen site's argument
  evaluation, a site whose argument is the predecessor is taken only while that measure is positive,
  and `guarded_terminates` proves an accepted family admits no such run. It also proves the bridge
  to the abstract `Run` and shows why the guard is not decoration (at zero the predecessor does not
  drop the measure);
- `SizeChangeRefusal` — the **constructive refusal witness**: `certifiedAll_refusal_site` names, for
  every measure, a site that does not decrease it, and `certifiedAll_refusal_iff` makes the report
  exact. Both are proved without `Classical.choice`;
- `SizeChangeMatrix*`, `SizeChangeCallProgram`, `SizeChangeSCT` — matrix composition and path
  semantics, a bounded closure certificate with exact path witnesses, validation of every
  idempotent strict-diagonal witness, and an idempotent no-strict-diagonal refusal witness.

## Bounds and interpretation

The direct-self-call analyzer supports at most **8 measures**, **256 call sites**, and **256 distinct
closure matrices**. Its symbolic walk additionally caps the frame at 64 cells, the per-definition
walk at 65,536 word steps, and quotation nesting at 32. Exceeding an analysis cap is a distinct
`bound-exceeded` result; it is not a claim of divergence.

For a retained finite closure, the model checks that every call-site graph is present, every listed
matrix has a non-empty site path, the list is composition-closed and duplicate-free, and each
idempotent matrix has a strict diagonal witness. A refusal retains an exact idempotent closure entry
without such a diagonal. The arbitrary-run size-change soundness theorem is **open**: the existing
Lean run theorem covers periodic site sequences only. Therefore `accepted` means that the bounded
closure criterion holds in the checked matrix model; it does not establish termination of every
possible call sequence. A refusal means **not shown terminating**, not divergence.

The current checked source profile admits direct `SelfOwner` calls only. Named/helper calls are
unsupported; mutual-recursion groups are not admitted by the current source resolver and are not
part of this profile. There is no correspondence theorem from the Rust classifier to source
execution, nor a proof connecting structural `List` tails to lengths or guarded wrapping `I64`
counters to natural numbers.

## `check.mjs` — the axiom lane

```
SELECTED_NODE verification/size-change/check.mjs [--allow-dirty] --output "<new directory outside the repository>"
```

Builds every `SizeChange*.lean` module in an isolated project under the pinned Lean 4.35.0, reads
the family's import closure from the sources (it must stay inside `Std` and the family), and audits
a closed inventory (`proofs/m3/size-change-inventory.json`): every declared theorem is inventoried,
and every inventoried theorem's compiled statement and transitive axioms are reported.

The policy is stricter than the m3 gate's: **every theorem in this family must keep to
`propext` and `Quot.sound`**. A theorem needing `Classical.choice` fails the lane. The receipt
(`receipt.json`) records the closed theorem set with each theorem's axioms, the Lean binary digest
and the source digests.

## `scenarios.mjs` — the conformance lane

```
SELECTED_NODE verification/size-change/scenarios.mjs "<new directory outside the repository>"
```

Builds the CLI from the checked-out commit (selected Rust toolchain, reviewed vendor, offline) and
runs one framed signed session per case, each with `--size-change-evidence` and a fresh
`--size-change-certificate` directory. For every case the lane asserts the reported verdict,
measures, self-call sites and argument classes; the certificate file's digest; an independent
`lean` check of that certificate against an isolated library built from the model sources; and the
CLI's own re-check under `--size-change-lean`.
By default the runner builds under its output directory; `NOBLE_TARGET` may name an existing
external Cargo target directory to reuse build artifacts.

| Case | Definition | Verdict |
|---|---|---|
| REC-SC-01 | structural list recursion, the cons branch passes the tail | accepted, measure 0 |
| REC-SC-02 | countdown under `0 <`, passing its predecessor | accepted, measure 0 |
| REC-SC-03 | non-decreasing self-call (the measure passed through) | refused, witness site 0 / `same` |
| REC-SC-04 | wrapping `I64` arithmetic used as a measure | refused, witness site 0 / `opaque` |
| REC-SC-05 | guarded two-measure recursion that swaps argument positions | accepted by the composed closure; no shared measure |
| REC-SC-06 | recursive argument swap with no decrease | refused with the idempotent identity graph as cycle witness |
| REC-SC-07 | nine declared measures, beyond the parameter cap | `bound-exceeded`; no termination or divergence claim |

The lane also checks that ordinary signed-recursion reports omit `size_change` when the flag is
absent, and that two reports in one session get distinct files inside one certificate directory.

The receipt (`scenarios-<commit>.json`) records each report, the certificates' digests, the Lean
version and binary digest, the isolated library's source digests, and the CLI binary digest.

## What the evidence does and does not claim

**Model.** The finite certificate checker proves the listed closure exactly matches its matrix
algebra and checks the criterion's idempotent witnesses. The termination theorem in this lane covers
periodic site sequences only; arbitrary-run SCT soundness remains open. The older shared-measure and
guarded-run theorems retain their own narrower contracts. A refusal means **not shown terminating**,
not diverging.

**Front end.** The Rust classifier (`crates/noble-contracts/src/source/sizechange.rs`) is an
over-approximation of the body: values are tracked only as far as the classifier needs (the measure,
its predecessor, a list tail, a `0 <` comparison), and every path it cannot model becomes opaque or
`unsupported` rather than an assumed decrease. A counter predecessor counts as a decrease only under
an active `0 <` guard *on that measure* at the site; a list tail counts without a guard because a
cons tail is structurally shorter. Named helper calls are unsupported rather than assumed to return,
so an unanalyzed recursive helper cannot be hidden behind a decreasing direct self-call.

**Not claimed.** There is no arbitrary-run soundness theorem for the bounded closure criterion, no
correspondence theorem between the classifier and Rust source execution, and none between `List`
structure and its length or between a wrapped `I64` counter and a natural number. No mutual-recursion
analysis is performed. These boundaries are explicit so the receipts are not read as a verified
refinement or a general termination result.

## Receipts

The model audit receipt is `receipt-<date>-<source-commit>.json`; the production-session receipt is
`scenarios-<source-commit>.json`. Both are produced from clean commits and committed as evidence.
Scenario certificates are retained under `evidence/<source-commit>/` beside the scenario receipt, at
the relative paths recorded in it. The source-commit directory keeps the original four-case
certificates immutable when the expanded lane is retained. These receipts bind their named source
commits; they are not evidence about later revisions.
