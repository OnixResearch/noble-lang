# Intrinsic source proof diagnostics

The earlier developer-experience delivery exclusion now concerns **general computational dependent types for ordinary runtime programs**, not the separately selected nonexecuting logical dependent Pi/Eq proof declarations. The three ordinary expression forms remain unchanged.

## MODIFIED Requirements

### Requirement: DX-TYPE-04
r[DX-TYPE-04]

A declaration owner MUST enumerate every admitted construction path and the invariant each path establishes. Constructors, conversions, defaults, decoders, and package imports MUST preserve that invariant. A public field, wrapper tag, or representation alias MUST NOT bypass validation. Invalid inputs MUST remain errors rather than sentinel or default values that appear valid.

Use ordinary opaque declarations and explicit fallible library constructors. These requirements add no general refinement inference, computational dependent program type system, or runtime proof search. The separate optional `Intrinsic-Proofs-Draft` selects predicative dependent *logical* proof declarations without changing this ordinary domain-data invariant. Nominal data remains subject to recursive eligibility. It does not become a capability merely because its name includes authorization.

### Requirement: DX-PROOF-01
r[DX-PROOF-01]

`Intrinsic-Proofs-Draft` tooling MUST retain source spans and exact resolved identities across Noble `.noble` module proof/contract parse, binder scope, Type0 code eligibility, proposition elaboration, equality-motive typing, reviewed-rule premises, MC1 claim export, restricted Lean translation, independent proof checking and optional admission. A diagnostic MUST distinguish lexical/mode or delimiter failure, unsupported logical form/type, unsatisfied goal, unproved bridge/join, changed subject/law, disallowed assumption, exhausted budget, checker/translator internal failure and unavailable independent recheck. It MUST state that an accepted source proof is not a proof of termination, runtime Wasm, backend or host authorization. Editor holes may expose constraints but MUST NOT become proof terms, axioms or accepted evidence; diagnostics and explanation MUST NOT execute a guest body, host operation, proof macro or tactic. Proof erasure MUST preserve inspectable separate evidence metadata without creating executable proof values.

For `contract 2` / `proof 2 ... for`, diagnostics MUST distinguish revision mismatch/unsupported colon form, changed imported version or lexical occurrence, wrong owner/span/ordinal, reused or missing fresh specialization slot, incomplete typing/instantiation, orphan or unreachable graph row, failed typed pre/post or exact named claim, and unavailable host source authentication. A checked synthetic Lean fixture MUST be labeled separately from a reusable reviewed rule instantiated with authenticated source, admitted Noble source, owner law and runtime artifact; accepted revision-1 reports and receipts remain unchanged.
