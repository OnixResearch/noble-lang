# Selected numeric graph profile boundary

## MODIFIED Requirements

### Requirement: NGRAPH-SCOPE-01
r[NGRAPH-SCOPE-01]

**NGRAPH-SCOPE-01.**

`NumericGraph-Design` MUST be a distinct, opt-in profile that lowers an already independently accepted pure Program to a fixed typed numeric graph. Its two sub-profiles, exact `ngraph-i64/0` and experimental `ngraph-f32/0`, MUST NOT share a value edge or an implicit conversion. An existing `I64` Program MUST NOT be reinterpreted as `F32`, and no real-valued derivative MAY be claimed for an `ngraph-i64/0` graph. Requesting the profile MUST NOT change ordinary Program checking, execution, identity, `run`, captures, live publication or K-NUM-01. A Program that the profile refuses MUST remain executable unchanged on its existing path. No source syntax or kernel type of this profile is implemented by this requirement.

The selected implementation is the opt-in `Numeric-Graph-v1` profile. It MUST be reachable only through the explicit `noble ngraph` entry point and the `noble-ngraph` library, and no existing entry point, including `run`, `session`, `compile`, `live` and `calculator`, MAY select it. Under `Numeric-Graph-v1`, `ngraph-f32/0` graphs MUST be authored directly in `ngraph-ir/0` and admitted only by the independent graph checker. Source extraction MUST be limited to `ngraph-i64/0`. No kernel `F32` type, kernel word or frontend syntax MAY be introduced. Selecting `Numeric-Graph-v1` MUST NOT be reported as a performance, speedup, GPU or backend-selection result, MUST NOT discharge any numeric graph proof obligation, and MUST report agreement between evaluators only as finite differential evidence.

#### Scenario: NGRAPH-CASE-01 selected entry leaves ordinary identities unchanged

- GIVEN the `Numeric-Graph-v1` profile and every `NGRAPH-CASE-01` entry-point and ordinary-vector input in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN the ordinary entry points are compared before and after the profile is added and the kernel type universe is reviewed
- THEN ordinary reports MUST be byte-identical, no kernel `F32` type or word MUST exist and the profile MUST be reachable only through `noble ngraph`; this active delta does not change that case's `state` or `evidence`

#### Scenario: NGRAPH-CASE-05 extraction is I64-only and refusal leaves the source unchanged

- GIVEN the `NGRAPH-CASE-05` sources, refusals and `--profile ngraph-f32/0` request in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN `noble ngraph extract` runs each input
- THEN the I64 sources MUST extract and every refusal MUST report its recorded code and exit status without writing a file; this active delta does not change that case's `state` or `evidence`
