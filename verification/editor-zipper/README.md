# Editor zipper cursor lane (#142)

Issue: [#142](https://github.com/OnixResearch/noble-lang/issues/142).
Decision record: [decision.md](decision.md).
Frozen corpus: [corpus.json](corpus.json). Frozen criteria: [criteria.json](criteria.json).
Measured evidence: `receipt-<commit>.json` in this directory, produced from a clean commit by
the harness below.

This lane answers one question: over the five structured-editor node kinds, is a focus-and-trail
cursor — one focused node plus the quotation frames around it, addressed with the transport's
existing `node_path` scheme — node-equal to a direct JSON edit of the same tree, and
indistinguishable from it by `noble editor analyze` and `noble editor admit`?

## What this lane is not

- It is not a theorem, a refinement, or a model-to-implementation correspondence. Every claim
  here is a behavioural comparison of two documents through the unchanged editor transport.
- It is not a reproduction of the cited paper. Nothing was vendored or ported: no Kamot Haskell
  modules, no Lean development, no prefetching workload, and no media from the post.
- The authors' trail-depth bounds, cost measurements and completeness results are their claims.
  `criteria.json` records them as non-gates; nothing here measures or thresholds them.
- It is not evidence about issue #141's renderer, about `noble-kernel` or `noble-contracts`, or
  about admission semantics beyond the verdicts compared below.

## Default-off

`crates/noble-cli/src/core/editor/zipper.rs` is declared `#[cfg(test)] mod zipper;` in
`crates/noble-cli/src/core/editor.rs`, and nothing else reaches it. The shipped CLI is built
without `cfg(test)`, so it contains no cursor code — and the measurement checks the artifact
instead of trusting the source layout: the receipt's `default-off:shipped-binary` cell requires
the editor report schema to be present in `CARGO_BIN_EXE_noble` as a positive control, requires
the cursor module to spell its refusal-class markers, and requires none of them in the artifact.

`noble editor analyze` and `noble editor admit` keep their JSON ingress, their exit codes, their
generated spans, their bounded `node_path` addresses and `noble-editor-report/v1`. The cursor
adds no command, no flag, no report field and no default path. `decision.md` records why this
mechanism was chosen over a cargo feature (which `policy/tool-selection.json` pins) and over a
new subcommand (which would change the transport's argument surface).

## The cursor

| Move | Effect | Refusal |
|---|---|---|
| `at(node_path)` | focus the addressed node | `no-such-node` |
| `enter` | focus the first child of a focused quotation | `not-a-quotation`, `empty-quotation` |
| `leave` | focus the quotation that contains the focus | `at-root` |
| `next`, `previous` | focus the neighbouring sibling | `off-end` |
| `replace(node)` | substitute the focus with one of the five kinds | none: the node type fixes the five kinds |
| `reconstruct` | fold the trail back into the root node list | none |

Identity reconstruction is what makes the lane measurable: navigation changes no node, so every
address must reconstruct the original tree, and `replace` is the only tree-changing move. The
trail stores, for each enclosing quotation, the siblings of the quotation node that holds the
focus; `reconstruct` wraps the focus back up through them and splices it into the root list.

## The frozen corpus

Twelve format-1 editor documents — 10 navigation sequences (31 steps), 23 replacements with their
independent JSON edits, and 32 illegal moves with their refusal classes:

| Case | What it adds |
|---|---|
| `complete` | the `1 2 +` case from `tests/editor.rs`: admits and runs |
| `incomplete` | `1 <hole> +`: analysis constraints, then hole refusal |
| `effectful` | the hole before `test.emit` |
| `nested` | a hole inside two quotations; the refusal addresses `[0,1,0]` |
| `root_hole` | a direct root hole |
| `nested_holes` | a nested hole and a root hole; filling one leaves the other |
| `nested_quotations` | hole-free three-level nesting; admitted bytes compared |
| `boolean` | the boolean kind as a whole tree |
| `empty_quotation` | `[]`: entering it has nothing to focus |
| `hostile_word` | `[` refused as word syntax, then repaired through the cursor |
| `check_refusal` | `drop` type-rejects at the check stage |
| `empty_tree` | the transport accepts `[]`; the cursor cannot address it |

Every tree in the corpus is a document the transport itself reads, so the same bytes the cursor
is measured against are the bytes the two editor commands parse. Each replacement carries the
independent JSON edit of the whole tree (authored as a direct edit, not produced by the cursor),
each sequence carries the `node_path` the focus must report after every step, and each illegal
move carries the class it must return.

## Criteria

Frozen in `criteria.json` before the measurement ran:

- **C1 identity reconstruction** — every address reconstructs the original tree and reports the
  address it was given.
- **C2 documented moves** — every navigation step reaches its documented address, and the
  sequence reconstructs the documented tree.
- **C3 replacement equals the JSON edit** — the cursor's tree equals the independent edit, and
  the two documents produce equal `analyze` and `admit` verdicts; hole-free admitted pairs also
  produce equal `engine/module-1.noble` bytes and equal module digests.
- **C4 holes still refuse admission** — with `accepted_program_created` false and no guest
  request, at the first hole's own `node_path` when the tree's syntax builds.
- **C5 illegal moves refuse before admission** — the class matches and the editor-command
  counter does not move.
- **C6 default-off** — no cursor code in the artifact, and no report shape other than
  `noble-editor-report/v1` or `noble-core-report/v1`.

The go rule is "GO if and only if every criterion holds on every case with zero mismatches";
either verdict leaves the layer default-off.

## Method

The harness is `crates/noble-cli/tests/editor_zipper/measurement.rs` plus its
`measurement/{corpus,paired,receipt,transport,walk}.rs` modules, declared from the existing
`crates/noble-cli/tests/editor.rs` target with one `#[path]` module line, so no test target is
added. It includes `crates/noble-cli/src/core/editor/zipper.rs` with `#[path]`: the code measured
is byte-identical to the module the `noble-cli` test build compiles, and every file of both sides
stays inside the repository's 300-line file budget.

Every cell is a pair — the document the cursor produces and the corpus's independent JSON edit:

1. both documents go to `noble editor analyze` and to `noble editor admit` as a child process,
   and the whole parsed report plus the exit code must agree (the receipt also records
   `stage`, `outcome` and `failure.category`);
2. for admitted trees, both documents are admitted again with `--emit`, and the literal
   `engine/module-1.noble` bytes must be equal. Those are the bytes the route passes to
   submission; the report's `module.source_sha256` names the same bytes, and the receipt records
   both digests plus `module.wasm_sha256`;
3. for hole-bearing trees, the refusal must keep `accepted_program_created` false, report no
   guest request, and — when the tree's own syntax builds — name exactly the `node_path` the
   cursor reports for the first hole;
4. illegal moves are produced in process: the harness asserts the refusal class and that the
   editor-command counter did not move, so "refused before admission" is measured rather than
   assumed. A replacement with something that is not one of the five kinds cannot be expressed
   in the cursor's typed API, and the transport refuses such a document at its own boundary
   before this lane is involved (see the `hostile_word` and `empty_tree` cells).

The corpus's own document decoder is validated against the ingress by construction of the
pairs: if it decoded a tree differently, the cursor-side document would differ and the report
comparison would fail.

## Receipt summary

`receipt-e82bf64f.json` is the current receipt: it was produced from source commit `e82bf64f` on
a clean tree, with corpus
sha256 `c17a922f4baa3604bd9fb890460791d82494b75e104bd86cd154f07cd00ba515` and criteria sha256
`59466b4d07456dce…` (both recorded in full in the receipt, together with the sha256 of the
eleven implementation files, the toolchain versions and the command that ran). Its `summary`
block counts the cells:

| Criterion | Cells | Result |
|---|---|---|
| C1 identity reconstruction | 29 addresses over 12 cases | every address reconstructs the original tree and reports itself |
| C2 documented navigation | 10 sequences, 31 steps | every step reaches its documented address; each sequence reconstructs the documented tree |
| C3 replacement pairs | 23 replacement pairs plus 12 original pairs | equal trees and equal verdicts in all 35 pairs |
| admitted source | 17 of those pairs admit | equal `engine/module-1.noble` bytes, equal `module.source_sha256`, equal `module.wasm_sha256` |
| C4 hole refusals | 14 pairs still hold a hole | 13 refuse at hole acceptance and name the cursor's own first-hole `node_path`; 1 (`hostile_word`) refuses earlier, at word syntax, before hole acceptance is reached |
| C5 illegal moves | 32 | each returns its documented class, with the editor-command counter unchanged |
| C6 default-off | 1 artifact probe | the editor report schema is present in the shipped binary; both cursor markers are absent |
| editor commands run | 174 | 35 verdict pairs × four runs, plus 17 admitted pairs × two `--emit` runs |

No cell mismatched: `summary.mismatches` is 0, and the harness fails its test rather than record
a mismatch. The 17 admitted pairs are the five hole-free originals (`complete`,
`nested_quotations`, `boolean`, `empty_quotation`, `empty_tree`), the three hole fills that reach
admission (`incomplete`, `nested`, `root_hole`), the boolean toggle, and eight other replacements,
including a whole-tree replacement of the underflowing word and of the empty quotation. They
cover added nodes, changed nodes, nested quotations and whole-tree replacement.

The retained receipt records the command that produced it: the compiled `editor` test binary
invoked with `--exact zipper::walk::cursor_corpus_cells` and `NOBLE_EDITOR_ZIPPER_RECEIPT` set,
because the build environment used here clears unknown environment variables. `cargo test` with
the same variable is the documented recipe below and runs exactly the same assertions.
`receipt-34035390.json` and `receipt-dcbeb019.json` are retained unchanged as the two earlier
file layouts' evidence; all three measure the same frozen corpus with the same counts.

The repository gates at `e82bf64f`, run from the repository root with the pinned toolchain:

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets --all-features --offline --locked -- -D warnings` | exit 0 |
| `cargo test -p noble-cli --all-targets --all-features --offline --locked` | 26 suites, 408 passed, 0 failed |
| Octet deny-all gate (lint + architecture) | `Status: clean`, `Findings: 0`, `Lint phase: clean (static_lint)`, `Architecture phase: clean (policy-check)`, exit 0 |

## Reproducing

From the repository root, with the repository's pinned toolchain:

```sh
cargo test -p noble-cli --test editor -- cursor_corpus_cells
# the same assertions plus the receipt, which refuses a dirty tree:
NOBLE_EDITOR_ZIPPER_RECEIPT=<absolute path outside the repository> \
  cargo test -p noble-cli --test editor -- cursor_corpus_cells
```

The receipt is written to the path given, outside the repository; it is copied in as
`receipt-<commit>.json` from a clean commit, and the commit is recorded inside it.

## Non-claims

- Passing cells are not theorems, refinements or correspondences, and they do not make the
  cursor a verified data structure.
- The corpus is a sample of editor trees, not an exhaustive class; nothing here claims coverage
  of a tree shape outside it.
- No cost, latency or memory bound is measured. The lane has no performance criterion, and the
  authors' numbers are not used as one.
- The editor transport's own limits (bytes, nodes, depth, work) are unchanged and are not
  re-measured here; the transport-refusal cells of `tests/editor.rs` remain the evidence for
  them.
