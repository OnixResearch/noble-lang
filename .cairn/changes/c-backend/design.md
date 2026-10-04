# Opt-in C backend candidate design

## Context

`Wasm-Draft` requires compiled Wasm, and M4 selected managed-linear-memory Wasm with a Node/V8 engine. The new native profile applies only when explicitly requested from a separately accepted Core-Bootstrap candidate. It is not the opt-in live Wasm VM session; `W-EXEC-01` need not be reinterpreted and the verification-toolchain's Eurydice/Scylla Rust↔C discussion is not a Noble→C implementation path. r[CB-SCOPE-01]

## Decisions

### Selection and first subset

**Choice:** Keep C nonselected and opt-in. Scope first target to x86_64-unknown-linux-gnu with exact C11 compiler/linker, Linux runner, ABI and runtime revisions. Core-Bootstrap, resource-free computation is the initial complete subset, with only an explicitly supplied and independently accepted `test.emit` identity where effects are requested. Unsupported resource owners, declared modules/components, arbitrary FFI, other targets, hot reload and unbound imports refuse rather than silently invoke Wasm or narrow the requested computation. r[CB-SCOPE-01] r[CB-BUILD-01]

### Build, identity and admission

**Choice:** Specify `noble build SOURCE --target c --out DIR --contract CONTRACT_JSON --policy POLICY_JSON [--native]` as C source/build manifest production, with explicit host-owned SOURCE, ordered expected interface and immutable resolved operation contract in CONTRACT_JSON, and allowed effect bound/limits/tool pins in POLICY_JSON; `--native` is the separately selected pinned compiler invocation. Exact SOURCE bytes join candidate/semantic subject, acceptance policy and emitter/compiler/runtime/target/ABI in the build key, while Noble semantic identity remains layout, source-spelling and optimization independent. The existing build grammar without C target stays unchanged. Only `noble execute --target c --artifact DIR --input INPUT_JSON [--bindings HOST_FILE]` explicitly invokes admitted bytes through the sandbox/private mediator; current replaceable/revocable host grants are read outside the guest and independently checked per request, never passed as paths/FDs or inferred from allowed effects. Existing `noble run`/`noble session` and build do not execute C. Source and ELF have distinct digests and distinct validation. Before *any* native launch, compare candidate C with pinned trusted emission of independently accepted host-selected input and candidate ELF with exact trusted build (or a future explicitly reviewed authenticated equivalent), inspect C11 source and ELF target/ABI/loader/imports, and deny self-asserted manifest trust. Source/ELF emission without sandbox remains nonrunnable. This is correspondence for a finite build, not proof that the compiler preserves Noble semantics. r[CB-BUILD-01] r[CB-IDENTITY-01] r[CB-ADMIT-01]

### Defined C11 representation

**Choice:** Require verified 8-bit bytes/exact uint64_t, unsigned-only modulo arithmetic, explicit sign-and-magnitude decimal output and signed ordering, checked tagged aggregate lengths/indexes and bounded immutable program capture/recipe DAG. Lower operations into sequenced temporary statements, not function argument expressions of unspecified order. Fail explicitly on every overflow, expired capture, malformed tag or quota; assign trap/exhaustion/OS termination separately. r[CB-NUM-01] r[CB-VALUE-01] r[CB-ORDER-01] r[CB-LIMIT-01]

### Host and actual native confinement

**Choice:** Restrict guest effects to the exact independently checked `test.emit` identity through a narrow synchronous checked text ABI. Inside the sandbox, a pinned C shim checks and copies bounded bytes into framed private request/response IPC FDs. A separate trusted host mediator revalidates exact operation, effect, frame, invocation and current authority, copies into host-owned storage, then alone dispatches protected work. Guest pointers/callable host pointers and authority never cross address spaces. Partial/oversized frames, forged operation/context and after-revocation requests fail closed with zero protected actions. An ELF can issue syscalls independent of ABI or source policy, so require a separately pinned Linux runner/worker: closed unrelated FDs, no ambient secrets, confined filesystem, no network/clock/process/exec authority, explicit loader/static-link and syscall profile, finite memory/CPU/wall time. The emitted source/runtime shim, runner and mediator are distinct pinned subjects. Explicitly refuse execution on a host without this runner. Hostile admitted test ELF syscall attempts, framed-IPC/ABI mutation and retention, quota boundaries and a benign same-sandbox control are necessary but finite evidence, not a universal sandbox proof. r[CB-HOST-01] r[CB-SANDBOX-01] r[CB-LIMIT-01]

### Evidence and proof

**Choice:** Require execution of actual emitted and pinned-C-compiled code in sandbox versus actual selected managed-linear-memory Wasm under existing Node/V8 profile for identical accepted source and inputs, including exact values/signed rendering, recipes, effect prefix, authorization, traps and quota classifications at agreed bounds. Run off/on optimizations, sanitizer and adversarial ABI controls, retain all source/tool/native bytes and failures. Never import M4 receipt, selected Wasm PO-17/18 or an unexecuted scenario as C success. C lowering, compiler, native loader, host and sandbox refinement remain OPEN until separately discharged and selected. r[CB-GATE-01]

## Risks / Trade-offs

- General-purpose Linux native execution cannot be made safe by C syntax inspection or a host capability table alone. A strong sandbox depends on kernel, loader, syscalls, FD state and attack surface; absent pinned controls block runnable C entirely. r[CB-SANDBOX-01]
- Byte reproduction of trusted compiler output couples admission to a pinned toolchain and reproducibility; a future authenticated build route would need separate reviewed trust assumptions. Byte equality is not a semantic preservation proof. r[CB-ADMIT-01]
- Tool/version support, backend correctness, ABI ownership, resource exhaustion and source/native refinement are unverified and no feature or production claim is made by this spec-only change. r[CB-GATE-01]
