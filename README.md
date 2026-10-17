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
verified by this check; CALC-EVIDENCE-01/02 remain open,
CALC-01 and related scenario statuses remain unchanged, and the separate
VC-NUM-01 contract obligation does not inherit a calculator proof.

[`proofs/foundation`](proofs/foundation/README.md) (issue #15) proves that
this oracle agrees with the MC1 `BitVec 64` model, which gives universal
instruction, sequence and quote/compose/run statements over that model.
Its seeded sibling `verification/foundation/differential.mjs` runs
generated programs with opt off and on, and Lean kernel-checks every
expected value. Both remain model theorems plus finite tests. The README
there lists every unproved edge to the executed Wasm.

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
cells (only the separate Calculator-v1 profile compacts, at submission
boundaries). Use `noble session --framed` for framed multiline input and
`--emit NEW_DIR` to retain reports and emitted artifacts. Run `noble --help`
for available CLI flags.

### Experimental calculator (opt-in; acceptance not established)

`noble calculator` reads stdin, one submission per line (`--framed` selects
byte-count framing instead). Each line is an infix expression over exact
rationals with `+ - * / ^`, unary minus, parentheses and exact decimals
(`0.1 + 0.2` prints `3/10`), a definition `let NAME = EXPR` or
`fn NAME(P, ...)=EXPR`, or an expression calling a defined function
(`fn f(y)=y*2` then `f(3)` prints `6`). Results print on stdout as `n/d`, or
`n` when the denominator is 1. `--finite-decimal` prints terminating values as
decimals (`3/10` prints `0.3`, `1/3` stays `1/3`). Errors print on stderr as
`Category [start,end)` byte spans, and the exit status is 1 if any line
failed. Guest budgets are set with `--input-bytes`, `--numeric-digits`,
`--intermediate-bits`, `--syntax-depth`, `--call-depth`, `--evaluation-work`
and `--output-size`. Parsing, name resolution, exact arithmetic and display
run in checked Noble guest modules
(`crates/noble-contracts/src/fixtures/calculator-*.noble`); the host shell
orchestrates the checked guest phases and supplies bounded bytes and budgets.

It is slow: each line runs about 300–450 separately checked guest modules.
Measured on a release build of `7bae596` (with module reuse, below, and
before lines ran on one committed session), on a 32-thread host at 1-minute
load average 45.74–60.54 with four calculator processes at a time, a fresh `1+2`
took 267 s, `2*3` 296 s, `12.34` 240 s and `0.3` with `--finite-decimal`
292 s; without reuse the same lines took 464, 538, 349 and 569 s in the same
window. A module seen for the first time costs about 1.1–1.5 s there (source
check and backend preparation about 0.45–0.65 s each, worker assembly and V8
compilation about 0.1 s, commit 0.06–0.07 s); a reused one about 4 ms. A debug
build is slower still: `0.3 --finite-decimal` without reuse took 1,141 s
against 471 s for a release build run alongside it (load about 30).

Lines run on one committed session: a failed line restores a byte-exact
snapshot of it instead of replaying earlier definitions, so a line's cost
does not grow with the number of earlier `let`/`fn` lines. (With replay, the
session `fn f(y)=y*2`, `fn g(x)=f(x)+1`, `g(2)` ran 2,193 modules in 866 s
with reuse and 2,721 s without, in the window above.)
On the committed session, the reuse also spans lines: a debug build of the
merged tree `0b8c7f9` (load average 8–12, the two runs side by side) ran
`1+2` twice in 308 s with reuse and 1,088 s with
`NOBLE_CALCULATOR_MODULE_CACHE=off`. Each line ran 441 modules. With reuse
the first line spent 303 s in them (219 reused) and the second 0.7 s (all
441 reused); without, 547 s and 537 s.
Both runs executed byte-identical WAT, Wasm, execution reports and issued
Work for every module.
The [latency record](verification/calculator-v1/latency/README.md) keeps
the measured tables, binary hashes and load for these runs.
The [calculator acceptance runner](verification/calculator-v1/README.md) runs
the canonical CALC-01/02/03/06/07/09 CLI vectors against an exact prebuilt
binary, one process per vector, and keeps raw I/O and a JSON receipt outside
the repository. Its results are observations; they do not promote cases.
On the `44329dc` release snapshot all 34 runnable vectors passed
([receipt](verification/calculator-v1/receipt-44329dc-release.json)); a
single-line vector took 51–2,051 s and the CALC-06 session 8,689 s.
On the integrated `eccbc02` release build all 34 passed again
([receipt](verification/calculator-v1/receipt-eccbc02-release.json)): a
single-line vector took 15–177 s, the CALC-06 session 686 s and the whole
sweep 13.5 minutes at six processes at a time.

Setting `NOBLE_CALCULATOR_TRACE=<file>` appends one JSON line per executed
guest module (source, stage identity, source-check / backend / worker times,
WAT and Wasm sizes and digests) to that file; the record schema is documented
in `crates/noble-cli/src/calculator/trace.rs`. Tracing is off by default and
does not change any output.

Most guest modules of a line repeat earlier ones exactly (same source, same
stack types, same compiler epoch). The calculator reuses such a module's
already checked artifact instead of re-running the source check and backend
preparation, and each worker reuses the assembled Wasm and compiled V8 module
of byte-identical WAT; every module still runs in the guest. Reuse is exact
(full source, type and WAT comparison, never a digest alone), keeps every
cap, and is bounded to 96 MiB of host and 64 MiB of per-worker retained
bytes. Neither cache is part of the committed session: a module reused after
a failed line's restore is reused only at exactly its recorded compiler
epoch. `NOBLE_CALCULATOR_MODULE_CACHE=off` disables it; outputs are identical
either way.

The `Calculator-v1` source/worker profile is separate from Core-Bootstrap,
Declared-Modules-v1, Text-Byte-Cursor-v1 and live profiles. Its compiler has a
4 MiB emitted WAT ceiling rather than the ordinary 1 MiB ceiling recorded in
the unchanged [Core ABI](crates/noble-cli/src/core/runtime/abi.json). Its
modules import a Wasm memory of 16 to at most 64 pages, which the worker
allocates at 64 pages so that restores never meet a resized arena; the other
profiles remain fixed at 16 pages. Its worker selects ceilings of 72,362
session cells / 1,376,208 allocation bytes rather than 4,096 / 196,608; raw
Text ingress accepts at most 65,536 bytes. Source preparation separately
bounds 512 reachable named bodies and 896 environment rows, including derived
nominal operations, and the calculator source compiler bounds distinct
signature keys at 8,192. Before each submission the runtime compacts the cells
reachable from the operand stack in handle order and zeroes the vacated tail;
calculator handles are valid only until the next submission, so the worker
refuses `park` and `project`. When no reachable Program runs compiled code,
the next module reuses dead function-table slots and restarts the
definition-identity ledger; when no Program, Syntax or recipe cell is
reachable, it also restarts the signature pool. Declared source signatures can
now name concrete nominal schemas of the same module or an exact exported
`module@N.Type`. These experimental limits are not a completed calculator or
acceptance evidence; see the
[calculator-v1 change](.cairn/changes/calculator-v1/proposal.md).

### Experimental numeric graphs (opt-in; no performance claim)

`noble ngraph` selects the opt-in **Numeric-Graph-v1** profile: fixed, typed
numeric graphs in the text format `ngraph-ir/0`, admitted by an independent
graph checker and evaluated on the CPU. Only `noble ngraph` and the
`noble-ngraph` library select it. `run`, `session`, `compile`, `live`,
`calculator`, `build`, `component` and `verify` are unchanged, and the profile
adds no kernel type, word, source syntax or runtime asset. A graph belongs to
one of two disjoint sub-profiles, with no shared value and no implicit
conversion: exact `ngraph-i64/0` (wrapping `I64`, as in K-NUM-01) and
experimental `ngraph-f32/0` (IEEE binary32, round to nearest even, no FMA, no
flush to zero). `F32` graphs are authored directly in `ngraph-ir/0` and
admitted only by the graph checker (decision B0(b)); there is no kernel `F32`.
Extraction from Core source produces `ngraph-i64/0` graphs only. Facts with
`evidence` provenance are refused (`E-FACT-UNCHECKED`) until issue #8 supplies
witness binding. The subcommands run no Node, Wasm, `wasm-opt` or other
process, so they need none of the pinned runtime tools. The contract is the
[numeric-graph-v1 design](.cairn/changes/numeric-graph-v1/design.md), which
builds on the archived
[numeric-graph-profile design](.cairn/archive/2026-10-16-numeric-graph-profile/design.md).

Each subcommand prints JSON report lines with the schema
`noble-ngraph-report/v1`:

| Command | Arguments and main options | Report |
|---|---|---|
| `noble ngraph check GRAPH` | Checker budgets. | One line: `graph` (the `GraphLogicalId`), `profile`, `subject`, `closure`, `inputs` and `outputs` (types), `params` (count), `live_nodes`, `layers`, `facts`, `param_layout`. |
| `noble ngraph extract SOURCE --inputs T[,T…] --out GRAPH` | `SOURCE` holds newline-separated submissions as in `noble session`, definitions first and the root last (`--framed` selects session framing). Every submission before the root must be a definition. `--inputs` lists the root's input types (`i64` or `bool`), bottom of the stack first. `--profile ngraph-i64/0` is the only accepted profile. Extraction and checker budgets. | One line: the `check` fields plus `work_units`, `expanded_nodes`, `pruned_nodes`, `inline_depth`. |
| `noble ngraph eval GRAPH VECTORS` | `--reference` (the compiled evaluator is the default), `--snapshot FILE`, `--snapshot-id HEX`, `--fuel N`, checker budgets, `--budget-forward-scratch N`. | A header line (`graph`, `snapshot` or null), then one line per vector: `vector`, `outcome`, and `outputs` or `code` and `site`. |
| `noble ngraph train MODEL --loss LOSS --data DATA --policy ngraph-deriv-real/0 --optimizer adam/0 --seed U64 --steps N --out SNAPSHOT` | `--from PARENT`, repeated `--opt-in`, `--adam-lr`, `--adam-beta1`, `--adam-beta2`, `--adam-eps`, checker budgets, `--budget-backward-scratch`, `--budget-snapshot-bytes`, `--budget-steps`. | One line: `snapshot`, `graph`, `parent`, `steps`, `loss_initial`, `loss_final` (binary64 bits). |
| `noble ngraph verify GRAPH SNAPSHOT` | `--snapshot-id HEX`; `--replay --loss LOSS --data DATA [--from PARENT]`. | One line: `snapshot`, `graph`, `bound: true`, `replayed: true` or `false`. |

Report values are typed strings. An `F32` value is its exact binary32 encoding,
`"f32:0x%08x"`, except that every NaN encoding renders as the single class
`"f32:nan"`; `I64` and `Bool` values render as `"i64:<decimal>"` and
`"bool:true"` or `"bool:false"`. An unknown, duplicated, missing or
inapplicable flag is a usage error. Run `noble --help` for the complete usage.
For example, to extract `x·x + y·y` from Core source and evaluate it:

```sh
printf '%s\n' 'def sq [ dup * ]' 'sq swap sq +' > squares.noble
./result/bin/noble ngraph extract squares.noble --inputs i64,i64 \
  --out squares.ngraph
./result/bin/noble ngraph check squares.ngraph
printf '%s\n' noble-ngraph-vectors/0 'types i64 i64' 'v 3 4' > squares.vectors
./result/bin/noble ngraph eval squares.ngraph squares.vectors
```

`extract` publishes `squares.ngraph` and prints (all three commands exit 0):

```text
{"schema":"noble-ngraph-report/v1","command":"extract","stage":"extraction","outcome":"normal","code":null,"site":null,"graph":"179a0a0e3529bf6ea90b2940ad63db4903a7a4ffdf57b7d3873dd8f0d5a70304","profile":"ngraph-i64/0","subject":"extracted 4a6f0f48eef6c936b33e1c5cbf1f12d2dd21dbcc230d9f01abac87168a79c357 closure 1","closure":[1],"inputs":["i64","i64"],"params":0,"outputs":["i64"],"live_nodes":5,"layers":3,"facts":0,"param_layout":"8cfa89c77cf14c664ffde4385e05ffe779538cacd3583d91e36c57a702330a3c","work_units":8,"expanded_nodes":5,"pruned_nodes":0,"inline_depth":1}
```

`check` prints the same fields without the four extraction counters, and
`eval` prints a header and one line per vector:

```text
{"schema":"noble-ngraph-report/v1","command":"eval","stage":"evaluation","outcome":"normal","code":null,"site":null,"graph":"179a0a0e3529bf6ea90b2940ad63db4903a7a4ffdf57b7d3873dd8f0d5a70304","snapshot":null}
{"schema":"noble-ngraph-report/v1","command":"eval","stage":"evaluation","outcome":"normal","code":null,"site":null,"vector":0,"outputs":["i64:25"]}
```

The written graph file is:

```text
ngraph-ir/0 ngraph-i64/0
subject extracted 4a6f0f48eef6c936b33e1c5cbf1f12d2dd21dbcc230d9f01abac87168a79c357 closure 1
slot input i64
slot input i64
%0 = input 0
%1 = input 1
%2 = i64.mul %1 %1
%3 = i64.mul %0 %0
%4 = i64.add %2 %3
out %4
```

**Graph files.** A graph file is UTF-8 text. This `ngraph-f32/0` model
computes `y = rn(rn(w·x) + b)`, with `w` and `b` in `param` slots and the input
`x` validated to `[-1, 1]`:

```text
ngraph-ir/0 ngraph-f32/0
subject authored-name cli-train-model
slot input f32
slot param f32
slot param f32
%0 = input 0
%1 = param 1
%2 = param 2
%3 = f32.mul %1 %0
%4 = f32.add %3 %2
fact %0 range 0xbf800000 0x3f800000 input-validated
out %4
```

The first non-comment line names the schema and the sub-profile. `#` starts a
comment, and blank lines, comments and runs of spaces carry no meaning. The
line `subject <authored|extracted|captured> <64 hex> closure <ids|->` is
required; `subject authored-name NAME` is authoring sugar for the authored
subject named by the rest of the line (its tokens joined by single spaces), and
printed graphs always use the hex form. Optional `shape input` and
`shape output` lines record stack shapes as trees `s`, `u` and `(p A B)`,
nesting at most 127 pairs (the deepest interface item that extraction can
produce; a deeper tree is `E-GRAPH-MALFORMED` at its line). Then come the slots
(`input`, or `param` in `ngraph-f32/0`), the nodes `%k = OP …` in order, the
`fact` records and the `out` records. The operations are `input`, `param`,
`const`, `i64.add`, `i64.sub`, `i64.mul`, `i64.eq`, `f32.add`, `f32.sub`,
`f32.mul`, `f32.affine` (a bias, then coefficient and operand terms folded in
stored order), `f32.relu`, `f32.reglu`, `f32.step`, `f32.select`, `f32.index`
(with a `refuse`, `clamp` or `zero` rule for each side out of range) and
`guard`. An `F32` literal is a `0x` bit pattern of one to eight hex digits or
an exactly representable decimal: `0.1` is `E-LITERAL-INEXACT`. No number in
any `ngraph` file or `noble ngraph` flag takes a leading `+`. Each fact has one
provenance: `input-validated` (checked on the actual input or param before
evaluation), `derived RULE` (re-derived by the checker), `guard %k`, `asserted`
(always refused) or `evidence HEX` (refused, see above). The `GraphLogicalId`
covers the sub-profile with its semantic and fact-rule revisions, the nodes in
order, the slots, both shapes, the outputs, and the subject kind, identity and
closure. Comments, whitespace, file names and paths never contribute, and
neither do fact records, which snapshots bind separately. An extracted graph's
subject identity is a digest of the kernel's rendering of the accepted subject,
so it is stable only within one kernel revision.

**Vector and data files.** A vector file (for `eval`) and a training-data file
(for `train`) declare the graph's `input` slot types on a `types` line; a data
file adds `;` and the loss graph's target types. Parameters never come from
these files, only from snapshots. An `f32` value is `0x` plus one to eight hex
digits (any encoding, including NaN, infinities and subnormals) or an exactly
representable decimal (`nan` and `inf` words are `E-LITERAL-NONFINITE`; give
those values as bits); an `i64` value is digits in range, optionally after `-`;
a `bool` value is `true` or `false`. A decimal is
`[-]digits[.digits][(e|E)[+|-]digits]` (either side of the `.` may be empty,
not both). Exactness is decided in time linear in the token: a binary32 value
written exactly has at most 112 significant digits, the last at `10^-149` to
`10^38`, so any other decimal is `E-LITERAL-INEXACT` without further
arithmetic:

```text
noble-ngraph-vectors/0
types f32 f32
v 0x3fc00000 -0.5
v 0x7fc00000 0x80000000
```

```text
noble-ngraph-data/0
types f32 ; f32
point 0 ; 1
point 0.5 ; 2
point -0.5 ; 0
point 1 ; 3
```

A `types` line that differs from the graph is `E-INTERFACE`. An unknown or
missing schema line, a row with the wrong number of values and any other
syntax error are `E-FORMAT`, and an inexact decimal is `E-LITERAL-INEXACT`;
each is refused before any evaluation: one bad row refuses the whole file.
Input-fact validation (`E-FACT-INPUT`) stays per vector. Row order is
preserved, and the training-data digest binds it.

**Training.** `train` changes only the model's `param` slots. The contract is
explicit: `--policy`, `--optimizer`, `--seed` and `--steps` have no defaults.
`--steps 0` publishes the seed-initialized snapshot S₀, the only way to create
an untrained snapshot, and `--from PARENT` starts from a published snapshot of
the same graph instead of the seed. The loss graph must be `ngraph-f32/0`, with
no `param` slot, with input slots equal to the model's outputs followed by the
data's target types, and with exactly one `F32` output; otherwise
`E-INTERFACE`. `ngraph-deriv-real/0` is the derivative of the intended
real-valued arithmetic, ignoring binary32 rounding. A param that reaches the
loss through `f32.step` or an `f32.select` gate is refused (`E-DERIV-BOOLEAN`)
unless that node `%N` is opted in with `--opt-in %N:boolean=straight-through`
or `--opt-in %N:boolean=clamped-linear:0xBITS`. A param that reaches an
`f32.index` operand is refused (`E-DERIV-INDEX`) unless opted in with
`--opt-in %N:index=zero` or `--opt-in %N:index=backward-difference`. Every
opt-in is recorded in the snapshot, and the surrogates are never described as
exact derivatives. An opt-in naming no node of the model, or a node its rule
does not apply to (`straight-through` takes a `step` or `select`,
`clamped-linear` a `step`, the index rules an `index`), is refused with the
same codes. An `ngraph-i64/0` model is refused with `E-DERIV-I64`. In each
step, every data point is first checked in point order as `eval` checks a
vector. A failing input fact (`E-FACT-INPUT`) or guard (`G-FACT-FALSE`), or a
non-finite gradient or update (`E-NONFINITE-UPDATE`), stops training and
publishes nothing. `--adam-lr`, `--adam-beta1`, `--adam-beta2` and `--adam-eps`
take `0x` plus the 16 hex digits of a binary64 encoding, or a decimal rounded
to nearest even. The defaults are the `adam/0` values 0.02, 0.9, 0.999 and
1e-8, and the exact bits are recorded. Values outside the domain that `adam/0`
needs (all finite, `lr > 0`, `eps > 0`, `0 ≤ β < 1`) are `E-PROFILE`, before
any file is read.

**Snapshot files.** A snapshot file records the parameters, their identities
and the complete training provenance, one field per line:

```text
noble-ngraph-snapshot/0
snapshot <64 hex>            # ParamSnapshotId = H(noble.ngraph.params.v0 ‖ graph ‖ slots ‖ LE bytes)
graph <64 hex>               # GraphLogicalId
layout <64 hex>              # param layout key
param <slot> 0x<8 hex>       # one line per param slot: interface slot index and bits
optimizer-state <64 hex>
parent <64 hex>
subject <kind> <64 hex> closure <ids|->
profile ngraph-f32/0 ngraph-sem/0
facts <64 hex>               # fact-binding digest of the model graph
loss <64 hex>
data <64 hex>
policy ngraph-deriv-real/0
opt-in %<node> boolean refuse|straight-through|clamped-linear 0x<8 hex> | index refuse|zero|backward-difference
seed 0x<16 hex>
prng splitmix64/0
optimizer adam/0 lr 0x<16 hex> beta1 0x<16 hex> beta2 0x<16 hex> eps 0x<16 hex>
steps <n>
tools noble-ngraph/0; ngraph-ir/0; ngraph-sem/0; ngraph-facts/0; ngraph-layered/0
provenance <64 hex>          # H(noble.ngraph.provenance.v0 ‖ every record above, in file order)
```

Records appear in this order (hex is lowercase). For a snapshot trained from
the seed, `parent` is the id of the seed-initialized snapshot S₀. The
provenance digest uses the same identity encoding as every other identity
(length-prefixed byte strings and fixed-width little-endian integers); its
exact field list is in design D6 of the
[numeric-graph-v1 change](.cairn/changes/numeric-graph-v1/design.md).

Loading a snapshot recomputes its layout key, its `ParamSnapshotId` and its
provenance digest; any mismatch, a non-finite parameter or recorded `adam/0`
values outside the optimizer's domain are `E-SNAPSHOT-INTEGRITY`, because
training never publishes them. Binding it to a graph then refuses a different
`GraphLogicalId` or a different fact-binding digest (`E-SNAPSHOT-STALE`, so a
graph whose fact records changed does not accept parameters trained under other
facts), a different param layout (`E-SNAPSHOT-LAYOUT`) and a snapshot other
than the one pinned with `--snapshot-id` (`E-SNAPSHOT-ID`). A graph with
`param` slots and no `--snapshot` is `E-SNAPSHOT-LAYOUT`. All of these refusals
happen before any graph node is evaluated.

**Immutable publication and replay.** `train --out` and `extract --out` publish
only new files. The CLI writes `<out>.partial` with create-new, syncs it,
hard-links it to `<out>` (which fails if `<out>` exists) and removes
`<out>.partial`. No path is ever truncated, renamed over or modified, and a
`--from` parent is only read. An existing `<out>` or `<out>.partial` is a host
refusal (exit 2) that leaves both untouched, and a refused or stopped command
publishes nothing at `<out>`. If writing or syncing `<out>.partial` or the link
fails, the CLI removes `<out>.partial`; if that removal fails too, the file is
left behind and the refusal says so. Once the hard link exists the output is
published: if removing `<out>.partial` then fails, the command still reports
success and warns on stderr about the leftover, a second name for the same
bytes.
`verify --replay` rebuilds the training contract from the model, loss and data
files, the recorded seed, policy, opt-ins, optimizer and step count, and the
parent given with `--from`. The supplied model, loss, data and parent must
match the recorded `graph`, `loss`, `data` and `parent` identities (otherwise
`E-SNAPSHOT-STALE`). Replay then trains again and admits the recorded contract
as `train` admits its flags, so a recorded opt-in that `train` would refuse is
refused with the same `E-DERIV-BOOLEAN` or `E-DERIV-INDEX`. It requires
byte-identical parameters, optimizer-state digest and snapshot identity
(otherwise `E-SNAPSHOT-REPLAY`). To train the model above, save it as
`model.ngraph`, the data file above as `data.points`, and this MSE point loss
as `loss.ngraph`:

```text
ngraph-ir/0 ngraph-f32/0
subject authored-name cli-train-mse
slot input f32
slot input f32
%0 = input 0
%1 = input 1
%2 = f32.sub %0 %1
%3 = f32.mul %2 %2
out %3
```

```sh
./result/bin/noble ngraph train model.ngraph --loss loss.ngraph \
  --data data.points --policy ngraph-deriv-real/0 --optimizer adam/0 \
  --seed 7 --steps 0 --out s0.snapshot
./result/bin/noble ngraph train model.ngraph --loss loss.ngraph \
  --data data.points --policy ngraph-deriv-real/0 --optimizer adam/0 \
  --seed 7 --steps 25 --out s25.snapshot
printf '%s\n' noble-ngraph-vectors/0 'types f32' 'v 0.5' 'v -1' > inputs.vectors
./result/bin/noble ngraph eval model.ngraph inputs.vectors --snapshot s25.snapshot
./result/bin/noble ngraph verify model.ngraph s25.snapshot \
  --replay --loss loss.ngraph --data data.points
```

Each command exits 0. The two `train` runs print:

```text
{"schema":"noble-ngraph-report/v1","command":"train","stage":"training","outcome":"normal","code":null,"site":null,"snapshot":"b877b38358afd078fdf3d0ad1152f1e474b8897d25f285b30cf27a4da4e8eb60","graph":"3ef6d14e9a0b429e9446b166b3db175b0fe308174eca8d622400a273f15540f7","parent":"b877b38358afd078fdf3d0ad1152f1e474b8897d25f285b30cf27a4da4e8eb60","steps":0,"loss_initial":"0x4015bd11d3800000","loss_final":"0x4015bd11d3800000"}
{"schema":"noble-ngraph-report/v1","command":"train","stage":"training","outcome":"normal","code":null,"site":null,"snapshot":"693184119553c5456c2319a3090ae14e4eb1b7cc51ce443a6d2c0b2491791b72","graph":"3ef6d14e9a0b429e9446b166b3db175b0fe308174eca8d622400a273f15540f7","parent":"b877b38358afd078fdf3d0ad1152f1e474b8897d25f285b30cf27a4da4e8eb60","steps":25,"loss_initial":"0x4015bd11d3800000","loss_final":"0x40064a1dbfc00000"}
```

`eval` and `verify --replay` print:

```text
{"schema":"noble-ngraph-report/v1","command":"eval","stage":"evaluation","outcome":"normal","code":null,"site":null,"graph":"3ef6d14e9a0b429e9446b166b3db175b0fe308174eca8d622400a273f15540f7","snapshot":"693184119553c5456c2319a3090ae14e4eb1b7cc51ce443a6d2c0b2491791b72"}
{"schema":"noble-ngraph-report/v1","command":"eval","stage":"evaluation","outcome":"normal","code":null,"site":null,"vector":0,"outputs":["f32:0x3e459c1c"]}
{"schema":"noble-ngraph-report/v1","command":"eval","stage":"evaluation","outcome":"normal","code":null,"site":null,"vector":1,"outputs":["f32:0xbec00d06"]}
{"schema":"noble-ngraph-report/v1","command":"verify","stage":"replay","outcome":"normal","code":null,"site":null,"snapshot":"693184119553c5456c2319a3090ae14e4eb1b7cc51ce443a6d2c0b2491791b72","graph":"3ef6d14e9a0b429e9446b166b3db175b0fe308174eca8d622400a273f15540f7","bound":true,"replayed":true}
```

**Outcomes and exit codes.** Every report line has `schema`, `command`,
`stage`, `outcome`, `code` (a string or null) and `site` (a string or null). An
optional `detail` string is informative only and is not part of the stable
contract. The outcome decides the exit status:

| Outcome | Exit | Codes |
|---|---|---|
| `normal` | 0 | — |
| `trap` | 1 | `G-FACT-FALSE` (a guard failure or a refusing `f32.index` read), `E-NONFINITE-UPDATE`, `C-CANCELLED` (listed for completeness; the CLI has no cancellation source) |
| `reject` | 2 | `E-GRAPH-MALFORMED`, `E-FORMAT`, `E-LITERAL-INEXACT`, `E-LITERAL-NONFINITE`, `E-TYPE-MIX`, `E-INTERFACE`, `E-SUBJECT` (including a Core frontend refusal of the extraction source), `E-CLOSURE`, `E-PROFILE` (an unknown sub-profile or one the command does not accept, a raised budget, a `--policy` or `--optimizer` other than `ngraph-deriv-real/0` or `adam/0`, or `adam/0` hyperparameters outside its domain), `E-FACT-UNCHECKED`, `E-FACT-INPUT`, `E-PARAM-UNDECLARED`, `E-SNAPSHOT-STALE`, `E-SNAPSHOT-LAYOUT`, `E-SNAPSHOT-ID`, `E-SNAPSHOT-INTEGRITY`, `E-SNAPSHOT-REPLAY`; host conditions with `code: null`: an I/O failure, an exceeded input bound, an existing output |
| `unsupported` | 4 | `E-EFFECT`, `E-RESOURCE`, `E-DYNAMIC-RUN`, `E-LIVE-SLOT`, `E-RECURSION`, `E-CONTROL`, `E-AGGREGATE`, `E-REFLECT`, `E-PROGRAM-OUTPUT`, `E-DERIV-BOOLEAN`, `E-DERIV-INDEX`, `E-DERIV-I64` |
| `exhausted` | 5 | `E-BUDGET-<kind>`, including `E-BUDGET-FUEL` (runtime) and `E-BUDGET-STEPS` |

`eval` exits with the status of the first non-`normal` vector in file order,
or 0. A refusal of the whole graph, by the checker or by snapshot binding,
prints one report line and no vector lines. Usage errors print nothing on stdout and the usage on stderr
and exit 2.

**Budgets and input bounds.** Every `/0` budget has a flag `--budget-<kind> N`,
and each subcommand accepts only the kinds it charges. A flag may only lower
a budget: a value above the `/0` default is `E-PROFILE`, because raising one
needs a new profile revision, and a value that is not a `u64` is a usage
error.

| Kind | `/0` default | Charged on |
|---|---|---|
| `work` | 4,194,304 | Extraction steps, including repeated expansions |
| `expansion` | 262,144 | Nodes created before pruning |
| `inline-depth` | 64 | Named call, `run` and `dip` nesting |
| `live-nodes` | 262,144 | Nodes after normalization |
| `layers` | 16,384 | The critical path |
| `inputs`, `outputs` | 65,536 each | Input and param slots; outputs |
| `affine-terms` | 4,096 | Terms of one `f32.affine` |
| `table-length`, `table-entries` | 65,536; 1,048,576 | One `f32.index` table; all tables of a graph |
| `facts` | 262,144 | Fact records |
| `graph-bytes`, `snapshot-bytes` | 32 MiB; 4 MiB (1,048,576 `F32`) | The retained graph; a snapshot |
| `forward-scratch`, `backward-scratch` | 64 MiB; 128 MiB | One evaluation |
| `steps` | 1,000,000 | Optimizer steps of one `train` call |

`--fuel N` bounds node evaluations. It is host policy with no default limit,
and running out is `E-BUDGET-FUEL`. The host input bounds are fixed limits,
not profile budgets: graph, vector and data files 64 MiB each, snapshot files
32 MiB, and extraction sources 1 MiB, with each submission at most 65,536
bytes. Each bound is checked before parsing, and a larger file is a host
refusal (`code: null`, exit 2).

**Determinism.** For identical arguments and input bytes, stdout is
byte-identical across runs: reports have a fixed key order and carry no
timing and no file path. The default compiled evaluator and `--reference`
print identical bytes for every `normal`, `reject` and `trap` line. Under
`--fuel`, the point at which an `exhausted` evaluation stops is
evaluator-specific, so those lines may differ. Two `train` runs with the same
inputs write byte-identical snapshot files.

**Non-claims.** Selecting this profile is not a performance, speedup, GPU or
backend-selection result. Phase C of the archived design recommended
**stop**. On warm W3-L, the compiled CPU graph took 9.71 µs per evaluation
against 1.27 µs for straightforward host-native Rust, which is 0.131× its
speed. W3-S, W3-M and W4 reached 0.068×, 0.061× and 0.182×. Continuing
required at least twice the speed of both baselines, and GPU work (issue #2)
stays blocked. The owner adopted the profile anyway as an opt-in feature, with
no performance or GPU claim. The proof obligations NG-PO-A..H (extraction,
graph checker, compiled evaluator, binary32 model, fact transfer, derivative
policy, snapshot binding and executed bytes) remain open and are not in the
obligation ledger. Agreement between the evaluators, matching identities and a
successful replay are finite differential evidence: a snapshot identity
identifies bytes, and neither it nor a replay shows that the trained
parameters, the optimizer or the evaluators are correct. Replay is defined on
one pinned platform, and there is no cross-platform bit-reproducibility claim.
The Phase B and Phase C receipts and their reproduction commands are in
[verification/ngraph/README.md](verification/ngraph/README.md). The registered
[NGRAPH cases](specs/conformance/numeric-graph-cases.json) and
[profile status](specs/STATUS.json) record a **14-passed/1-held candidate**.
The original cases 01/06/07/08/10/13/14/15 are `implemented/passed/open/explicit`, bound to the
immutable [acceptance receipt](verification/numeric-graph-v1/acceptance.json)
and [independent review](verification/numeric-graph-v1/review-receipt.json).
Six additional execution records (03/04/05/09/11/12) retain the preliminary
002d packet and bind a fresh full-source safety observation for the final
candidate. The final integrated-source cold-review receipt and bounded terminal
receipt bind the exact case records, source freeze and measurement packet;
proof remains open.
CASE-02 alone remains `absent/not-run/open/unassessed` with `evidence: []`:
complete zero-use capability closure is missing (637 relevant unknowns). The
[lifecycle evidence record](verification/numeric-graph-v1/README.md) gives the
exact library mappings, supplemental runtime bounds and current selected-tool
boundary. The eight-case historical acceptance and six additional finite
execution records are not whole-feature or global-gate acceptance. The
[change](.cairn/changes/numeric-graph-v1/design.md) stays active and is not archived.

**CPU follow-on (2026-10-16).** A source-pinned comparison of the existing CPU
workloads motivated removing per-instruction table copies and repeated derivative
reachability walks, plus specializing the default instruction loop away from
optional observation. In the selected pair, W4 cold compile fell from
0.919–1.012 ms to 0.085–0.124 ms, and W3-L derivative admission from 115.606 µs
to 30.357 µs. Warm W3-L evaluation ranges overlap and it remains slower than
host-native Rust; this is not a new performance/backend/GPU claim. Numerical,
snapshot and scratch/layout agreement is finite evidence, not proof. The
[separate CPU receipt](verification/ngraph/README.md#supported-profile-cpu-follow-on-2026-10-16)
records source/binary/harness hashes, all measured phases, shared-host limits,
resource-boundary observations and reproduction details. Historical receipts
and the frozen acceptance/review packets are unchanged.

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
vendor closure contains 238 archives, six beyond the formerly reviewed 232:
`unicode-width` 0.2.2, four BLAKE3-related crates and, for the issue #14
numeric graph experiment, `rustc_apfloat` 0.2.3. A hash-checked candidate
vendor was realized locally, with its measured path and NAR bound by M5/M6
in this WIP selection, without source-bound assurance. That is not a selected
Nix, source-inventory or native-assurance gate pass. Source locations, builder
D, VM addresses and code-owner handles are not part of D. General logical
source-binding aliases, independently named equal bindings, recursive lexical
owners, other returned interfaces and effectful graph shapes have no
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
| [Numeric graphs](#experimental-numeric-graphs-opt-in-no-performance-claim) | Opt-in `Numeric-Graph-v1`, only through `noble ngraph`: checked `ngraph-ir/0` graph files, exact `I64` graphs extracted from Core source, directly authored `F32` graphs (no kernel `F32`), reference and compiled CPU evaluators, `adam/0` training into immutable snapshot files, and provenance replay. [Recorded cases](specs/conformance/numeric-graph-cases.json): 14 passed / 1 held (CASE-02), with six additional fresh finite observations bound to the final candidate; the exact review and terminal receipts decide case registration. Implementation partial, change active, not archived. Phase C measured 0.06–0.18× the speed of host-native Rust, so there is no performance or GPU claim; NG-PO-A..H remain open. |

Ordinary `run` and `session` use the **resource-free Core-Bootstrap** profile.
Their host words are `test.emit` and `test.abort`. Resources and component
features use separate entry points. Recursive definitions, local bindings,
general libraries and capability modules, portable package identities, and
general distributed services remain open.

The [exact calculator](.cairn/specs/calculator/spec.md) is the planned reference
application; an [experimental guest-backed `noble calculator`](#experimental-calculator-opt-in-acceptance-not-established)
exists, but its acceptance criteria remain open. Its [numeric error schema prerequisite](verification/calculator-errors/README.md)
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
