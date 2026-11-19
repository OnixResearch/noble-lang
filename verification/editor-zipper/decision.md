# Decision: the editor zipper cursor stays default-off (#142)

Status: **selected** (2026-11-19). Owner: verification lane `verification/editor-zipper`.
Applies to: the opt-in cursor over the five structured-editor node kinds in `crates/noble-cli`.
Related: issue #142 (this experiment), issue #141 (a different opt-in layer over the same node
kinds), issue #79 (closed: lens-based focus elaboration into checked stack operations, a
language surface), compiler-lab issue 268 (the cited paper's artifact replay, outside this
repository).

## Decision

1. **The cursor stays out of every shipped path.** It is declared `#[cfg(test)] mod zipper;`
   and nothing else reaches it. `noble editor analyze` and `noble editor admit` keep their JSON
   ingress, their exit codes, their generated spans, their `node_path` addresses and their
   `noble-editor-report/v1` contract, and the shipped binary contains none of the cursor.
2. **The measured claim is limited to the frozen corpus.** Within it, identity reconstruction
   equals the original tree at every address, each documented navigation and replacement equals
   the independent JSON edit of the same tree, the two documents produce equal verdicts, and
   hole-free admitted pairs produce equal admitted-source bytes and module digests. That is a
   behavioural comparison, not a theorem and not a refinement.
3. **No second admission route.** The cursor produces node values and a document encoder for
   the existing transport. Every verdict this lane reports comes from the real
   `noble editor analyze` / `noble editor admit` child process; nothing in the cursor submits.
4. **Refusals are in-process and typed.** An illegal move returns one of five classes before
   anything is encoded, so an illegal tree cannot be handed to admission; and a tree that still
   contains a hole still refuses, at the first hole's own address.
5. **The authors' bounds are not this lane's thresholds.** The cited paper's prefetching
   timings, trail-depth affine cost bounds and Lean completeness results are the authors'
   claims. Nothing here measures, reproduces or gates them; a replay, if any, belongs to
   compiler-lab issue 268.

## What the measurement does and does not establish

- It establishes, for the twelve frozen cases, that a focus-and-trail cursor over the five node
  kinds reconstructs and edits node trees exactly as a direct JSON edit does, and that the
  transport's verdicts cannot tell the two apart, including the admitted source bytes.
- It does not establish that the cursor is complete, that any cost is bounded, or that it is a
  good editor interaction. No cost is measured and the lane has no cost threshold.
- It does not establish anything about `noble-kernel`, `noble-contracts`, admission semantics or
  the report schema beyond "unchanged on these cases".
- It says nothing about issue #141's renderer. The two lanes share node kinds and the editor
  entry point; this lane neither depends on nor widens that experiment.

## Why the mechanism is `#[cfg(test)] mod zipper;`

The alternatives were a non-default cargo feature and a new CLI subcommand.

- A cargo feature is pinned by `policy/tool-selection.json` (`workspace_features`), so adding
  one is a policy re-pin rather than an experiment-local change, and it would still compile the
  module into a reachable build of the CLI.
- A new subcommand changes the editor transport's published argument surface, which issue #142
  explicitly leaves alone, and creates a second editor command to keep in step with the first.

`#[cfg(test)] mod zipper;` compiles the module only for test builds, keeps the shipped binary
free of it (measured, not asserted: the receipt's `default-off:shipped-binary` cell searches the
artifact the integration test invokes and requires the editor report schema to be present as a
positive control), and still lets the harness measure the same source file through the real CLI.

## Withdrawal and reopen conditions

- Withdraw the lane if any receipt cell fails. A failed cell is a defect in the cursor, not a
  licence to widen a criterion.
- Reopen for adoption only with a separate issue and a spec change that names the editor surface
  the cursor would join, the interaction it would replace, and the report-contract impact. That
  work would need a fresh receipt under the extended criteria.
- Extend the corpus or a criterion only by a commit that freezes the extension before a new
  receipt is produced.
