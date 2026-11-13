# Typed-chunk fixed-baseline measurement

This lane measures the deterministic two-chunk, ordered stack-tail fixture used
by `chunks-completion.rs`. It records enumerated and checked candidates,
expanded nodes, dependency and instantiation work, checking work, source and
retained bytes, and monotonic end-to-end latency. The receipt binds the exact
clean source commit. These are measurements of one fixed fixture, not a
speedup, completeness, or universal-correctness claim.

`no-result` means every proposal in the finite shortlex search was refused
before a budget was exhausted. It does not mean that no completion exists
outside the bounded whitelist and search space; `exhausted` remains distinct.
The 128-candidate value is a ceiling, not a guarantee that every request can
reach it: each selected chunk is re-admitted per proposal, and that work shares
the 4,096 checking-work limit, which may exhaust first.

`checking_work` is the shared sum of source-aware editor-analysis work,
source-meter charges (including reported ordinary and signed-owner source-path
kernel-check work), independent kernel-recheck work, and ordered-stack replay
steps. A request-preflight refusal conservatively charges the checker's full
declared allowance when its local remaining counter is unavailable. This is
bounded checker-work accounting, not CPU time or instruction count.

## Reproduce

Run after committing the Rust implementation from a clean worktree, using the
repository-pinned Rust toolchain and offline locked dependencies. Set the
receipt path to a new absolute filename outside the repository and `TMPDIR` to
an existing private temporary directory. The test refuses to write a receipt
unless both environment values are supplied, the supplied commit matches
`git rev-parse HEAD`, the tree is clean, and the canonical output parent is
outside the worktree. The receipt filename must not already exist; the producer
refuses to overwrite an earlier observation.

```sh
test -z "$(git status --porcelain)"
NOBLE_TYPED_CHUNK_SOURCE_COMMIT="$(git rev-parse HEAD)" \
NOBLE_TYPED_CHUNK_RECEIPT="<absolute directory outside the repository>/typed-chunk-receipt.json" \
TMPDIR="<private temporary directory>" \
  cargo test -p noble-contracts --test editor \
    --all-features --offline --locked \
    chunks_completion::fixed_baseline_measurement_reports_bounded_work -- --exact
```

The generated receipt is copied into this directory only after the command
succeeds. It is a retained observation for its recorded commit, not an
acceptance authority. The fixed-baseline fixture has no learned ranking or
chunk discovery; #74 remains outside this package and depends on this baseline.

## Deferred proposals

The report-only Re-Pair/Sequitur miner is a follow-up only after the fixed
baseline's acceptance controls and measurement are complete. Its reconsidered
inputs must be checked real source, with per-module candidates/refusals and
independent source preparation plus kernel acceptance for every proposed
rewrite. A recurring candidate must show positive token savings before
library adoption is proposed. Agent-cost word scoring remains deferred until
that report identifies candidates worth testing and an LLM harness, frozen
held-out tasks, pinned sampling/model, and explicit model budget exist.
