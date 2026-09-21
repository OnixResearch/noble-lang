# M4 end-to-end Core-Bootstrap acceptance

## MODIFIED Requirements

### Requirement: B-GATE-01
r[B-GATE-01]

The first end-to-end milestone requires positive acceptance and Wasm execution, negative rejection, runtime-selected builders, and recipe observations. Document validation alone MUST NOT close it.

M4 MUST use the managed-linear-memory backend selected by M3. Its CLI and persistent-session entry points MUST process real source through parsing, immutable resolution, inference, an untrusted candidate, independent kernel acceptance, Wasm emission and actual execution. Fixture dispatch, unchecked execution, source replay as execution, and source/recipe interpreters MUST NOT substitute for compiled execution. The complete declared Core-Bootstrap subset remains the scope; unsupported features and exhausted limits MUST fail explicitly.

Acceptance MUST execute every CORE-01 through CORE-18 source or declared structured harness, including all declared variants and expected observations. It MUST demonstrate state-preserving static rejection with zero candidate-body host requests, hostile candidate rejection, lexical boundaries, ordered effects and traps, post-compilation builders, complete ordered interfaces, persistent compiled captures and optimization-independent exact reflection. Existing MC1 and M3 regression coverage MUST remain intact.

The M4 developer-workflow extension MUST execute DX-10 and DX-12 through the actual compiler and selected Wasm backend. Property evidence MUST retain bounded generated and replayed trials, independent result/recipe/effect observations, every declared shrinking and exhaustion control, and a separately classified malformed-candidate lane. Documentation evidence MUST retain both published source examples and all declared absence, nonexecution, unsupported, mismatch, timeout and missing-test-host controls. Resource-free test-host mappings and limits MUST be explicit; successful examples MUST NOT be promoted to universal proofs or coverage of unexecuted documentation.

Changed pure code MUST extend the source inventory, actual-Rust Charon → Aeneas → Lean extraction, dependency/axiom audits and refusal gates. Durable evidence MUST bind sources and selected tools, demonstrate actual CLI/session usage, report all acceptance and quality gates, and distinguish execution/extraction from universal correctness. External models, trusted tooling and open refinement/backend obligations MUST remain explicit. The conformance/status/roadmap records and completed Cairn archive MUST match the observed evidence before M4 is reported complete.

#### Scenario: Source and persistent-session execution

- GIVEN the declared Core-Bootstrap subset, explicit environment and finite limits
- WHEN real expression and definition submissions pass independent acceptance and execute through the selected compiled Wasm backend
- THEN the CLI and persistent session retain exact ordered values, resolved definitions, captures and recipes without executing source or recipes

#### Scenario: Complete bounded acceptance and honest assurance

- GIVEN every declared CORE-01 through CORE-18 source and structured harness plus the affected MC1 and M3 regressions
- WHEN execution, negative controls, quality, inventory, extraction, dependency/axiom and refusal gates run against the final source and selected tools
- THEN durable receipts record all outcomes and open obligations, and milestone completion follows passing acceptance rather than document validation alone
