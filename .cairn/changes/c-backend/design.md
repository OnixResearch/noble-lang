# Opt-in C backend candidate design

## Context

`Wasm-Draft` requires compiled Wasm, and M4 selected managed-linear-memory Wasm with a Node/V8 engine. An inert, nonselected C11 source emitter in this isolated WIP handles only checked, effect-free Core-Bootstrap I64/Bool/Unit expressions; it does not compile or execute native ELF. The held canonical status paragraph still describes the earlier I64/Bool slice and needs independent owner review before any integration. The proposed full native profile applies only when explicitly requested from a separately accepted Core-Bootstrap candidate. It is not the opt-in live Wasm VM session; `W-EXEC-01` need not be reinterpreted and the verification-toolchain's Eurydice/Scylla Rust↔C discussion is not a Noble→C implementation path. r[CB-SCOPE-01]

## Decisions

### Selection and first subset

**Choice:** Keep C nonselected and opt-in. Scope first target to x86_64-unknown-linux-gnu with exact C11 compiler/linker, Linux runner, ABI and runtime revisions. Core-Bootstrap, resource-free computation is the initial complete subset, with only an explicitly supplied and independently accepted `test.emit` identity where effects are requested. Unsupported resource owners, declared modules/components, arbitrary FFI, other targets, hot reload and unbound imports refuse rather than silently invoke Wasm or narrow the requested computation. r[CB-SCOPE-01] r[CB-BUILD-01]

### Build, identity and admission

**Choice:** Specify `noble build SOURCE --target c --out DIR --contract CONTRACT_JSON --policy POLICY_JSON [--native]` as C source/build manifest production, with explicit host-owned SOURCE, ordered expected interface and immutable resolved operation contract in CONTRACT_JSON, and allowed effect bound/limits/tool pins in POLICY_JSON; `--native` is the separately selected pinned compiler invocation. Exact SOURCE bytes join candidate/semantic subject, acceptance policy and emitter/compiler/runtime/target/ABI in the build key, while Noble semantic identity remains layout, source-spelling and optimization independent. The existing build grammar without C target stays unchanged. Only `noble execute --target c --artifact DIR --input INPUT_JSON [--bindings HOST_FILE]` explicitly invokes admitted bytes through the sandbox/private mediator; current replaceable/revocable host grants are read outside the guest and independently checked per request, never passed as paths/FDs or inferred from allowed effects. Existing `noble run`/`noble session` and build do not execute C. Source and ELF have distinct digests and distinct validation. Before *any* native launch, compare candidate C with pinned trusted emission of independently accepted host-selected input and candidate ELF with exact trusted build (or a future explicitly reviewed authenticated equivalent), inspect C11 source and ELF target/ABI/loader/imports, and deny self-asserted manifest trust. Source/ELF emission without sandbox remains nonrunnable. This is correspondence for a finite build, not proof that the compiler preserves Noble semantics. r[CB-BUILD-01] r[CB-IDENTITY-01] r[CB-ADMIT-01]

### Defined C11 representation

**Choice:** Require verified 8-bit bytes/exact uint64_t, unsigned-only modulo arithmetic, explicit sign-and-magnitude decimal output and signed ordering, checked tagged aggregate lengths/indexes and bounded immutable program capture/recipe DAG. Lower operations into sequenced temporary statements, not function argument expressions of unspecified order. Fail explicitly on every overflow, expired capture, malformed tag or quota; assign trap/exhaustion/OS termination separately. r[CB-NUM-01] r[CB-VALUE-01] r[CB-ORDER-01] r[CB-LIMIT-01]

### Host and actual native confinement

**Choice:** Restrict guest effects to the exact independently checked `test.emit` identity through a narrow synchronous checked text ABI. Inside the sandbox, a pinned C shim checks and copies bounded bytes into framed private request/response IPC FDs. A separate trusted host mediator revalidates exact operation, effect, frame, invocation and current authority, copies into host-owned storage, then alone dispatches protected work. Guest pointers/callable host pointers and authority never cross address spaces. Partial/oversized frames, forged operation/context and after-revocation requests fail closed with zero protected actions. An ELF can issue syscalls independent of ABI or source policy, so require a separately pinned Linux runner/worker: closed unrelated FDs, no ambient secrets, confined filesystem, no network/clock/process/exec authority, explicit loader/static-link and syscall profile, finite memory/CPU/wall time. The emitted source/runtime shim, runner and mediator are distinct pinned subjects. Explicitly refuse execution on a host without this runner. Hostile admitted test ELF syscall attempts, framed-IPC/ABI mutation and retention, quota boundaries and a benign same-sandbox control are necessary but finite evidence, not a universal sandbox proof. r[CB-HOST-01] r[CB-SANDBOX-01] r[CB-LIMIT-01]

### Native admission and runner prerequisite

**Choice:** `flake.lock` inherits selected `nixpkgs` through `octet/nixpkgs`;
the host's cached bubblewrap/libseccomp packages, the Lean proof runner,
compiler, loader, native runtime and host mediator are distinct trust
subjects. `nix/tool-selection.nix` enforces an exact selected `tool_paths`
key set. Do not weaken that comparison or turn C into a default selected
verification tool. A separate default-disabled C-native policy/observation
must compare reviewed derivation, output and byte identities for compiler,
linker, sysroot, runtime/startup, runner, syscall-filter generator/profile,
loader choice and mediator, with mismatch tests; derive proposed tools from
the already selected `octet/nixpkgs`, not a host-system package. Preserve all
selected Wasm/Lean pins and claims. r[CB-SANDBOX-01]

**Choice:** Open bounded, lexically absolute normalized host-selected paths
relative to trusted directory FDs, reject ancestor symlinks/magic links,
hard-linked mutable artifacts and directory-swap races, and snapshot exact
source, contract, policy, C and ELF bytes into sealed FDs. Independently
accept the selected source and resolved environment, re-emit and byte-compare
all C, compile the trusted C reproducibly under pinned flags, linker,
sysroot, runtime and ABI, then inspect and byte-compare the *complete*
candidate ELF, including inert sections. Launch the same admitted sealed
bytes without reopening the mutable path. Neither manifest hashes nor
successful compiler exit grant correspondence. r[CB-ADMIT-01]

**Choice:** A future owner-reviewed native launcher must use an explicit
FD table, read-only exact executable/loader closure, no ambient secrets,
reviewed namespace and cgroup limits, and a pinned syscall filter. No
bubblewrap command line or user-namespace behavior is selected:
`--unshare-all` expands to `--unshare-user-try` and
`--unshare-cgroup-try` among other namespace options; a `-try` option
may omit isolation if unavailable. A future reviewed runner MUST verify
that every required namespace actually exists and fail closed, not rely
on the flag spelling. `--share-net` can override network isolation and
`--not-a-security-boundary` explicitly disclaims it; neither belongs
in a confinement claim.
Initial executable load and later no-exec filtering need independently
reviewed ordering before candidate instructions. Seccomp cannot intercept
vDSO clock reads, direct vvar mapping reads or x86 `rdtsc`/`rdtscp`; a
static ELF or TSC-only trap/offset does not close all host time and
elapsed-time paths. Until the owner selects and reviews a complete
VM/virtualized-clock boundary or equally demonstrated native
clock/vvar/TSC and loader barrier, native execution MUST refuse.
Namespace/sandbox escape attempts MUST refuse or terminate with zero
protected operations. Do not run a hostile test ELF outside a
separately authorized sealed test-only lane. r[CB-SANDBOX-01]

| Negative control (not executed) | Insufficient shortcut | Required gate |
|---|---|---|
| Cached bubblewrap/libseccomp or the Lean proof sandbox | Package availability is not native runner selection | Reviewed separate default-disabled C policy, exact pins and owner approval |
| `../`, ancestor symlink, hardlink or swapped parent | `canonicalize`, final-component `O_NOFOLLOW` and pathname digest race | Descriptor-anchored no-symlink walk and sealed byte snapshot |
| Forged C comment or ELF inert-section change | Syntax, manifest flag or `.text` hash cannot establish native correspondence | Trusted C re-emission and reproducible full-ELF byte equality |
| `PT_INTERP`, `LD_PRELOAD`, writable loader or executable mapping | Outside code runs despite admitted ELF bytes | Reviewed static startup or exact immutable loader/ABI closure and post-load filter |
| Deny all `execve` before initial load, or allow it afterward | First prevents launch; second permits substitution | Trusted initial-load versus final no-exec stage before guest instructions |
| vDSO `clock_gettime`/`time`/`gettimeofday`, coarse clocks, direct vvar reads, x86 `rdtsc`/`rdtscp` | All bypass syscall seccomp/bubblewrap namespace flags; TSC-only trap/offset leaves host time or elapsed time | Reviewed complete clock/vvar/TSC barrier or equivalent; otherwise no native launch |
| Inherited secret FD, `/proc/self/fd` or revoked `test.emit` grant | Direct disclosure or forged/stale protected request | Fixed private FD allowlist, bounded IPC, independent mediator checking current authority on every request |
| Crash, timeout, leaked descendant or partial JSON | Partial output is not Noble completion | Parent-owned cgroup limits, kill/reap entire worker set and typed failure without post-failure effects |

The owner must approve exact runner/compiler/mediator pins, kernel and clock
assumptions, reproducible source-to-static-ELF correspondence and separately
authorize any native or hostile test execution. No package presence or C
syntax-only check authorizes MCB acceptance. r[CB-ADMIT-01] r[CB-GATE-01]

### Source-only candidate security design for independent review (HOLD)

This is a review plan, **not** a selected runner, compiler, VM, executable
recipe or authorization to launch a guest. The checked source-only emitter
does not spawn `cc`, produce ELF or satisfy CB-ADMIT-01. Its JSON duplicate
members and ancestor/output path races remain open. `--native` and
`execute --target c` continue to refuse. Canonical C text and all case
evidence remain HOLD until a separate owner decision.

1. **Pin an inert build service first.** Derive the proposed C11 compiler,
   linker, headers/sysroot, libc/startup and static runtime from the already
   selected `octet/nixpkgs`; review exact Nix derivation/output paths and
   byte hashes, fixed target, ABI, flags, linker script, locale and build
   environment. A host `cc`, existing Rust/Wasm component linker or
   successful syntax-only check is not that selection. Compile only trusted
   re-emitted C with an argument vector under a separately bounded,
   networkless compiler-service boundary. Record complete stdout/stderr,
   argv, diagnostics and tool bytes; build never launches the result.
   Prefer a reviewed static startup/ELF with zero dynamic imports. If
   dynamic imports are proposed, every loader/library byte and import must
   instead be pinned and admitted; missing or unreviewed imports refuse
   before guest instructions and before protected operations.
2. **Bind source and all native bytes independently.** Open host-selected
   SOURCE, contract, policy and candidate artifacts via trusted directory
   descriptors, rejecting ancestor symlinks, magic links, mutable hard
   links and replacement. Snapshot checked bytes in sealed read-only
   descriptors; reject duplicate JSON members before interpreting limits.
   Independently check the exact source/resolved contracts, re-emit C under
   the pinned emitter/runtime and compare *all* C bytes. Compile that
   trusted C in a hermetic fixed environment and compare *all* ELF bytes,
   including inert sections, to the sealed candidate; inspect target,
   program headers, mappings, relocations and imports. A digest, compiler
   success or signed manifest alone is not acceptance. The launch handle
   must refer to the same sealed ELF, not reopen an output pathname.
   Attestation records input/contract/policy and semantic identities,
   emitter/runtime/tool derivations and byte hashes, compiler argv,
   sysroot/ABI/static loader choice, full C/ELF hashes, runner/kernel/image
   policy and failed as well as successful observations. Current host C
   output cannot supply this attestation.
3. **Review a VM clock boundary rather than assume KVM is a clock
   barrier.** Aspen3 currently exposes
   `/nix/store/7yilgp2sawl51a36g2si9nnwfrliplxz-qemu-11.1.1/bin/qemu-system-x86_64`
   and an accessible `/dev/kvm`; those are host availability facts,
   not Noble pins or a tested barrier. An owner might evaluate a minimal,
   independently pinned QEMU machine and read-only kernel/initrd/root image
   with no host filesystem passthrough, guest network interface, RTC
   backed by host time or arbitrary devices. The guest sees only a
   bounded private request/response channel to a separate host mediator.
   The host VM process has no protected FDs/credentials and is confined
   in verified mount/user/net namespaces. Its startup and post-start syscall
   profiles must be separately reviewed: only an actually measured,
   fail-closed post-load allowlist may deny later host `execve`/`execveat`,
   network, file opens and namespace changes without breaking required
   QEMU operations. An unverified seccomp allowlist is not a barrier.
   A VM-image build, boot transcript, machine model, virtual-device list
   and QEMU/kernel/firmware byte pins are required. Ordinary KVM TSC
   offset/scaling still reveals elapsed host time; a TSC-only trap
   cannot stop vDSO/coarse `time` or direct vvar reads. A candidate
   deterministic virtual-time engine, such as independently evaluated
   software-emulated instruction time, is acceptable only after sealed
   hostile `__vdso_clock_gettime`,
   `__vdso_time`, `__vdso_gettimeofday`, coarse-clock, direct vvar,
   `rdtsc` and `rdtscp` controls prove no host wall or elapsed time
   disclosure in the actual pinned guest. A specific **unselected**
   software-emulation candidate for review is single-thread TCG with
   fixed `-icount shift=N,sleep=off,align=off`, a fixed-date
   `-rtc base=<reviewed-date>,clock=vm`, one vCPU and an enumerated
   minimal device set. Do not use `shift=auto`, `sleep=on`, `align=on`
   or a host-time RTC as a claimed deterministic clock: QEMU documents
   host-clock adjustment or synchronization for those modes. `icount`
   instruction time is not cycle-accurate, and this candidate has not
   been tested for guest TSC/vDSO/vvar nondisclosure or hypervisor
   escapes. It is not a KVM configuration; the host KVM device alone
   cannot establish an instruction-time barrier. Review the exact
   selected QEMU release and guest kernel, not only upstream
   [TCG icount](https://www.qemu.org/docs/master/devel/tcg-icount.html),
   [invocation](https://www.qemu.org/docs/master/system/invocation.html)
   and [KVM clock](https://www.qemu.org/docs/master/system/i386/kvm-pv.html)
   descriptions. No such VM, instruction-time behavior or equivalent
   sealed-loader barrier is currently verified.
4. **Fail closed on guest authority and resources.** Any guest loader
   setup must finish before a reviewed no-exec/no-writable-executable
   policy is active and before untrusted instructions; rejecting initial
   `execve` too early prevents launch, while permitting later exec or
   writable executable mappings permits substitution. Deny guest direct
   file, network, process/namespace, unreviewed clock and host-secret-FD
   paths. A successful guest `unshare` is not evidence of host namespace
   escape safety: attempt mount/user/net breakout under the actual VM and
   host runner and require refusal/termination with zero protected
   operations. The private mediator independently validates complete
   bounded frames, typed `test.emit`, invocation and current/revoked
   authority before dispatch. Missing or unreviewed ELF imports refuse
   admission; a missing/unknown guest ABI operation traps without a
   fallback host function and with zero protected operations. Malformed
   frames, crashes and partial output likewise cause no protected action.
   Charge candidate, stack, aggregate, list, recipe, text, output and
   step limits before growth. Separately apply parent-owned cgroup
   memory/CPU/task/output caps and a monotonic wall deadline; kill and
   reap the whole VM/process tree on failure. A language trap, in-guest
   quota, host kill/timeout, admission refusal and denial
   remain distinct; guest time cannot read the host monotonic budget.

For CB-CASE-01..10, a later differential harness must run the *same
independently accepted source and typed inputs* through selected Node/V8
managed-linear-memory Wasm and the separately admitted emitted-C/full-ELF
guest under identical checked contracts and explicitly recorded limits.
Compare ordered typed stack values (including signed I64 and immutable
Program recipes), traces and protected-action prefixes, normal/trap/quota/
denial classifications and both C optimization modes. Preserve the
different accounting of C source-only steps and Wasm fuel rather than
falsely calling unequal budgets equal. CB-01's supplied `list_length: 2`
still refuses under the present exact I64/Bool policy; CB-02 needs values
and runtime inputs not yet emitted; CB-03/07 need a real mediator; CB-04/05
need exact tool/ELF admission; CB-06 needs list quotas; CB-08 needs the
sealed hostile VM lane; CB-09/10 need scope and stale-receipt review.
None of the ten case designs has executed or been promoted. C source
syntax, compiler success, finite differential matches and M4 Wasm receipts
are not a C lowering, loader, confinement or refinement proof.
r[CB-BUILD-01] r[CB-LIMIT-01] r[CB-HOST-01] r[CB-SANDBOX-01]
r[CB-ADMIT-01] r[CB-GATE-01]

### Evidence and proof

**Choice:** Require execution of actual emitted and pinned-C-compiled code in sandbox versus actual selected managed-linear-memory Wasm under existing Node/V8 profile for identical accepted source and inputs, including exact values/signed rendering, recipes, effect prefix, authorization, traps and quota classifications at agreed bounds. Run off/on optimizations, sanitizer and adversarial ABI controls, retain all source/tool/native bytes and failures. Never import M4 receipt, selected Wasm PO-17/18 or an unexecuted scenario as C success. C lowering, compiler, native loader, host and sandbox refinement remain OPEN until separately discharged and selected. r[CB-GATE-01]

## Risks / Trade-offs

- General-purpose Linux native execution cannot be made safe by C syntax inspection or a host capability table alone. A strong sandbox depends on kernel, loader, syscalls, FD state and attack surface; absent pinned controls block runnable C entirely. r[CB-SANDBOX-01]
- Byte reproduction of trusted compiler output couples admission to a pinned toolchain and reproducibility; a future authenticated build route would need separate reviewed trust assumptions. Byte equality is not a semantic preservation proof. r[CB-ADMIT-01]
- Tool/version support, backend correctness, ABI ownership, resource exhaustion and source/native refinement are unverified and no feature or production claim is made by this spec-only change. r[CB-GATE-01]
