# Reusable contract-based named calls and law bundles

## ADDED Requirements

### Requirement: PREUSE-ENVELOPE-01
r[PREUSE-ENVELOPE-01]

A revision-3 caller proof MUST bind every call occurrence to an independently reconstructed envelope.

For each occurrence, the envelope MUST bind:

- the exact immutable module name, version and source bytes;
- the callee definition's name, lexical ordinal, span and source bytes;
- the accepted body and its complete ordered interface;
- the authenticated occurrence's source node, full-token span, ordinal and fresh specialization slot;
- the instantiation;
- the callee contract identity and claim;
- the callee proof identity;
- the model, checker and rule-library revisions;
- the assumptions;
- the finite, acyclic, ranked transitive proof closure.

Resolution MUST use the frozen import snapshot. This applies to both call resolution and `use` resolution.

Display names, import aliases, claimed digests and producer-supplied records MUST NOT supply authority. Callee assumptions MUST be within the consumer-selected allowed set.

### Requirement: PREUSE-CALL-01
r[PREUSE-CALL-01]

A revision-3 call MUST use the callee's accepted contract, not its body. The checker MUST derive the symbolic state itself. That state is the complete ordered stack, the unchanged tail, the input snapshot, the facts and fresh ghost values.

At each call, the checker MUST:

1. match the exact ordered argument sorts;
2. check an explicit bridge from the current facts to the substituted callee precondition;
3. carry the substituted callee postcondition into the next state, using capture-avoiding substitution.

A final join MUST establish the caller's ensures. Matching types alone MUST NOT discharge a bridge or a join.

The result claim MUST be normal-return partial correctness, which is the weakest component strength. It MUST NOT be reported as termination, prefix safety, quota success or artifact correspondence.

Independently of the proof, the consumer MUST re-derive each callee's effect bound and resource-freedom. It MUST carry forward the callee assumptions and the callee correspondence status. Callee opacity MUST NOT hide either.

### Requirement: PREUSE-LEMMA-01
r[PREUSE-LEMMA-01]

Logical lemma application MUST be a sort distinct from program call slices.

- `use`, `apply` and `subst` may name only pure proposition proofs or local facts.
- `pc-call` may name only an accepted contract proof whose subject equals the resolved occurrence.

Using either in the other's place MUST be refused.

Instantiation MUST use explicit, independently validated arguments with capture-avoiding substitution and binder rebasing. Applying a lemma MUST NOT execute a program or create a program claim.

An arithmetic law MUST enter only as a reviewed rule identity with an exact Lean declaration, never as an axiom.

### Requirement: PREUSE-CHECK-01
r[PREUSE-CHECK-01]

Revision-3 checking MUST be decidable and bounded. Every stage MUST be charged to the shared admission budget. The bounded quantities include occurrences, distinct callees, DAG depth, lemma uses, term sizes, symbolic stack depth, substitution and normalization work, and the size of the generated statement.

The consumer MUST regenerate every callee and lemma claim from its immutable source. It MUST replay every distinct closure member in the same isolated strict check, so that no accepted-status record substitutes for checking.

The outcome classes are accepted, invalid, unsupported, exhausted, internal and unavailable recheck. They MUST remain distinct, and none other than accepted may be reported as proved.

Any of the following changes MUST invalidate applicability until the envelope is revalidated:

- caller source;
- callee version or claim;
- import resolution;
- model, checker or rule revision;
- consumer policy.

Saved old definitions MUST keep only their exact old evidence.

### Requirement: PREUSE-BUNDLE-01
r[PREUSE-BUNDLE-01]

A law bundle MUST be an immutable index over already-accepted identities. It MUST record:

- its parameters;
- its exact operation identities, with complete ordered stack and effect interfaces;
- its semantic, profile, checker and rule revisions;
- for each law: the theorem identity, the instantiated statement, the required arguments, the observation relation, the strength, the scope (each scope element established or not established) and the finite dependency closure;
- its assumptions;
- every `Data`, `Capture` or move-only premise.

A consumer MUST select a bundle explicitly, either at the use site or through a named local parameter. Elaboration MUST resolve the selection to immutable identities. An ambiguous selection MUST be refused. Import order and unrelated imports MUST NOT change the selection.

Specialization MUST be a checked derivation bound to the new subject.

A bundle MUST NOT:

- grant `Data` or `Capture` eligibility;
- commute ordered effects;
- transfer evidence across operations, module revisions, observation relations or semantic models;
- transfer evidence by behavioral equality of distinct subjects.
