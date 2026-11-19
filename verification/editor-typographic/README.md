# Editor typographic renderer experiment (issue #141)

A default-off typographic view over the five existing editor node kinds:
integers as I64 range-checked numeric controls, booleans as checkboxes, words as
text controls, quotations as groups, and holes kept visible with the constraints
`analyze` reports for them. Widget gestures decode only to those five node
kinds and every round trip goes through the existing `noble editor analyze` /
`noble editor admit` ingress.

This directory freezes the corpus and the criteria for the experiment and
retains its receipt. The implementation is
`crates/noble-cli/tests/editor_typographic/`; it is compiled into the existing
`tests/editor.rs` integration target and is not part of the `noble` binary.

## Claim boundary

Measured, over the frozen corpus only, through the real CLI:

- identity: re-encoding the rendered tree with no edits is node-tree equal to
  the frozen document and produces the same `analyze` and `admit` reports;
- edited equality: each documented widget edit produces the frozen equivalent
  JSON-tree edit, and both paths have the same CLI verdicts, including equal
  `admitted_source` bytes for hole-free results;
- hole coverage: each of the five named hole states renders, in tree order,
  with the reported stacks, required output suffix and effect;
- refusal: each negative control is refused by the renderer before any CLI
  invocation, and every tree that still contains a hole is still refused by
  `admit`.

Not measured or claimed: any property of editor trees outside the corpus, any
GUI or rendering aesthetics, timing, fidelity to the posts and demos that
motivated the issue, or the authors' claims about other editors. There is no
theorem here; the renderer's own refusals are checked over the frozen negative
controls only.

## Opt-in mechanism

The view is a test-only Rust module. It is not behind a cargo feature, adds no
`noble editor` action, and no production source file is edited: the lane's only
change outside `crates/noble-cli/tests/` is documentation. The prohibition on a
second admission route is therefore structural rather than a runtime policy —
the binary has no code path to the renderer, and the harness asserts that the
binary does not contain the renderer's marker string and that an unknown
`editor` action is still a transport refusal. `decision.md` records the rejected
alternatives.

## View model

| editor node | control | legal gesture payload |
|---|---|---|
| `Integer(i64)` | numeric control | decimal text: optional `-`, at least one ASCII digit, checked I64 range |
| `Boolean(bool)` | checkbox | `true` or `false` only, or the toggle gesture |
| `Word` | text control | the source lexer's ASCII word grammar: initial `[A-Za-z_+*/=<>?!-]`, then those plus digits and `.`, never `def`, `true`, `false` or an integer literal |
| `Quotation` | group | structure only; edits address nodes inside it by `node_path` |
| `Hole` | visible hole with facts | leave it, or fill it with one of the five node kinds |

Hole facts are bound from the `analyze` report in tree order. A report whose
hole count disagrees with the rendered holes is refused rather than patched, and
the bound facts include the input stack, required output stack, required output
suffix, known effects and the unresolved flag. The program's own effect row is
rendered alongside the tree.

A fill that would put a node deeper than fourteen quotation levels is refused.
The editor JSON reader parses the envelope at depth 0, its `nodes` array at
depth 1 and every node object two levels deeper per enclosing quotation, and
refuses a value parsed at depth 32; a terminal node inside fourteen quotations
is parsed at depth 30. The bound is conservative: a fifteen-level chain whose
innermost quotation is empty would still be readable, and it is refused anyway.

## Corpus

`corpus.json` pins ten documents, twenty-three edits, five named hole states,
twenty-five negative controls, a depth pair and eleven hostile transport
documents. `criteria.md` states the ten graded criteria and `criteria.json`
holds their ids, which the harness requires in order. Both were committed
before the receipt-producing run; the receipt names that commit.

## Reproduce

```
cargo test -p noble-cli --test editor --offline --locked
```

To regenerate the receipt (it must be written outside the repository and a
retained receipt is never overwritten):

```
NOBLE_EDITOR_TYPOGRAPHIC_RECEIPT=<new directory outside the repository>/receipt.json \
NOBLE_EDITOR_TYPOGRAPHIC_SOURCE_COMMIT=$(git rev-parse HEAD) \
cargo test -p noble-cli --test editor --offline --locked -- typographic_receipt
```

The producer refuses to write unless the worktree is clean at the declared
commit, and the receipt records that commit, the corpus and criteria digests,
every measured verdict, the admitted-source digests, the negative-control codes
and the CLI invocation count.

## Receipt

`receipt-<source commit>.json` in this directory is the retained measurement.
Its `source_commit` is the commit the run measured; the corpus and criteria
blake3 digests are recorded inside it.
