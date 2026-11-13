# Size-change termination evidence (issue #101)

The analysis placement decision and its reopen conditions are recorded in [`decision.md`](decision.md).

Two lanes give the opt-in size-change checker a model check and executed observations. The checker
is restricted to the signed declared-text profile (`--text-byte-cursor --signed-self-recursion`)
and is requested with `--size-change-evidence`.

The model is `proofs/m3/NobleM2/SizeChange*.lean`:

- `SizeChange`, `SizeChangeSites`, `SizeChangeClosure` — one self-loop, the shared-measure criterion
  over a family of sites, and the closure language;
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
- `SizeChangeExact`, `SizeChangeMatrix*`, `SizeChangeCallProgram` — the exactness theorem, the
  matrix lane and its periodic-cycle results.

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

The lane also checks that ordinary signed-recursion reports omit `size_change` when the flag is
absent, and that two reports in one session get distinct files inside one certificate directory.

The receipt (`scenarios-<commit>.json`) records each report, the certificates' digests, the Lean
version and binary digest, the isolated library's source digests, and the CLI binary digest.

## What the evidence does and does not claim

**Model.** A certified family admits no infinite run that respects the classified relations, and
the guarded run model supplies the positivity premise the classifier needs. A refusal names a site
per measure, and only that: **a refusal means "not shown terminating", not "diverges"**.

**Front end.** The Rust classifier (`crates/noble-contracts/src/source/sizechange.rs`) is an
over-approximation of the body: values are tracked only as far as the classifier needs (the measure,
its predecessor, a list tail, a `0 <` comparison), and every path it cannot model becomes opaque or
`unsupported` rather than an assumed decrease. A counter predecessor counts as a decrease only under
an active `0 <` guard *on that measure* at the site; a list tail counts without a guard because a
cons tail is structurally shorter. Named helper calls are unsupported rather than assumed to return,
so an unanalyzed recursive helper cannot be hidden behind a decreasing direct self-call.

**Not claimed.** There is no correspondence theorem between the classifier and the Rust source, and
none between `List` structure and its length or between a wrapped `I64` counter and a natural
number. Those are the front end's assumptions, stated here so the receipts are not read as a
verified refinement. The general size-change closure (composed graphs over paths) is not decided;
the bounded shared-measure criterion is.

## Receipts

The model audit receipt is `receipt-<date>-<source-commit>.json`; the production-session receipt is
`scenarios-<source-commit>.json`. Both are produced from clean commits and committed as evidence.
Scenario certificates are retained under `evidence/` beside the scenario receipt, at the relative
paths recorded in it. These receipts bind their named source commits; they are not evidence about
later revisions.
