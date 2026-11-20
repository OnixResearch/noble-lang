# Reconciliation: owner packet items 2 (FD set) and 6 (status line), input integrity, and the aggregate-result disclosure

This record reconciles the owner-requested corrections of 2026-11-20, the
source-only input-integrity binding and the source-only aggregate-result
disclosure against `.cairn/specs/c-backend/spec.md`. It is a document
comparison, not owner acceptance, review approval, native execution or case
evidence.

## Basis (after the round-1 review)

The round-1 review is `review-2026-11-20-fd.md` (verdict REVISE). This record
and the delta were revised for its findings; the round-1 hashes are therefore
superseded. Current values:

| Artifact | BLAKE3 |
|---|---|
| canonical `.cairn/specs/c-backend/spec.md` (before sync) | `7475077ae533fd4795c790e90583fdaf73e60f0d4fe2db42639ca225898baff9` |
| delta `.cairn/changes/c-backend/specs/c-backend/spec.md` | `c01cf771988f42209fc3727983e5e1167b6c835782612a35b815b59b54c8f77a` |
| dry-run `accepted_before_hash` | `7475077a…baff9` (equals canonical above) |
| dry-run `delta_hash` | `c01cf771…8f77a` |
| dry-run `expected_after_hash` | `2cb73f8618b186f2b5075db39dd4485f11eb03b9aeaa98a50079e6311d53ae9d` |
| dry-run `plan_hash` | `deb2da2a1cf637cdc7ebc469d3a35d05cc4ab7ab4c4cf6dcbc3c5a35a8436cd6` |

The dry run is unblocked with eleven `modified` operations. Block hashes:
CB-SANDBOX-01 `a36ffbe0…48ec2d` (unchanged), CB-BUILD-01 `312add93…b7381`,
CB-ADMIT-01 `050294b9…79159`.

A mechanical comparison of every delta block against canonical, excluding the
generated `<!-- cairn:scenario-links:… -->` regions and `## ` headings,
reports: nine blocks identical; CB-SANDBOX-01 exactly one changed line;
CB-BUILD-01 and CB-ADMIT-01 one appended paragraph each.

## Item 2: CB-SANDBOX-01 open-FD enumeration

Owner answer: **Revise**. "A later, separately reviewed canonical edit aligns
¶1's FD enumeration with ¶2 (protocol and output FDs), because the runtime
writes stdout and stderr."

Exactly one phrase changes in ¶1's single sentence:

- before: `closes inherited file descriptors except explicitly enumerated private request/response protocol FDs`
- after: `closes inherited file descriptors except explicitly enumerated private request/response protocol and output FDs`

The emitted runtime writes the normal result to stdout
(`crates/noble-cli/src/c_backend.rs:535`, `fputs(output, stdout)`) and every
failure to stderr (`crates/noble-cli/src/c_backend.rs:67`,
`fprintf(stderr, …)`), so ¶2's existing "fixed private protocol/output FDs"
enumerates a class ¶1 previously omitted. The other ¶1 sentences and every ¶2
sentence are unchanged. No other canonical sentence may change for this item.

## Item 6: non-normative document status line

Owner answer: **Revise** — "include Unit/Text results and internal
aggregates." The round-1 review found the first wording went beyond that
authorization and contradicted the not-yet-synced CB-BUILD-01 sentence; the
line therefore uses the owner's scope exactly:

- before: `Status: Proposed C-Backend-Draft contract; restricted inert I64/Bool source emission exists, while native compilation/execution, sandbox, conformance evidence and backend proof remain absent`
- after: `Status: Proposed C-Backend-Draft contract; restricted inert I64/Bool/Unit/Text-result source emission with internal Pair/Sum/List aggregates exists, while native compilation/execution, sandbox, conformance evidence and backend proof remain absent`

Every word is checked against the source-only emitter:

- **I64/Bool/Unit/Text results**: `controls` admits exactly one such result
  (`crates/noble-cli/src/c_backend.rs:686`, `result_text`), and the emitted C
  renders all four.
- **internal Pair/Sum/List aggregates**: `pair`/`unpair`, `inl`/`inr`/`case`
  and `nil`/`cons`/`list.case` lower through the tagged aggregate arena.
- **restricted inert**: the route emits C source and a manifest only.
- **native compilation/execution, sandbox, conformance evidence and backend
  proof remain absent**: `--native` refuses
  (`crates/noble-cli/src/c_backend.rs:556`), `execute --target` refuses
  (`crates/noble-cli/src/main.rs:318-326`), the manifest records
  `"native_artifact": null` and `"source_to_elf_admitted": false`, and no CB
  case has executed or been promoted.

The line is canonical preamble, so it is a direct canonical edit; a delta
merge leaves it unchanged. Nothing normative changes.

## Aggregate-result disclosure (CB-BUILD-01)

The source-only emitter accepts one checked `Pair`, `Sum` or `List` result
over `I64`/`Bool`/`Unit`/`Text`/nested aggregate elements and renders it as
bounded JSON value text in the selected runtime's encoding — `I64` decimal
string, `Bool` boolean, `Unit` `{"type":"Unit","value":null}`, `Text`
escaped string, `Pair` an ordered two-element array, `Sum` with its
`left`/`right` variant, `List` an ordered array — matching
`crates/noble-cli/src/core/worker_program_transition.rs:332-380`. The C
rendering is `emit_value` (`crates/noble-cli/src/c_backend.rs:483`) with a
depth bound and the output-byte quota. The route remains inert; no aggregate
result has been executed or compared to selected Wasm.

The canonical CB-BUILD-01 result-form sentence (`Final Pair/Sum/List/… results
… still refuse`) is the source-only WIP text the `8c6ab3d0` mid-change sync
applied; the delta appends a paragraph that dates that text, records the
current result forms and refusal set, and states the absence of native or
differential evidence.

## Input integrity (CB-ADMIT-01)

The canonical CB-ADMIT-01 paragraph describing final-component-only reads
with no sealing and re-resolved outputs is the state the `8c6ab3d0` sync
applied. The delta appends a paragraph dated to that sync and records the
current source-only binding: a component-by-component walk from `/` without
following symlinks, lexical refusal of relative/`.`/`..`/empty/non-UTF-8
components, refusal of non-regular, multiply-linked and stage-oversized
inputs, a memfd snapshot sealed with `F_SEAL_WRITE`/`F_SEAL_SHRINK`/
`F_SEAL_GROW`/`F_SEAL_SEAL` read back for the build, a device/inode/link/size/
mtime/ctime recheck of the leaf, and output writing through the created
directory descriptor with `O_CREAT|O_EXCL|O_NOFOLLOW`. It also states what
remains open: the sealed descriptor is released after the read, the recheck
compares identity (not in-place rewrites), no trusted anchor is selected and
no native bytes are admitted, compared or launched.

## Post-sync verification to record

After the reviewed sync, this record is extended with the applied canonical
BLAKE3 and a sentence-level comparison against `7475077a…`, which must show
only the item-2 phrase, the CB-BUILD-01 and CB-ADMIT-01 insertions, and
regenerated scenario links/headings; a second view regeneration that is
byte-stable; `cairn validate`; and the document self-tests. The round-2
review record is separate from `review-2026-11-20-fd.md`.

## Non-claims

This is a document reconciliation. It is not the owner's acceptance of the C
backend, native GO, `CB-CASE` evidence, a C/Wasm differential, or permission
to archive. `--native` and `execute --target c` continue to refuse.
