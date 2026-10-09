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

The `.#noble` package declaration builds the CLI with `--no-default-features`;
the `m3-wasm` app selects that binary by default. The private
`live-test-barrier` and `live05-harness-effects` modes require an explicitly
feature-built binary and are not enabled by the normal Cargo build either.
All-feature verification tests are separate from the binary installed by the
package.

The runtime uses pinned Node/V8, wasm-tools, and Binaryen paths from
[its configuration](crates/noble-cli/src/core/runtime/config.json). Missing or
incompatible tools produce an explicit refusal. The development shell alone
does not install these runtime tools.

### Inert C11 source candidate (nonselected)

The explicit `build SOURCE --target c` route emits **source only**, not an
executable or an alternative to the selected Wasm runtime. This isolated
WIP independently checks an effect-free Core-Bootstrap expression with one
I64, Bool, Unit or Text result, then lowers ordered stack operations, the
`unit` constructor and exact checked UTF-8 Text payloads, immutable quotation
bodies, runtime `quote` capture,
`compose`, `run`, pure `dip` and checked `if` branches into bounded C11 instructions and
Program values. This WIP also lowers checked `pair`/`unpair`,
`inl`/`inr`/`case` and `nil`/`cons`/`list.case` **internally** to a
256-slot immutable tagged aggregate arena. Checked monomorphic Core types
are interned as local shape IDs; every supported operation checks the
exact ordered operand shapes before mutation. Pair/Sum/List references
and static/dynamic Program shapes are checked before use; quotation
captures retain arena values, and an iterative, step-charged walk bounds
nested data and Program capture recipes. This does not admit aggregate
or Program final results. Text literals are
bound by node identity to exactly one
host-selected checked `Body.texts` payload; static C bytes and bounded JSON
output preserve escapes and UTF-8 without retaining a borrowed buffer.
The host-supplied `expected_interface` is parsed into ordered Core `Ty`
values with the existing bounded source type grammar, then compared with
independent kernel acceptance. The parser recognizes nested Pair/Sum/List,
Syntax and pure Program contracts, but source-only final Pair/Sum/List/Syntax/
Program outputs still refuse. `reflect`, host effects, named definitions,
runtime inputs and other profiles refuse; there is no Wasm fallback.
The new internal Pair/Sum/List path is unreviewed
source-only WIP, reconciled in the active change delta but not yet synced
into the held canonical CB-BUILD-01 paragraph.

```sh
printf '20 2 quote [ + ] compose run\n' > capture.noble
printf '%s\n' '{"expected_interface":"-- I64","resolved_host_contracts":{}}' > c-contract.json
printf '%s\n' '{"profile":"Core-Bootstrap","target":"x86_64-unknown-linux-gnu","allowed_effects":[],"limits":{"stack_slots":8,"steps":100,"output_bytes":1024}}' > c-policy.json
noble build capture.noble --target c --contract c-contract.json \
  --policy c-policy.json --out NEW_OUTPUT_DIRECTORY
```

The new directory contains the exact source, contract, policy, `artifact.c`,
and `manifest.json` with exact-byte file digests and a build key. The checked
candidate and resolved-environment digest fields hash this toolchain's Rust
Debug rendering, not a canonical external encoding. This is candidate
evidence, **not** independent admission or C/Wasm equivalence. Contract and
policy JSON reject duplicate member names recursively before checking limits,
including names whose Unicode escapes decode to the same key. The exact
source-only policy still rejects supplied `limits.list_length` rather than
admitting an unenforced bound: the internal List arena enforces its own
fixed 255-element ceiling, **not** the CB-CASE-01/06 policy controls.
Input files are read through one
bounded file descriptor each, then the observed bytes are
copied; this does not authenticate the host-selected pathname against ancestor
symlinks, hard links or concurrent replacement, nor seal output files against
subsequent mutation. Output child paths are also re-resolved after creating the
directory, so a concurrently replaced output directory is not authenticated.
The manifest declares fixed ceilings: 256 dynamic Program slots, 256
aggregate slots, List length 255, 512 shape identities, 512 frames,
256 recipe leaves, and depth 256. Every C
dispatch iteration consumes the
policy step budget, including empty quotations, composition traversal,
selected branches and `dip` restore continuations. Quotation capture also
charges a step per visited nested datum; the aggregate arena has no
deallocation or borrowed references during an invocation.
Finishing a frame also costs a step; these counts do not claim Wasm parity.
These are source-level bounds, not native runtime acceptance. Both `--native`
and `execute --target c` refuse: no compiler/ELF execution is authorized until
a separately reviewed pinned native toolchain, complete source-to-ELF
correspondence, sandbox, loader, and clock barrier are selected.
The existing bubblewrap runner is for Lean proofs, not C; installed QEMU/KVM
or host compiler binaries do not supply a selected VM image, immutable
runner/filter/loader/mediator pins or a verified vDSO/vvar/TSC barrier.
For a **developer-only, nonexecuting** C11 syntax diagnostic on emitted source,
the isolated CLI test uses the existing pinned component GCC15 wrapper with
`-std=c11 -pedantic-errors -fsyntax-only` against `artifact.c`. That compiler
pin is not a selected C backend compiler or a native artifact admission. It
produces no admitted ELF and cannot establish C/Wasm differential results.
All CB-CASE-01..10 remain absent/not-run/open/unassessed; the active C delta
has been reconciled against all eleven canonical requirement bodies and
their linked scenario designs, but structural Cairn proposal/design/tasks
gates and a nonblocked *preview* are not independent owner approval or
permission to sync it. The eight remaining implementation, owner-review
and assurance tasks stay open. The
canonical CB-BUILD-01 still describes the earlier narrower source-only slice,
not the unreviewed internal Pair/Sum/List WIP. The [review-only native security design](.cairn/changes/c-backend/design.md)
lists prerequisites, not a selected
runner. No clock/vvar/TSC nondisclosure barrier or C selection follows
from source syntax diagnostics.

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
Ordinary `run`, ordinary declared modules, and live REPL cannot select this
standalone profile.
Managed-Wasm reports from the opted-in session identify `Text-Byte-Cursor-v1`.

For **pure declared Text modules**, select the separate combined profile with
`noble session --framed --declared-modules --text-byte-cursor` or
`noble compile SOURCE --declared-modules --text-byte-cursor`. It retains the
same `text.byte` behavior as the standalone cursor and additionally resolves
`<` as a pure *signed* `I64 I64 -- Bool` comparison (`-1 0 <` yields `true`).
The combined prefix is selected by the host's flags, never by source or a
forged kernel candidate. No `--bindings` file or host operation is permitted.
Add `--signed-self-recursion` to allow an exported declared word with an
explicit `signature` to call its own unqualified name at the *same* owner
signature; different-owner/mutual or polymorphic recursion remains refused.
Missing signatures, wrong module versions and invalid candidates do not
publish a partially checked module. Reports identify `Declared-Text-v1` or
`Signed-Declared-Text-v1`; no pure guest imports a host function.

This combined profile also accepts immutable lexical `let name` bindings.
Each `let` consumes the current top value until its enclosing definition,
quotation, or root expression ends. Initial binders spell ordered parameters:
`def add [ let right let left left right + ]` preserves the expected
`I64 I64 -- I64` order. A nested quotation captures the *current checked
value*, so later name shadowing cannot rebind it. Duplicate names in one
scope and resource copying, implicit discard, or capture without the
kernel's `Data`/Capture eligibility are refused before execution. Reflection shows
the checked capture and stack operations, not a dynamic local name lookup;
ordinary declared modules and standalone cursor do not acquire `let`.

The selected profile makes bounded byte-wise guest parsing possible; it does
not introduce exact BigInt/Rat values, the calculator's seven numeric budgets,
or any CALC acceptance status. The compiled signed decimal-fold and lexical
parameter precursors under `verification/calculator-precursor/` still use
**wrapping** I64 arithmetic and are not exact-number implementations.
The signed lexical countdown there executes 600 checked self calls in each
optimization mode with 12,661 guest steps and a 40-byte peak continuation
queue. This scalar tail witness is not an exact numeric width or general
nominal-match tail-recursion claim.

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

For declared modules, `import m@2 as m` may replace an earlier `m@1` alias
only after replaying each current outside definition that used `m.*`, plus
its current transitive dependents, from the exact retained source. The
replacement must preserve ordered stack types and not increase effects.
The alias and all rebuilt bodies publish together; a previously captured
program, version-qualified calls, and another alias to `m@1` still use
their original owners. Missing provenance, incompatible exports, or a
current body reaching a shadowed old alias owner rejects the import without
changing the previous namespace. The opt-off/on guest scenarios under
`verification/calculator-precursor/` exercise old captured `21`, another
alias's `21`, new transitive `24`, and incompatible-import recovery.
The finite language guest observations above belong to isolated compiled
donor `209c478`; this combined Worker/C integration has not replayed them
and does not inherit a source-bound acceptance receipt.
Separately, the source-equivalent combined successor at `2621b07` ran the
existing LIVE REPL 62/62 suite and a direct first-guest zero-code bridge
retirement, saved old Program `21`, new `22`, and retired old-code reimport
`21`. The signed framed guest completed 65 same-session decimal folds at
306 cells/14,688 bytes each under the original caps, then a separate
externally pinned Program refused the 14th fold at the 4,096-cell limit;
foreign and premature observation releases refused. The selected Worker
guest tests passed 22/22, its child tests 4/4, and a direct child committed
256 effects before denying request 257 without an I64 result; the separate
fifth ledger request also refused. These scoped raw reports are under
`/home/brittonr/.cache/noble-live-epoch-combined-guest-tmp-20261008/combined-{live,epoch,worker}-direct-raw.json`
for binary SHA-256
`ac2eaeecfddf673f56e636ccb4e032e4ee2cb41050ba4ee942cd4ee02482ca38`.
They predate fixture-only test-budget corrections and do not establish an
exact-number current28 gate, complete Worker package suite, selected vendor,
canonical acceptance, or a full Cairn closeout.
The later combined successor closes a Worker package authority escape:
only the checked selected named root or a bounded composition containing it
may be packaged or installed as a WIT Program. A checked dependency alone,
including a composition of dependencies, cannot inherit the selected root's
export grant; a foreign or retired source instance remains invalid. The
source-bound scoped rerun passed all eight Worker Program package tests,
22 installed-guest tests, and four host-child tests. A fresh direct Worker
guest denied request 257 after 256 committed effects without issuing an I64
value and refused the fifth ledger byte-quota request. Its raw report is
`/home/brittonr/.cache/noble-live-epoch-combined-guest-tmp-20261008/root-authority-f324/combined-worker-direct-raw.json`
for binary SHA-256
`604a37322cce7ab8a880134cb0f73f2e1fc85ca89942172e49b91d1a4af77aef`.
These patched-source checks do not rerun the prior LIVE or signed-epoch
scenarios, certify Worker Send, or promote canonical acceptance.

**Declared source retention is a separate opt-in bound**, not a larger input
limit. Without `--retained-source-bytes`, the combined original module-source
and outside-definition-history ceiling equals `--source-bytes` (65,536 by
default); each outside definition also uses a four-byte history header.
With `--declared-modules` on `compile` or `session`, an explicit
`--retained-source-bytes N` selects `1..=262144` aggregate bytes while every
individual source input remains limited by `--source-bytes` (at most 65,536).
For example, the pure combined session can explicitly select 262,144 with
`noble session --framed --declared-modules --text-byte-cursor --retained-source-bytes 262144`.
An explicit zero or larger value, or this
flag without declared modules, is refused. An implicit `--source-bytes 0`
remains a valid closed quota: nonempty source exhausts without a panic.
Increasing aggregate capacity does not evict prior module sources or replace
original bytes with hashes; alias replay must use the exact retained source.
In framed declared sessions, a source frame above the per-input limit but at
most 262,144 bytes is drained in bounded chunks and refused without checking
or executing it; a following valid frame can reuse the unchanged session.
Frames above that safe drain ceiling, malformed headers and truncated bodies
terminate the stream instead. Ordinary sessions keep their prior framing rules.
Proof workflows select their own 65,536-byte retention policy and do not
silently inherit a larger session setting; a larger proof path requires
separate user authorization.
The active [retained-source change](.cairn/changes/declared-retained-source-budget/proposal.md)
defines DX-14 as **absent/not-run**, not a compiled guest or proof acceptance.
Guest runtime fuel/memory and ordinary sessions retain their own policies.

A static rejection preserves the previous stack and namespace. A runtime trap
or exhausted execution quota ends the session, retaining any host requests
already observed. Sessions use a bounded arena that does not reclaim individual
cells. Use `noble session --framed` for framed multiline input and
`--emit NEW_DIR` to retain reports and emitted artifacts. Run `noble --help`
for available CLI flags.

### Experimental checked live slots (opt-in; acceptance not established)

`noble live slot --engine v8 --authority ABS_PATH` selects a separate resident
Node/V8 worker and reads one bounded host-operator JSON command per line from
an anonymous OS pipe. The launcher must retain the pipe's write descriptor
exclusively: passing it to a guest or an untrusted same-UID helper delegates
operator authority. Pipe ownership/mode does **not** authenticate individual
writers. The authority file must be an absolute path under trusted directories;
symlinks and writable or non-owner-controlled path components are refused.
Its `sources` list pairs each source ID with the SHA-256 of its exact UTF-8
bytes. It separately declares the owner, retained-version quota, available
effects, scoped operation grants, resource catalog, and slot interfaces.
Candidate source text cannot grant itself a slot, resource, effect, or proof.

Host commands include `define` for an allowlisted named declaration, `install`
for a checked expression, `candidate` to stage a checked Program while idle,
and `bootstrap` for one physical epoch-1 genesis of exactly two or three
independently authorized selected slots, for example
`{"operation":"bootstrap","expected_epoch":"0","members":[{"slot":"B","id":"B1"},{"slot":"A","id":"A1"}]}`.
Every member must match its host-selected ordered interface and effect
ceiling; a refused member, duplicate slot, exhausted capacity, or previous
registry/root state publishes nothing. Successful genesis reports epoch 1 and
incarnation/generation 1 for every slot. Later `publish`, `rollback`, and
`delete` retain their ordinary single-slot global epoch-CAS behavior; genesis
does not supply a proof or independent guest authority. `invoke` uses typed inputs
and borrowed reference bindings; an actual returned Program receives an opaque
saved owner token, which `release-program` retires at its last owner. `discard`
removes an authorized never-published install. `reflect` and `trace` observe
the host ledger. Exact trace retention is bounded by serialized UTF-8 bytes
(1 MiB per root and 2 MiB across a session, including replay labels), not just
request count. When retention fills, `invoke` returns `trace-capacity-refused`
before starting a guest; a root that reaches its per-root limit fails closed
before another protected effect and reports its retained exact request trace.
No prior trace rows are silently discarded. Start a fresh session to continue
after the global trace capacity is reached. `hold-checkpoint` selects a
protected import occurrence in the next checked root; after
`checkpoint-entered`, a permitted publication or
revocation receives a committed `control_id` receipt before
`resume-checkpoint`. The held root keeps its pinned epoch, while the next root
observes the committed change. An in-flight control with no remaining
protected checkpoint receives an `operator-control` `refused` receipt with
`when: "too-late"` after its pending router request is cancelled; it does
not change the epoch or leak into the next root, and the session stays usable.
An unaccounted-for delivery still fails closed. Interrupted checkpoint reads
retry only the unconsumed packet within the same eight-second deadline; other transport
failures poison the root. Frozen `record`/`replay` and `release-replay`
use retained roots rather than replacing live effect policy. Every reply is
JSON; inspect each `outcome`, not only the process exit status. Source-bound
named selection requires `install` with `selected_name` after the corresponding
`define`. A narrow checked anonymous returned target can receive experimental
local D/P in its saved-owner receipt, but has no independently installed
captured-target Wasm candidate, proof, or source-bound frozen replay. Recorded
protected responses are replayed as an
exact ordered script without executing another real host effect; they do not
grant permission for a new live effect. An optional `expected_identity` on
`replay` is compared with the host-retained frozen receipt before starting a
guest; it cannot replace that retained authority. Proof-required publication
currently fails closed: there is no independently pinned Lean receipt for the
exact selected, installed target. Neither this opt-in CLI nor its unit tests
constitute LSLOT-01..09 canonical acceptance.

A directly returned I64 `quote` is also a saved Program: its value is read from
the actual boxed I64 capture, not from the Program's latent-effect payload.
For example, a checked `def q [ quote ]` invoked with `5` returns an owner
whose captured program can be passed to another checked caller with `20`;
`swap [ run ] dip +` returns `25`. Release that owner before discarding unused
code. For a selected named body with bounded, unambiguous I64
quote/compose lineage, `verified_origin` reports its caller, artifact, and
output position after
the host matches the complete source operation map to decoded Wasm functions,
passive element entries, the exported installer call to its initializer and
the native core plus checked code table initialization, rejecting alternate
table writers. The worker checks the actual epoch-qualified Program graph
and ordered captured operand before retaining the owner. The host assembles
once with the pinned wasm-tools binary; the worker instantiates those exact
bytes after a SHA-256 cross-check. For
example, `def builder [ quote [ + ] compose ]` captures root I64 `2` and
returns a saved `Program<I64,I64,pure>` that a checked `run` caller can execute
with `1` to obtain `3`; `def fixed [ 2 quote [ + ] compose ]` retains the same
fixed capture even when the caller root differs. For the exact pure returned
`quote(I64) [ + ] compose` graph with one checked I64 capture and a checked
static `+` child, the trusted worker checks the selected VM capture and inert
quote atom against the source-to-compiled-site lineage. After retaining
the owner and clearing the root stack, the trusted resident worker resolves
that owner back to its saved backend handle and re-reads the complete bounded
Program, capture and inert recipe graph without guest execution or host
effects. Rust compares the owner/handle correspondence and every returned
node, edge, signature and effect against checked source/compiled-site
metadata before deriving `target_identity` in the saved-owner receipt. This
separate Rust check still trusts the resident worker to report the VM graph
and owner mapping faithfully; it is not malicious-worker attestation or a
proof of arbitrary replacement Wasm helpers. Varying root input 2→3 keeps
the anonymous template D and changes P; source literal 2→3 and independently
computed constant 2→3 each change D. These local, versioned experimental IDs
bind the target's checked interface, empty effects, builtin addition
dependency and exact I64 capture; `artifact_sha256` is null, not the builder
artifact. The local `v2-blake3` IDs use BLAKE3 1.8.7 with separate template
and captured-value domains, over this strict checked I64 projection only;
they are not the canonical portable P-ID-04 encoding. The eight-lock offline
vendor closure contains 237 archives, five beyond the formerly reviewed 232:
`unicode-width` 0.2.2 and four BLAKE3-related crates. A hash-checked candidate
vendor was realized locally, with its measured path and NAR bound by M5/M6
in this WIP selection, without source-bound assurance. That is not a selected
Nix, source-inventory or native-assurance gate pass. Source locations, builder
D, VM addresses and code-owner handles are not part of D. The selected offline
vendor helper now computes the entire pinned SHA-256 NAR with the selected
Node process instead of starting a Nix client: it includes sorted directory
entries (raw byte order), owner-executable regular-file markers (not group
or other execute bits) and file bytes. It enumerates directories with a
bounded iterator before sorting, limits traversal to 50,000 nodes, rejects
depth 64 (root at zero), and caps individual files, cumulative contents and
encoded NAR work.
It rejects symlinks, requires root-owned nonwritable entries and refuses if
the caller can write the store ancestry. This assumes the root-owned store
is not rewritten by privileged actors; a shared-daemon GC may still remove
an unrooted output, which fails closed rather than providing an alternate
registry. This helper check does not establish that a GC root protects the
output or complete any gate. Other M5/M6 closure checks still invoke Nix;
this helper does not make the full selected collection daemon-free.
General logical source-binding aliases, independently named equal bindings,
recursive lexical owners, other returned interfaces and effectful graph shapes have no
identity-bearing witness here; they remain without target D/P rather than
using bit equality or heap topology.
The same-interface pure `quote [ - ] compose` result remains an executable
checked owner but cannot borrow the add-only anonymous D/P.
Ambiguous or unsupported dynamic Programs remain usable opaque owners without
checked origin. A
verified selected origin proves only this supported saved owner's bounded
source/Wasm/graph correspondence from compiler-emitted Wasm; the custom Wasm
name section alone does not authenticate arbitrary replacement runtime helper
bodies or the installer's complete control flow. The bounded local D/P receipt
is **not** symbolic P2/D2, captured P/D/C/Q/A/Wasm Lean proof, effect
permission, publication permission, or frozen replay authorization. Saved
dynamic targets still refuse publication even into a non-proof slot until the
exact target's installed Wasm correspondence exists; proof-required slots
also refuse without independently checked exact-target evidence. Canonical
LSLOT-05 and LSLOT-08 remain blocked.

The selected dynamic owner also has a narrower retention consequence: with
`def builder [ quote [ 1 + ] compose ]`, the same builder module's independently
admitted pinned static `[ 1 + ]` child can be published to a non-proof pure slot.
The saved composite captures I64 `2`, runs as a checked
`Program<empty,I64,pure>` to return `3`, and
transitively pins that exact published child across a held generic root. With
a one-version quota, a new candidate cannot publish while the root is held
or while only the composite owner survives; release the owner, freshly admit
the replacement, and the slot advances with the old code span retired. This
is a D-neutral transitive-code-owner control, **not** publication of the
dynamic Program, the canonical saved exact v1, proof admission, or captured
target frozen replay.

Run the diagnostic `verification/live-slot-current-source/gate.mjs` with the
Node executable pinned in `crates/noble-cli/src/core/runtime/config.json`;
pass an absolute externally built `/tmp/.../debug/noble` and a **new** direct
`/tmp` output directory as its two arguments. Start that pinned Node from a
clean environment, not from an inherited `NODE_OPTIONS`, `BASH_ENV`,
`LD_PRELOAD`, `NODE_PATH`, `DYLD_*`, or loader-injected process:

```sh
env -i HOME="$HOME" TMPDIR=/tmp PATH=/run/current-system/sw/bin:/usr/bin:/bin \
  /nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node \
  verification/live-slot-current-source/gate.mjs \
  /tmp/SELECTED_TARGET/debug/noble /tmp/NEW_RECEIPT_DIRECTORY
```

Use the current path in runtime `config.json` if the pinned selection changes;
the final receipt directory must not already exist. The gate rejects visible
dangerous startup variables and records its restricted child environment,
but cannot attest to a preload that ran before gate JavaScript and concealed
itself. It runs a selected offline Cargo build, records before/after binary
hashes (unchanged bytes do **not** prove fresh recompilation), and freezes
selected source-tree paths and bytes, exact used test scripts/helpers, fixture,
authority inputs and documentation. Its `receipt.json` records Git
HEAD/selected dirty status as diagnostics and points to retained real command
streams and raw responses.
LSLOT-02 independently exercises a held A→B→B root and a held A→B→C root:
each independently authorized two-/three-slot genesis publishes one physical
epoch-1 map. A Rust-preflight invalid member is refused before the worker;
an independent quota-1 two-member session passes Rust preflight but the
worker refuses the assembled map with `retention-budget-refused`: both slots
remain absent at epoch 0 without guest requests or effects. A committed
leaf publication at epoch 2 leaves the old root pinned while a new root selects
the new target. Checked nested-site reflection reports the exact effect set
`E ∪ {live.dispatch}` without duplicating `live.dispatch` when E already
contains it; exact slot effect ceilings and current per-dispatch host policy
remain distinct checks. Both selected real-CLI LSLOT-02 variants match their
canonical fixture inputs/expected observations in this external diagnostic;
the canonical case records still remain absent/not-run/open/unassessed, with no
evidence or proof promotion. LSLOT-04 authorized
rollback and LSLOT-07 publication denial retain runnable controls but remain
blocked canonically without their respective exact-evidence/proof premises. A
separate LSLOT-05 control observes the real host's generic missing-proof
refusal, not selective evidence applicability; a typed checked legacy-compiler
control does not substitute for LSLOT-09's typed CLI caller. A nonzero
`blocked` result is also intentional while exact LSLOT-05 selected-target
proof, captured LSLOT-08 replay, and other variants remain open; this external
receipt must not be copied into canonical case evidence.

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

The exact `:generation` line inspects the current selected live namespace
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

On Linux, `noble live watch --source /absolute/math.noble --engine v8`
also accepts a **cooperative** complete save: write and `fsync` its
same-directory `math.noble.tmp` sibling, then atomically rename it onto
`math.noble`. The host cannot observe the editor's `fsync`: the paired
inotify events only nominate a save. It pins the selected directory and file,
requires a bounded regular single-link snapshot, writes those exact bytes to
a separate host-owned private file, syncs that file and its directory, and
checks both the private bytes and the unchanged selected source again before
publication. It never replaces the editor's selected path. A later complete
save supersedes an in-flight candidate rather than receiving a false ACK.
On an automatic watch ACK, `watch_save_protocol` describes the observed
paired rename and host-fsynced snapshot, **not** producer fsync;
`watch_sequence`, `source_sha256`, and `source_freshness` identify the
published version and bounded ACK-time path observation. Explicit `:reload`
and startup file selection have no automatic-save protocol attestation.
Unpaired renames, in-place writes, symlinks, hardlinks, and a changed
candidate refuse; an outside writable mapping or alias can still mutate a
file without a selected-directory event. This cooperative watch is *not*
proof of immutable file bytes after ACK or full H-LIVE-01/LIVE-05
acceptance. Explicit `live repl --source` and `:reload` remain available.

For a separate, private watched-Core LIVE-05 diagnostic, build the CLI with
`cargo build -p noble-cli --bin noble --features live05-harness-effects`.
Only that feature-built `live watch --source` host can accept a one-shot
`NOBLE_LIVE05_HARNESS_GRANT` naming the selected watched source SHA-256, its
publication generation, and the SHA-256 of the exact root submission
`"A" test.emit test.abort`, in that order separated by colons. It does not
apply to declared modules, self-edit, named definitions, saved Programs,
ordinary live sessions or `run`. The worker separately checks the actual
typed Wasm import pair and the active guest/source identity on each callback;
the host requires a terminal six-field receipt. This is an opt-in diagnostic,
not a production grant or canonical LIVE-05 acceptance.

On an isolated private target, the offline locked feature-built watched
real-child tests passed for the issued grant and for default, stale, and
foreign-grant refusal. A separately driven CLI JSONL smoke observed watched
source generations 1 then 2; the selected watched source SHA-256 was
`4e49ee40f4b4a51eff90e9a7820d2bdc1d9b12f8b8c558adeff5ba741075e9ad`,
the one-shot root submission SHA-256 was
`b77aa84381c1c1fc9c59111eda2d07890bc2d7ac7db5661f76a148896aaee832`,
and the checked Wasm SHA-256 was
`c45f527885b972bd5229cad2baaa82430dd930449fe4aaea7f499878b3fa6098`.
The Wasm imported `test_emit` and `test_abort`; the report's six-field receipt
contained `selected_source_sha256`, `source_generation`, `submission_sha256`,
`wasm_sha256`, `allowed_effects` (`["test.emit", "test.abort"]`), and
`consumed` (`true`). Its exact `request_trace` was `test.emit:A` then
`test.abort`, with two guest and two host requests, zero denials, two protected
operations, status 6, and `terminated-after-runtime-failure`.
`CoreEngine.checkTools()` validates and hashes selected Node 24.13.0 / V8
13.6.233.17-node.37 before returning early for in-process Core live, which
assembles checked Wasm bytes without external `wasm-tools` or `wasm-opt`.
Artifact and non-live modes still enforce their selected external tools.
Scoped source, Wasm lowering, and byte-policy unit tests passed. A separately
built `--no-default-features` CLI denied an injected fixture grant as
`unbound-word` with zero guest requests, then executed ordinary live source.
The separately combined watch and harness source rebinds the changed
selection-policy file SHA-256 entries in authored Nickel and exported JSON.
Extraction `configuration.features` remains `[]`, while the reviewed
workspace feature-name set is exactly `["live-test-barrier",
"live05-harness-effects"]`; unreviewed extra names are refused. This is a
source-bound policy update, not a selected Nix execution or LIVE-05
acceptance: selected Octet standards, reviewed vendor and NAR inputs remain
unavailable here.

Scoped real-child tests also stop inside an actual old Wasm guest frame before
queuing a reload, and after candidate A's checked V8 Wasm compilation but
before its staging reply while a second fsynced rename selects B. They observe
the old result before the reload ACK, only B acknowledged as current, and
zero guest requests for an unsupported partial in-place write. A separate
external `MAP_SHARED` smoke similarly refuses observed source drift before
guest execution. Two further scoped real-child watch controls ran. One rejects
a complete save containing canonical missing-import bytes
`import missing@1 as m` at the Core lexer, before any declared-module lookup,
without changing the old generation, typed stack or definition. The other
pauses a checked `20 addone` submission before commit while an fsynced paired
rename selects its replacement.
The accepted old submission returns `21` before the new watch ACK, whose
successor then returns `22`. While the refused bytes remain selected, the
last-ACKed-byte gate refuses new execution; restoring those exact bytes permits
the old definition to run, but does not ACK an in-place write as a new save.
These controls do not promote a canonical LIVE-05 case. The private scoped
receipt at
`$HOME/.cache/noble-live05-host-seal-20261009-scoped-receipt.json` binds the
resulting commit and bounded logs. This path is only a pointer; its SHA-256
is reported separately, not embedded in the commit it binds. The prior
combined worker reported 55 passing default live
tests and 57 feature-enabled tests. Separately, isolated LIVE06 source
`a35194da1776747e8478414d04a106162aca7733` passed 55 default and 59
feature-enabled live tests on feature CLI SHA-256
`f98b6b5eee1fa9fcea01c83d159a26c89dcd18a62948fe69f2bdeb2105ec8406`,
including two actual A-active host-policy guest controls. With the same
checked A source and alias, an allowed request had
one guest request and one protected operation; a feature-only host revoke
changed the resident A binding and slot from allowed/active to denied/inactive
before the old Wasm request, which trapped with one guest request and zero
protected operations. A separately loaded B did not substitute its adapter
for the stale A request. The isolated scoped receipt is
`$HOME/.cache/noble-mlive-private-20261007/tmp/live06-a35194d-scoped-receipt.json`;
individual child stdout/stderr were not preserved separately from the Rust
harness summaries. Those feature-only hardcoded controls are removed by the
subsequent production host-only `:revoke-effect` cutover. On its isolated
source, the default `live_repl` suite passed 66/66 and the feature suite
passed 69/69 against actual Node 24/V8 child guests. In a separate direct
default-CLI session, an old A `Program` remained typed on the stack after B
loaded; revoking the exact checked A source/adapter/effect changed its
binding and active slot from allowed to denied with zero guest requests. B
still executed through `emit-b` (one request, one protected operation);
invoking the saved A trapped through `emit-a` (one request, zero protected
operations), poisoned the session and exited 2. The executed default CLI
SHA-256 was
`bb738f38f887c9f08cf40c727af9b0bdb47198e37a85c1f940031ae614cf6676`;
the raw direct-child JSON reports and empty stderr are retained under
`$HOME/.cache/noble-live06-policy-cutover-20261008-yD7w-tmp/direct-live06-lIcbkl/`.
This is scoped evidence, not a canonical LIVE gate: LIVE-02/LIVE-05
variants, complete LIVE-06 acceptance, scenario receipts and source-bound
proof remain open, with canonical cases absent/not-run.

The separate successor that combines the tested returned-identity source
`1d5ac07674389c7a344d187762f991ed18ca1db9` with LIVE06 donor
`8483c17b6ac2d3c6e0d3e876216fbe92c09e09f4` is isolated at
`ac3f60c9820acd0103a017ca8aad8fe17c3a6041`. Its separately authorized,
offline scoped CLI/V8 run produced binary SHA-256
`4024a0c6a1cb5b132f15be7beccd0756a899d2aba017e55bf46a490c8804a63b`.
A direct same-process Node 24.13.0 child revoked exact
`ledger@1 emit-a test.emit`: the saved old A request was denied with one
guest/host request and **zero** protected operations, while B still dispatched
through `emit-b` with one request/one protected operation. The raw direct
receipt is `/tmp/noble-ac3f-combined-2dFTCMG6/live06-exact-host-revoke.receipt.json`
(SHA-256 `ea233d815daf2c4f54c8aa06eb06be4d5641c6bc8c76cf0d64a9a81606718094`).
Separate selected real-guest runs exercised returned-target D/P, physical
aliasing, distinct-source old/new publication, replay and stale global-epoch
CAS; targeted Worker package-root and Signed source-epoch checks also passed.
The earlier donor binaries and suites above remain historical, not results
of a full combined suite. This successor has **no current document-validation
receipt**: `specs/VALIDATION.json` is donor-only, with stale input hashes.
These checks neither seal the branch nor establish canonical LIVE/LSLOT,
Octet, or Lean acceptance or proof.

For a separate **opt-in source-change execution gate**, start the same REPL
with `--verify-source-before-execute`. Before each source submission, after
frontend preparation, and after Wasm candidate preparation but before guest
execution, the host opens a bounded, directory-pinned snapshot of the
selected path and compares its bytes' SHA-256 with the last explicitly
acknowledged file bytes. A changed or unverifiable path produces
`source-change-refused` with the prior stack and namespace, both hashes when
observable, and zero guest requests; it does not commit or run that
submission. To accept changed bytes after a complete save,
explicitly `:reload` and wait for its ACK. A path changed through atomic
rename, an external hardlink, or a writable shared mapping is compared by
bytes at the next source submission. Identical bytes remain executable even
after an inode replacement; retained old Programs still execute after a checked
reload. `:generation` remains an inert inspection without file access. This
optional gate is **not itself an idle filesystem watcher**, does not detect
a change between its finite snapshot and the subsequent guest invocation,
and makes no claim that an external writer's in-place bytes are a complete
save. The default REPL continues to allow old-namespace execution after a
refused `:reload`.
The `MAP_SHARED` adversarial control is a separate real-child smoke, not an
ambient compiler dependency of `cargo test`: `tools/live-mmap-smoke.mjs`
requires absolute `--noble` and `--cc` paths, an explicitly selected Nix-store
compiler, and its exact `--cc-version` first line and `--cc-target` triple.
It reports a scoped observation, not a canonical LIVE case receipt.

For the tested acyclic Core `addone→twice→four` chain, selected-file reload
rebuilds transitive dependents in dependency order. New top-level calls see the
rebuilt generation after one source-generation ACK; saved old Programs still
use their original identities and dependencies. Scoped indirect self-rebind
refusal was exercised in a live child, but this guarded in-process Wasm path
does not establish general cycle safety or source-bound proof acceptance.

The separate selected declared-module lane uses:

```sh
noble live repl --source /absolute/module.noble --engine v8 \
  --declared-modules --bindings /absolute/bindings.txt \
  --verify-source-before-execute
```

It keeps checked module/version/import snapshots and a saved old Program through a selected
module replacement; a fresh import sees the new version. Rebinding an alias
rebuilds compatible transitive dependents without rewriting old Programs;
incompatible replacements refuse that import. The old bound adapter identity
stays fixed while invocation checks the **current** host authorization. At
the declared live REPL's **host stdin**, an operator can revoke one
initially allowed manifest binding after its immutable module root is checked:

```text
:revoke-effect ledger@1 emit-a test.emit
```

The three arguments are the exact `MODULE@VERSION`, adapter identity and
effect selected in the host bindings manifest. The report binds the checked
module identity and source SHA-256, retains generation and typed stack, and
records the formerly allowed binding and active slot becoming denied/inactive.
Malformed, foreign, not-yet-checked and repeat selections refuse without a
guest request. Revocation is monotone for the session: subsequent imports
cannot reauthorize that binding; only a new host-selected session can have
its initially allowed policy. Guest code cannot invoke or regrant this host
command. The test-only safepoint feature is not required.
Candidate source, import version, source-history and proof requirements are
checked on a private frontend snapshot before a candidate is published; a
refusal preserves the old session. This is scoped runtime evidence, not
canonical LIVE-05/LIVE-06 proof or a claim that all declared/effectful
transitions are accepted.

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
An optional host-selected `--max-guest-steps N` (1–100000) tightens the
per-invocation V8 guest work bound; it does not raise the existing maximum
or apply to inert source staging. A queued self-edit candidate held by the
backend remains inert until normal guest return. If later guest work traps or
exhausts, the worker cancels it, releases only its exact retained code root,
and reports the backend's before/after queued-root and owner-byte counts.
Terminal denial still preserves the completed host-request prefix and does
not release an unrelated saved Program. This is a scoped terminal-cancellation
control, not canonical LIVE-10 acceptance.

This is a **guarded partial** live profile, not complete source-bound
LIVE-01–10 acceptance, interpreter execution, general guest mutation, or
PushGP evolution. Plain Core live now uses its own **16-page code-owner
API**, not the slot profile's separate 20-page ABI. The selected CLI registers
each installed code span, retains externally observed raw Program roots,
releases superseded module roots only at idle, and asks the worker for an
exact owner inventory. Only zero-owned spans receive stale-handle
invalidation, exact physical table clearance and
`Compiler::retire_code` reuse. Normal execution reports
`installed_code_span`; selected reload reports `retired_code_spans` with the
worker's `code_owners: 0` and `table_cleared: true` receipts. Ten real
selected reloads and a following top-level call exercised first-fit reuse
after clearance; a saved old Program remains callable instead of being
retired. Unknown owners refuse retirement. Code-table reuse does not reclaim
Core's append-only 4096-cell heap or text allocation. The checked compiler's
one 2-million-work submission cap is unchanged: exact contract checking
charges the selected scheme, while the full environment remains charged for
its own scan and each independently admitted kernel body.
The integrated source `8b1504b` passed all 55 scoped live CLI tests, including
terminal guest trap and 300-step quota after a real queued proposal. The
selected compiled CLI (SHA-256
`2de6d8ebb2f99c48e370866a9e13892c96fbb8c0af0ec568a7ce4dbb505b4e39`)
also passed five finite actual-guest traces: dependent rebuild with an old
Program, old-adapter denial after alias rebind, trap and quota queued-root
release (both one to zero), and one paired watch save (sequence zero to one).
The private source manifest SHA-256 is
`33317914fd0f057b9eb45047ac21f052833334ad25910c30fce2154e7c40fd5a`;
the scoped guest receipt SHA-256 is
`9376b0d13aa1c0cd2f852925fc9af528821428ee36cb6cdc6044dc6bc7eab5d5`.
LIVE-02 active in-flight reload, complete LIVE-05 forced-overlap coverage,
canonical LIVE-06 host revocation, general cancellation and canonical
LIVE-01–10 remain **open**.

`--engine interpreter` refuses rather than falling back to V8.
Ordinary `run` and `session` are unchanged. The canonical LIVE-01–10 cases
remain `absent`/`not-run`, proof open, with no accepted source-bound assurance;
see the [active change](.cairn/changes/live-wasm-reload/tasks.md) and
[status ledger](specs/STATUS.json).

For a repeatable scoped selected-CLI trace, run the pinned Node 24 binary
against an already-built absolute `noble` binary:

```sh
/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node \
  tools/live-selected-receipts.mjs --noble /absolute/path/to/noble
```

The JSON lines contain the selected binary SHA-256, initial source SHA-256,
raw guest reports, exact process exits, and explicit `unproven` controls.
They are finite real-child observations, **not** canonical LIVE statuses or
proof receipts.

## What works today

These features have separate implementation and acceptance scopes. Enabling one
does not make every other feature available in that runtime.

| Area | Implemented scope |
|---|---|
| [Core language and Wasm sessions](specs/ROADMAP.md#m4-implementation-and-retained-acceptance) | Wrapping `I64` arithmetic; Bool, Text, Unit, Pair, Sum, and List values; stack operations; checked branches; quotation, capture, composition, execution, and reflection; nonrecursive definitions. |
| [Guarded live REPL](#guarded-live-repl-opt-in-partial) | One resident Node/V8 arena supports explicit checked reload, cooperative Linux paired-rename watch, optional before-execution selected-byte verification, old Program preservation, Core zero-owner table-span retirement/reuse, and separately selected declared-module snapshots with host-only exact binding revocation. Scoped source/guest regressions do not establish complete LIVE-01–10 acceptance or source-bound proof. |
| [Declared modules](specs/ROADMAP.md#dxm1-selected-declared-modules-bounded-acceptance-complete) | Opt-in opaque types, two-constructor variants, versioned session-local modules, imports and exports, and an explicit typed `test.emit` adapter. Available through `compile` and framed `session`. |
| [Local typed Worker Send](verification/worker-send/test.mjs) | Separate opt-in source-checked pure worker transition and host-selected WIT Send grant with local Unix datagram/cancellation controls; no canonical WORKER or MW1/MW2 acceptance. |
| Worker Program test host | A separate source-bound `own<program>` I64 WIT guest can request a finite synthetic `test.emit` ledger only after the built-in host explicitly issues its checked installed cell. `noble worker program test-host emit|emit-wrong|emit-ledger-denied|emit-request-denied|emit-panic|emit-revoked|wit-export|core-export` accepts only exact built-in fixture source bytes on bounded stdin: the trusted fixture owner, binding, destination and epoch are selected before reading candidate input; arbitrary checked source is denied without issuing. It accepts no grant or destination document, and denies WIT and Core export independently. The donor's unchanged ledger-byte control observed four real appends then denied the fifth. Its separate host-selected batch used one checked source cell, Store, task and issuer, admitting and consuming a fresh linear owner per iteration; scoped real guest runs observed 512 pure steps from 41 to 553 with zero live owners, and 256 committed 21-byte appends (5,376 bytes) followed by a denied 257th request with no delivered final I64. Between-step cancellation and revocation acknowledged after the first committed append prevented a second request or result. These are donor observations pending execution against this combined successor; issued-cell revocation and caught after-append panic preserve redacted effect counts, and canonical WORKER-01–12 acceptance remains open. |
| [Contracts and proof checking](specs/PROGRAM-CONTRACTS.md#12-verification-tooling) | A pure contract language, Lean obligations, proof/refutation checking, and `verify` / `explain-proof`. |
| [Evidence companions](specs/PROGRAM-CONTRACTS.md#83-mc2-companion-implementation-and-evidence-boundaries) | Live certified programs, evidence composition, guarded invocation, and proof-required build admission for the selected core scope. |
| [Synchronous components](specs/WIT-WASI.md) | Selected WIT bindings, component compilation, host resources, ownership transfer, and authorization checks. |
| [Native async components](specs/WIT-WASI.md#selected-native-async-boundary-and-compatibility) | A separate bounded component profile with native async execution and cancellation controls. General guest async syntax remains open. |
| [Local services](specs/ROADMAP.md#m7-selected-local-synchronous-service-bounded-acceptance-complete) and [choreography](specs/ROADMAP.md#m8-selected-choreography-projection-bounded-acceptance-complete) | A bounded local synchronous publisher/subscriber service and finite one- or two-round protocols over that service. |

Ordinary `run` and `session` use the **resource-free Core-Bootstrap** profile.
Their host words are `test.emit` and `test.abort`. Resources and component
features use separate entry points. Signed self recursion and lexical locals
exist only in the separate opt-in declared-module profiles; general recursion,
general libraries and capability modules, portable package identities, and
general distributed services remain open.

The [exact calculator](.cairn/specs/calculator/spec.md) is a planned reference
application. Its [numeric error schema prerequisite](verification/calculator-errors/README.md)
has a finite 18-case compiled-guest observation on a rebuilt binary, explicitly
**not source-bound**; the opt-in byte cursor does not establish MA1 or CALC
acceptance. The [typed-worker design](specs/WORKER-CONFORMANCE.md) remains a
portable-runtime contract, not completed acceptance. The opt-in `noble worker
typed` executes a source-checked, closed pure transition; `noble worker shell`
and [the independent WIT peer](verification/worker-send/test.mjs) exercise
exact host-selected Send grants, native Unix datagram delivery and future
cancellation with retained task/resource pins. These are local-only controls:
there is no external destination, rollback claim, owner-selected portable
G-03/G-05 package/export, or canonical WORKER-01–12 acceptance. Neither the
calculator nor Worker is a completed application or agent runtime.
This source includes `58b9d6c` host-fault cleanup and `9c687ed` source
binding/fault assertions. Nine fresh local WIT/UnixDatagram cases passed
against the integrated CLI above and a fresh offline-locked peer (SHA-256
`49b86d5362a5d442c6011a869837b0623502201f9c0aaeee29371fc5bc88b20c`);
the component SHA-256 was
`798bf9b3a0114aebd9954f62a5352a284df2cb24d94d760e7fd93edc443dc826`.
Both injected receive faults saw one datagram, retired the task and resource,
and left zero outstanding/native pins, reserved bytes and result deliveries.
The external peer used its own pinned Cargo.lock and offline cached registry,
not Noble's selected vendor or a source-bound Nix gate. Portable G-03/G-05,
broader host-fault proof and canonical WORKER-01–12 remain open.

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
