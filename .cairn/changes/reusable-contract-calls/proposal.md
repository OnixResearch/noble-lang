# Reusable contract-based named calls and explicit law bundles

## Why

Issue #7 asks callers to use independently checked callee contracts and reusable lemmas instead of re-proving inlined bodies. The current baseline cannot do that.

- The accepted `contract 2`/`proof 2` slice (CONTRACT-26) proves `twice [ d.step d.step ]` with `NamedV2.twoStepClaim`. That theorem takes each callee body `[.lit (.i64 1), .word 4]` as a premise. Callees are therefore re-derived, not reused.
- `named_v2.rs` accepts exactly three use rows, two modules, callee bodies of at most two nodes, and the fixed `+2` claim.
- `check_dependency` refuses contract theorems as `use` dependencies: "contract theorem dependency is not in the pure fragment".
- The logical `Eq` only relates bound variables. An arithmetic lemma such as wrapping-add associativity is refused with "expected logical atom".

The program-contracts draft already requires intermediate implications, checked composition, exact immutable proof reuse and erasure (VC-COMPOSE-01, VC-LIB-02, VC-LIB-04, VC-VALUE-03, VC-INTRINSIC-01, VC-INTRINSIC-05). This change extends that baseline with a bounded reusable-call profile. r[PREUSE-CALL-01] r[PREUSE-CHECK-01]

Issue #11 needs a way to package wiring laws for reuse. The #7 comment asks for explicit, Arend-inspired law bundles. These must never override eligibility or transfer evidence to a different subject (P-ID-06, VC-REWRITE-02, V-OBS-01). r[PREUSE-BUNDLE-01]

## What Changes

- Add an opt-in `Named-Contract-Calls-v3` profile: additive `contract 3` and `proof 3 ... for` with the same fields as revisions 1 and 2. It covers finite acyclic DAGs of pure named calls over a bounded I64/Bool vocabulary. r[PREUSE-GRAMMAR-01]
- Define an exact subject and dependency envelope for every call occurrence. It binds the immutable module, version and source; the definition and accepted body; the instantiation; the callee contract and proof identities; the model and checker revisions; the assumptions; and the transitive closure. Display names, aliases and digests are not authority. r[PREUSE-ENVELOPE-01]
- Specify the contract-based call rule. The caller's current assertion must imply the callee precondition, with an explicit bridge. The callee postcondition then carries forward over a symbolic ordered stack with fresh ghost values and an unchanged tail. Claim strength is the weakest component strength; effects, resources, assumptions and correspondence status stay visible. r[PREUSE-CALL-01]
- Separate reusable logical lemma application (`use`/`apply`/`subst` over pure `Type0` propositions) from program invocation, with capture-avoiding substitution. r[PREUSE-LEMMA-01]
- Specify bounded checking, dependency replay, invalidation and diagnostics. r[PREUSE-CHECK-01]
- Add explicit law bundles. Each bundle is an immutable index of operation identities, theorem identities, profile, observation relation, assumptions and resource premises. A consumer selects a bundle explicitly, and checked specialization is required. Bundles grant no eligibility and transfer no evidence by behavioral equality. r[PREUSE-BUNDLE-01]
- Keep correspondence obligations separate from finite tests. r[PREUSE-EVID-01]

## Impact

This change is design-only. It touches only `.cairn/changes/reusable-contract-calls/`: proposal, design, tasks and ADDED requirement deltas for `language`, `program-contracts` and `verification`.

Nothing else changes: no compiler, kernel, runtime, Lean library, conformance JSON, canonical spec, roadmap, status, generated view or ledger. No execution, proof or acceptance status is promoted. The worked examples and scenario designs in `design.md` are unexecuted unless an explicit design-time observation says otherwise.

Future sync must add a `PREUSE` route to `tools/check-specs.mjs` `ROUTES` and register conformance designs before archive. Revisions 1 and 2, MC1, MC2, CONTRACT-18 and CONTRACT-26 stay unchanged. r[PREUSE-GRAMMAR-01] r[PREUSE-EVID-01]
