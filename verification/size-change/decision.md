# Decision: analyze checked signed-self candidates with bounded closure

## Decision

Attach size-change classification to the checked declared-module candidate, at its source-level signed-self call sites, before the namespace transition commits. Do not derive the call graph from lowered Wasm.

## Criteria

- The checker must see the typed source derivations that establish `SignedSelf` ownership and the checked signature. Re-parsing source or inferring recursion from backend instructions would use a weaker representation.
- The lowered backend is not the authority for recursive source calls: named calls lower to `call $enqueue_program`, not Wasm calls. That instruction stream does not preserve the source-level call graph needed by this criterion.
- The analysis is opt-in and observational. Ordinary execution and reports remain unchanged when `--size-change-evidence` is absent. When requested, the certificate and optional Lean re-check happen before the checked namespace transition commits.
- The accepted profile builds one old-parameter/new-parameter graph for each direct self-call site. Edges are strict, non-increasing, equal, or unknown; only the existing guarded `I64` predecessor and structural-list-tail facts produce strict edges. The closure under composition is capped at 256 distinct matrices. The criterion is checked on every idempotent closure graph: each must have a strict diagonal. An idempotent graph without one is returned as the refusal cycle.
- Explicit caps are 8 measures, 256 self-call sites, and 256 closure matrices. Crossing a cap is reported as `bound-exceeded`, not as a termination or divergence result. The symbolic walk also has internal frame, step, and nesting budgets.
- The reference model proves finite certificate completeness and soundness relative to its matrix algebra, refusal-witness validity, and a periodic-site-run result. It does **not** yet prove the criterion for arbitrary infinite call-site sequences. Until that lemma is proved, `accepted` means only that the bounded closure criterion holds; it is not a general termination conclusion. A refusal means “not shown terminating,” not divergence.
- The source profile is direct signed self recursion only. `SelfOwner` call sites are analyzed; named/helper calls are unsupported, and the current source resolver does not admit mutual-recursion groups. Mutual recursion is not part of this source profile. The Rust classifier is not proved to correspond to checked source execution; the `List`-length and guarded wrapped-`I64`/`Nat` interpretations also remain open.
- This mutual-recursion boundary follows the current language contract, not just this checker's implementation: `specs/SPEC-0001.md` §10 states that this revision permits no mutual-recursion groups, and `specs/conformance/recursion-cases.json` case `REC-04` requires even/odd mutual calls to refuse at resolve. Supporting mutual-call SCT graphs therefore requires a language-spec/resolver change and a new conformance contract; it is not a checker-only extension.

## Evidence

- `crates/noble-cli/src/core/declared/mod.rs` derives evidence from the checked candidate and constructs the report before committing the namespace transition.
- `crates/noble-contracts/src/source/sizechange.rs` walks checked source nodes; unmodelled calls and unsupported shapes do not become assumed decreases.
- `crates/noble-cli/src/core/declared/sizechange.rs` emits Lean-checkable finite closure certificates for supported accepted/refused classifications and can re-check them with the caller-selected Lean binary/library.
- `crates/noble-contracts/tests/declared/sizechange.rs` pins refusal for an unanalyzed helper and the accepted/refused shared-measure, closure-permutation, and idempotent-cycle boundaries. The production-session scenarios and their receipts are retained under `verification/size-change/`.
- The size-change lane's model receipt audits the reference-model theorem inventory under the strict `{propext, Quot.sound}` axiom policy. It is model evidence, not a Rust refinement proof or an arbitrary-run SCT soundness proof.

## Reopen conditions

Reopen this decision if the arbitrary-run size-change soundness lemma is proved, if the language admits mutual-recursion groups or named helper calls in the signed source profile, if a backend begins preserving a source-bound recursive call graph with enough checked metadata to replace this checked-source placement, or if a separately proved correspondence connects the classifier to checked source execution. Until then, do not promote this evidence to a general program-termination claim or require it for ordinary execution.
