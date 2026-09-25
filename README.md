# Noble

**Start with [the Cairn specs](.cairn/README.md).** Noble's implementation is staged; the specifications describe more than the currently delivered fragments.

Noble is a statically typed concatenative language designed for first-class, inspectable programs and Wasm execution.

**Implementation direction: Rust → Charon → Aeneas → Lean 4.** This route is mandatory for the entire semantic kernel and the target for all Noble-owned production Rust. Host effects require explicit proof boundaries. See [the toolchain policy](.cairn/specs/verification-toolchain/spec.md).

**Application verification:** the optional [contract profile](.cairn/specs/program-contracts/spec.md) has an MC1 frontend, versioned source/IR, Lean rules, and the `noble verify` / `noble explain-proof` CLI. MC2 adds first-class runtime evidence companions and proof-required admission on the selected Core-Bootstrap Wasm runtime. Their evidence scopes remain separate; ordinary execution does not require an application proof.

```text
(contract 1 increment
  (input (x I64))
  (output (y I64))
  (program [ 1 + ])
  (requires true)
  (ensures (eq (out y) (add (in x) 1))))
```

This is the actual [increment fixture](verification/mc1/increment.noble-contract), not general-purpose Noble runtime syntax. It asks for wrapping `I64` partial correctness on **every normal return**, not termination.

**First reference application:** [an exact programmable calculator](.cairn/specs/calculator/spec.md) with rational division and exact decimal input. It will evaluate AI-authored changes under independent acceptance criteria. This application follows the core and library gates. No calculator exists yet.

**Worker substrate contracts:** [the typed-worker conformance slice](specs/WORKER-CONFORMANCE.md) connects generated-code admission, explicit state, and cancellation-safe resource ownership. It adds no agent syntax or completed swarm runtime.

**Octet adoption:** [the boundary-contract map](specs/OCTET-ADOPTION.md) adds typed authorization/receipt rules and explicit Rust architecture gates. Octet evidence stays separate from Noble semantic proofs.

## Running Core-Bootstrap source

`noble run` resolves and infers actual source, constructs an untrusted candidate,
independently checks it, and executes compiled Wasm using **managed linear memory**.
It does not interpret source or reflected recipes. Build with `nix develop` and
`cargo build -p noble-cli --bin noble`; use Cargo's configured target directory
when locating the binary. The runtime requires the immutable Node/V8, wasm-tools
and Binaryen selections in [the runtime configuration](crates/noble-cli/src/core/runtime/config.json).
Ambient replacements are not accepted.

```sh
printf '40 2 quote [ + ] compose run\n' > capture.noble
noble run capture.noble --opt off --emit capture-artifacts
noble run capture.noble --opt on --emit capture-optimized

# Every line is one submission. The compiled old definition stays bound.
printf 'def n [ 1 ]\n[ n ]\ndef n [ 2 ]\nrun n\n' | noble session
```

The first program returns `42`. The session returns `1 2`, not `2 2`: definitions
are nonrecursive rank-1 words, and existing first-class Programs retain their
resolved bindings. Program values themselves are monomorphic and carry complete
ordered input/output interfaces and latent effects. Sessions retain real compiled
Programs, captured values, stack and namespace; they do not replay previous source.

`run SOURCE`, `session`, and `compile SOURCE` accept `--opt off|on` and
`--emit NEW_DIR`. `--emit` never overwrites an existing destination and retains
source, reports, emitted WAT, assembled/optimized Wasm and tool observations when
those stages run. `compile` emits independently accepted WAT without running it;
repeated `--input-type I64|Bool|Text|Unit` flags declare its ordered initial stack.
The execution harness must supply matching live values through the runtime ABI.
`--opt` selects execution optimization, not a different source typing rule.

For multiline or non-UTF-8 submissions, use `session --framed`: each request is
an ASCII decimal **byte count**, newline, then exactly that many source bytes.
There is no separator after the payload. Invalid source is reported without
changing prior stack or namespace. A runtime trap or exhaustion terminates the
session; already observed host requests are retained, not rolled back.

Preparation budgets can be reduced with `--source-bytes`, `--source-nodes`,
`--source-depth`, and `--source-work`. Their inclusive ranges are respectively
0–65536, 0–16384, 0–64, and 0–2000000. Exceeding a configured budget is an explicit
`exhausted` result. Runtime memory, cells, stack, continuation, recipe, depth and
step ceilings are recorded in [the ABI](crates/noble-cli/src/core/runtime/abi.json).
The bounded persistent arena does not reclaim individual cells; it eventually
reports exhaustion. There is no unbounded-memory or GC-reclamation claim.

The CLI writes `noble-core-report/v1` JSON lines. Stack order is bottom-to-top;
`I64` values use decimal strings. Program and Syntax observations include exact
normalized recipes and complete structural witnesses.

| Outcome | Exit | Meaning |
|---|---:|---|
| `normal`, `defined`, `ready` | 0 | Executed normally, installed a definition without executing its body, or emitted accepted WAT |
| `trap` | 1 | Runtime trap; request prefix retained and session terminated |
| `reject`, `type-reject`, `unbound-word`, `invalid-input`, `internal-failure` | 2 | Invalid preparation/input or internal failure; never successful execution |
| `unsupported` | 4 | Outside the declared profile or required configuration |
| `exhausted`, `runtime-exhausted` | 5 | Preparation budget or runtime quota exhausted |

The profile includes wrapping `I64` arithmetic, Bool/Text/Unit, Pair/Sum/List,
quotation, composition, execution, and inert reflection. `dup`, `drop`, `swap`
and `dip` manipulate the typed stack; `if`, `case` and `list.case` check both
branches and execute only the selected branch. Pair/Sum/List values, Programs
and Syntax can be captured by `quote`; `compose` preserves ordered interfaces,
captures, resolved identities and conservative latent effects. `reflect` returns
an exact normalized recipe, not executable source or a runtime interpreter.
Its only host words are resource-free `test.emit` (`Text --`) and `test.abort`.
Resources, imports, recursive definitions and advanced inference are not
implemented by this runtime. The optional MC2 companion and proof-required
build commands are described below. Running a program is not application proof
acceptance.

Preparation is fail-closed: parsing, resolution, inference, independent kernel
acceptance and backend rechecking precede candidate-body execution. A rejected
candidate, forged witness, unsupported profile or exhausted preparation budget
cannot fall back to interpretation or unchecked Wasm. Static refusal issues no
candidate-body host requests and preserves the prior session stack and namespace.
Runtime requests instead retain their observed prefix if a later operation traps.

### M4 acceptance evidence

M4's source frontend, compiler, persistent runtime and acceptance harnesses are
implemented for this selected **resource-free Core-Bootstrap** scope. The retained
[runtime receipt](verification/m4/acceptance.json) passed all 18 CORE cases,
11 controls and both integrated developer workflows, DX-10 and DX-12. Final
runtime acceptance used a read-only binary snapshot after Cargo tests, rather
than a mutable Cargo target that another build could replace during execution.
The receipt binds source revision
`sha256:141a385489070ef0ab681d7f78bfe8e2260fcfe8963454fd3d25b1ed1e2881a2`
and binary SHA256
`8a29f24cb1a0fa8563676fa18bbb80a87a308c5053593fe8f0ad6a31d6a6fd95`.

- [DX-10's property harness](verification/m4/property.mjs) passed 100 seeded
  arithmetic/interface/exact-recipe/effect trials, 100 replay trials, all eight
  hostile controls and bounded 50-step shrinking. Its separate malformed-kernel
  input check covers 200 cases, not an additional Wasm property trial set.
- [DX-12's executable-documentation harness](verification/m4/documentation.mjs)
  passed the two exact declared examples and all six hostile controls with
  explicit resource-free test hosts. Unsupported, failed, timed-out and unrun
  examples remain visible in coverage. A whole-command timeout alone does not
  establish guest entry, and these examples do not mean every document executes.

The [runtime gate](verification/m4/gate.mjs) writes `acceptance.json`,
`property-workflow.json` and `documentation-workflow.json` into its new artifact
directory. The durable summary is `verification/m4/evidence.json`, and the
retained runtime receipt is `verification/m4/acceptance.json`; raw runtime and
scoped workflow receipts are retained in `verification/m4/runtime.tar.gz`.
The summary records each evidence
lane separately; runtime acceptance is not extraction or refinement acceptance.

The separate [implementation gate](verification/m4/implementation.mjs) extracts
the actual whole kernel, frontend and compiler crates. Independent frontend and
compiler Lean environments avoid collisions in the pinned Aeneas discriminants.
Discovery produces a review candidate, not acceptance; check mode independently
re-extracts and compares sources, generated code, inventories, dependencies and
axioms with the reviewed lock. The durable authorities are
`verification/m4/extraction-lock.json`, `verification/m4/implementation.json`
and `verification/m4/assurance.tar.gz`, not fixed function/model counts in prose.
At M4 completion, fresh discovery and independent check both passed its reviewed lock:
no stale generated files, both compiled audits accepted, and all 27 refusal
controls refused. The full 73-rule deny-all and architecture gate passed, the
source-coverage comparison was valid, and all 13 Nix checks passed.
The [M4 completion record](verification/m4/evidence.json) binds these observations
to retained runtime and assurance archives, including the final MC1 regression.
MC2 renewed the shared reviewed lock for its source changes. M5 independently
renewed it for resources, authority and component compilation, including seven
strict actual-Rust resource roots and 52 refusal controls. Archived M4 and MC2
receipts do not substitute for that later source-bound acceptance.

MC1's 36-case regression and M3's four-configuration regression have passed in
their own scopes. They do not replace those M4 gates. The current inventory's
2,006 authored production body obligations remain open in the
[reviewed inventory](verification/source-inventory.md).
Neither extraction nor executed examples establish universal frontend, kernel,
compiler or backend refinement; PO-17/18 and SO-07 remain open. MC2 adds optional
checked companions; M5, M6 and M7's bounded resource/component/service boundaries
remain separate.

## Compiling synchronous WIT components

`noble component` selects **Component-Sync-Bootstrap**, separate from the
resource-free `run`/`session` runtime. The pinned interoperability world is
[`noble-test:sync/bootstrap@1.0.0`](crates/noble-wasm/wit/bootstrap.wit).
Generated imports retain their fully qualified, versioned request effects:

```sh
noble component bindings crates/noble-wasm/wit/bootstrap.wit bootstrap
noble component check-effect crates/noble-wasm/wit/bootstrap.wit bootstrap \
  arithmetic.inc 'noble-test:sync/arithmetic@1.0.0#inc'
```

Compilation requires exactly one source file for every selected export and a
fresh artifact directory:

```sh
noble component compile crates/noble-wasm/wit/bootstrap.wit bootstrap component-out \
  inc=inc.noble echo-text=echo-text.noble echo-bytes=echo-bytes.noble \
  read-counter=read-counter.noble transfer-counter=transfer-counter.noble \
  protected=protected.noble
```

The corresponding source bodies are:

| Export | Source |
|---|---|
| `inc` | `arithmetic.inc` |
| `echo-text` | `echo.text` |
| `echo-bytes` | `echo.bytes` |
| `read-counter` | `counter.read drop` |
| `transfer-counter` | `counters.transfer` |
| `protected` | `dup authorization.prepare swap authorization.protected` |

The CLI independently checks each complete input/output stack and effect set
before emitting `module.wat`, `core.wasm`, and validated `component.wasm`.
The complete bundle, including its report, WIT and source snapshots, is staged
privately and published by one same-filesystem rename. A write failure can leave
an empty reserved directory, but not a partially published bundle. A pre-existing
destination is refused. Valid WIT names such as `memory` and `noble-cleanup` do
not collide with canonical ABI or internal diagnostic exports.
The selected synchronous memory32/UTF-8 ABI copies strings and byte lists at the
boundary, validates ranges and encodings, and completes owned-argument preflight
before calling the host. Borrowing returns the same owner; an owned transfer
does not implicitly return it. Borrow tokens are adapter-local and have no guest
value or serialization constructor.

The [independent Rust/Wasmtime peer](verification/m5/peer) exercises the emitted
component with pinned Wasmtime 40.0.2, rather than the emitter's binding
implementation. It independently supplies component linking and typed conversion,
but reuses Noble's production resource/authority decisions for host policy. The
[source-bound build](verification/m5/build.mjs) and
[conformance gate](verification/m5/gate.mjs) retain binary, source, input and raw
command identities. [Tool and peer pins](verification/m5/pins.json) fix the world,
engine, assembler and dependency source/vendor trees.

Lossless UTF-8 and byte-list round trips use that component peer. Allocation
failures, malformed UTF-8 and truncated ranges use separate hostile-import
probes against unchanged emitted core Wasm, checking cleanup without partial
trusted results; they are not malformed typed peer values. Core allocation/copy
counters do not measure all engine allocations or physical copies.

Owners remain move-only and noncapturable. Bounded cancellation and
unexpected-suspension controls revoke guest access while retaining the native
pin through actual guest GC; late completion, duplicate callbacks and repeated
retirement cannot resurrect ownership or release twice. These local controls
are not native async execution.

Protected calls require trusted host-established one-shot authorization for the
exact plan. A request, handle, effect annotation or receipt-shaped value is not
authority. Admission commits the consumed witness and attempt before work;
receipts require an authentic, applicable observation. Cancellation of an
invocation cannot be reversed by a late successful operation observation.
The [bootstrap operation policy](.cairn/specs/wit-wasi/spec.md#bootstrap-host-operation-classification)
explicitly classifies protected work, unprotected local helpers, authorization
requests and obligation-only cleanup; it grants no ambient or remote authority.
Native release, authentic host facts/callbacks and engine/ABI correctness remain
explicit host assumptions, separate from the [strict resource proof
lane](proofs/m5/M5Resources.lean). Its seven theorem roots concern actual extracted
Rust transitions and checked table decisions, including owner return, busy-owner
exclusion, cancellation pins and late completion. Whole-source authority and
component extraction/dependency coverage is not universal refinement.

M5 is complete for this bounded profile. Acceptance is recorded in separate
[source-bound build](verification/m5/build.json),
[runtime acceptance](verification/m5/acceptance.json),
[extraction/check](verification/m5/extraction.json),
[Nix checks](verification/m5/nix-checks.json) and
[completion evidence](verification/m5/evidence.json) receipts. Their exact source,
binary, proof and archive bindings determine acceptance: 24 cases, all 84
variants, 44 native resource/authority tests and 15 controls; seven strict
resource roots and 52 extraction/audit refusal controls. The frozen CLI also
passes the retained MC1, MC2, M3 and M4 regressions. Complete collection covers
23 workspace units and the 25-unit positive boundary fixture under the unchanged
73-rule deny-all policy. The final document/Cairn Nix receipt is a separate
required closeout gate. At M5 completion, all 1,620 authored-body inventory
obligations remained open; that historical count is not the current inventory.

This synchronous profile does not implement the full Component-Draft, WASI,
native async, or a portable ABI for arbitrary first-class Noble Programs. Unsupported
signatures and bodies fail before component emission; a reviewed-pure import
still has its exact guest-request effect.

## Compiling native async WIT components

The implemented **Component-Async-Bootstrap** selection is a separate bounded
component path, not an expansion of resource-free `run`/`session` or permission
to suspend an M5 borrowed call. Its primary world is
[`noble-test:async-boundary/bootstrap@1.0.0`](crates/noble-wasm/wit/async.wit).
The following uses the exact WIT and all five Noble export bodies compiled by
the [native gate](verification/m6/gate.mjs); the output directory must be new:

```sh
noble component bindings crates/noble-wasm/wit/async.wit bootstrap
noble component compile crates/noble-wasm/wit/async.wit bootstrap async-component-out \
  order=verification/m6/cases/order.noble \
  future-result=verification/m6/cases/future-result.noble \
  stream-result=verification/m6/cases/stream-result.noble \
  future-cancel=verification/m6/cases/future-cancel.noble \
  stream-close=verification/m6/cases/stream-close.noble
```

The selected world determines the async profile; no new Noble `async`/`await`
syntax or guest scheduler is involved. The `order` body is
`host.first host.second`: native suspension completes the first import before
the second starts. The other bodies create and consume
`future<result<s64,string>>` or `stream<u8>` through typed host operations.
Live values, including a live stream nested in a Noble `Pair`, are move-only,
non-`Data` and non-`Capture`; generic duplication, dropping, capture and the
quote-then-reflect serialization route are rejected. Explicit future
cancellation, stream read-end closure and invocation cancellation are different
operations; closure alone is not producer completion or a terminal error.

Independent export/kernel checking precedes component emission. The selected
memory32/UTF-8 ABI uses native async lowering, stackful async lifting,
waitable-set/subtask operations and `task.return`; mixed synchronous members
keep their synchronous ABI. [The pins](verification/m6/pins.json) select
`wasm-tools` 1.245.1, Wasmtime and `wasmtime-wit-bindgen` 40.0.2, and
`wit-parser`/`wit-component` 0.243.0. The
[independent Rust peer](verification/m6/peer) supplies Component Model linking,
binding generation and typed conversion while deliberately reusing Noble's
production task, resource and authority decisions.

The peer's selected driver uses one cooperative engine thread, 100,000 guest
fuel, a 1,000-fuel yield quantum, a 4,194,304-byte linear-memory limit and at
most 32 retained external native jobs. It awaits task exit before observing a
successful export. Up to eight live values each reserve 128 result bytes and
128 local plus 128 external parked payload bytes. A stream has a one-byte
producer buffer and a 64-byte bounded consumer payload. These are payload and
linear-memory limits, not whole-process heap accounting or a scheduling bound.
The [resource contract](.cairn/specs/resource-adapters/spec.md#async-extension-gate)
records the task, pin, wakeup and retirement reservations separately.

Cancellation revokes guest access through the production ownership table and
invocation-isolated Store destruction; host state and native pins survive until
actual native stop and accounted retirement. Wasmtime 40.0.2 does **not** supply
a per-task cancellation API here. Protected admission still requires a trusted
one-shot witness for the exact plan. A late authentic operation success may
justify an operation receipt, but cannot uncancel the invocation or restore
guest ownership.

### M6 native execution and scoped completion

The retained [native receipt](verification/m6/acceptance.json) passes WI-11,
WI-12, WI-16 and WORKER-08: 38 variants, 62 controls, 13 native kernel tests
and 157 recorded commands. Its [source-bound build](verification/m6/build.mjs)
binds CLI SHA256
`dc951c1a12512669857a96ba3a0765569e28024db13923cf92dd3ebde0755377`.
The lanes are deliberately separate:

- **Compiled Noble:** sequential native imports, future/stream terminal cases,
  live-value eligibility refusals and positive construction controls run through
  actual component compilation and the independent peer. Additional compiled
  worlds exercise owned-resource return/domain errors and five-parameter calls.
- **Local ownership and progress:** WORKER-08's task-owner races, the complete
  five-state/fifteen-event schema controls, and fuel/epoch/blocking-deadline
  probes exercise production decisions and the peer. They are not a completed
  worker service or compiled Noble coverage for every local probe.
- **Official WASI compatibility:** a separate
  [hand-written clock component](verification/m6/peer/src/clock.wat), not Noble
  output, executes `wasi:clocks/monotonic-clock@0.3.0-rc-2025-09-16#wait-for`
  and awaits task exit. Stable `0.3.0` linkage is rejected, not substituted.
  This is neither stable WASI 0.3 support nor full WASI conformance.

[The M6 completion record](verification/m6/evidence.json) binds the native,
formal, regression and quality lanes for the selected profile. The
[runtime archive](verification/m6/runtime.tar.gz) and its
[verified manifest](verification/m6/runtime-manifest.json) retain 1,709 files,
including all 1,708 gate-retained members. Hardlink deduplication preserves their
paths, bytes and permission modes, not execution-time inode identity.

The [independently checked extraction](verification/m6/extraction.json) binds
526 current source files, 14 strict actual-Rust async correspondence and
lifecycle roots, all 75 declared constructor pairs, and 176 M6 refusal
controls (228 across the inherited lanes). Its
[reviewed lock](verification/m4/extraction-lock.json) and
[roundtrip-verified discovery/check archives](verification/m6/formal-archives.json)
retain the translated sources, generated Lean and compiled audits. These
qualified proof results do not establish physical native release or universal
host/backend refinement. The [full MC1/MC2/M3/M4/M5 regressions](verification/m6/regressions.json)
and [all thirteen declared Nix checks](verification/m6/nix-checks.json)
pass; the [roundtrip-verified assurance](verification/m6/assurance-archive.json)
and [quality](verification/m6/quality-archive.json) archives retain raw
observations. Native execution or extraction alone does not substitute for
these separate gates.
Callback/fact authenticity, physical native release, engine/ABI correctness,
clocks and OS scheduling remain explicit trust boundaries. General async
borrowing, full Component-Draft/WASI, MW1/MW2, general Syndicate and universal
frontend/kernel/compiler/backend refinement remain open.

## M7 local synchronous Syndicate service

The separately accepted [M7 completion record](verification/m7/evidence.json)
selects a finite local host-owned dataspace, facet-scoped assertions and
exact-pair interests, canonical bounded Preserves `service(name:Text,ready:Bool)`
values and the synchronous `noble:syndicate@1.0.0` WIT `service` world.
The [fresh acceptance receipt](verification/m7/acceptance.json) passes
S-CASE-09/10/11/12/17 and WI-14/18 with all 16 declared hostile variants.
Two separately compiled Noble components exercise true and false readiness,
typed add/remove, withdrawal and compiled publish-then-trap cleanup through an
independently typed Wasmtime 40 peer; the real shared-memory Wasm refusal,
Preserves schema/authority controls and facet retirement have their own case
observations. The peer supplies trusted linking and conversion, not a general
Syndicate runtime or a proof of Wasmtime/host semantics.

The [source-bound build](verification/m7/build.json) and
[independent extraction](verification/m7/extraction.json) retain a reviewed
548-source lock, exactly three pure actual-Rust dataspace functions at compiler
DefIDs 31/32/33, nine strict Lean theorems, 37 M7 mutation refusals and 265
combined refusals. The [sixteen-command assurance](verification/m7/assurance.json)
separately passed prior regressions, workspace Rust checks, published Octet
deny-all, complete Nix flake and pre-promotion Cairn/documents. The
[archived Cairn change](.cairn/archive/2026-09-25-m7-syndicate-service/proposal.md)
preserves the selected contract; `verification/m7/nix-checks.json` separately
binds the final promoted document/Cairn source. All 2,006 current authored
production bodies remain open inventory refinement obligations. This bounded
synchronous selection is not general Syndicate/Preserves, native async, full
Component-Draft/WASI, fairness, distributed exactly-once or universal
host/engine/backend refinement.

## Using MC1 contracts

From the repository root, build the `noble` binary with the pinned Rust environment:

```sh
nix develop
cargo build -p noble-cli --bin noble
export PATH="$PWD/target/debug:$PATH"
export NOBLE_CONTRACT_LIBRARY="$PWD/proofs/mc1"
export NOBLE_LEAN="$(nix build .#lean --no-link --print-out-paths)/bin/lean"
```

Proof checking requires Linux, working bubblewrap namespaces, `prlimit`, and an active user systemd manager with a usable `XDG_RUNTIME_DIR/bus`. The development shell alone does not provide that host isolation setup. Select trusted executables using absolute `NOBLE_BWRAP`, `NOBLE_PRLIMIT`, and `NOBLE_SYSTEMD_RUN` paths when they are not in the CLI's conventional `/run/current-system/sw/bin`, `/usr/bin`, or `/bin` locations. A Nix-installed Lean also requires `nix-store`; `NOBLE_NIX_STORE` can select its absolute path. `NOBLE_LEAN` must resolve to the release installation's `bin/lean`, not an elan dispatcher: Lean **4.31.0**, commit `68218e876d2a38b1985b8590fff244a83c321783`, is required. The selected contract library must contain the matching `lean-toolchain`, `NobleContracts.lean`, and its source modules.

```sh
# Prepare and explain the exact claim without launching Lean or a proof worker.
noble explain-proof verification/mc1/increment.noble-contract

# --emit must name a new directory; existing destinations are never overwritten.
noble verify verification/mc1/increment.noble-contract \
  --proof verification/mc1/increment-proof.lean \
  --emit mc1-increment --timeout-ms 120000

# An accepted refutation is "disproved" and exits 1, not a tool failure.
noble verify verification/mc1/monotonic.noble-contract \
  --refutation verification/mc1/monotonic-refutation.lean

# Statement generation only: emits artifacts, reports "unknown", exits 3.
noble verify verification/mc1/increment.noble-contract --emit mc1-statement
```

`verify` accepts one contract path followed by optional `--emit DIR`, either `--proof FILE` or `--refutation FILE`, and `--timeout-ms N`. Options cannot be repeated. `N` is 1–600000 ms; the default is 120000 ms for the entire proof workflow, including rebuilding the selected rule library. `explain-proof` accepts only the contract path. Neither explanation nor verification without supplied evidence runs proof tools; missing evidence is not automatic proof search.

Both commands write a `noble-mc1-report/v1` JSON report to stdout. Supplied Lean source imports the generated `MC1Obligation` and must define `MC1Proof.proof : MC1Obligation.claim`, or `MC1Proof.refutation : Not MC1Obligation.claim`. `--emit` retains `report.json` and, when available, `contract.noble`, `MC1Obligation.lean`, `MC1Proof.lean`, and accepted declaration data in `MC1Proof.json`.

| Outcome | Exit | Meaning |
|---|---:|---|
| `proved` | 0 | Independent consumer accepted a proof of the exact generated claim |
| `disproved` | 1 | Independent consumer accepted a refutation of that claim |
| `error` | 2 | Invalid source/options, rejected evidence, I/O failure, or a non-timeout checking failure |
| `unknown` | 3 | Prepared claim has no supplied evidence |
| `unsupported` | 4 | Unsupported contract feature or unavailable/incompatible required proof configuration |
| `timeout` | 124 | Proof workflow exhausted its wall-clock or recognized CPU-time budget |
| `not-run` | 0 | Explanation succeeded; no proof was checked |

Failed proof checking is not disproof. Missing tools, wrong Lean pins, and unavailable sandboxing fail closed; there is no unsandboxed fallback. Proof source runs without network or ambient credentials, with read-only inputs, fixed writable artifact files, and aggregate memory/task plus CPU, output, and file-size limits. The fresh trusted consumer decodes bounded inert JSON, reconstructs declarations, checks the exact expected theorem type and transitive assumptions, and **never loads producer `.olean` files**. Only `propext`, `Classical.choice`, and `Quot.sound` are allowed logical axioms; `sorry` and native-evaluation proof shortcuts do not satisfy this strict application lane.

### What MC1 establishes

- The production `no_std` [frontend](crates/noble-contracts/src/lib.rs) accepts `(contract 1 ...)` and exports contract IR revision 1. Ordinary typing comes from the inherited `noble-kernel` acceptance path, not witness inference. Export consumes an immutable `Prepared` containing that accepted candidate and resolved claim.
- The pure fragment supports explicit initial/final observations, universally quantified ghost parameters, total logical definitions, scalar/structural predicates, and higher-order `maps` claims. Ghost values cannot supply executable captures. `I64` addition, subtraction, and multiplication wrap at 64 bits; ordering is signed. Text literals, observable `Program`/`Syntax` equality, host/resource contracts, and total-correctness or prefix-safety claims are unsupported. See the [versioned grammar, logical partiality, and limits](.cairn/specs/program-contracts/spec.md#81-delivered-mc1-source-and-ir).
- Strict [application/rule proofs](proofs/mc1/NobleContracts.lean) concern the reviewed pure normal-return model, including increment, composition, and a universally quantified runtime-capture family. They do not prove current invocation preconditions, successful resource-bounded execution, host behavior, or termination.
- The separate [implementation correspondence lane](proofs/mc1/NobleContractImpl.lean) contains actual-source extraction, universal node/body projection preservation, and source-bound full preparation/export equations for the increment, composed, capture-family, structural, syntax, and signed-wrap fixtures. Those full-source equations use a separately reported native-evaluation assumption. They are **not universal frontend/compiler correctness**, and their modules are not imported by the strict application library.

The [increment](verification/mc1/increment.noble-contract), [capture-family](verification/mc1/family.noble-contract), and [structural frontend](crates/noble-contracts/src/fixtures/structural.noble) fixtures show the supported source forms. Durable run records belong in [MC1 evidence](verification/mc1/evidence.json) and [CLI acceptance](verification/mc1/acceptance.json); consult their exact source/tool identities and scopes rather than treating this overview as acceptance evidence. MC1 does not close runtime companion construction/replay, `noble build --require-proof`, MC2/Wasm, backend correspondence, or whole-language refinement.

Run the two evidence lanes independently:

```sh
# Re-extract both actual crates; compare generated code and audit compiled proofs.
# The artifact directory must not exist. Selected Nix tools and Lake dependencies
# must already be available; this gate does not substitute ambient tool versions.
bun verification/mc1/implementation.mjs .pi/mc1/implementation-run

# Exercise the real CLI, hostile proof transport, isolation, and timeout cleanup.
bun verification/mc1/gate.mjs target/debug/noble verification/mc1/acceptance.json
```

The implementation gate audits strict bridge/projection theorems and
source-equation cases separately from strict application/rule theorems. Consult
its source-bound receipt and reviewed inventory for the exact set, rather than
treating a fixed prose count as current coverage. Native computation also
discharges generated string-bound obligations; its exact axiom inventory is
retained only in the implementation lane.

## MC2 companion sessions and proof-required builds

Using the same trusted proof-tool setup as MC1, a companion session admits a
contract once and then operates on live compiled values:

```sh
printf '%s\n' \
  'contract verification/mc2/contracts/guarded.contract verification/mc2/contracts/guarded.proof.lean' \
  'submit verification/mc2/contracts/q-one.noble' \
  'certify 0 0' \
  'invoke 3 41' \
  'inspect 3' \
  'project 3' |
  noble companions --timeout-ms 600000 --opt on
```

These operations report `proved`, `normal`, `certified`,
`certified-invocation`, `inspected` and `projected`; the invocation returns
`I64` value `42`. Only the initial proof admission invokes the checker.
Indexes are zero-based live stack indexes; certification appends Contract,
Evidence and Certified cells after the original Program. Each accepted input
line emits and flushes one `noble-companions-report/v1` JSON record. Script
processing exits zero even when an operation reports a refusal, so callers must
check every record's outcome. Malformed command-line options and script I/O
failures exit 2.

`compose`, `instantiate` and `derive` use retained evidence and the finite
versioned rules. Failed applicability or intermediate preparation restores the
original frame instead of dropping its aggregate tail. `invoke` requires an
installed guard for the exact contract; an admitted theorem alone does not
synthesize an executable guard. Unsupported guards fail explicitly. Ordinary
`noble run` remains available without contract evidence or proof services.

Proof-required release binds the exact accepted quotation to the actual
compiled artifact:

```sh
noble build --require-proof verification/mc2/contracts/q-one.noble \
  --contract verification/mc2/contracts/guarded.contract \
  --proof verification/mc2/contracts/guarded.proof.lean \
  --timeout-ms 600000 --out /tmp/noble-certified-build
```

`--out` must name a new directory. A `noble-mc2-build/v1` report keeps the claim
outcome separate from `release.allowed`. Refutation, missing/unfinished/rejected
evidence, timeout, unsupported claims, stale applicability and unrelated
artifacts cannot release. Accepted artifacts carry **trusted-build
correspondence**, never a verified-backend claim.

The deterministic core uses an opaque admission request and a complete
host-check observation, not an I/O callback or a producer's certification flag.
The source-bound CLI constructs that observation only after the isolated
independent check. Arbitrary host Rust can construct observations; their
authenticity and checker soundness remain explicit trusted-host assumptions.
Strict semantic proofs, extracted correspondences, closed native source
equations and runtime tests have separate scopes. The
[implementation gate](verification/mc2/implementation.mjs) and
[runtime gate](verification/mc2/gate.mjs) retain source/tool identities,
frozen executable bytes and raw observations.

The [MC2 completion record](verification/mc2/evidence.json) retains passing
source-bound acceptance for all 15 CONTRACT cases and every declared variant,
including 18 actual independent-checker/core controls. The same frozen CLI
also passes the MC1, M3 and M4 regressions. Fresh whole-crate extraction accounts
for 312 companion bodies and 21 protocol roots; its reviewed independent check
passes all 28 refusal controls.

The proof lanes remain separate: 35 strict semantic theorems, seven strict
actual-source lemmas, two renderer-layout equations with disclosed literal-size
obligations, and 17 closed native source equations. At MC2 completion, the
73-rule deny-all and complete 21-unit source gates passed; its inventory had
1,319 open authored-body obligations. M5 renews that classification separately.
The broad PO-16/20/21 claims remain open.
See [runtime acceptance](verification/mc2/acceptance.json),
[independent extraction](verification/mc2/extraction.json) and
[the status ledger](specs/STATUS.json) for exact scopes and assumptions.

## Bounded Wasm experiment

M3 executes both WasmGC and managed-linear-memory representations, with optimization
off and on. Each configuration covers 25 declared workload variants and 27 boundary
controls: post-compilation captures, program-list selection, returned/reflection-only
programs, exact recipes, ordered interfaces, and bounded composition/cleanup.

```sh
# Uses the Nix-built Noble CLI and pinned Node/V8, wasm-tools, and Binaryen.
# The output directory must not exist; complete raw samples and artifacts are kept.
nix run .#m3-wasm -- --out /tmp/noble-wasm-comparison

# Separate actual-Rust extraction and compiled dependency/axiom audit.
bun verification/m3-wasm/implementation.mjs /tmp/noble-wasm-extraction
```

`noble wasm-experiment wasm-gc` and
`noble wasm-experiment managed-linear-memory` emit the checked experimental
vocabulary as WAT. They are not general source-compilation commands.

[M3 evidence](verification/m3-wasm/evidence.json) selects **managed linear memory
for M4** because its bounded arena is observable and does not require WasmGC,
not because it wins a speed or physical-memory comparison. Both candidates pass;
the selected layout retains one extra 64-KiB linear-memory page. Physical GC
allocation/reclamation and isolated engine peaks remain unknown.

The pure emitter's actual extraction, explicit external models and strict kernel
bridge are audited separately from runtime execution. The source-bound M3, M4
and MC2 implementation receipts own their historical inventories and reviewed
locks. M5 separately renews the shared lock and extended inventory for its sources.
Owned WAT, assembler, optimizer, engine and loader remain trust boundaries;
PO-17/18 and SO-07 are open. M4 end-to-end Core-Bootstrap acceptance is retained
in its completion record. MC2 companion assurance is a separate lane; the
accepted M5 resource/component slice has its own source-bound evidence above,
not acceptance inherited from the historical M3 component probe.

## Current work

1. Read [SPEC-0001](.cairn/specs/language/spec.md) and [the bootstrap scope](.cairn/specs/core-bootstrap/spec.md).
2. Follow [the implementation roadmap](specs/ROADMAP.md).
3. Run the document checks with Bun:

```sh
bun tools/cairn-specs.mjs --self-test
bun test tools/cairn-specs.test.mjs tools/check-specs.test.mjs
bun tools/check-specs.mjs --self-test
bun tools/cairn.mjs validate --root .
```

These checks do not run Noble or prove its safety.

## Source of truth

| Location | Role |
|---|---|
| `.cairn/specs/` | Canonical Cairn specifications, revision `0.1.0-draft.5` |
| `specs/` | Supporting ledgers, roadmap, scenario designs, and generated compatibility views |
| [specs/STATUS.json](specs/STATUS.json) | Milestone state and separate assurance dimensions |
| [specs/REVIEW-RESOLUTION.md](specs/REVIEW-RESOLUTION.md) | Disposition of the nine review findings |
| [REVIEW.md](REVIEW.md) and `review/` | Original review and its evidence |
| `noble-project-handoff-2026-09-12/` and the ZIP | Preserved historical snapshot, not current instructions |

The status ledger, not a successful example or document check, determines milestone acceptance. MC1 is a frontend/rules milestone, not a completed language runtime.
Earlier records retain their original scopes: the [budget extraction probe](proofs/m1/README.md), [component review](verification/component-review.md), [tool-selection boundary](verification/tool-selection.md), [M1 runbook](verification/m1-runbook.md), [control matrix](verification/m1-controls.md), and [boundary controls](verification/boundary-controls.md) document implementation experiments, observed controls, and historical open work. They do not substitute for current MC1, M3 or M4 evidence.
