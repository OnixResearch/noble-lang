# Decision: the synthesis grammar, what is expressible, and what counts as proved (issue #116)

This records the decisions the lane implements, the evidence for them, the criteria they were
frozen with, and what would reopen them. The frozen benchmark set they govern is
[`benchmarks-v1.txt`](benchmarks-v1.txt); the corrected run that reports against it is
[`receipts/2026-11-13/synthesis-v2.json`](receipts/2026-11-13/synthesis-v2.json). The earlier
`synthesis.json` is retained as superseded historical evidence after the first foundation gate
found an emitter tactic following a simplifier-closed goal.

## D1 — Grammar `synth-i64/0`

A program is a straight-line sequence of words over the interface's inputs. The stack holds the
declared inputs deepest first (slot 0 at the bottom); a program is complete when exactly one value
of the task's output type is left.

| Words | Meaning |
|---|---|
| `+` `-` `*` | `a b op` with `a` deeper and `b` on top; wrapping I64 arithmetic, as `PackedSchedule.evalStep` |
| `=` | `a = b`, yielding Bool. No word consumes a Bool, so `=` can only be a program's last word |
| `dup` `swap` | wiring on the stack top; both are Core's own builtins |
| `0 1 2 3 5 7` | the declared constants, pushed as I64 |

The arithmetic fragment is exactly `+ - * =`, as the issue's plan directs. `dup` and `swap` add no
arithmetic — they are the Core builtins a caller already has, and the frozen or/xor encodings
cannot be *written* without them. `drop` is excluded because a discarded value never needed to be
computed, so it cannot shorten a size-minimal program; it would only enlarge the search.

Bounds, all declared in the manifest: the size bound (words) per task, the depth bound
(`max-stack`), the fuel bound (`max-candidates` admitted one-word extensions in total) and the
refinement budget (`max-rounds`). A candidate is admitted only after type checking, output-type
reachability, and a lower-bound check that it can complete within the remaining size bound;
rejected word/stack combinations and impossible continuations do not consume candidate fuel.
Observational-equivalent extensions are counted before deduplication. The plain baseline applies
the same admission and fuel rules but retains equivalent states. The final size row is marked
`partial` if discovery stops as soon as it generates a matching program at that size. Nothing is
unbounded, and a bound is reported with every status.

## D2 — Bounded expressibility and proof boundary

`synth-i64/0` is the operational stack grammar in the manifest, including `dup` and `swap`; it is
not a claim that all programs in the grammar are affine. In particular, `dup` permits
input-dependent products, and `swap` can expose either of the two inputs. The search is finite at
the declared word-count, stack-depth, candidate-fuel and refinement bounds. Its negative results
are only the reported results for those bounds and the frozen examples; no unbounded
inexpressibility theorem is claimed.

The bitwise-family references are I64 encodings over the explicit 0/1 input domain: multiplication
for `and`, `x + y - x y` for `or`, and `x + y - 2 x y` for `xor`. The `bitOr` and `bitXor` entries
are bounded negative searches. The lane records any solver counterexample and refines; it reports
`exhausted` only when its bounded enumeration completes, `inconclusive` if a solver, round or
candidate budget does not settle the search. No general 64-bit bitwise operator is introduced.

`arithProduct` exercises a nonlinear reference `x + (y+5)(y+3)`. It may be discovered and
solver-checked, but it is outside the conservative certificate class because its graph multiplies
two input-dependent values. `bv_omega` is used only when both graphs' affine forms are equal
modulo 2^64; this is a sufficient, explicit criterion, not a claim about every identity
`bv_omega` might prove.

## D3 — What counts as proved

**Proved** requires all of: the winner's and the reference's graphs are I64-valued; both are
affine over the declared inputs; their affine forms are equal modulo 2^64. The lane computes that
predicate *before* generating anything, and then emits
`<task>_certified : ∀ inputs, agreesOn <task> <inputs>` into
`proofs/foundation/NobleFoundation/SynthesisCertified.lean`, proved by unfolding the model and
closing with `bv_omega`. The theorem is proved only when the foundation lane
(`verification/foundation/check.mjs`) accepts it; this lane does not check the kernel and says so
in the receipt.

Everything else is **tested**, at one of these strengths, reported separately per program:

| Class | Status | Why it is not proved |
|---|---|---|
| nonlinear winner (a product of two input-dependent values) | `unproven` | the affine forms are undefined; `bv_omega` cannot prove distributivity |
| Bool output (`=` last) | `unproven` | a Bool equivalence is not a linear equivalence of I64 graphs |
| 0/1 domain target (`bitAnd`) | `unproven` | the domain restriction is a solver assumption, not a graph equivalence; a theorem would need the precondition as a hypothesis, which is outside the frozen proved class |
| list family | `run-only` | the graph IR has no list type, so extraction refuses aggregates and nothing solver-checkable exists |

Solver UNSAT is recorded and never certifies — the rule [`../symbolic/README.md`](../symbolic/README.md)
already fixes for this repository.

## D4 — Where it runs

A Node lane over the existing command line: no Rust, no new crate, no new compilation unit, and no
source-inventory renewal. The encoder, the lane's own evaluator, strict admission, the encoder
checks and the measured solver runs are *imported* from `verification/symbolic/qfbv.mjs` (the part
of issue #56's lane this one reuses) rather than copied, so a candidate is decided by exactly the
code the solver lane decides its own claims with.

## D5 — Trust separation, per program

| Step | Trusted? | How it is reported |
|---|---|---|
| discovery (bottom-up enumeration with observational-equivalence pruning) | no | candidate counts and states per size |
| refinement (a solver model becomes a counterexample) | no — the model must replay through this lane's evaluator | the witness's exact bits, per round; the first refused candidate per task also receives a separate kernel-replayed witness theorem when available |
| proof | yes, kernel-checked, and only after the foundation lane runs | the theorem name and the emitted module's SHA-256 |
| re-admission (`noble ngraph extract`, `noble ngraph eval` both evaluators, `noble worker scalar`, `noble live repl`) | the CLI's own paths | per program, per point |

Re-admission is finite evidence, not a theorem: extraction acceptance is the kernel's own
admission, the two graph evaluators are the CLI's, and the scalar endpoint is a compiled Wasm
module run on V8. Extraction preservation (NG-PO-A) stays open.

## Reopen conditions

1. **A grammar version bump** (`synth-i64/1`, a new frozen manifest) is required to add words or
   change existing word semantics, including a native bitwise operator. Do not edit `/0`:
   retained receipts bind its SHA-256.
2. **A wider proved class** (for example distributive identities, or a domain-restricted theorem
   carrying the 0/1 precondition as a hypothesis) is a new theorem family with its own frozen
   criteria; it must not be folded into `_certified` silently.
3. **Re-run the lane and re-bind the receipt** if `verification/symbolic/qfbv.mjs`,
   `crates/noble-ngraph/src/ir.rs`'s semantic revision or I64 profile, the manifest, or the
   selected CLI changes.
