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

### Independent Mathlib comparison for implemented I64 arithmetic

`verification/mathlib-i64/compare.mjs` compiles
[`MathlibI64.lean`](proofs/m3/MathlibI64.lean) with pinned Lean 4.31.0 and
explicitly imports Mathlib4 at `fabf563a7c95a166b8d7b6efca11c8b4dc9d911f`
(the existing `proofs/m3/lake-manifest.json` dependency). The mathematical
model takes an `Int` through `ZMod (2^64)`, then interprets residues below
`2^63` as nonnegative and residues above it as negative. The Lean module
checks the general model identities for addition, subtraction and
multiplication and three signed-boundary examples. The comparator asks Lean
to calculate finite expected values, rebuilds the actual Noble CLI from
this checkout, executes Noble-compiled managed Wasm for those same inputs,
checks emitted source/WAT/Wasm hashes and selected tool identities, and
rejects a deliberately wrong subtraction in place of addition.

With the existing pinned Lake package cache and selected Nix tools available:

```sh
NODE=/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node
env -i "$NODE" verification/mathlib-i64/compare.mjs \
  --output "$HOME/.cache/noble-i64-comparison-$(date +%s)"
```

For another checkout, select `tool_paths.node.output` from
`policy/tool-selection.json` and invoke its `bin/node` directly. `--output`
requires a **new** directory and retains `comparison.json`, the generated
Lean oracle, and each actual emitted `.noble`, `.wat`, `.wasm` and tool receipt.
Without `--output`, the same check uses a disposable private directory.
The example removes all inherited variables; if passing an environment
directly, the comparator explicitly refuses Lean/Lake, Node, Bash, loader,
Cargo, Rust, Git and Nix overrides rather than letting them reach Lean, Lake,
Cargo or the Wasm worker. The comparator creates its own
temporary directory under the account's `.cache`, passes an explicit minimal
child environment and requires the Noble worktree and pinned Mathlib Git
checkout clean with unchanged HEAD/tree before and after. Its `source_tree`
identifies the *whole committed Noble tree*, not merely the listed diagnostic
file hashes. Ignored build outputs (`target`, `.lake`), the compiler's
preexisting cache, and hostile changes that race or tamper with the verifier
are not attested. The selected `proofs/m3/.lake/packages` Mathlib source
checkout must match the pinned manifest; the already compiled Mathlib
`.olean` import cache remains a **separate trust assumption**. This command
checks the local import and Noble model with Lean but intentionally does not
rebuild or formally recheck every Mathlib source file.

This is a finite backend comparison, **not** a universal compiler/Wasm proof.
The exact BigInt/Rat calculator and its error/state behavior are not
implemented or verified by this check; CALC-EVIDENCE-01/02 remain open,
CALC-01 and related scenario statuses remain unchanged, and the separate
VC-NUM-01 contract obligation does not inherit a calculator proof.

To retain the source, reports, WAT, Wasm, and tool observations:

```sh
./result/bin/noble run example.noble --opt on --emit example-artifacts
```

`--emit` requires a new directory. `--opt off|on` controls Wasm optimization;
both modes use the same source typing rules. `noble compile SOURCE` emits
checked WAT without executing the program.

Use `noble session --text-byte-cursor` or
`noble compile SOURCE --text-byte-cursor` to select the **Text-Byte-Cursor-v1**
experimental profile (contract in the active
[cursor change](.cairn/changes/text-byte-cursor/design.md), not yet selected
for the ordinary language). Only this opt-in profile resolves pure `text.byte`.
It consumes `Text I64` and leaves the original Text and one `I64`: unsigned
UTF-8 byte 0..255 for an in-range offset, -1 at exactly the byte length
(including empty Text), -2 for a negative offset, or -3 for an offset beyond
the length. Increment the offset only after a nonnegative byte. The operation
does not copy a suffix, allocate an aggregate, or request a host effect.
Ordinary `run`, declared modules, and live REPL cannot select this profile.
Managed-Wasm reports from the opted-in session identify `Text-Byte-Cursor-v1`.

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
cells. Use `noble session --framed` for framed multiline input and
`--emit NEW_DIR` to retain reports and emitted artifacts. Run `noble --help`
for available CLI flags.

### Guarded live REPL (opt-in, partial)

`noble live repl` runs a separate resident Node/V8 session with an explicitly
selected absolute source-file path. Given `/absolute/math.noble` containing
`def addone [ 1 + ]` (replace `/absolute` with a real directory):

```sh
./result/bin/noble live repl --source /absolute/math.noble --engine v8
# After the initial file ACK, enter:
20 addone
```

In another shell, save a complete replacement in the same directory and
atomically rename it onto the selected file. For example:

```sh
python3 - <<'PY'
import os
from pathlib import Path

source = Path("/absolute/math.noble")
temporary = source.with_name(source.name + ".tmp")
with temporary.open("xb") as file:
    file.write(b"def addone [ 2 + ]\n")
    file.flush()
    os.fsync(file.fileno())
os.replace(temporary, source)
PY
```

Back in the REPL, enter `:reload /absolute/math.noble`, wait for a
`reload-committed` ACK, then enter `20 addone` to use the new definition.
Replies are JSON lines. An ACK's `source_sha256` identifies the checked bytes
committed as that generation, not a promise that a concurrently changed path
remains at those bytes. Failed reloads preserve the prior stack and namespace.
If the selected candidate fails source checking, its `reload-refused` report
retains structured source diagnostics such as `source_span`, `word_or_join`,
ordered stack constraints and value origin when available.
`source_span_basis: "refused-candidate-file-byte-offsets"` means those spans
refer to the refused candidate file's bytes, not the installed old definition.
Existing saved pure Programs retain their old definition even when a new
top-level call uses the replacement.

The exact `:generation` line inspects the current Core-only live namespace
without reloading, reading the source file, invoking the worker or consuming
a self-edit grant. Its one `noble-live-report/v1` JSON line has
`stage: "inspection"`, `outcome: "generation-observed"`, the host publication
`generation` (not the Wasm submission count), `engine: "v8"` and an ordered
`stack` of checked typed values, including retained Programs. Ordinary
expression submissions and refused reloads do not advance this generation;
direct host definition publication, successful selected-file reload and
host-admitted guest proposal publication do. Extra text, including a trailing
space, is not the command. This inspection is not a reload ACK or a current
selected-file hash, and does not complete DX-LIVE-01 or any LIVE case.

`noble live watch` is unavailable. A directory rename notification proves
only that a pathname changed, not which bytes the renamed inode contained
at that instant. A writable mapping or a hardlink outside the selected
directory can change those bytes without a corresponding selected-basename
content notification. Automatic publication could therefore ACK bytes
written in place after the rename. Use `live repl --source` and explicit
`:reload /absolute/math.noble` for a complete save; no automatic watch ACK
or full H-LIVE-01/LIVE-05 acceptance is claimed.

For the tested acyclic Core `addone→twice→four` chain, selected-file reload
rebuilds transitive dependents in dependency order. New top-level calls see the
rebuilt generation after one source-generation ACK; saved old Programs still
use their original identities and dependencies. Scoped indirect self-rebind
refusal was exercised in a live child, but this guarded in-process Wasm path
does not establish general cycle safety, proof or declared-module snapshots.

An exact host grant can allow the selected named definition to propose its
own *pure* replacement from an actual runtime input. Put this in
`/absolute/evolve.noble` and start a separate session:

```text
def evolve [ dup 18 - quote [ + ] compose self.generation swap self.propose drop 1 + ]
```

```sh
./result/bin/noble live repl --source /absolute/evolve.noble --engine v8 \
  --self-edit evolve --expect-generation 1
# After the file ACK and grant-selected report, enter:
20 evolve
# The old body returns 21; wait for proposal-committed (generation 2).
20 evolve
# The new pure body returns 22.
```

The first invocation captures `20 - 18 = 2` in a composed `Program`; an
independent fresh session starting with `21 evolve` instead captures `3` and
later makes `20 evolve` return `23`. The guest only queues a bounded snapshot
during actual Wasm execution. After return, the host independently checks the
candidate's restricted pure I64 arithmetic recipe and one-use generation
grant before publication. `self.generation` and `self.propose` are live-only.
The selected file remains unchanged: a guest-derived `source_sha256` is not the
`selected_file_sha256`, and its freshness is `not-file-backed`.
The bounded guest self-edit lane refuses a proposal when the selected name
has any current effective dependent; it does not rebuild those dependents.
Only host-selected file reload and direct host definition admission exercise
the acyclic Core transitive rebuild. A rejected guest candidate publishes
neither a successor nor a stale ACK.

This is a **guarded partial** live profile, not automatic H-LIVE-01/LIVE-05
watch acceptance, selected declared-module snapshots, complete source-bound LIVE-02
acceptance, interpreter execution, code retirement, general guest mutation,
or PushGP evolution.
`--engine interpreter` refuses rather than falling back to V8.
Ordinary `run` and `session` are unchanged. The canonical LIVE-01–10 cases
remain `absent`/`not-run`, proof open, with no accepted source-bound assurance;
see the [active change](.cairn/changes/live-wasm-reload/tasks.md) and
[status ledger](specs/STATUS.json).

## What works today

These features have separate implementation and acceptance scopes. Enabling one
does not make every other feature available in that runtime.

| Area | Implemented scope |
|---|---|
| [Core language and Wasm sessions](specs/ROADMAP.md#m4-implementation-and-retained-acceptance) | Wrapping `I64` arithmetic; Bool, Text, Unit, Pair, Sum, and List values; stack operations; checked branches; quotation, capture, composition, execution, and reflection; nonrecursive definitions. |
| [Guarded live REPL](#guarded-live-repl-opt-in-partial) | In-process Wasm emission and explicit reload use one resident Node/V8 arena and checked publication; tested acyclic dependent rebuild and retained old Programs. Automatic watch is unavailable because rename events cannot authenticate bytes. Guest self-edit refuses current dependents; scoped indirect self-rebind/cycle refusal was exercised without general proof or canonical LIVE case acceptance. No declared-module snapshots or full LIVE-05 acceptance. |
| [Declared modules](specs/ROADMAP.md#dxm1-selected-declared-modules-bounded-acceptance-complete) | Opt-in opaque types, two-constructor variants, versioned session-local modules, imports and exports, and an explicit typed `test.emit` adapter. Available through `compile` and framed `session`. |
| [Contracts and proof checking](specs/PROGRAM-CONTRACTS.md#12-verification-tooling) | A pure contract language, Lean obligations, proof/refutation checking, and `verify` / `explain-proof`. |
| [Evidence companions](specs/PROGRAM-CONTRACTS.md#83-mc2-companion-implementation-and-evidence-boundaries) | Live certified programs, evidence composition, guarded invocation, and proof-required build admission for the selected core scope. |
| [Synchronous components](specs/WIT-WASI.md) | Selected WIT bindings, component compilation, host resources, ownership transfer, and authorization checks. |
| [Native async components](specs/WIT-WASI.md#selected-native-async-boundary-and-compatibility) | A separate bounded component profile with native async execution and cancellation controls. General guest async syntax remains open. |
| [Local services](specs/ROADMAP.md#m7-selected-local-synchronous-service-bounded-acceptance-complete) and [choreography](specs/ROADMAP.md#m8-selected-choreography-projection-bounded-acceptance-complete) | A bounded local synchronous publisher/subscriber service and finite one- or two-round protocols over that service. |

Ordinary `run` and `session` use the **resource-free Core-Bootstrap** profile.
Their host words are `test.emit` and `test.abort`. Resources and component
features use separate entry points. Recursive definitions, local bindings,
general libraries and capability modules, portable package identities, and
general distributed services remain open.

The [exact calculator](.cairn/specs/calculator/spec.md) is a planned reference
application. Its [numeric error schema prerequisite](verification/calculator-errors/README.md)
has a finite 18-case compiled-guest observation on a rebuilt binary, explicitly
**not source-bound**; the opt-in byte cursor does not establish MA1 or CALC
acceptance. The [typed-worker design](specs/WORKER-CONFORMANCE.md) describes
future worker admission and lifecycle contracts. Neither is a completed
application or agent runtime.

## Verification

Noble has two distinct verification efforts: checking claims about Noble
programs, and proving properties of the Rust implementation.

Application contracts are optional. The current pure contract profile checks
claims about every normal return, including wrapping `I64` arithmetic. It does
not establish termination or host behavior. `noble verify` checks supplied
evidence; it does not automatically search for a proof. The
[contract specification](specs/PROGRAM-CONTRACTS.md) states the bounded
semantics; the [toolchain specification](specs/VERIFICATION-TOOLCHAIN.md#52-mc1-consumer-configuration-and-limits)
lists the pinned Lean installation and required Linux isolation tools.

Implementation verification follows **Rust → Charon → Aeneas → Lean 4**.
That route is mandatory for the semantic kernel and the target for all
Noble-owned production Rust. Scoped proofs and extraction checks exist, but
universal frontend, kernel, compiler, backend, and host correctness remain
open. Runtime tests, application proofs, extraction results, and implementation
proofs are recorded separately.

The [status ledger](specs/STATUS.json),
[proof obligations](specs/verification/obligations.json), and
[source inventory](verification/source-inventory.md) record the exact scope.
The [roadmap](specs/ROADMAP.md) links milestone receipts and their scoped
assumptions; retained verification runners carry reproduction commands.

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
