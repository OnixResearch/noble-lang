## ADDED Requirements

### Requirement: NGRAPH-NICKEL-01
r[NGRAPH-NICKEL-01]

The opt-in `noble-ngraph-nickel/v1` authoring route MUST lower to the existing `ngraph-ir/0` representation and MUST be independently admitted by Noble's native checker under explicit construction and graph budgets. Its distinct `authored-nickel` producer identity MUST NOT be labeled as source-extracted. The first supported route MUST admit only finite acyclic `ngraph-i64/0` graphs with ordered concrete I64 inputs, exact signed-I64 constants, wrapping Add/Sub/Mul, ordered outputs including repetition, and explicit sharing. Unsupported operations, profiles, facts, guards, dynamic/recursive constructs, and resources MUST be rejected from submitted constructor data before simplification.

The `noble-ngraph-nickel/v1` schema MUST keep graph handles distinct from Nickel numbers, assign deterministic instance-scoped identities, preserve intentional shared references, and refuse duplicate or cross-scope handles. Exact constants MUST cross the JSON boundary as canonical decimal strings; construction parameters MAY determine graph shape or exact constants, runtime numeric arguments MUST become `Input` slots, and Noble's F32-only trainable `Param` MUST NOT stand for either.

The evaluator MUST be the repository-pinned Nickel 1.17.0 and MUST remain outside the pure no-std graph core. The evaluator lane MUST bind an explicitly declared, observed, ordered import closure and MUST force complete demanded constructor export before lowering, so import-order ambiguity or delayed contract failure cannot yield an admitted graph. A successful export MUST still undergo native `check_with` admission; a Nickel contract alone is not graph admission.

The retained provenance record MUST bind exact source and import bytes, construction parameters, constructor library and contract versions, evaluator identity, declared and observed import closure, exported bytes, lowerer revision, selected profile, graph bytes, and checked graph identity. A mismatch in any bound field MUST refuse stale receipt or consumer-record reuse. Hashes and receipts MUST NOT be described as proofs, equivalence relations, or permission grants.

Composition observations MUST name the observer as ordered outputs under exact wrapping-I64 evaluation and retain independent expected graphs and values. They MUST distinguish real evaluator execution, native admission, finite differential results, model-level theorems, and any implementation/artifact correspondence. They MUST NOT imply effect independence, resource permission, or arbitrary reordering.
