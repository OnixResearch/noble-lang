# Issue #58 optimizer-kernel stop decision

Date: 2026-11-13  
Decision: **STOP the `ngraph-opt/1` performance comparison.** The conditional CPU comparison was not run. No latency, speedup, convergence, stability, descent or generalization result is claimed.

## Decision basis

The published `ngraph-opt/1` whitelist is exactly `constant`, `broadcast`, `Binary32.mul` and `Binary32.sub`; the candidate has zero persistent bytes, cannot read `StepDescriptor`, and receives only the current parameter/gradient lanes and `eta`. Thus every admitted candidate is a bounded stateless per-lane expression over those values. That is a structural reading of the grammar, not a measured result. For the proposed comparison against a tuned fixed optimizer plus schedule search, this envelope does not introduce a distinct learning mechanism that warrants the cost of a performance arm. A faster run would not remedy that lack of novelty, and a timing-only result could not close the model/checker/compiler boundaries.

The selected slice is therefore the executable conformance work only: bounded admission, exact-bit evaluation, finite Rust-to-Lean rows, immutable snapshot/provenance binding, whole-trial rollback and frozen-artifact tampering controls. Semantic validation and any future performance comparison remain separate.

## Frozen conditional comparison protocol (not activated)

If a later owner decision reopens a comparison, the comparison must use these controls before collecting timing data:

- Pair the fixed `ngraph-opt/1` candidate with the tuned fixed-optimizer/schedule-search baseline on the same frozen ordered data, initialization, iteration count and CPU host.
- Use three paired seeds: `5801`, `5802`, and `5803`; keep each pair's seed, gradient-evaluation/work budget and total wall-clock cap identical across arms.
- Count proposal/search, admission, correctness checks, checkpointing, failed or refused trials and optimizer execution inside each arm's wall-clock cap; fix the cap and warm/cold setup policy before any timing run.
- Freeze candidate and baseline identities before the run. Only candidates that pass the independent correctness checks may enter the timed arm.
- Report each seed pair separately and keep correctness/refusal outcomes separate from timing. Do not use a pooled speedup to erase a failing or refused pair.
- Run no performance arm if the candidate still expresses only the same stateless per-lane rule family that the baseline schedule search covers.

This is a preregistered *conditional plan*, not a record that the comparison was executed. No timing command or timed candidate/baseline arm is part of this closeout.

## Reopen condition: separately versioned trace-monomial family

A future learning-rule proposal must not widen or silently revise `ngraph-opt/1`. Reopen only through a separately versioned `trace-monomial/1` design. Before coefficient search, that design must freeze the tiny MLP and budgets, measure the generated order-4 dictionary per layer at locality degrees 1 and 2 plus contraction work, and check rather than assume the paper's reported dictionary-size range. It must name a Lean locality theorem and an out-of-neighbourhood refusal control, choose whether downstream activations are held constant or differentiated through, and separate real-model permutation symmetry from binary32's order-sensitive contractions. Its first executable proposal must be one hand-written Hebbian monomial checked against an independent reference and the same admission/snapshot/rollback controls. Those design gates are recorded in `.cairn/changes/optimizer-kernel-experiment/design.md`; no such family, search, or measurement is implemented here.

## Evidence boundary

- **Model:** `proofs/foundation/NobleFoundation/SgdReference.lean` is the kernel-checked reference model; the generated `SgdReferenceCorpus.lean` contains finite Rust-row propositions decided by Lean.
- **Checker and execution:** `noble-ngraph-experiment/src/opt/` implements the typed parser/interpreter and host trial boundary. Its controls are finite Rust tests; they do not prove general checker soundness or compiler/executed-artifact correspondence.
- **Snapshots and rollback:** the host harness binds exact candidate bytes, parent parameter/state identities, graph/loss/derivative/data identities, seed, numeric semantics and tool revisions. Caller-provided graph/loss/derivative digests are recorded identities, not independently verified artifact proofs.
- **Evaluation isolation:** candidate execution is a pure closed interpreter over frozen bytes and explicit numeric inputs. This is not an OS process sandbox and does not prove the host's secret custody.
- **Performance:** not run, by decision. No conditional item is treated as a measurement pass.
