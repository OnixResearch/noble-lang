# lean-crush as a reconstruction-only MC1 producer (issue #83, evaluation)

Evaluation record for a **bounded experiment** under PAUTO-FO-6. It adopts no
dependency, changes no authoring default, adds no certification and does not
reopen design #6 or its selected first profile PAUTO-P1.

## Decision

**Compatibility: go.** The pinned crush builds with Noble's pinned Lean
4.31.0 once one upstream module is patched by a single line, its declared
dependency closure is empty, and under `crush.trust "reconstruct"` the goals it
does close depend only on `propext`, `Classical.choice` and `Quot.sound`.

**Adoption today: no-go**, on two measured grounds:

1. **The consumer cannot admit the producer's library.** Submission
   elaboration mounts only `/library:/obligation`
   (`crates/noble-cli/src/workflow/verification/compilation.rs`, the
   `compile_submission` search path). A candidate proof that imports `Crush`
   is refused with `proof-rejected`; the adapter's oleans have no path into
   that sandbox, and `compile_library` recompiles every library module from
   source rather than copying prebuilt objects.
2. **Usefulness on the real MC1 claim corpus is nil.** None of the seven frozen
   contracts' claims closes with strict reconstruction inside a 60 s solver
   budget; the reviewed template proofs prove all seven. The producer is
   useful on the first-order sub-goals those claims *reduce to* (5 of 5 true
   BitVec goals closed in under 1 s each), not on the claims themselves.

Per issue #83, a precise no-go with reproducible evidence is a valid result. No
repository code, contract, profile, axiom allowlist or Lean pin is changed, and
the experiment grants no release authority.

## Pinned identities

| Item | Value |
|---|---|
| Noble source | `2238cb51` |
| Upstream | `AD1024/lean-crush` at `16d19d729fdaa3bf6fc527f9734700b0d149d0c4` (upstream HEAD, MIT) |
| Upstream toolchain | `leanprover/lean4:v4.34.0` |
| Upstream dependency closure | **none** (`lake-manifest.json` `packages: []`) |
| Noble pin | `leanprover/lean4:v4.31.0`, commit `68218e876d2a38b1985b8590fff244a83c321783` |
| Adapter | `receipts/2026-10-31/adapter-431.patch`, one line, sha256 `16becdf9…` |
| Solver | z3 5.1.0 (`/nix/store/c1vmvbqyqzzpndnb46v6b9h09jbbyrva-z3-5.1.0`) |

Exact digests are in `receipts/2026-10-31/pin.json` and
`receipts/2026-10-31/sha256sums.txt`.

## Compatibility measurement

The one-line adapter changes `rw [ite_eq_right nonnegative]` to
`rw [if_neg nonnegative]` in `Crush/Solver/Alethe/ReplayRules.lean`: Lean 4.31
has no `ite_eq_right`, and `if_neg` states the same step. Nothing else changes —
no axiom, no toolchain file, no lakefile option, no solver default, no trust
default.

| Run | Result | Transcript |
|---|---|---|
| Unpatched, Lean 4.31 | fails at target 95/108, `Unknown identifier ite_eq_right` | `receipts/2026-10-31/build-unpatched-4.31.log` |
| Patched, Lean 4.31 | **108/108 targets**, no warning treated as an error | `receipts/2026-10-31/build-adapter-4.31.log` |

Trust-policy probe at the pinned release (`receipts/2026-10-31/trust-policy.log`):

| Goal | Policy | Axioms Lean reports |
|---|---|---|
| `(x = y) → (y = 3) → x = 3` | default (`crush`) | `Crush.crushSorry` |
| the same goal | `crush.trust "reconstruct"` | `propext, Classical.choice, Quot.sound` |
| `a = b → a + 0 = b` over `BitVec 64` | `crush.trust "reconstruct"` | `propext, Classical.choice, Quot.sound` |

So the default policy is exactly the trusted-solver escape the issue names, and
strict reconstruction is the only admissible mode.

**Constant audit** (`receipts/2026-10-31/constants.log`): the reconstructed
terms for those two goals reference only core constants (`Int`, `Eq`,
`OfNat.ofNat`, `instOfNat`, `BitVec`, `Nat`, `instOfNatNat`) — no `Crush.*`
constant survives in the proof term. A self-contained-term route is therefore
possible in principle for first-order goals, which is one of the unblock
options below; it is **not** sufficient for the claim corpus, where nothing
closes at all.

## The admissibility blocker, measured

`compile_submission` mounts the workspace library and the generated obligation
and runs Lean with search path `/library:/obligation`. Neither the Crush
adapter nor any olean-only directory is on it. Control `crush-import` submits
the patched producer's candidate for `increment` to the unmodified consumer:
it is refused at elaboration, not checked.

`compile_library` does not help: it walks the enumerated library modules and
recompiles each from source with the pinned release, so prebuilt adapter
objects are not copied into the workspace.

This is a consumer-side gap, not a crush defect. Admitting an adapter would be
a consumer change with its own design questions (which library is
independently selected, how its oleans are bound, what replay then proves), so
this evaluation records the gap instead of widening it.

## Corpus

The frozen corpus is the existing MC1 contract and evidence set in
`verification/mc1/`, emitted by the consumer itself (`noble verify … --emit`) so
each producer run sees exactly the obligation the consumer regenerates.

| Contract | Consumer outcome with reviewed evidence | Producer outcome (strict, 60 s solver budget) | Producer wall time |
|---|---|---|---|
| `increment` | proved | refused: solver `unknown` (no reason given) | 588 765 ms |
| `wrap` (signed wrap boundary) | proved | refused: solver `unknown` | 564 225 ms |
| `composed` | proved | refused: solver `unknown` | 569 805 ms |
| `syntax` | proved | refused: solver `unknown` | 574 994 ms |
| `family` | proved | refused: solver `unknown` | 566 390 ms |
| `structural` | proved | refused: solver `unknown` | 577 941 ms |
| `monotonic` (false monotonicity claim) | disproved by the reviewed refutation | refused: solver `unknown` | 576 197 ms |

**0 of 7** claims close. The requested 60 000 ms budget produced roughly
570 s of wall time per claim (crush spends far longer than the requested
budget; the increment claim alone ran 590 s under a 120 000 ms setting) and
still returned no verdict. Every refusal names the solver, not a semantic
finding, so no claim is misreported as unprovable.

Per-contract producer transcripts, candidates and consumer reports are in
`receipts/2026-10-31/corpus/`.

### First-order sub-goals (where the producer is useful)

| Goal | Closed | Wall time | Axioms |
|---|---|---|---|
| `a = b → a + 0 = b` | yes | 907 ms | allowlist |
| `a = b → a * 1 = b` | yes | 816 ms | allowlist |
| `(a - b) + b = a` | yes | 867 ms | allowlist |
| `a + 1 - 1 = a` | yes | 952 ms | allowlist |
| `a = b → b = c → a = c` | yes | 857 ms | allowlist |
| `a < a + 1` (false, monotonicity-shaped) | refused: solver model | 836 ms | `sorryAx` only |

That is the honest usefulness split: the SMT translation closes the arithmetic
facts a contract's claim depends on, and refuses the claim itself, whose shape
is a stack-typed invariant with an existential witness rather than a
first-order formula.

## Retained-proof replay

The consumer already replays a submission as bounded textual declarations in a
fresh kernel process, with `independent_recheck: true` on every accepted run
(`recheck_method` in the control reports). Two facts bear on replaying a
crush-produced proof:

- the reconstructed terms of the goals crush **does** close reference only core
  constants (`receipts/2026-10-31/constants.log`), so no producer object,
  dynlib or `Crush` declaration would have to cross;
- a producer-local axiom is rejected at export
  (`MC1_AXIOM_REJECTED`), before replay, so a trusting-mode escape cannot be
  replayed into acceptance.

Neither is a replay result for a crush proof: no claim proof exists to replay,
and the submission that would create one is refused at elaboration. The
replay question is therefore **blocked by the same admissibility gap**, and is
not reported as answered.

## Controls

| Control | Measured outcome | Exact diagnostic |
|---|---|---|
| `crush-import`: candidate importing the adapter | refused at elaboration | `proof-rejected`: `error: unknown module prefix 'Crush'`, search path `/library`, `/obligation` |
| `unapproved-axiom`: `∀ p, p` standing in for the solver's word | refused before replay | `proof-export-rejected`: `MC1_AXIOM_REJECTED: producer-local axiom MC1Proof.unapprovedEscape` |
| `wrong-name`: right proof shape, wrong theorem name | refused | `proof-export-rejected`: `MC1_THEOREM_MISSING: expected theorem MC1Proof.proof` |
| `kind-mismatch`: proof offered as a refutation | refused | `proof-export-rejected`: `MC1_THEOREM_MISSING: expected theorem MC1Proof.refutation` |
| `stale-subject`: increment evidence against the `signed-wrap` contract | refused | `proof-rejected`: `'change' tactic failed` against `signed-wrap`'s claim |
| positive: reviewed `increment` template | **proved** | `outcome: proved`, `independent_recheck: true` |
| producer: dependent-type goal | refused | solver found a model plus "The encoding is incomplete… a model does not necessarily describe a Lean counterexample" |
| producer: variable-width `BitVec w` | refused | "crush: cannot translate the dependent type BitVec w … without risking false proofs" |
| producer: reconstruction exhaustion at 200 ms | refused | "solver reported `unsat`, but reconstruction failed — no finishing tactic could replay it from the core" |

Refusal classes stay distinct — unsupported translation, solver unknown, solver
model, reconstruction failure and consumer rejection are never collapsed into
"unsolved" or, worse, into an accepted theorem.

The axiom control matters most: a producer-local axiom is rejected **at export**,
before any replay, so an unapproved escape claim never becomes a theorem the
consumer could accept. That is the same mechanism that rejects
`Crush.crushSorry` if a trusting-mode candidate ever reaches export.

## Non-claims

- A passing consumer check is not evidence that the claim is useful, nor that a
  proposed refutation matches the original semantics.
- The SMT translation's fidelity to Lean semantics is unproven; the encoding is
  visibly incomplete in the dependent-type case above.
- Solver SAT models describe the encoding, not necessarily a Lean
  counterexample.
- Nothing here speaks to termination, implementation correctness or
  correspondence.
- Crush remains an untrusted producer. No new certification or release
  authority is granted, and PAUTO-P1 does not change.

## What would unblock adoption (owner decision)

1. **A consumer-selected adapter profile.** Admit one independently selected
   olean directory to the submission search path, bound to the pinned release
   and bound to a reviewed source digest, so a submission may import it. The
   replay would then still have to prove the imported declarations are the
   reviewed ones.
2. **Self-contained producer output.** Require the producer to inline its proof
   term and the rule lemmas it uses, so no import is needed. The constant audit
   shows the first-order terms already reference only core constants; the claim
   corpus has no proof to inline yet.
3. Both options keep the reconstruction policy mandatory; trusting mode must
   stay inadmissible, as the axiom audit already enforces.

Until one of them exists, crush cannot produce an independently checked MC1
claim proof through Noble's selected consumer, and the pre-registered outcome of
this experiment is no-go.

## Reproduce

```sh
# adapter build (Lean 4.31 on PATH, adapter patch applied)
lake build Crush
# trust-policy and constant probes
lake env lean AdapterCheck.lean ; lake env lean AdapterConstants.lean
# corpus emission with the real consumer
noble verify verification/mc1/increment.noble-contract --proof verification/mc1/increment-proof.lean --emit NEW_DIR
```

The exact commands, tool paths and transcripts behind every number here are the
files in `receipts/2026-10-31/`.
