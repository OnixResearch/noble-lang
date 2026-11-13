# Shared candidate execution: criteria frozen before measurement (issue #117)

This file and [criteria.json](criteria.json) were committed **before** any number in this lane was
measured. The machine-readable half is `criteria.json`: `run.mjs` reads it, records its SHA-256 in
the receipt, and refuses to write a receipt on a tree that does not match it. Nothing here may be
edited after a receipt exists; a later decision may only quote it.

The question is narrow. Evaluating a *family* of candidate programs can be done three ways that
share semantics and budgets:

| Arm | Programme | Processes |
| --- | --- | --- |
| `separate` | one graph per candidate, each evaluated over every row | one per candidate |
| `merged` | one multi-output graph holding every candidate, sharing the value the candidates have in common | one |
| `merged-duplicated` | the same candidates in one graph, recomputing the shared value instead of reusing it | one |

The third arm is the decisive control: `merged` and `merged-duplicated` differ only in whether one
computed value is reused, so their difference isolates *sharing* from the process count that
`separate` confounds it with. `floor` (the merged graph over a single row) only attributes
per-invocation cost and is not gated.

## Families (frozen in `criteria.json`)

`ngraph-i64/0`, one `i64` input, twelve candidates per family, every candidate a Core source the
CLI's own extractor turns into a graph.

- **`shared-prefix`** — every candidate is the same prefix value followed by its own tail, written
  so that the merged source computes the prefix once and duplicates the value with `dup`. This is
  the only family in which a content-sharing saving is possible, and the saving is entirely
  structural: extraction emits one node per shared value.
- **`distinct-prefix`** — every candidate is an independent program over the input; no value is
  common to two candidates beyond the input node. Nothing can be shared, so the two merged forms
  are the same program. Their measured difference is therefore the host's **noise floor**, and the
  sharing test (`d2`) is only read when that control stays inside its band.

Both families are also the *poor-sharing* case the issue asks for: `distinct-prefix` is the corpus
where merging can only amortise process starts.

## What the optimisations depend on

| Optimisation | Depends on | Status in this lane |
| --- | --- | --- |
| Reusing one computed value (the CSE that matters here) | the candidates sharing a subterm **and** the producer emitting it once; extraction shares the values a *source* shares, it has no structural CSE pass over duplicated source text | measured as `merged` vs `merged-duplicated` |
| Process amortisation | the candidate count, not the template structure | measured as `merged` vs `separate` |
| Constant pooling across candidates | a structural pass that deduplicates equal constants; none exists (`i64.mul %0 %2` and `i64.mul %0 %8` carry separate `const 3` nodes in the recomputing graph) | not available, recorded |
| Unreachable-node pruning | nothing about the template; extraction prunes and reports `pruned_nodes` | reported from the extract report |
| Interaction counting | the interaction-net reducer (#37) | pending |
| Labelled DUP/SUP over disjoint branches | the labelled contract (#105) **and** candidates that touch disjoint data | pending |
| Row batching (`batch::Prepared`) | the row count; the CLI's `eval` loop already uses one workspace for every row | not part of these arms |

## Metrics and instruments

- **Wall time** — `process.hrtime.bigint` around `spawnSync`, one sample per invocation; a
  `separate` round is the sum of its twelve invocations, a `merged` round is one invocation. Rounds
  are interleaved in a rotated arm order so no arm always runs first. Reported: first round, warm
  median, interquartile range, minimum.
- **CPU** — user+system from `GNU time 1.10`, reported beside the wall time because it is less
  sensitive to a shared host.
- **Memory** — peak resident set from `GNU time`'s `%M` (the `wait4` high-water mark of the child).
  Allocation counts are **unavailable**: the workspace forbids `unsafe`, so no counting allocator
  can exist in any target. Only process peak RSS is claimed.
- **Structure** — `live_nodes`, `layers`, `expanded_nodes`, `pruned_nodes`, `work_units` from the
  `extract` report, and the graph digests. The lane asserts the exact arithmetic: the shared merged
  graph has `prefix + Σ tails` live nodes and the recomputing one has `prefix + 11 further copies +
  Σ tails`.
- **Compilation overhead** — `extract` is timed separately (twelve invocations vs one) and reported;
  it is producer work and belongs to no evaluation arm.
- **Cold** — there is no page-cache drop on this host without root, so "first" only means "not
  previously executed by this run". Every comparison uses warm medians, which are equally warm on
  both sides.

## Correctness gates (all required, before any number is read)

1. The independent oracle of [oracle.mjs](oracle.mjs) — a JS stack machine for the fragment these
   programs use, sharing no code with the CLI — agrees with every candidate, on every row.
2. The merged graph and its recomputing twin agree with the oracle on every row, and with each
   other.
3. The normative reference interpreter (`noble ngraph eval --reference`) agrees with the oracle on
   the first 256 rows.
4. Extraction of one source twice is byte-identical; evaluation twice prints identical lines.
5. The structural arithmetic above holds exactly for both families.
6. Thirteen negative controls keep their codes and exits (permuted outputs caught only by the
   oracle, truncated family, `if`, `test.emit`, Program output, unbound word, wrong profile, raised
   budget, unknown op, dangling operand, wrong vector types, missing `types` line, and the absent
   net arm).

## Decision rule (frozen)

Let `t(arm)` be the warm median wall time of an arm, all at twelve candidates and 8192 frozen rows:

- **d1 — process amortisation:** `t(merged) ≤ 0.75 · t(separate)` on `shared-prefix`.
- **d2 — sharing:** `t(merged) ≤ 0.90 · t(merged-duplicated)` on `shared-prefix`.
- **d3 — noise control:** the two identical merged programs on `distinct-prefix` differ by at most
  10 %; a larger difference invalidates the run rather than supporting d2.
- **d4 — memory:** every arm's second warm round exceeds its first by at most 1024 KiB of peak RSS.

**GO** for further merged-execution work needs every correctness gate and d1, d2, d3 and d4.
**d1 without d2** is recorded as process amortisation only — it is not a common-subexpression
result, and no outcome licenses an optimal-runtime claim. A failed gate is published as a failure;
the thresholds are not adjusted afterwards.

## Not measured here

- The **net arm**: `crates/` has no interaction-net entry point, `noble ngraph` has no net
  subcommand and `noble-ngraph` has no feature gate; pending #37. A control asserts this absence, so
  a future #37 landing trips the lane instead of silently measuring something else.
- The **labelled-SUP arm**: no labelled DUP/SUP contract exists; pending #105.
- Allocation counts, interaction counts, GPU behaviour and any claim about a purpose-built shared-
  candidate engine.
