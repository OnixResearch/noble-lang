# Noble

Noble is a statically typed concatenative language that compiles to WebAssembly.
Programs are immutable values: you can build them, combine them, inspect their
structure, and run them. Each program carries its input and output types, the
host effects it may request, and a recipe describing its composition.

Noble is under active development. The core compiler and runtime work within
defined, bounded profiles; the language and interfaces are not stable. See
[what works today](#what-works-today) for the current scope.

## Programs as values

Operations run from left to right on a typed stack. Literals push values;
words consume values and produce results. For example, `40 2 +` leaves `42`.
Putting code in brackets creates a program without executing its body:

```text
40 2 quote [ + ] compose run
```

This also leaves `42`, but builds the operation as a value first:

| Step | What happens |
|---|---|
| `40 2` | Push two integers. |
| `quote` | Capture `2` in a program that pushes it when run. |
| `[ + ]` | Construct a program that adds two integers. |
| `compose` | Combine the two programs: push the captured `2`, then add. |
| `run` | Execute that program with `40` already on the stack. |

Composition checks that the programs' stack interfaces fit together. Captured
values and resolved definitions stay attached to the resulting program.
`reflect` exposes its normalized recipe as inert `Syntax` data; inspecting a
program does not execute its effects.

The design centers on a few properties:

- **Typed composition.** The frontend infers stack types and effects. An
  independent kernel checks the candidate, and the backend rechecks it before
  emitting Wasm.
- **Stable meaning.** Rebinding a name does not change programs that already
  refer to its previous definition.
- **Explicit effects and ownership.** Effect bounds describe possible host
  requests. Host authorization is separate; supported component profiles track
  ownership and transfer of host resources.
- **Optional behavioral proofs.** Contracts can describe a program's behavior
  and be checked using Lean. Ordinary execution does not require a proof.

## Try it

The supplied Nix flake targets **x86_64 Linux**. It requires Nix with flakes
enabled and GitHub SSH access for the pinned `OnixResearch/octet` and
`OnixResearch/cairn` inputs.

```sh
git clone https://github.com/OnixResearch/noble-lang.git
cd noble-lang

# Realize the exact runtime tools selected by this checkout.
nix build .#node .#wasm-tools .#binaryen --no-link
nix build .#noble

printf '40 2 quote [ + ] compose run\n' > example.noble
./result/bin/noble run example.noble --opt off
```

The CLI writes JSON reports. A successful run reports `normal`, with one
`I64` value of `"42"` on the output stack. Stack order is bottom-to-top;
integers are encoded as decimal strings.

The runtime uses pinned Node/V8, wasm-tools, and Binaryen paths from
[its configuration](crates/noble-cli/src/core/runtime/config.json). Missing or
incompatible tools produce an explicit refusal. The development shell alone
does not install these runtime tools.

To retain the source, reports, WAT, Wasm, and tool observations:

```sh
./result/bin/noble run example.noble --opt on --emit example-artifacts
```

`--emit` requires a new directory. `--opt off|on` controls Wasm optimization;
both modes use the same source typing rules. `noble compile SOURCE` emits
checked WAT without executing the program.

### Persistent sessions

A session keeps its stack, definitions, captured values, and compiled programs
between submissions. Each line is one submission:

```sh
printf '%s\n' \
  'def n [ 1 ]' \
  '[ n ]' \
  'def n [ 2 ]' \
  'run n' |
  ./result/bin/noble session
```

The final stack contains `1 2`. The saved program still uses the first
definition of `n`; the later direct call uses the second.

A static rejection preserves the previous stack and namespace. A runtime trap
or exhausted execution quota ends the session, retaining any host requests
already observed. Sessions use a bounded arena that does not reclaim individual
cells. See the [runtime guide](verification/implementation-guide.md#running-core-bootstrap-source)
for quotas, framed multiline input, exit codes, and artifact formats.

## What works today

These features have separate implementation and acceptance scopes. Enabling one
does not make every other feature available in that runtime.

| Area | Implemented scope |
|---|---|
| [Core language and Wasm sessions](verification/implementation-guide.md#running-core-bootstrap-source) | Wrapping `I64` arithmetic; Bool, Text, Unit, Pair, Sum, and List values; stack operations; checked branches; quotation, capture, composition, execution, and reflection; nonrecursive definitions. |
| [Declared modules](verification/implementation-guide.md#declared-modules-v1-dxm1) | Opt-in opaque types, two-constructor variants, versioned session-local modules, imports and exports, and an explicit typed `test.emit` adapter. Available through `compile` and framed `session`. |
| [Contracts and proof checking](verification/implementation-guide.md#using-mc1-contracts) | A pure contract language, Lean obligations, proof/refutation checking, and `verify` / `explain-proof`. |
| [Evidence companions](verification/implementation-guide.md#mc2-companion-sessions-and-proof-required-builds) | Live certified programs, evidence composition, guarded invocation, and proof-required build admission for the selected core scope. |
| [Synchronous components](verification/implementation-guide.md#compiling-synchronous-wit-components) | Selected WIT bindings, component compilation, host resources, ownership transfer, and authorization checks. |
| [Native async components](verification/implementation-guide.md#compiling-native-async-wit-components) | A separate bounded component profile with native async execution and cancellation controls. General guest async syntax remains open. |
| [Local services](verification/implementation-guide.md#m7-local-synchronous-syndicate-service) and [choreography](verification/implementation-guide.md#m8-finite-local-choreography-projection) | A bounded local synchronous publisher/subscriber service and finite one- or two-round protocols over that service. |

Ordinary `run` and `session` use the **resource-free Core-Bootstrap** profile.
Their host words are `test.emit` and `test.abort`. Resources and component
features use separate entry points. Recursive definitions, local bindings,
general libraries and capability modules, portable package identities, and
general distributed services remain open.

The [exact calculator](.cairn/specs/calculator/spec.md) is a planned reference
application. The [typed-worker design](specs/WORKER-CONFORMANCE.md) describes
future worker admission and lifecycle contracts. Neither is a completed
application or agent runtime.

## Verification

Noble has two distinct verification efforts: checking claims about Noble
programs, and proving properties of the Rust implementation.

Application contracts are optional. The current pure contract profile checks
claims about every normal return, including wrapping `I64` arithmetic. It does
not establish termination or host behavior. `noble verify` checks supplied
evidence; it does not automatically search for a proof. The
[contract guide](verification/implementation-guide.md#using-mc1-contracts)
covers the pinned Lean installation and required Linux isolation tools.

Implementation verification follows **Rust → Charon → Aeneas → Lean 4**.
That route is mandatory for the semantic kernel and the target for all
Noble-owned production Rust. Scoped proofs and extraction checks exist, but
universal frontend, kernel, compiler, backend, and host correctness remain
open. Runtime tests, application proofs, extraction results, and implementation
proofs are recorded separately.

The [status ledger](specs/STATUS.json),
[proof obligations](specs/verification/obligations.json), and
[source inventory](verification/source-inventory.md) record the exact scope.
The [implementation guide](verification/implementation-guide.md) links the
milestone receipts, assumptions, and reproduction commands.

## Working on Noble

```sh
nix develop
cargo build -p noble-cli --bin noble
cargo test --workspace --all-targets --all-features --locked
```

Build and test from the repository root. Source builds use the pinned Rust
toolchain; runtime checks also need the tools realized in the quickstart.
Use Cargo's configured target directory when locating the resulting binary.

| Location | Purpose |
|---|---|
| [`crates/noble-kernel`](crates/noble-kernel) | Independent acceptance checks and deterministic semantic state transitions. |
| [`crates/noble-contracts`](crates/noble-contracts) | Source parsing, resolution, inference, modules, and application contracts. |
| [`crates/noble-wasm`](crates/noble-wasm) | Checked Wasm lowering, runtime assets, and component compilation. |
| [`crates/noble-cli`](crates/noble-cli) | CLI, compiled sessions, host orchestration, and isolated proof checking. |
| [`crates/noble-syndicate`](crates/noble-syndicate) | Bounded local service and choreography support. |
| [`.cairn/specs`](.cairn/README.md) | Canonical language and implementation specifications. |
| [`specs`](specs/README.md) | Roadmap, status, conformance scenarios, and generated compatibility views. |
| [`proofs`](proofs) and [`verification`](verification) | Lean sources, acceptance harnesses, and recorded evidence. |

Start with the [language specification](.cairn/specs/language/spec.md),
[bootstrap scope](.cairn/specs/core-bootstrap/spec.md), and
[implementation roadmap](specs/ROADMAP.md). Specifications include planned
features; consult the status ledger for implementation progress. The
[Cairn guide](.cairn/README.md#inspect-and-validate) explains document validation
and how to edit specifications.
