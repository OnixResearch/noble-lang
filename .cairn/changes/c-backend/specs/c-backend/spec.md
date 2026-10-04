# Opt-in C backend candidate

## ADDED Requirements

### Requirement: CB-SCOPE-01
r[CB-SCOPE-01]

`C-Backend-Draft` MUST be a distinct nonselected opt-in AOT target only for independently accepted full requested Core-Bootstrap computation (resource-free, with only exact checked `test.emit` operation contract when effectful). Expected interface, finite limits, immutable resolved operation contracts and allowed effect bound MUST be checked before emission; current host authorization MUST instead be checked at native invocation and on every out-of-process mediator request. It MUST refuse unsupported profiles, resources, arbitrary FFI, recursion and components rather than silently narrow, interpret source or fall back to Wasm. M4 managed-linear-memory Wasm, Node/V8 and W-EXEC-01's `Wasm-Draft` contract remain selected and unchanged. No live VM reload or previous Wasm proof/evidence transfers.

#### Scenario: CB-CASE-09 unsupported scope

- GIVEN the explicit unsupported resource, component, recursion, FFI and live-session inputs of CB-CASE-09
- WHEN the C build is requested
- THEN every input refuses without native artifact, guest effect, Wasm fallback or session mutation

### Requirement: CB-BUILD-01
r[CB-BUILD-01]

The proposed `noble build SOURCE --target c --out DIR --contract CONTRACT_JSON --policy POLICY_JSON [--native]` MUST accept explicit host-owned positional SOURCE and mandatory bounded host-owned contract/policy inputs, independently validated before candidate acceptance. Contract supplies complete ordered expected interface and exact resolved environment identities; policy supplies supported subset, allowed effect bound, target/ABI/tool pins and finite limits. An effect bound is NOT host authority. It MUST produce C11 source and pinned build manifest with exact SOURCE byte digest; `--native` builds ELF with a pinned argument-vector compiler/linker invocation, never a candidate shell command. Existing build syntax without `--target c` MUST remain unchanged. Building MUST NOT run native output. Only explicit `noble execute --target c --artifact DIR --input INPUT_JSON [--bindings HOST_FILE]` MAY launch independently admitted bytes under the pinned native sandbox/private-IPC runner, after host-side typed input and binding validation. Missing `HOST_FILE` grants no protected action; current invocation authority is checked on each mediator request, including after revocation. Typed normal, trap, quota, denial or non-success refusal outcomes MUST be distinguished. It MUST NOT pass host file paths, FDs or arbitrary arguments to the guest; ordinary `noble run`/`noble session` never opt into C. First target is exactly x86_64-unknown-linux-gnu with pinned Linux loader/libc, compiler, linker, sysroot, options, ABI/runtime and finite limits; absent/wrong target/tool MUST refuse without fallback. Missing confinement blocks `noble execute` while leaving source or ELF outputs nonrunnable. Source and executable remain distinct artifacts and compiler-service effects remain separate from candidate effects.

#### Scenario: CB-CASE-04 wrong target and tool

- GIVEN host-accepted `1 2 +` and the six wrong-target/tool/ABI/stale-policy/source-byte/absent-sandbox variants in CB-CASE-04
- WHEN the explicit C build or admission is requested
- THEN each fails before launch with zero guest/protected operations and no alternate backend/toolchain

### Requirement: CB-IDENTITY-01
r[CB-IDENTITY-01]

Versioned domain-separated C build identity MUST bind exact host-selected SOURCE byte digest in addition to accepted candidate/semantic recipe, independently expected interface, exact resolved immutable dependencies/host effect *contract* identities from host-owned `CONTRACT_JSON`, policy, emitter/runtime, pinned compiler/linker/sysroot, target, ABI, options and bounds. Display alias changes MUST NOT rebind old immutable captures; referenced contract identity changes MUST invalidate the old subject. Revocable runtime `HOST_FILE` authorization is NOT part of semantic/build identity; compatible new grants are checked per request, incompatible host operation contracts refuse before launch. Changed source spelling or optimization/target changes build key, not Noble semantic program identity if resolved recipe is unchanged. Stale or self-asserted metadata cannot authorize an executable.

### Requirement: CB-NUM-01
r[CB-NUM-01]

C11 `I64` MUST require eight-bit bytes and exact `uint64_t`; unsigned modulo-2^64 addition/subtraction/multiplication and bit equality MUST implement checked Core numeric semantics without signed overflow or implementation-defined casts. Range-checked literals, signed comparisons and exact decimal output including `-9223372036854775808` MUST avoid `INT64_MIN` negation. Division and floating point are unsupported; optimized and unoptimized outcomes MUST agree.

#### Scenario: CB-CASE-01 signed boundary program

- GIVEN the actual checked wrap/multiply/equality source in CB-CASE-01 and both optimizer modes
- WHEN pinned C execution is compared with selected managed-linear-memory Wasm
- THEN both yield Bool(true), preserve both intermediate minimum I64 values and reject out-of-range literals statically

### Requirement: CB-VALUE-01
r[CB-VALUE-01]

C11 `Unit`, `Bool`, `I64`, UTF-8 `Text`, inert `Syntax`, homogeneous `List`, `Pair`, `Sum` and `Program` values MUST have initialized checked tags, lengths, offsets and ownership. No unchecked access, overflow, forged/cyclic value, expired capture or native layout as semantic identity is allowed. Compiled `quote`, `compose`, `run` and `reflect` MUST retain exact immutable resolved interface, identity and normalized finite recipe, including runtime input after compilation, without source/recipe interpretation.

#### Scenario: CB-CASE-02 concrete program-value sources

- GIVEN the CORE-05 source and exact checked builder/reflection/branch sources in CB-CASE-02
- WHEN runtime inputs and display aliases vary after C/Wasm compilation
- THEN complete stack values, captures and normalized recipes agree without recompilation or alias rebinding

### Requirement: CB-ORDER-01
r[CB-ORDER-01]

Emitted C MUST explicitly sequence left-to-right Noble operations, deeper/top operand order, effects, traps, branches and quotation through temporaries/statements independent of unspecified C evaluation order and optimization; static rejection produces zero candidate-body requests and no Wasm session mutation.

#### Scenario: CB-CASE-03 ordered denied effects

- GIVEN the exact two-emission and quotation sources in CB-CASE-03
- WHEN authorized A, denied B and a separately static-refused effect bound run in both optimizer modes
- THEN request/protected-action prefixes and denials agree with selected Wasm; the quoted B is never requested

### Requirement: CB-LIMIT-01
r[CB-LIMIT-01]

Emission/runtime MUST check finite candidate/output, stack, aggregate, list, recipe, composition, step, text, diagnostic and native CPU/memory/wall bounds before growth or access; it MUST avoid unbounded C recursion. Successful results, traps, quota exhaustion and native termination remain distinct, with retained effect prefix and bounded cleanup, never partial success.

#### Scenario: CB-CASE-06 bounded list/compose and test-only index controls

- GIVEN the actual CORE-05 list and Core composition sources, explicit quotas and test-only runtime ABI index probes in CB-CASE-06
- WHEN at-bound and one-over cases execute
- THEN in-bound results match selected Wasm; excess and out-of-range native accesses refuse without fabricated Noble index words

### Requirement: CB-HOST-01
r[CB-HOST-01]

Only independently checked `test.emit` may be requested through a sandbox-local checked C shim with an opaque guest context. It MUST copy bounded stable text into length-delimited PRIVATE request/response IPC over allowlisted FDs; guest pointers/function pointers and host authority MUST NOT cross processes. A separate trusted mediator MUST validate complete framing, UTF-8, operation, effect, invocation and current authorization and copy into host-owned memory before dispatch. Partial/oversized/trailing/forged frames, expired/revoked identity and unsafe lifetime MUST fail closed with zero protected actions; denied requests retain trace. No arbitrary FFI or ambient host operations are authorized.

#### Scenario: CB-CASE-07 forged and revoked private IPC

- GIVEN checked source `"A" test.emit` and the exact valid, partial, oversized, forged and after-revocation frames in CB-CASE-07
- WHEN the mediator receives those requests from its sandbox-local shim
- THEN only the authorized complete request causes one protected action; every malformed or revoked request causes none

### Requirement: CB-SANDBOX-01
r[CB-SANDBOX-01]

Native ELF can call syscalls directly. Runnable C therefore MUST require a separately pinned x86_64 Linux native sandbox runner isolating admitted worker address space from host mediator, closing unrelated inherited FDs and secrets, confining filesystem, network, clock, process/exec and direct syscalls while bounding memory/CPU/wall time. Runtime/shim, runner, private IPC, mediator, loader/static-link assumptions, target and syscall profile MUST be separately pinned/tested. Absent confinement blocks execution even if C syntax is valid. Direct-syscall hostile controls run only through a sealed test-only runner entry without production source-to-ELF admission; production hostile ELF substitution MUST fail admission. No crash counts as success.

#### Scenario: CB-CASE-08 direct-syscall sandbox control

- GIVEN sealed test-only openat/connect/clock/exec/FD/fork probes and a separately production-admitted benign emit source under the same pinned runner profile
- WHEN hostile and benign programs run in their separated lanes
- THEN every hostile action is contained with zero protected operations while the benign authorized request succeeds

### Requirement: CB-ADMIT-01
r[CB-ADMIT-01]

Prior to launch independently accept host-selected source/candidate/policy; separately validate C11 source and its exact pinned emitter/runtime inventory, and ELF target/ABI/loader/imports and exact trusted reproducible native bytes (or separately reviewed authenticated trusted-build equivalent). Trusted source emission must byte-match candidate C; native build must byte-match candidate ELF. Candidate digest, manifest flag, self-claimed source or successful compiler exit cannot establish correspondence. Wrong/stale source, modified inert/executable sections, malformed C/ELF, target, tool, ABI or missing sandbox MUST refuse before effects. This finite byte correspondence is not a semantic lowering proof.

#### Scenario: CB-CASE-05 forged C and ELF

- GIVEN host source `1 2 +`, wrong-source `2 2 +`, C comment mutation, modified ELF sections and candidate trust metadata in CB-CASE-05
- WHEN C and ELF undergo separate admission checks
- THEN every forged candidate refuses with no guest operations, while unchanged trusted bytes admit without executing during admission

### Requirement: CB-GATE-01
r[CB-GATE-01]

Before any C selection, the project MUST execute actual emitted C compiled under the pinned C11 toolchain in the pinned sandbox and compare it with independently accepted selected managed-linear-memory Wasm executed by the existing Node/V8 profile on the same finite source/runtime inputs: ordered stacks, signed I64, recipes, identities, effect/trap/exhaustion prefixes, controls under off/on optimization, hostile ABI/direct syscall, tool/target/identity mismatch and sanitizer observations. Exact source/emitter/compiler/ELF/runner/mediator subjects and all failures MUST be recorded. Finite tests, sanitizer success and existing Wasm PO-17/18 evidence do not establish C lowering/compiler/native loader/host/sandbox refinement; these remain OPEN until independently discharged and an explicit later acceptance and selection decision occurs.

#### Scenario: CB-CASE-10 stale and forged receipt review

- GIVEN the three stale-source, cross-target and candidate-self-claimed passed receipts in CB-CASE-10
- WHEN production C selection evidence is reviewed against the exact target, source and tool subject
- THEN each receipt is rejected for its specific mismatch rather than accepted as native execution or proof
