# Editor typographic renderer: frozen criteria (issue #141)

Frozen before the receipt-producing run. `criteria.json` is machine-read by the
lane's harness, which requires the criterion ids below, in this order. Changing
this list after a measurement means the measurement no longer answers the
committed question.

## Question

Can a typographic live-value view sit on top of the five existing editor node
kinds — integers as I64 range-checked numeric controls, booleans as checkboxes,
holes kept visible with their `analyze` constraints, words and quotations as
typographic structure — so that every widget edit round-trips through the
existing `noble editor analyze` / `noble editor admit` ingress and the rendered
view never produces a program the JSON path would reject?

## Criteria

| id | rule |
|---|---|
| `default-off` | The renderer is compiled only into the experiment's integration target, adds no `noble editor` action, and no production source file is edited. |
| `five-kinds` | The corpus exercises exactly `Integer`, `Boolean`, `Word`, `Quotation` and `Hole`, and each renders as its own widget. |
| `hole-coverage` | Every `analyze` hole report in the corpus has a rendered hole, in source order, carrying the reported input stack, required output stack, required output suffix and effect. A count mismatch is refused, never patched. |
| `identity-tree` | Re-encoding the rendered tree with no edits is node-tree equal to the frozen document. |
| `identity-verdict` | `analyze` and `admit` verdicts are equal between the frozen document and the re-encoded view document: exit code, `stage`, `outcome`, `category`, `source_stage`, `node_path`, `accepted_program_created`, `guest_requests`, `protected_operations`, and the whole printed report. |
| `identity-source` | For every hole-free tree, `admitted_source` bytes are equal on both paths; for a refused tree both refusals carry the same stage and category. |
| `edit-tree` | Each documented widget edit yields a node tree equal to the frozen equivalent JSON-tree edit, which was authored independently in the corpus. |
| `edit-verdict` | The gesture-produced tree and the frozen JSON-tree edit have equal `analyze` and `admit` verdicts, neither is refused at stage `transport`, and a hole-free result reaches execution. |
| `negative-refusal` | Every frozen negative control is refused by the renderer with its frozen code before any CLI invocation, and the frozen hostile transport documents are still refused by the ingress with no accepted program and no guest request. |
| `hole-admission` | Every tree that still contains a hole is refused by `admit` at stage `admission`, category `invalid`, `accepted_program_created: false`, `guest_requests: 0`, `protected_operations: 0`. |

## Corpus

`corpus.json` pins ten editor documents: the cases already exercised by
`crates/noble-cli/tests/editor.rs` (complete, incomplete, effectful,
nested-quotation hole, root hole, two holes) plus a boolean program, a word
program, a nested quotation and a hole whose program carries only an
unresolved effect. Hole states are named explicitly: root, nested quotation,
multiple, unresolved effect and known effect (the program's effect row is
`[0]`; a hole's own effect row is always reported unresolved).

Every edit pair in the corpus is `gesture` plus an independently authored
`json_nodes` value: the measurement checks that the widget gesture and the JSON
edit produce the same node tree before comparing CLI verdicts.

Twenty-three edits cover every documented widget edit; each carries an
independently authored JSON-tree equivalent. Twenty-five negative controls
cover out-of-range and non-decimal numeric values, non-boolean checkbox
payloads, word gestures that would inject quotation or source syntax,
wrong-widget gestures, illegal fills (not a hole, syntax-injecting payload, a
fill that reaches the transport's quotation-nesting bound), and unroutable
paths. Eleven hostile transport documents are frozen verbatim from the
existing CLI editor tests.

The depth pair measures the transport bound: a fill that reaches fourteen
quotation levels is accepted and read by the CLI, while a fill that reaches
fifteen levels (fourteen inside the fixture's own quotation) is refused by the
renderer before a document exists; the same fifteen-level tree with a terminal
node was measured as a transport refusal during authoring. The renderer's
bound is deliberately conservative: a fifteen-level chain whose innermost
quotation is empty would still be readable by the transport, and it is refused.

## Non-criteria

Wall-clock timings, rendering aesthetics, GUI behaviour, asset fidelity to the
referenced posts, and any claim about programs outside the frozen corpus are
not measured here.
