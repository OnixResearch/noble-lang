# ADAPT-17 finite layout-trial decision

## Decision

**NO-GO. Keep the selected managed-linear-memory backend as the default.** The explicit experimental path, `noble compile --lowering type-directed-experimental`, remains opt-in and reports fallback for unsupported shapes. No measured result changes the default compiler, the ordinary CLI path, or completed M3 acceptance.

The reason is the predeclared warm-time ceiling: `runtime-captured-quote-and-composition` with optimization off regressed from a 5 μs baseline median to 6 μs for the candidate (**20.00%**), above the 5% maximum per-cell regression. All correctness and control checks passed, but one failed gain criterion makes the outcome NO-GO. The decisive cell differs by one microsecond; this follows the frozen rule rather than treating the small absolute duration as a post-hoc exception.

## Frozen protocol and evidence

The selected candidate is a bounded specialization for the statically known finite-Sum fragment. It preserves the checked source recipe, avoids boxing a known scalar and allocating its transient Sum wrapper, and schedules only the statically selected quotation. Unsupported operations use baseline lowering. The constructor-reuse part of unit 4 was dropped separately: current one-level reachability evidence cannot establish uniqueness over shared tails, roots, captures, or observable recipes. The candidate does not mutate an existing payload or infer uniqueness. The exact reason and reopen conditions are in the unit-4 section of `freeze.md`.

The final thresholds and controls are frozen in `freeze.md`. Earlier baseline preflight and paired-harness calibration attempts exposed design defects and are excluded from final evidence; the final corrected paired protocol was frozen before the full paired measurement. The final run records four workloads × two lowering arms × two optimizer settings (16 cells), plus 52 controls, under the same pinned Node/V8 and declared limits. It passed all **2,562** gate checks.

To reproduce, use the selected Node 24 binary and flags shown by the reviewed runtime configuration, set `NOBLE_TARGET` to the Cargo target directory containing `debug/noble`, and follow the invocation comment at the top of `gate.mjs` with a fresh `<new directory outside the repository>` and `full` mode. The harness refuses a dirty source tree and writes raw artifacts and its receipt only to that external directory.

- Final paired receipt: `receipts/2026-11-13-full.json`, SHA-256 `8781a8e3c2ca81ea91ffadeab5b12232fd22965406687fdf96543f4c9cb1ca48`; source commit `6b1b193673ba2fba14508a4edff9a5c854b0954e`, clean tree `a0acf4beacd65bcfdca6fae4d5c2cf16a703c05b`.
- Baseline-only companion receipt: `receipts/2026-11-13-baseline.json`, SHA-256 `2d1ee2e894d2bada21fb22ae17b50ee7a12869802b55512ea804043fe845ff8f`.
- The retained receipts include the pinned tool and binary identities, source and input digests, module/WAT identities, complete typed output stacks, normalized recipes and identities, ordered request prefixes, fallback reports, quota/trap results, cleanup, allocation metrics, memory metrics, and cold/warm timing.

## Results

Warm medians are microseconds, from seven samples after three warm-ups on each already-compiled artifact. Negative regression means a candidate improvement.

| Workload | Baseline off → candidate off | Baseline on → candidate on | Optimizer-averaged gain |
|---|---:|---:|---:|
| `two-arm-variant-both-arms` | 22 → 7 (−68.18%) | 8 → 7 (−12.50%) | 53.33% |
| `nested-pair-list` | 7 → 7 (0%) | 7 → 7 (0%) | 0% |
| `runtime-captured-quote-and-composition` | 5 → 6 (+20.00%) | 6 → 6 (0%) | −9.09% |
| `reflection-only-reachable-recipe` | 12 → 9 (−25.00%) | 10 → 8 (−20.00%) | 22.73% |

The candidate reduced aggregate guest allocation charge from **11,808 to 11,424 bytes** (384 bytes, 3.25%). Receipt-level linear-memory size was **8,388,608 bytes** for each arm; per-run `memory_bytes` was **1,048,576 bytes**; retained-before-begin guest bytes were **0** for both. Candidate savings and the 95/96/97-byte allocation transition matched the corrected two-cell expectation. Full stacks, recipes, identities, ordered effect/request prefixes, fallback, quota stages, traps, and cleanup agreed with the independently derived finite expectations. No accepted case made candidate preparation requests.

## Scope and limitations

The result is a finite experiment, not a universal semantic-equivalence theorem, a source-level linearity/ownership proof, a WIT ownership claim, or a promotion of resource-positive Wasm behavior. The Node/V8 host reports guest linear-memory and allocation observations; complete engine/process RSS is unsupported, not zero. The measurements do not establish physical-GC behavior or cross-engine portability. The independent one-level `ConstructorReuse.lean` model does not prove transitive source/runtime ownership correspondence.

## Reopen conditions

Keep the default unchanged unless a new clean, source-bound paired trial passes **all** frozen criteria: 100% exact semantic/control agreement; per-specialization allocation savings matching the eliminated 48-byte cells, with aggregate guest charges, linear-memory size, and retained-before-begin bytes no worse than baseline; at least 5% warm-time gain in two or more workload families; and no workload × optimizer cell regressing more than 5%. Preserve the same pinned configuration and finite observables, and record unsupported measurements honestly. Do not relax the thresholds to admit the current 20% cell regression.

Reconsider constructor reuse only with a source-checker-derived transitive ownership/reachability witness for every live root and saved recipe, a model connecting that witness to actual runtime cell layout/replacement semantics, and executed shared-tail, capture, observable-recipe, quota, trap, and cleanup controls. Last-use analysis or the present depth-one model is insufficient.
