## Context

`W-EXEC-01` selects compiled Wasm; M4's accepted resource-free Core subset
uses managed-linear-memory Wasm and Node/V8. `C-Backend-Draft` is another
nonselected, absent native target, not a grant to run arbitrary code. The
existing `noble build --require-proof SOURCE --contract FILE` has a proof
contract and must not be reinterpreted as this backend's JSON build contract.
Gleam's [v1.19 announcement](https://gleam.run/news/gleam-doesnt-compile-to-erlang-source-anymore)
describes its own annotated abstract-form route and measured Gleam-specific
speed, not Noble behavior or performance. OTP's
[compiler documentation](https://www.erlang.org/doc/apps/compiler/compile.html#module-recommendations-for-language-implementors)
recommends abstract forms or Erlang source; direct Core Erlang risks unsupported
primops and direct BEAM assembly can crash ERTS. r[EA-SCOPE-01]

## Decisions

### Decision: A deliberately narrow first target

**Choice:** `Erlang-Abstract-Draft` is opt-in and nonselected. First positive
slice consists of one independently checked resource-free Core source unit
using in-range `I64` literals and `+`, `-`, `*`, `=` with exact ordered stacks.
Require explicit expected stack interface, checked builtin identities, empty
effect bound and finite source/check/emission/runtime limits; refuse `Text`,
`List`, `Program`, `quote`/`compose`/`run`/`reflect`, named modules, resources,
WIT, arbitrary Erlang calls and even `test.emit` in this first slice. Record
the full Core-Bootstrap vocabulary as future separate work, not implicit
partial success. r[EA-SCOPE-01] r[EA-I64-01]

**Rationale:** A positive arithmetic slice can test real source-to-forms-to-
OTP-to-BEAM execution and source annotations without asserting an unsupported
Program runtime, host authority or existing Wasm evidence transfer.

### Decision: Explicit build and compiler route

**Choice:** Only `--target erlang-abstract` selects the new grammar. Its
mandatory host-owned `CONTRACT_JSON` and `POLICY_JSON` describe complete
interface, immutable builtin/environment identities, empty effect bound,
versioned target/OTP/compiler options and finite limits. An effect-bound
field never grants host authority. `noble build --require-proof` retains its
existing proof contract, release decision and artifact behavior. A bounded
emitter emits an ordered list of approved Erlang abstract forms with `erl_anno`
line/file information from the original Noble byte positions; a bounded,
versioned ETF envelope transports that **list of terms**, not ETF bytes as
`compile:forms` input or a Noble instruction set. An independent adapter
checks framing, term shape/depth/size, permitted atoms (including a finite
host-chosen module-name pool), form attributes and source mapping before
decoding/compilation. Do not rely on `binary_to_term(...,[safe])` alone for
resource or atom limits; no unbounded caller-selected atom creation. In a
controlled compiler worker, invoke only
`compile:noenv_forms(Forms, [binary | pinned_whitelisted_options])`, reject
all source `-compile` attributes/parse transforms and nonallowlisted options,
and record exact OTP/ERTS/compiler version plus output bytes. Disable or
refuse ambient paths/configuration that can affect source or compiler
behavior. `compile:noenv_forms` bypasses `ERL_COMPILER_OPTIONS`, not arbitrary
host trust. r[EA-BUILD-01] r[EA-FORMS-01]

**Rationale:** OTP's [noenv_forms/2 API](https://www.erlang.org/doc/apps/compiler/compile.html#noenv_forms/2)
accepts forms and excludes that environment option; source attributes may
override command options, so whitelist form structure before calling it.
Gleam's announcement motivates the representation only, not a speed claim.

### Decision: Exact numeric and location semantics

**Choice:** Range-check signed source literals before lowering. BEAM integers
are unbounded: implement `I64` addition/subtraction/multiplication as explicit
modulo-2^64 normalization back to signed [-2^63, 2^63-1], preserving exact
bit equality and ordered stack operands; do not report arbitrary bignum
results as Noble values. Preserve original source file/line on accepted
forms and attributable runtime quota/trap frames; compiler diagnostics and
stacktraces must identify Noble locations rather than a generated `.erl`.
r[EA-I64-01] r[EA-FORMS-01]

### Decision: Independent admission and OS confinement

**Choice:** Domain-separated build subject binds exact host-selected source
bytes, independently checked candidate/expected interface, resolved builtin
contracts, effect and limit policy, emitter/form schema, ETF bytes, pinned OTP
toolchain/compiler options, module identity, generated BEAM bytes and source
map. Before `code:load_binary`, the invoker independently reconstructs and
compares that subject (or a separately reviewed authenticated trusted-build
route) and checks the module's actual BEAM imports/exports, on-load behavior
and exact allowed inventory. Claims/digests inside candidate metadata cannot
authorize code. Compilation never executes the candidate; runnable execution
needs a disposable separately OS-confined VM/worker with no ambient secrets,
network/filesystem/process authority or unrelated FDs, finite wall/CPU/memory
and output bounds, and a narrow checked input/result protocol. Erlang process
isolation is not that OS boundary. No arbitrary BIF, FFI, Erlang module call,
dynamic code load or `test.emit` is exposed in the first slice; a later
effectful target requires separately designed per-request host authorization.
Confinement failure or unknown OTP behavior blocks execution.
r[EA-ADMIT-01] r[EA-ISOLATE-01]

**Rationale:** Loading an admitted BEAM file is still an execution trust
boundary. Matching bytes does not prove compiler semantics; an ERTS process
must not inherit ambient machine authority or grant a capability from an
effect bound.

### Decision: Evidence before selection

**Choice:** Record source/form/BEAM/OTP/runner bytes and exact actual BEAM
execution beside actual selected managed-linear-memory Wasm for equal accepted
source/inputs and declared common budgets, including wrap, refusal,
source-location, artifact forgery and OS-boundary controls. Unsupported cases
remain unsupported; normal, static rejection, quota, trap and worker failure
are distinct. Source/OTP/host assumptions and open compiler/backend/loader
refinement remain explicit. No C, Wasm or Gleam receipt transfers.
r[EA-GATE-01]

## Risks / Trade-offs

- Whitelisted forms and pinning reduce attack surface, not the ERTS/compiler
  trusted computing base. ETF `[safe]` alone does not bound decode work,
  accepted atoms or generated module names. r[EA-FORMS-01]
- An OS-confined BEAM worker and byte-level admission are prerequisites for
  **any** runnable untrusted code. No BEAM-native process supervision or
  unsupported runtime setting substitutes for confinement. r[EA-ISOLATE-01]
- This proposal does not promise Gleam's build speed, equal performance,
  source annotation correctness, full Core-Bootstrap, selected execution or
  proof: each requires later actual source-bound observation. r[EA-GATE-01]
