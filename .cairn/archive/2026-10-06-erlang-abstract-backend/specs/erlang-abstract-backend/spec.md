# Opt-in Erlang abstract-form backend candidate

## ADDED Requirements

### Requirement: EA-SCOPE-01
r[EA-SCOPE-01]

`Erlang-Abstract-Draft` MUST be a distinct explicit nonselected target for independently accepted resource-free Core-Bootstrap **subset** computation, not an alternative interpretation of Noble source or an amendment to `Wasm-Draft`. The first positive slice MUST include complete ordered interfaces for signed `I64` literals and `+`, `-`, `*`, `=` with no host effects; every unsupported Core word (`Text`, aggregates, `Program`, quotation, `quote`, `compose`, `run`, `reflect`, named definitions and `test.emit`), resource, component, module, arbitrary Erlang operation, or unsupported profile MUST explicitly refuse before emission without silently narrowing the requested source, interpreting it or falling back to Wasm. A full Core-Bootstrap positive target, including runtime-created Program values, requires later separate selection and execution, not a claim from this arithmetic slice. `W-EXEC-01`, selected managed-linear-memory Wasm, ordinary `noble run`/`session` and accepted M4 evidence remain unchanged.

#### Scenario: EA-CASE-01 arithmetic positive and unsupported refusal

- GIVEN the exact first-slice arithmetic sources and unsupported-word variants of EA-CASE-01 and EA-CASE-05
- WHEN an explicitly selected Erlang abstract build is requested
- THEN only independently checked in-slice sources can reach form emission; unsupported sources refuse without a BEAM artifact, Wasm fallback or guest request

### Requirement: EA-BUILD-01
r[EA-BUILD-01]

The proposed only build command MUST be `noble build SOURCE --target erlang-abstract --out DIR --contract CONTRACT_JSON --policy POLICY_JSON`, with one host-selected exact SOURCE and mandatory bounded host-owned contract/policy inputs. `CONTRACT_JSON` MUST supply the exact ordered expected input/output stack and resolved immutable builtin/environment operation contracts; `POLICY_JSON` MUST supply the selected subset, empty allowed effect bound, target, pinned OTP/ERTS/compiler/form revisions and finite preparation, emission, compilation and runtime limits. Independently check both before candidate acceptance. Build MUST output bounded validated abstract-form ETF transport, exact source/form/BEAM manifest and, only after pinned OTP compilation of accepted forms, BEAM bytes; build itself MUST NOT load or execute the candidate. The proposed *only* invocation is `noble execute --target erlang-abstract --artifact DIR --input INPUT_JSON` after EA-ADMIT-01 and EA-ISOLATE-01, using host-selected typed input. Existing `noble build --require-proof SOURCE --contract FILE` remains a distinct proof-release grammar and retains its meaning/output; no target is selected by default and no `--native`/C/alternate target alias is implied. Unknown target/tool/option and conflicting proof/target mode MUST refuse, not fallback.

#### Scenario: EA-CASE-05 explicit target and existing CLI distinction

- GIVEN the explicit target/contract/policy command and wrong-target, changed-tool or proof-mode conflict variants of EA-CASE-05
- WHEN the requested build or execute selection runs
- THEN every unsupported variant refuses before load without changing the existing proof build or Wasm defaults

### Requirement: EA-FORMS-01
r[EA-FORMS-01]

Emission MUST lower accepted Noble nodes to an exact allowlisted ordered **list of Erlang abstract forms** with original Noble file/line annotations (`erl_anno`) on attributable expressions, preserving source order and stack operand order; no generated `.erl` text, direct Core Erlang or BEAM assembly is selected. A versioned bounded external term format (ETF) envelope MAY transport that list across the process boundary, but ETF bytes are not the `Forms` argument to `compile:noenv_forms/2`. Before decoding or compiler invocation, an independent adapter MUST validate byte size, nesting, term count/shape, approved form constructors and attributes, permitted finite pre-existing atom/module vocabulary, annotation/file identities and source-map bounds; reject unknown atoms/module names, `-compile` attributes, parse transforms, nonallowlisted imports/options and malformed or truncated terms. `binary_to_term(...,[safe])` alone does not meet limits or make a form trusted. The selected pinned OTP worker MUST invoke `compile:noenv_forms(Forms, pinned_whitelisted_options)` with binary output and no environment-derived `ERL_COMPILER_OPTIONS`; compiler options embedded in forms MUST NOT override policy. Compiler errors and attributable runtime traces MUST map to actual Noble source location rather than a generated Erlang file. Toolchain and form schema revisions are recorded; compiler success is not semantic proof.

#### Scenario: EA-CASE-03 original Noble source location

- GIVEN a multiline Noble source with a forced second-line quota failure and a separately malformed test-only form bearing a second-line annotation in EA-CASE-03
- WHEN the admitted BEAM runs or pinned compiler rejects the controlled malformed form
- THEN the classified failure and source location identify the original Noble file and line two, without treating fabricated annotations as trustworthy

#### Scenario: EA-CASE-04 bounded ETF and ambient option rejection

- GIVEN EA-CASE-04's complete bounded valid list and malformed, oversized, unknown-atom, injected-attribute and ambient-compiler-option controls
- WHEN form transport and pinned compilation are attempted
- THEN only the exact whitelisted forms/options can produce an admitted BEAM candidate; other variants refuse before load with no candidate host request

### Requirement: EA-I64-01
r[EA-I64-01]

Noble signed `I64` literals MUST be range-checked before emission; the backend MUST NOT accept out-of-range signed source literals merely because BEAM integers are arbitrary precision. For checked `+`, `-`, `*`, normalize intermediate BEAM integers modulo 2^64 and explicitly interpret the high bit as a negative signed Noble result, including `9223372036854775807 1 +` yielding `-9223372036854775808`; no unbounded BEAM integer may escape as an `I64` result. `=` MUST compare exact normalized bit patterns; complete ordered stack inputs/outputs, left-to-right operation order, an in-budget result and a distinguished quota outcome MUST agree with the accepted source semantics under declared finite bounds. Numeric helpers are part of the pinned admitted generated inventory, not arbitrary BIF calls.

#### Scenario: EA-CASE-01 real arithmetic and wrap

- GIVEN checked `20 1 +`, signed wrap/multiply/equality and out-of-range literal variants of EA-CASE-01
- WHEN actual pinned OTP-compiled BEAM and selected managed-linear-memory Wasm execute the same accepted sources and inputs
- THEN in-budget exact signed stack results agree, and out-of-range sources reject before any BEAM load

### Requirement: EA-ADMIT-01
r[EA-ADMIT-01]

Before any BEAM load, the invoker MUST independently bind and accept exact host-selected source bytes, checked candidate revision and complete expected stack interface, resolved immutable builtin contracts, empty effect bound and finite policy, emitter/form schema, ETF bytes, validated Forms, pinned OTP/ERTS/compiler/options, source map, generated module identity and exact resulting BEAM bytes. It MUST reconstruct/compare these against trusted admission and inspect actual BEAM module, imports, exports and on-load behavior, refusing unknown instructions/operations, modules, tool versions, source maps, modified ETF or substituted BEAM bytes. A candidate manifest, hash, claimed validated flag or successful compiler exit alone MUST NOT authorize load; a separately reviewed authenticated trusted-build route MAY replace reproducible byte comparison only with its explicit trust assumptions. Admission rejection causes zero candidate-body host requests and cannot mutate a current Wasm session.

#### Scenario: EA-CASE-04 malicious form/ETF substitution

- GIVEN valid accepted source plus one-at-a-time ETF/form/source-map and compiler-option forgeries of EA-CASE-04
- WHEN independent form and final BEAM admission inspect them
- THEN every substituted subject refuses before load despite its manifest or compiler success claim

#### Scenario: EA-CASE-05 modified BEAM and wrong tool

- GIVEN a valid `20 1 +` build, a changed BEAM byte, mismatched module, stale source and wrong OTP pin in EA-CASE-05
- WHEN the explicit executor independently admits each artifact
- THEN only the byte-exact admitted source-bound artifact can reach the isolated loader

### Requirement: EA-ISOLATE-01
r[EA-ISOLATE-01]

ANY execution of untrusted compiled BEAM, including positive tests, MUST occur only in a disposable separately pinned OS-confined VM/worker after EA-ADMIT-01. An Erlang process, supervisor or application is not an OS sandbox. The worker MUST start without inherited credentials/secrets/unrelated FDs and deny arbitrary network, filesystem, process/exec, code loading, Erlang module/BIF and host API authority; its admitted byte inventory and narrow typed input/result protocol MUST enforce finite preparation, term/result, memory, CPU, wall, steps and output limits. `code:load_binary` MUST receive only independently admitted bytes inside that worker. The first slice has an empty effect bound and no `test.emit` binding: an unmatched `test.emit` request or hidden effect MUST refuse before emission/load with zero guest and protected requests. If a later profile permits that effect, an out-of-process trusted mediator MUST check current invocation authority on each request; an effect bound never grants it. Worker crash, exhaustion and runtime trap MUST be distinct from normal output; sandbox absence blocks execution.

#### Scenario: EA-CASE-02 pure runtime and forbidden effect

- GIVEN the actual admitted pure `20 1 +` and separately effectful `"A" test.emit`/forged empty-effect variants of EA-CASE-02
- WHEN isolated execution or static/admission checking is requested
- THEN pure actual BEAM yields 21; all effectful variants refuse before load with zero guest/protected requests

#### Scenario: EA-CASE-06 OS confinement and hostile code

- GIVEN EA-CASE-06's isolated admitted positive control plus separately test-only hostile BEAM attempts to access file/network or load an unrelated module
- WHEN the disposable OS-confined VM executes each appropriate control
- THEN protected effects and disclosure stay zero, positive `20 1 +` works, and sandbox absence blocks all execution

### Requirement: EA-GATE-01
r[EA-GATE-01]

Before any runnable or production selection, the project MUST execute actual accepted source lowered through validated Erlang abstract forms and pinned `compile:noenv_forms` into admitted BEAM in the OS-confined worker, alongside independently accepted selected managed-linear-memory Wasm for the same first-slice source and runtime inputs. Compare complete ordered typed stack values, signed wrapping text, empty effect traces, static/admission refusals, attributable original-source diagnostics, traps and quota classification at declared common bounds. Record exact source/candidate, ETF/forms/BEAM bytes, source maps, OTP/ERTS/emitter/worker/host/tool revisions, limits and all failures; separately exercise optimizer/tool changes and hostile imports/byte substitutions. A structural Cairn gate, a Gleam benchmark, M4/M3/C-backend evidence, successful OTP compilation, or identical selected examples does NOT prove general backend/host refinement, PO-17/18, or production safety. The target remains nonselected absent case execution and separately reviewed source-bound assurance.

#### Scenario: EA-CASE-07 false selection receipt

- GIVEN stale Wasm/C/BEAM receipts, an unexecuted EA-CASE-01 design and a forged candidate manifest in EA-CASE-07
- WHEN a future backend-selection review is requested
- THEN each false receipt fails subject/tool/execution checks, leaving selected Wasm unchanged and backend proofs open
