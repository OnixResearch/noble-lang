# Decision: analyze checked signed-self candidates

## Decision

Attach size-change classification to the checked declared-module candidate, at its source-level signed-self call sites, before the namespace transition commits. Do not derive the call graph from lowered Wasm.

## Criteria

- The checker must see the typed source derivations that establish `SignedSelf` ownership and the checked signature. Re-parsing source or inferring recursion from backend instructions would use a weaker representation.
- The lowered backend is not the authority for recursive source calls: named calls lower to `call $enqueue_program`, not Wasm calls. That instruction stream does not preserve the source-level call graph needed by this criterion.
- The analysis is opt-in and observational. Ordinary execution and reports remain unchanged when `--size-change-evidence` is absent. When requested, the certificate and optional Lean re-check happen before the checked namespace transition commits.
- The accepted profile is deliberately bounded: one shared measure must decrease at every direct self-call site. List tails and guarded `I64` predecessors are classified; unmodelled operations, helper calls, and out-of-profile signatures are refused or marked unsupported. This is not general size-change closure or mutual-recursion analysis.
- `Accepted` means the model accepts the classified relations. The model does not prove that arbitrary source execution agrees with the Rust classifier; refusal means “not shown terminating,” not divergence.

## Evidence

- `crates/noble-cli/src/core/declared/mod.rs` derives evidence from the checked candidate and constructs the report before committing the namespace transition.
- `crates/noble-contracts/src/source/sizechange.rs` walks checked source nodes; unmodelled calls and unsupported shapes do not become assumed decreases.
- `crates/noble-cli/src/core/declared/sizechange.rs` emits a Lean theorem only for accepted classifications and can re-check the certificate with the caller-selected Lean binary/library.
- `crates/noble-contracts/tests/declared/sizechange.rs` pins refusal for an unanalyzed recursive helper and the accepted/refused shared-measure boundaries. The production-session scenarios and their receipts are retained under `verification/size-change/`.
- `verification/size-change/receipt-2026-11-13-73e9fd4.json` audits the reference-model theorem inventory under the strict `{propext, Quot.sound}` axiom policy. It is model evidence, not a Rust refinement proof.

## Reopen conditions

Revisit this placement if the language requires termination evidence for mutual recursion, named helper calls, or general size-change closure; if a backend begins preserving a source-bound recursive call graph with enough checked metadata to meet the criteria above; or if a separately proved correspondence connects the classifier to checked source execution. Until then, do not promote this evidence to a general program-termination claim or require it for ordinary execution.
