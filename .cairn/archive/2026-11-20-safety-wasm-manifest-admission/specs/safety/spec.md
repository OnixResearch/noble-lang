# Finite Wasm manifest admission

## MODIFIED Requirements

### Requirement: S-EFFECT-01
r[S-EFFECT-01]

For every finite execution prefix, every guest-requested host operation MUST be included in the instantiated effect bound of the executing computation. For the selected Core-only S-CASE-16 Wasm admission path, this bound MUST come from host-invoker-supplied source prepared through actual resolution/checking and independent kernel acceptance, including complete latent quoted program bodies, NOT from an artifact's claimed manifest or effects observed only on the chosen top-level path. The exact final validated artifact MUST match bytes compiled from that checked source under host-selected pinned compiler/assembler/optimizer options. Independently reflect actual binary import module/field/kind before linking: `noble.test_emit` represents Core `test.emit`, `noble.test_abort` represents Core `test.abort`; `noble.test_emit_bound` and `noble.test_clock_bound` have distinct bound `test.emit` and `test.clock` identities but MUST fail closed as unconfigured, not acquire fabricated adapter bindings. Selected import function signatures are assured by final-byte equality with the validated pinned host compilation before instantiation, NOT by an independent pre-correspondence signature decoder. Compiler-emitted but unused Core ABI imports do not by themselves enlarge the checked source's effect bound or establish a guest call; an out-of-bound request MUST NOT be admitted as authorized.

#### Scenario: S-CASE-16 finite manifest/admission footprint

- GIVEN S-CASE-16's canonical `actual_imports:["test.emit"]`, `claimed_effects:[]`, `trusted_correspondence:false` and a real validated artifact representing that claimed footprint
- WHEN the selected admission procedure examines its actual binary imports and supplied claim before any guest execution, and separately checks positive host-supplied source and rebuilt final bytes with latent effects
- THEN the forged empty claim refuses as `effect-manifest-reject` with zero guest requests and protected operations; no manifest or physical import list alone proves the source effect bound, and any admitted execution accounts requests against its checked complete bound

### Requirement: S-EFFECT-02
r[S-EFFECT-02]

A program with an empty effect bound MUST NOT issue a guest host request. This does not imply termination, bounded resource use, or freedom from runtime housekeeping outside guest effects. A denied request still counts as a request under S-EFFECT-01. Static effect rejection and forged-metadata admission rejection require zero candidate-body host requests; runtime authority denial with a source-declared effect is distinct but is NOT selected in this Core artifact admission path. A claimed empty set MUST NOT suppress actual binary import reflection or independently checked source/latent effect accounting. The canonical forged footprint MUST reject at admission as `effect-manifest-reject` before candidate instantiation, start, active-segment application, guest request or protected operation, even though independent source/byte correspondence is absent. An honest empty-effect checked source MAY retain unused compiler-emitted Core ABI imports only when exact final-byte correspondence and the separate host gate ensure those imports cannot authorize an out-of-bound guest request. Optional effectful-host policy denial occurs before guest invocation, with zero guest requests and protected operations.

#### Scenario: S-CASE-16 zero-execution refusal and honest source controls

- GIVEN the canonical forged empty effect claim and separately generated honest empty/effectful sources under the same selected host configuration
- WHEN each submitted final binary is checked without instantiation and only admitted bytes are installed in an isolated fresh host
- THEN the forged candidate has zero guest requests and protected operations, honest empty source never requests an effect, effectful host-policy refusal happens at admission with zero requests, and separately allowed source executes its actual guest request in the fresh host

### Requirement: S-HOST-03
r[S-HOST-03]

A valid Wasm artifact, digest, signature, recipe hash, source identity, or attached proof object is not by itself authorization and is not by itself sufficient evidence of source/recipe correspondence. Loading policy MUST separately establish artifact validity, interface compatibility, provenance/correspondence policy, and concrete runtime authority. For selected Core-only S-CASE-16 admission through `noble admit-artifact WASM --effects CLAIMS_JSON [--source HOST_SOURCE] [--allow-effects ...] [--opt off|on]`, the invoker MUST supply source bytes, pinned compilation options, allowed effects and optional effectful-host policy independently of candidate-attached claims; missing source cannot establish correspondence. Validate the final binary and independently reflect import module/name/kind; compare byte-for-byte with the host-controlled validated Core compilation under exactly those options to establish the selected complete interface and function signatures. Neither a matching digest nor a manifest substitutes for equality. Reject submitted starts and active element/data segments before instantiation; the current selected Core compilation has no start and uses passive segments. No untrusted submitted artifact may be instantiated or mutate host state during admission. Only a fully admitted matching artifact MAY execute in a fresh isolated host; effectful-host policy denial MUST precede execution, while actual allowed guest invocation MUST separately establish whether effects execute. This finite selected observation does not discharge declared-binding artifact admission or source-, backend-, engine- or host-universal correctness/refinement.

#### Scenario: S-CASE-16 correspondence-independent manifest rejection

- GIVEN the canonical valid-import/empty-claim/no-correspondence candidate, mismatched source/options/bytes and interface mutation controls, and independently supplied positive source/policy pairs
- WHEN actual final bytes are validated, classified and compared to independently rebuilt source bytes before any candidate instantiation
- THEN the canonical case refuses specifically as `effect-manifest-reject` with zero guest requests and protected operations; unrelated correspondence/interface failures also refuse without side effects, while only matching positive bytes proceed into a fresh host and an actual governed guest invocation
