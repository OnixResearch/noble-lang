# Typed-chunk learned-ranking decision (#74)

**NO-GO for learned-ranking adoption.** Keep the deterministic #73 fixed-chunk completion path; no scorer is required to serve a request. This decision applies only to the frozen corpus, libraries, scorer, budgets and toolchain below. It is not a claim about every possible scorer or task family.

## Bound evidence and reproduction

The criteria (`criteria-v1.json`, committed before the experiment), 480-task manifest (`corpus-v1.json`) and source at `64b164f408f95f80310b96310199add43b1cac1b` define this run. The retained complete run is `run-64b164f408f9.tar.gz` (SHA-256 `7edfce2c00ef638f538f6dc67daaa7d770f9f1ac7073a89e57feab00fc085ed8`): `receipt.json`, the selected model/parameters and selection record, all 1,440 training/validation rows, all 4,800 evaluation rows, 1,200 paired task/seed comparisons, per-task and per-family summaries, the full analysis, and the no-cherry-picking roster. The receipt binds the clean source commit, criteria/corpus BLAKE3 digests, model identity, Rust/Cargo toolchain, all four modes, 120 held-out tasks and ten paired repetitions. `analysis.json` binds the exact source-file and run-artifact SHA-256 digests. The recorded corpus SHA-256 is `861d66edfbb271934c3a101f6edc6f348312349f694d82ed4cb6cd7f22836d07`; the frozen model identity is `9a32deed645432f07a93f626d1b325a13b1676e1f6d36b4917d7cf3f16d27660`.

The completed TCRANK-01–05 change was synced into `.cairn/specs/typed-chunk-ranking/spec.md` and archived at `.cairn/archive/1970-01-01-typed-chunk-ranking-experiment/` (the pinned Cairn CLI's deterministic default archive-date label; not the measurement date). The compatible normative view and requirement ledger are registered under `specs/`; archive structure is not experiment evidence.

From that clean source commit, the lane is reproduced with the repository's reviewed offline Rust/toolchain environment, a private `TMPDIR` and an external output directory:

```sh
node verification/typed-chunk-ranking/runner.mjs
cargo test -p noble-ngraph-experiment --test b5_training --all-features --offline --locked ranking::measure::experiment::frozen_trial_run -- --ignored --exact --nocapture
node verification/typed-chunk-ranking/analyze.mjs <external-run-directory-printed-by-test>
```

The producer rejects a dirty tree or a corpus differing from the committed manifest; the analyzer requires the same clean source commit and checks the full row ordering, identity links, provenance, leakage exclusions, budget fields and frozen decision rule. Run artifacts are retained as data, not as a proof of semantic equivalence.

## Frozen comparison and decision

All four modes ran on the same 120 held-out tasks × ten paired seeds. Each mode contributes 1,200 rows; no failed, unsupported or exhausted task was removed. Work is `Report.checking_work` shared-budget units, not the sum of different resource units. Primitive-only host preparation has no comparable #73 work counter, so its work median is deliberately unreported.

| Mode | Median charged work | Median total latency | Finite fixture success | Independent finite replay |
|---|---:|---:|---:|---:|
| Primitive-only | not instrumented | 286,831 ns | 100% | 100% |
| Fixed-chunk shortlex | 442 | 788,031.5 ns | 60% | 60% |
| Learned ranking | 732 | 2,821,138.5 ns | 60% | 60% |
| Splitmix64 shuffled | 793 | 1,025,960.5 ns | 60% | 60% |

Against fixed-chunk, learned ranking changed median charged work by **−65.61% reduction** (95% paired bootstrap interval −65.61% to −65.61%) and warm scorer-inclusive latency by **−270.58% reduction** (interval −277.13% to −258.25%). Negative reduction means an increase in cost. The 10,000 fixed-seed bootstrap replicates resample 120 task means after averaging ten paired repetitions within each task. Finite success, structural admission and independent finite replay regressions were each 0 percentage points overall and in every family. Learned did not beat shuffled on both primary reductions: shuffled latency reduction was −27.49%, far less adverse. The tracked library/request/model retained-byte proxy ratio was 8.52× versus the frozen 1.25× ceiling.

The frozen GO rule requires *all* conditions: at least 15% median work reduction with lower 95% bound above 5%; at least 10% warm-latency reduction with lower bound above zero; bounded overall/family success, structural and replay regressions; retained bytes at most 1.25×; invalid/fallback rows at most 1%; and an improvement over shuffle on **both** primary metrics. Work, latency, memory and shuffle conditions fail. The gate also fails closed because allocator/process peak retained bytes were **not measured**: results retain the model graph's measured scratch peak, the #73 report's charged retained bytes, library retained bytes and serialized model payload, but label their sum a lower-bound proxy rather than an actual process peak. The negative work and latency results independently determine NO-GO; unmeasured memory cannot turn it into GO. Fewer proposals alone cannot offset scorer and queue cost. The primitive-only 100% finite success is a distinct control, not a proof or an automatic production policy change.

Offline training remains separate: 280 training tasks/1,120 candidate rows, 80 validation tasks/320 rows, zero held-out rows in training; 504 refused attempts retained. Three fixed seeds ran 256 Adam steps each (768 total), selected seed `0x74031a7e2026102a` by the lowest validation F32 loss. Total measured training time was 2,205,612,710 ns; parameter/state bytes were 260/1,064 and measured training scratch peak was 4,176 bytes. No inference cost amortization was claimed. Teacher-assisted labels used 1,080 checked candidate calls with 874,752,453 ns of verification/labeling cost; held-out learned completion was student-only (1,200 rows, zero teacher calls or teacher-assisted completions). Training actual process peak retained bytes also remain unmeasured.

## Authority, controls and ownership

The scorer is a fallible local advisory adapter, not acceptance, proof, behavior or effect authority. Every selected proposal is re-admitted by #73's ordinary source/kernel checker under the same caller budgets; invalid/nonfinite scores, stale provenance and serving absence/failure preserve bounded deterministic fallback. The fixed library and eligibility are identical in fixed, learned and shuffled modes; no library-growth benefit is included. `noble_contracts::source::chunks::CandidateRanker` is explicitly reviewed as an open no-default-body extension in `policy/architecture.ncl`, not described as a closed local trait. Octet collects workspace implementation bodies. Reopen that architecture review if a scorer acquires a capability effect, moves outside the workspace, or core re-admission changes.

The editor ranking controls in `crates/noble-contracts/tests/editor/chunks-completion/ranking/` exercise finite sorting and ordinal tie-breaks, nonfinite scores, stale model/library/goal identities, unknown candidate IDs, equal-signature wrong behavior, bounded work/retention and no partial publication. The underlying #73 metadata/body/capture/dependency/revision/interface/effect forgery controls are in `crates/noble-contracts/tests/editor/chunks/`; the ranked path uses the same admitted library and re-admission. `observations.rs` binds a consumer-owned fixture, optional scorer response, exact candidate and checker outcome to measured cost; `durability.rs` replays a stored ranking without model calls and refuses stale/unknown IDs. The corpus/analysis checks keep held-out expected values out of features, targets and optimizer updates, and retain refused attempts and exact proposer body/dependency provenance with slow re-admission cost. A matching type signature does not transfer #118 procedure behavior or proof evidence.

This is selection learning over fixed candidates, not generation learning. Student-only and teacher-assisted outcomes are separately reported above. Finite fixtures/replay are observations, not universal proof, completeness, semantic equivalence or cross-task generalization. The compiled numeric evaluator's select chooses between already-computed values; this experiment does not claim skipped branch execution, sparse-compute savings, a topology/backend rewrite or learned chunk discovery.

## Issue acceptance mapping

| #74 item | Result | Witness |
|---|---|---|
| Bounded runner, three baselines, independent held-out expectations | DONE | `criteria-v1.json`, `corpus-v1.json`, `runner.mjs`, archive `results.jsonl`/`no-cherry-picking.json`; 120 tasks × 10 paired seeds × 4 modes. |
| Unchanged checker/contracts/effects/budgets; no scorer authority | DONE | `chunks.rs` ranked queue/re-admission, `ranking/ordering.rs`, `ranking/budgets.rs`; archive per-row budgets and results. |
| Training/feature/snapshot provenance, leakage controls, fallback | DONE | `model.json`, `training.jsonl`, `analysis.json`, `ranking/fallbacks.rs`; zero test rows in training and matched source/model digests. |
| Invalid/adversarial/stale/metadata/exhaustion/equal-signature controls | DONE | `ranking/{fallbacks,durability,goals,budgets,ordering}.rs` and `tests/editor/chunks/{envelope,captures,dependencies}.rs`; contracts editor test suite passed. |
| Scorer-inclusive latency/work/memory, separate training, intervals and full roster | DONE with explicit limit | Archive `receipt.json`, `results.jsonl`, `analysis.json`, `paired.jsonl`: work/latency/scratch and charged retained bytes reported; actual allocator/process peak retained bytes **not measured**, so memory cannot support GO. |
| Explicit GO/NO-GO and cost/proposal/proof distinctions | DONE | Frozen thresholds above and `analysis.json` decision `NO-GO`; finite fixture success is not proof. |
| Consumer-owned observation, adapter/checker/cost identity; stale/unknown IDs | DONE | `ranking/{observations,durability,fallbacks}.rs`; archive bound identity and work rows. |
| Refused/slow proposer data, held-out answers excluded | DONE | Archive `training.jsonl` and `analysis.json`: 504 refused attempts, exact proposer source, 0 held-out rows or expected-value feature fields. |
| Student-only versus teacher-assisted calls and verification | DONE | Archive `receipt.json`: 1,200 student-only test rows, 0 inference teacher calls, 1,080 training label calls and 874,752,453 ns verification cost; selection is not generation. |
| #118 exact body/dependency identity; separate library-growth attribution | DONE as scope boundary | `training.jsonl` proposer provenance and frozen library identity in the archive; no signature-only behavior/proof transfer and no library-growth gain claimed. |
| Serving absence/failure keeps bounded deterministic completion | DONE | `ranking/{observations,fallbacks,durability}.rs`; unchanged checker/effect/authority boundaries. |

**Reopen only with** a new precommitted corpus/library and independent task expectations, an immutable scorer and tool snapshot, leakage-safe train/validation/test split, a measured actual memory peak if used for GO, the same checked authority and bounded budgets, and a fresh complete paired run that passes every predeclared threshold. This NO-GO cannot be revised by selecting favorable rows or changing thresholds after inspection.
