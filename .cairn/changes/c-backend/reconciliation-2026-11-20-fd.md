# Reconciliation: owner packet items 2 (FD set) and 6 (status line), and the aggregate-result disclosure

This record reconciles the two owner-requested corrections of 2026-11-20 and the
source-only aggregate-result disclosure against the canonical
`.cairn/specs/c-backend/spec.md` produced by the accepted mid-change sync.
It is a document comparison, not owner acceptance, review approval, native
execution or case evidence. The independent review of this record and its
edits is recorded separately.

## Basis

| Artifact | BLAKE3 |
|---|---|
| canonical `.cairn/specs/c-backend/spec.md` (before sync) | `57cbb263475140ed53408400d6875a6c9a64d21f7e466513e0118f4f9c78c734` |
| delta `.cairn/changes/c-backend/specs/c-backend/spec.md` | `c97b3c8cc025e151d83f9dfb15f7dce0b225fd0069c738606093b0b017358a7a` |
| dry-run `accepted_before_hash` | `57cbb263…c78c734` (equals canonical above) |
| dry-run `delta_hash` | `c97b3c8c…358a7a` |
| dry-run `expected_after_hash` | `41726d7465bbc1e303ae7e74d39b3a11989b0ac7e21b8bcdc7f6b24f68aa16ed` |
| dry-run `plan_hash` | `5b1cf6b349a1982bea493bf875ed3b5c856164d9ffdb5be69964870f06ab51f4` |

The dry run is unblocked with eleven `modified` operations. `CB-SANDBOX-01`
has block hash `a36ffbe0…dd820`; `CB-BUILD-01` has block hash
`a3a3e82f…d50f3`.

## Item 2: CB-SANDBOX-01 open-FD enumeration

Owner answer: **Revise**. "A later, separately reviewed canonical edit aligns
¶1's FD enumeration with ¶2 (protocol and output FDs), because the runtime
writes stdout and stderr."

Exactly one phrase changes in ¶1's single sentence:

- before: `closes inherited file descriptors except explicitly enumerated private request/response protocol FDs`
- after: `closes inherited file descriptors except explicitly enumerated private request/response protocol and output FDs`

The emitted runtime writes the normal result to stdout
(`crates/noble-cli/src/c_backend.rs:503`, `fputs(output, stdout)`) and every
failure to stderr (`crates/noble-cli/src/c_backend.rs:66`,
`fprintf(stderr, …)`), so ¶2's existing "fixed private protocol/output FDs"
enumerates a class ¶1 previously omitted. The other ¶1 sentences and every ¶2
sentence are unchanged; a mechanical sentence-level comparison of the delta
block against canonical reports only this line. No other canonical sentence
may change for this item.

## Item 6: non-normative document status line

Owner answer: **Revise** — "include Unit/Text results and internal
aggregates."

- before: `Status: Proposed C-Backend-Draft contract; restricted inert I64/Bool source emission exists, while native compilation/execution, sandbox, conformance evidence and backend proof remain absent`
- after: `Status: Proposed C-Backend-Draft contract; restricted inert I64/Bool/Unit/Text and Pair/Sum/List-result source emission with internal aggregate construction and elimination exists, while native compilation/execution, sandbox, conformance evidence and backend proof remain absent`

Every word is checked against the source-only emitter:

- **I64/Bool/Unit/Text and Pair/Sum/List results**: `controls` admits exactly one
  result whose complete element type is encodable (`crates/noble-cli/src/c_backend.rs:686`,
  `result_text`), and refuses `Syntax` and `Program` results
  (`crates/noble-cli/src/c_backend.rs:724`).
- **internal aggregate construction and elimination**: `pair`/`unpair`,
  `inl`/`inr`/`case` and `nil`/`cons`/`list.case` lower through the tagged
  aggregate arena; the C runtime retains `OP_PAIR`/`OP_UNPAIR`,
  `OP_INL`/`OP_INR`/`OP_CASE` and `OP_NIL`/`OP_CONS`/`OP_LIST_CASE`.
- **restricted inert**: the route emits C source and a manifest only; it does
  not compile or run it.
- **while native compilation/execution, sandbox, conformance evidence and
  backend proof remain absent**: `--native` refuses
  (`crates/noble-cli/src/c_backend.rs:558`), `execute --target` refuses
  (`crates/noble-cli/src/main.rs:318-326`), the manifest records
  `"native_artifact": null` and `"source_to_elf_admitted": false`, and no CB
  case has executed or been promoted.

The status line is canonical preamble, so it is a direct canonical edit rather
than a delta edit; a delta merge leaves the line unchanged. Nothing normative
changes.

## Aggregate-result disclosure (CB-BUILD-01)

The source-only emitter now also accepts one checked `Pair`, `Sum` or `List`
result over `I64`/`Bool`/`Unit`/`Text`/nested aggregate elements and renders
it as bounded JSON value text in the selected runtime's encoding
(`{"type":"Pair","value":[…]}`,
`{"type":"Sum","variant":"left"|"right","value":…}`,
`{"type":"List","value":[…]}`), matching
`crates/noble-cli/src/core/worker_program_transition.rs:345-380`. The C
rendering is `emit_value` (`crates/noble-cli/src/c_backend.rs:484`) with a
depth bound and the output-byte quota. The route remains inert; no aggregate
result has been executed or compared to selected Wasm.

The canonical CB-BUILD-01 sentence "Final Pair/Sum/List/Syntax/Program
results, `reflect`, typed runtime inputs, named definitions and host effects
still refuse" is kept in place, as CB-LIMIT-01 keeps its stale canonical
sentence. The delta appends a paragraph that dates that sentence to
`e76d8469` and records the current result forms, their refusal set, and the
absence of native or differential evidence. The delta block otherwise equals
canonical; the mechanical comparison reports only the appended paragraph.

## Post-sync verification to record

After the reviewed sync, this record is extended with:

- the applied canonical BLAKE3 and a sentence-level comparison against
  `57cbb263…`, which must show only the item-2 phrase, plus the CB-BUILD-01
  insertion and regenerated scenario links/headings;
- a second view regeneration that is byte-stable;
- `cairn validate` and the document self-tests.

## Non-claims

This is a document reconciliation. It is not the owner's acceptance of the C
backend, native GO, `CB-CASE` evidence, a C/Wasm differential, or permission
to archive. `--native` and `execute --target c` continue to refuse.
