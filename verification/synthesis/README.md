# Bounded typed program synthesis (issue #116)

[`cegis.mjs`](cegis.mjs) is a deterministic, finite search and counterexample-guided refinement
lane over Noble's existing CLI. The benchmark set is frozen in [`benchmarks-v1.txt`](benchmarks-v1.txt);
[`decision.md`](decision.md) records the grammar, proof boundary, criteria and reopen conditions.
The canonical corrected run is [`receipts/2026-11-13/synthesis-v2.json`](receipts/2026-11-13/synthesis-v2.json).
The earlier [`synthesis.json`](receipts/2026-11-13/synthesis.json) is retained as historical
evidence; its generated control theorem exposed the emitter's `simp`-closed-goal issue in the
foundation gate and was superseded by this corrected run.

## Grammar and trust boundary

`synth-i64/0` is a typed straight-line stack grammar over the declared inputs, constants
`0, 1, 2, 3, 5, 7`, wrapping `+`, `-`, `*`, terminal `=`, and wiring words `dup`/`swap`.
The run uses seed `0x9e3779b97f4a7c15`, maximum 8 words, stack depth 4, 400,000 admitted
extensions and 4 refinement rounds; each solver query is limited to 10,000 ms. Rejected
ill-typed/depth-impossible extensions do not consume candidate fuel; observational duplicates
are counted before pruning. The plain enumerative baseline uses the same admission and fuel
rules without observational-state deduplication. `synth-list/0` is separately bounded and
run-only. The complete frozen specification is the source of truth for each task and split.

A solver model is a counterexample only after it replays through this lane's evaluator on the
original reference and candidate graphs. Solver `UNSAT` is never a proof. A program is **proved**
only when its I64 graphs have equal affine forms modulo $2^{64}$ and the foundation kernel accepts
the generated `bv_omega` theorem. The theorem is about the graph model, not about extraction,
encoding or the source-to-model translation. Other successful candidates remain **unproven**
with finite held-out evidence. List programs are only run through `noble live repl` because the
current graph IR cannot represent aggregates. The existing shared QF_BV implementation is
[`../symbolic/qfbv.mjs`](../symbolic/qfbv.mjs); this lane does not duplicate the issue #56 encoder,
evaluator or solver harness.

## Frozen run results

`discovered / plain` is the number of admitted one-word extensions in observationally-pruned
search versus the plain baseline; each is bounded by the same declared fuel. A `-` means no
candidate was emitted. Held-out values were not shown during search.

| Task | Candidate | Outcome / verifier | Discovered / plain | Held-out |
|---|---|---|---:|---:|
| `arithCommuted` | `7 + +` | **proved**, `arithCommuted_certified` | 235 / 360,180 | 2/2 pass |
| `arithSubtract` | `- 7 +` | **proved**, `arithSubtract_certified` | 127 / 73 | 2/2 pass |
| `arithScaled` | `7 + 5 * +` | **proved**, `arithScaled_certified` | 9,906 / 400,000 (plain fuel) | 2/2 pass |
| `arithProduct` | `5 + dup 2 - * +` | **unproven**; nonlinear graph, both solvers `UNSAT` | 318,345 / 400,000 (plain fuel) | 2/2 pass |
| `arithEquality` | `=` | **unproven**; Bool output outside affine theorem class | 4 / 69 | 2/2 pass |
| `bitAnd` | `*` | **unproven**; 0/1 precondition is not a theorem hypothesis | 9 / 359,619 | 2/2 pass |
| `bitOr` | — | **exhausted** at size 8 after two refinements | 258,284 / 1 | not run |
| `bitXor` | `- dup *` | **unproven**; nonlinear graph, both solvers `UNSAT` | 203 / 1 | 2/2 pass |
| `listSingleton` | `nil cons` | **run-only**; `I64 -- List<I64>` | 15 / 137 | 2/2 pass |
| `listDouble` | `dup nil cons cons` | **run-only**; `I64 -- List<I64>` | 798 / 169 | 2/2 pass |
| `contradictory` | — | **unsatisfiable**: one input has two expected outputs | 0 / 400,000 (plain fuel) | not run |
| `overBound` | — | **exhausted** at bound 6; the same task solves at control bound 7 | 13,584 / 50,731 | not run |

The checked `overBoundControl_certified` theorem certifies the wider-bound control candidate. For
For every emitted I64 candidate, re-admission used `noble ngraph extract`, both `noble ngraph eval`
evaluators (which agreed), and `noble worker scalar` on the task's declared interface. The Bool
output candidate was re-admitted through extraction and both graph evaluators; it is not an
`I64 I64 -- I64` scalar call. The two list candidates were run through `noble live repl`. Each
reported held-out result is 2/2; no held-out example was used to refine search.

### Counterexample traces

The bounded negative searches retained these exact, lane-replayed refinements:

| Task | Round | Candidate | Inputs | Candidate value | Reference value |
|---|---:|---|---|---:|---:|
| `bitOr` | 1 | `-` | `(1, 1)` | 0 | 1 |
| `bitOr` | 2 | `0 * -` | `(0, 1)` | 0 | 1 |
| `bitXor` | 1 | `-` | `(0, 1)` | -1 | 1 |
| `bitXor` | 2 | `+` | `(1, 1)` | 2 | 0 |

The generated Lean module also includes exact-witness refusal controls for the first solver-refuted
candidate of each bitwise task. These are negative model controls, not certificates about later
candidates. The four positive certificates cover the three affine arithmetic winners and the
bound-7 control. No theorem is emitted for a solver `UNSAT` alone.

## Shared symbolic solver regression

After moving the shared query core to `verification/symbolic/qfbv.mjs`, the clean reproduction at
the same source commit retained the outcomes: 12 distinct models replayed as `refutes`, 8 models
were rejected, and 9/11 mutated queries were detected. The seven corpus pairs retained three
kernel-certified equalities and four refutations; solver `UNSAT` remained inconclusive. This is
the issue #56 lane's model-level evidence, separate from the synthesis certificates above.

## Provenance and resource report

The receipt was produced by pinned Node 24 from clean source commit
`7add60d54abaaf8f7d48f15d3de1ac2635bc1fe6` (`clean: true`); it binds the frozen manifest,
`cegis.mjs`, and `qfbv.mjs` hashes. Its Lean-emission freshness check is true: both the generated
and committed module are 7,482 bytes with SHA-256
`ba6c03baef3e1f497bf87334c1a04f72fb962ef4e55661bc178d89e868d7a64f` (9 theorem emissions).
The strict `slot.sh lean 3 verification/ci/lean-proofs.sh --allow-dirty ...` run passed
`m3-build=0`, `m3-gate=0`, `foundation-build=0`, and `foundation-check=0`; a separate clean
foundation receipt recorded `allow_dirty: false` for the solver-lane reproduction. The corrected
CEGIS process reported 62.521 s wall, 11.938 s user CPU, 1.555 s system CPU, 4,100,824 KiB
process peak RSS, 22 solver runs and 47,236 KiB solver peak RSS. Per-task search,
baseline, solver and CLI timings/hashes are retained in the receipt.

Reproduce from a clean tree after building the CLI with the pinned `cargo.sh`; use new output paths
outside the repository:

```sh
env -i PATH=/run/current-system/sw/bin:/usr/bin:/bin \
  HOME="$HOME" USER="$USER" LANG=C.UTF-8 TMPDIR="$HOME/.cache/noble-iss/tmp" \
  /nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node \
  verification/synthesis/cegis.mjs \
  --noble "$HOME/.cache/noble-iss/target/synth-116/debug/noble" \
  --bitwuzla /nix/store/5i2dxw5dxqx73sb235zgb904yc3jlh49-bitwuzla-0.8.2/bin/bitwuzla \
  --z3 /nix/store/qzngzax9x0f84g5iapqr3xjc4vg1a2sz-z3-4.15.8/bin/z3 \
  --time /nix/store/mqldjw98f2f4w2glz0f12qdnvv8m4aq9-time-1.9/bin/time \
  --output "$HOME/.cache/noble-iss/scratch/synth-116/reproduced.json" \
  --check-lean proofs/foundation/NobleFoundation/SynthesisCertified.lean \
  --scratch "$HOME/.cache/noble-iss/scratch/synth-116"
```

The reproduction records search and run evidence; it does not itself run Lean. Run
`~/.cache/noble-iss/slot.sh lean 3 verification/ci/lean-proofs.sh --allow-dirty <scratch>`
to build and kernel-check the foundation proofs. No claim is made about unrestricted synthesis,
all programs in the grammar, general 64-bit bitwise operators, solver proof production, list
extraction, or source-to-graph preservation.
