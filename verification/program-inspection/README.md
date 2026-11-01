# Bounded Program inspection (issue #76)

`:inspect <path>` in a live REPL session is presentation over an observation the session already
captured when a line was submitted. It reads the captured stack only: it never runs, installs,
re-checks or reloads anything, and its report always says `guest_requests: 0` and
`protected_operations: 0` because the module cannot reach a runtime at all.

This is the in-repo slice: a versioned report plus a documented expand reference over the current
JSON-line serialization. No other presentation surface and no alternate admission path is added.

## Invocation

```sh
printf '[ addone ]\n:inspect 0\n' | noble live repl --source math.noble --engine v8
```

with `math.noble` holding `def addone [ 1 + ]`. The path is a stack index, optionally followed by
recipe-node indices: `:inspect 0` selects the Program at stack slot 0, and `:inspect 0.3` selects the
Program quoted at recipe node 3 of it.

## Report (`noble-live-inspection/v1`)

[`example-report.json`](example-report.json) is the exact report the command printed for the
invocation above (verified against the binary, not transcribed).

| Field | Meaning |
|---|---|
| `outcome` | `selected`, or the one reason selection failed: `no-observation`, `path-malformed`, `out-of-range`, `stack-entry-not-a-program`, `node-out-of-range`, `node-not-a-quotation`, `quotation-depth-bound` |
| `selection` | the path as given, the stack index, the node path |
| `generation` | the **current** namespace generation at inspection time |
| `observed.generation_scope` | states that the observation is not the current namespace |
| `observed.source_epoch`, `observed.source_store_id` | the observation's own epoch and store, `null` when the session observed without them — never filled in from today's state |
| `observed.dependency_identity_scope` | every dependency id below is session-local, not portable content identity or authority |
| `program.interface` | ordered inputs/outputs and the declared effect ceiling, as observed |
| `program.effects` | the interface ceiling followed by witness-named effects, de-duplicated in order |
| `program.recipe`, `program.witnesses` | bounded copies of the observation's own entries |
| `program.nodes` | `total` seen, `kept` carried, and the depth and node bounds |
| `program.dependencies` | one entry per `invoke: definition:<id>`: the exact id the observation carries, labelled `session-local-definition-id`, with `provenance.status = unavailable-origin` when this observation carries no retained source metadata for it |
| `evidence` | `not-checked` with empty `subjects`, `claims`, `context`, `assumptions`: no evidence is bound to this observation, and that is neither a proof failure nor a proof success |
| `truncated` | every bound that fired, with `kept` and `total` |

Bounds: quotation depth 8, 512 recipe nodes, 128 witnesses and 128 dependency entries per report. A
truncated traversal returns, and names what it cut; it is never presented as complete.

## Expand reference

The reference view is the same serialization, expanded one path segment at a time:

1. `:inspect 0` — the Program at stack slot 0 with its interface, effects and dependencies;
2. read a recipe entry, e.g. `{"quotation": …}` at node `k`;
3. `:inspect 0.k` — the quoted Program at that node, with its own interface and witnesses;
4. repeat from 3.

`:generation` beside it prints the captured stack itself (`noble-live-report/v1`), so a client can
compare the two without any further observation.

## Explicit non-success states

- An empty captured stack is `no-observation`, not an empty Program.
- A node that is not a quotation is `node-not-a-quotation`: literals, invocations and nested value
  observations are not Program selections.
- A path deeper than 8 nested quotations is refused before it is walked.

## Inertness evidence

`crates/noble-cli/tests/live_repl.rs::inspection_reports_the_observed_program_and_stays_inert`
drives a real child session and asserts, against the live binary:

- repeated `:inspect` lines leave the captured stack and namespace generation byte-identical, with
  `protected_operations: 0` on every report;
- rebinding `addone` and committing a reload leaves the saved observation's recorded dependency id
  verbatim and its own epoch untouched while `generation` advances;
- the failure outcomes above each surface as their own `outcome`.

## Non-claims

No evidence, proof, refinement or implementation claim is bound to an inspected Program, no guest
is executed, and there is no mutable image, frame restart, general debugger or package browser here.
