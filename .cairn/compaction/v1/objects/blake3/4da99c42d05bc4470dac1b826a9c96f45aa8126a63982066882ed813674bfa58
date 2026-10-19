# Opt-in numeric graph profile: scope, FP32 reference, extraction and facts

## ADDED Requirements

### Requirement: NGRAPH-SCOPE-01
r[NGRAPH-SCOPE-01]

`NumericGraph-Design` MUST be a distinct, nonselected, opt-in profile that lowers an already independently accepted pure Program to a fixed typed numeric graph. Its two sub-profiles, exact `ngraph-i64/0` and experimental `ngraph-f32/0`, MUST NOT share a value edge or an implicit conversion. An existing `I64` Program MUST NOT be reinterpreted as `F32`, and no real-valued derivative MAY be claimed for an `ngraph-i64/0` graph. Requesting the profile MUST NOT change ordinary Program checking, execution, identity, `run`, captures, live publication or K-NUM-01. A Program that the profile refuses MUST remain executable unchanged on its existing path. No proposed syntax, kernel type or diagnostic of this profile is implemented by this requirement.

### Requirement: NGRAPH-F32-01
r[NGRAPH-F32-01]

`ngraph-f32/0` MUST define one reference contract over IEEE 754 binary32 with roundTiesToEven. Constants MUST be finite and exactly representable; inexact or nonfinite literals MUST be rejected rather than rounded. Each affine node starts from its stored bias and adds its terms in stored order, rounding once per product and once per partial sum. A source sum with no bias MUST use bias `-0`. Fused multiply-add, reassociation, distribution, term reordering, flushing subnormals to zero and wider intermediate precision that changes any result MUST NOT be used. Signed zero, infinities and subnormals MUST propagate as IEEE values. Observation equality MUST be bitwise on binary32 encodings, except that every NaN encoding MUST belong to one class whose sign and payload are unobservable. Index reads MUST require a checked integral index inside the table range or an explicitly declared out-of-range rule; a NaN or fractional index MUST NOT select a candidate silently.

### Requirement: NGRAPH-EXTRACT-01
r[NGRAPH-EXTRACT-01]

Graph admission MUST take the accepted root subject, its exact resolved dependency and capture closure, the requested sub-profile and finite budgets. It MUST recover each node's complete ordered interface from the checked derivation rather than from producer metadata. It MUST require an empty effect bound and reject, with a named diagnostic, every effect, resource or LiveRef type, live-slot dispatch, `run` of a Program that is not statically known, recursion, control flow, unsupported aggregate, reflection, Program-valued output and budget exhaustion, all before any graph evaluation. Expansion work MUST be charged against the unpruned inlined graph, so pruning cannot make an over-budget Program admissible. Pruning MUST NOT remove a runtime guard or another operation whose failure is observable. The resulting graph MUST be revalidated by a graph checker that does not trust the extractor's node types, facts or counts. Its logical identity MUST be distinct from any packed execution layout.

### Requirement: NGRAPH-FACT-01
r[NGRAPH-FACT-01]

Range, grid, exact-bit, one-hot, finiteness, shape and index-integrality facts used to choose or justify a graph operation MUST each record exactly one provenance. The allowed provenances are a sound transfer rule, validation of a graph input before evaluation, accepted evidence applicable under the #8 applicability contract, or a runtime guard node that fails closed before the dependent operation. Each fact MUST be bound to the graph logical identity, its value node, the profile semantic revision and the fact-rule revision. A caller assertion without one of these provenances MUST be rejected. Validating graph inputs MUST NOT establish a fact about an intermediate value except through transfer rules. A false fact MUST produce a named refusal or guard failure, never an optimized result.
