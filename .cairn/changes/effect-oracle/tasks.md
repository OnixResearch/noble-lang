## Phase 1: Design (this change)

- [x] [serial] Re-verify the existing effect solver at `de1044b`: constructors and bounds (`effects.rs:8,54`), rigid symbolic pass (`solve.rs:37`), neutral-only equation propagation (`solve.rs:136`), fixed-point concrete passes and constant exactness (`solve.rs:192-343`), universe derivation (`source.rs:815`), signature application (`source/inference/mod.rs:233`; `completion.rs:5`) and the existing inline regression (`solve.rs:416`). Record drift from the issue's `66eb7cd`. r[FXORACLE-SEM-01]
- [x] [serial] Select `finite-effects/v1`: independent finite universe, supported fragment, concrete assignment semantics, greatest-solution theorem, rigid quantifier order, universal-hole goals, inconsistency versus vacuity, and the cyclic/unsupported boundary. r[FXORACLE-SEM-01]
- [x] [serial] Specify the test-local independent oracle, complete-bound comparison, production/reference outcome matrix, conservative-refusal characterization, symbolically exact subfamily and low/sufficient-work lanes. r[FXORACLE-DIFF-01]
- [x] [serial] Author the deterministic corpus and bounds, SplitMix64 seed schedule, metamorphic identities, outcome-preserving shrinking, seeded faulty variants M1–M6, reproducible runner, receipt fields, acceptance mapping and non-claims. Validate only the change documents with the pinned Cairn runner. r[FXORACLE-CORPUS-01] r[FXORACLE-CTRL-01]

## Phase 2: Implementation and evidence (this change)

- [x] [serial] Implement the `#[cfg(test)]` oracle module and its independent node/assignment representation; drive production only through public effect constructors. The concrete greatest-solution and rigid soundness arguments are recorded in `design.md`; the corpus confirms them on the bounded domain. r[FXORACLE-DIFF-01]
- [x] [serial] Implement the 1,300-case exhaustive box, the named corpus, SplitMix64 seed generator, caps, metamorphic checks, shrinking and the low/sufficient-work lane; retain `solve.rs:416` unchanged and add the named families for neutral unions, hidden host effects, shared constraints and rigid identity. r[FXORACLE-CORPUS-01]
- [x] [serial] Implement M1–M6 as test-local variants and verify each is caught by the named deterministic case; the lane fails if a variant escapes the expected class. r[FXORACLE-CTRL-01]
- [x] [serial] Add the Bun runner and execute the oracle with a source-bound receipt; record the conservative refusal and the outcome counts. r[FXORACLE-CTRL-01]
- [x] [serial] Keep malformed/cyclic arenas in the separate robustness test. No minimized applicable discrepancy arose in the executed lane, so there is no source-level or kernel submission to add; a future discrepancy keeps the shrinking lane. r[FXORACLE-DIFF-01]
