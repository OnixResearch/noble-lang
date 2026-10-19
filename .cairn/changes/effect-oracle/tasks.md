## Phase 1: Design only (this change)

- [x] [serial] Re-verify the existing effect solver at `de1044b`: constructors and bounds (`effects.rs:8,54`), rigid symbolic pass (`solve.rs:37`), neutral-only equation propagation (`solve.rs:136`), fixed-point concrete passes and constant exactness (`solve.rs:192-343`), universe derivation (`source.rs:815`), signature application (`source/inference/mod.rs:233`; `completion.rs:5`) and the existing inline regression (`solve.rs:416`). Record drift from the issue's `66eb7cd`. r[FXORACLE-SEM-01]
- [x] [serial] Select `finite-effects/v1`: independent finite universe, supported fragment, concrete assignment semantics, greatest-solution theorem, rigid quantifier order, universal-hole goals, inconsistency versus vacuity, and the cyclic/unsupported boundary. r[FXORACLE-SEM-01]
- [x] [serial] Specify the test-local independent oracle, complete-bound comparison, production/reference outcome matrix, conservative-refusal characterization, symbolically exact subfamily and low/sufficient-work lanes. r[FXORACLE-DIFF-01]
- [x] [serial] Author the deterministic corpus and bounds, SplitMix64 seed schedule, metamorphic identities, outcome-preserving shrinking, seeded faulty variants M1–M6, reproducible runner, receipt fields, acceptance mapping and non-claims. Validate only the change documents with the pinned Cairn runner. r[FXORACLE-CORPUS-01] r[FXORACLE-CTRL-01]

## Phase 2: Future implementation and evidence (not undertaken by this change)

- [ ] [serial] Implement the `#[cfg(test)]` oracle module and its independent node/assignment representation; drive production only through public effect constructors. Prove or review the greatest-solution and rigid-soundness arguments before trusting expectations. r[FXORACLE-DIFF-01]
- [ ] [serial] Implement the deterministic corpus, seed generator, caps, metamorphic checks, shrinking and the low/sufficient-work lane; retain `solve.rs:416` unchanged and add the named families for neutral unions, hidden host effects, shared constraints and rigid identity. r[FXORACLE-CORPUS-01]
- [ ] [serial] Implement M1–M6 as test-local variants and verify each is caught by a named deterministic case; fail the lane if a variant escapes. r[FXORACLE-CTRL-01]
- [ ] [serial] Add the Bun runner and execute the oracle with source-bound receipts at a pinned revision; record every conservative refusal and every minimized discrepancy. r[FXORACLE-CTRL-01]
- [ ] [serial] Convert minimized applicable discrepancies into source-level regressions and independently checked kernel submissions; keep malformed/cyclic arenas in a separate robustness lane. r[FXORACLE-DIFF-01]
