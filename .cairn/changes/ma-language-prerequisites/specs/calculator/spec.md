# Narrow language prerequisites, not calculator acceptance

## MODIFIED Requirements

### Requirement: CALC-GATE-01
r[CALC-GATE-01]

Calculator acceptance MUST follow M4 and explicit library, declaration/module, iteration or recursion, text-processing, error-schema, and budget gates. The complete application MUST NOT block M1 through M4. Bootstrap arithmetic demonstrations MUST NOT count as an implemented exact calculator.

| Slice | Entry gate | Required evidence |
|---|---|---|
| Bootstrap precursor | M2/M4 | Existing checked `I64` arithmetic and runtime composition, labeled as a narrower slice |
| Exact library | After M4 and library/type gates | Big integers, normalized rationals, exact decimals, checked conversions, and independent limits |
| Calculator v1 | Exact library and expression/session gates | Actual Noble/Wasm parser, lexical functions, exact results, and unchanged state after failure |
| AI-authoring report | Calculator harness and declared authoring tools | Recorded change tasks, held-out acceptance, negative controls, costs, and visible failures |
| Optional proof report | Applicable MC1/MC2 support | Exact subject/model correspondence and accepted evidence under SPEC-V002 |

The calculator scenarios are unexecuted harness designs. `Calculator-Design` and `AI-Authoring-Design` label application/test lanes, not Noble language conformance profiles. JSON exact-number fields use decimal strings. They do not define portable Noble serialization. Source provenance records the mathematical-computing references and their limits; no calculator, benchmark, or proof execution is claimed by this amendment.

The opt-in signed declared-text profile supplies only guest-native byte-offset inspection, one pure signed-I64 less-than for bounds and conversions, explicitly signed self recursion under finite runtime limits, immutable versioned module/Result identities and independent checked Wasm admission. A compiled wrapping-I64 digit fold, native source and compiler tests, or a green structural Cairn gate MUST NOT be counted as exact decimal parsing, arbitrary-precision BigInt/Rat, seven independent calculator ResourceLimit categories, expression grammar/functions, unchanged calculator state after numeric failure, or any accepted CALC case. CALC-01 through CALC-13 remain absent/not-run until independently executed against the actual integrated exact library and application under their declared inputs.
