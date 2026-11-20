# CB delta reconciliation (task 2)

Scope: the eleven active `## MODIFIED Requirements` blocks in
`specs/c-backend/spec.md` in this change, reconciled against the held
canonical requirement text and its generated scenario links in
`.cairn/specs/c-backend/spec.md`.

Method: a dry-run `cairn sync c-backend --root .` preview only; no
`--execute`, no archive, no canonical edit. The preview's per-requirement
operations, `accepted_before_hash` (blake3 of the canonical document),
`delta_hash` and `expected_after_hash` were reproduced independently from the
two documents, so the tables below describe exactly what a sync would write.
The current pinned Cairn merge replaces a `MODIFIED` requirement block with
the delta block verbatim; it does not merge prose or regenerate scenario
links.

## Summary

Classification is the state **before** this reconciliation. "Delta stronger"
means every canonical sentence survives byte-identical and the delta only
adds reviewed text. "Both changed" means a sync would replace canonical
sentences. No block was delta-weaker: no canonical obligation was missing
from any delta.

| Requirement | Canon. chars | Delta chars (before) | Classification (before) | Action (after) |
|---|---:|---:|---|---|
| CB-SCOPE-01 | 1731 | 1731 | identical | identical; generated region removed |
| CB-BUILD-01 | 3087 | 5497 | delta stronger (canonical verbatim + 1 paragraph) | unchanged prose; generated region removed |
| CB-IDENTITY-01 | 1408 | 1408 | identical | identical; generated region removed |
| CB-NUM-01 | 887 | 887 | identical | identical; generated region removed |
| CB-VALUE-01 | 1116 | 1116 | identical | identical; generated region removed |
| CB-ORDER-01 | 697 | 697 | identical | identical; generated region removed |
| CB-LIMIT-01 | 1469 | 1717 | both changed (one status sentence replaced, one obligation added) | canonical verbatim + 2 inserted sentences |
| CB-HOST-01 | 1426 | 1426 | identical | identical; generated region removed |
| CB-SANDBOX-01 | 2954 | 3296 | both changed (two canonical sentences replaced by stronger clauses) | canonical verbatim + 2 inserted sentences |
| CB-ADMIT-01 | 2345 | 2808 | delta stronger (canonical verbatim + 1 paragraph) | unchanged prose; generated region removed |
| CB-GATE-01 | 1510 | 1510 | identical | identical; generated region removed |

After the fix, seven blocks are byte-identical to the canonical block and the
other four differ only by inserted sentences; a sentence-level diff reports
no deleted or replaced canonical sentence in any block.

## Exact differences before the fix

### CB-BUILD-01 (no loss)

Canonical paragraphs 1-4 are reproduced verbatim; one paragraph of 16
sentences is inserted after them. It records the nonselected source-only
route: no runtime inputs, definitions or host contracts; one
I64/Bool/Unit/Text result; `stack_slots`/`steps`/`output_bytes` plus an
optional exact `list_length` 0..255 (default 255); `expected_interface`
comparison against source preparation and kernel acceptance; internal
Pair/Sum/List lowering with checked shapes and a 256-slot arena; effective
list bound retained in the manifest and emitted C; recursive duplicate JSON
member refusal; and the explicit statement that this is not native
compilation, admission, sandbox evidence or CB-CASE-01/06 observation.

### CB-LIMIT-01 (fixed)

Canonical sentence (still present in the held canonical):

> CB-CASE-01 retains its supplied list bound and remains OPEN while the
> restricted source-only emitter refuses it.

Delta previously replaced it with:

> CB-CASE-01 retains its supplied list bound and remains OPEN despite
> restricted source-only emission accepting that policy field; no native
> runtime or selected-Wasm comparison has run.

and appended the duplicate-member obligation:

> Contract and policy JSON MUST reject duplicate object member names at every
> nesting level so an earlier supplied bound cannot be overwritten by a later
> value before validation.

Both sentences are correct for the current emitter, but the canonical status
clause is stale. The delta now keeps the canonical sentence verbatim and
inserts a paragraph that (a) marks that refusal clause stale, (b) records the
exact accepted range and enforcement, and (c) keeps the duplicate-member
obligation. No canonical sentence is deleted, so a sync cannot drop the
canonical `CB-CASE-01` OPEN sentence; the review packet flags the stale
clause as an owner decision.

### CB-SANDBOX-01 (fixed)

Canonical sentence replaced by the delta before the fix:

> Hostile direct `openat`, `connect`, `clock_gettime`, `execve`, inherited-FD
> and escape attempts MUST refuse or terminate with zero protected
> operations, with a benign authorized `test.emit` control under the same
> confinement.

The replacement narrowed "escape attempts" to "namespace/sandbox escape
attempts" and split the benign-control clause into a separate sentence, so a
sync would have dropped the unqualified escape obligation.

Second canonical sentence replaced before the fix:

> Seccomp denial alone does NOT stop vDSO clock calls or x86 `rdtsc`; without
> an owner-selected and verified VM/virtualized-clock or equivalent clock/TSC
> and loader barrier, native execution MUST remain unavailable.

The replacement strengthened it to "direct vvar reads or x86
`rdtsc`/`rdtscp`" and "complete clock/vvar/TSC", but again by replacing the
canonical wording.

The delta now keeps both canonical sentences verbatim and adds one inserted
sentence to each paragraph:

> Hostile vDSO `__vdso_clock_gettime`, `__vdso_time`,
> `__vdso_gettimeofday`, coarse-clock, direct-vvar and x86 `rdtscp` attempts
> likewise MUST refuse or be demonstrably contained without host-time or
> elapsed-time disclosure; trapping or virtualizing only the TSC is
> insufficient, and a vDSO-only or coarse-clock-only mechanism is not
> equivalent.

> The barrier MUST also cover direct vvar reads and x86 `rdtscp`; a
> vDSO-only, coarse-clock-only, TSC-only or offset-TSC mechanism does not
> satisfy it.

### CB-ADMIT-01 (no loss)

Canonical paragraphs 1-2 are reproduced verbatim; one three-sentence
paragraph is added. It states that the restricted source-only emitter is not
CB-ADMIT-01 admission, that its final-component `O_NOFOLLOW` reads do not
authenticate ancestors or seal inputs and its output child paths are
re-resolved after directory creation, and that the gap remains open until a
separately admitted descriptor-bound path exists.

## CB-LIMIT-01: the exact supplied limit

Emitter facts checked in `crates/noble-cli/src/c_backend.rs`:

- `limits` accepts exactly `stack_slots`, `steps`, `output_bytes` and an
  optional `list_length`; any other field count refuses.
- A supplied `list_length` is read as `u64`; absent means default 255; values
  above 255 refuse before emission (0 is valid).
- The effective value is emitted as `#define LIST_LIMIT` and recorded in
  `manifest.runtime_limits.list_length` with the internal
  `list_length_cap: 255`.
- `cons` checks `previous->length == LIST_LIMIT` before growth and fails with
  `list length exhausted`; element/length reads are also bounded by
  `LIST_LIMIT`.

Canonical obligation retained verbatim: the supplied bound is an exact policy
value, must be retained in the manifest/build subject, must be enforced
before list growth, and a profile that cannot honour it must refuse rather
than strip or advertise it. The delta adds only the stale-clause correction
and the duplicate-member obligation.

CB-CASE-06 remains a design only: its `quota_controls` supply
`list_length: 1` (expects normal stack `21`) and `list_length: 0` (expects
`list-quota-exhausted-before-cons`), and its expected
`manifest_list_length_by_quota_control` is `[1, 0]`. No native execution,
manifest receipt or Wasm comparison exists, so the case stays
`absent/not-run/open/unassessed` and the requirement stays OPEN.

## CB-SANDBOX-01: sealed loader, clock and escape obligations

Canonical obligations retained verbatim:

- separate reviewed runner derivation and policy, not the Lean proof runner
  or a cached host package;
- exact sealed executable and loader/library closure mounted read-only, no
  writable executable path, all but fixed protocol/output FDs closed, ambient
  secrets and credentials cleared;
- initial ELF load ordered before a fail-closed post-load
  no-exec/no-writable-executable-mapping filter; denying `execve` before the
  load blocks launch, allowing it after permits substitution;
- unreviewed imports, loader escapes, direct file/network/process access and
  host-clock reads denied;
- seccomp insufficiency for vDSO clock calls and `rdtsc`;
- parent-owned cgroup/process-tree memory, CPU, task, output and monotonic
  wall bounds that kill and reap the worker tree; signal, crash, timeout and
  partial output cannot be success.

Added by the delta: the vDSO function family (`__vdso_clock_gettime`,
`__vdso_time`, `__vdso_gettimeofday`), coarse clocks, direct vvar reads and
`rdtscp` are named channels that must refuse or be demonstrably contained
without host or elapsed time; a TSC-only or offset-TSC barrier is
insufficient and a vDSO-only or coarse-clock-only mechanism is not
equivalent. Without a reviewed complete barrier, native execution stays
unavailable.

CB-CASE-08 remains a design only. Its attempts enumerate exactly those
channels (`vdso-clock-gettime`, `vdso-time`, `vdso-gettimeofday`,
`vdso-coarse-clock`, `direct-vvar-read`, `x86-rdtsc`, `x86-rdtscp`,
`openat`, `connect`, `clock_gettime`, `execve`, secret FD read, `clone`,
`unshare`, loader escape) plus a benign authorized `test.emit` control. No
runner exists and no probe has run.

## CB-ADMIT-01: sealed path and byte obligations

Canonical obligations retained verbatim: descriptor-relative, lexically
absolute normalized opens for source, contract, policy, current authority and
candidate; rejection of `.`/`..`, ancestor symlinks, magic links, mutable
hard links, directory retargeting and file replacement; sealed read-only
snapshots reused for acceptance, re-emission, compilation, full-ELF
inspection and launch; complete-ELF comparison under pinned flags, compiler,
linker, sysroot, startup/runtime, ABI and environment; refusal before effects
for unsupported architecture, interpreter, dynamic import, load segment,
relocation or writable/executable mapping.

The delta discloses, without claiming conformance, that the current
source-only emitter opens only the final path component with `O_NOFOLLOW`,
does not authenticate ancestor components or seal inputs, and re-resolves
output child paths after creating the output directory. Code inspection
confirms both facts. CB-CASE-05 remains an unexecuted design.

## Generated scenario links

The canonical document carries 25 generated `#### Scenario:` designs in
eleven `<!-- cairn:scenario-links -->` regions. The delta previously carried
copies of all 25 regions with paths relative to its own location. They are
generated by `tools/cairn-specs.mjs` from `specs/conformance/*.json` and are
not reviewed prose; the delta now omits them, matching a `MODIFIED` delta in
an archived change (for example
`.cairn/archive/2026-10-23-typed-worker-program-v2-native/specs/language/spec.md`,
whose requirement blocks contain no generated regions).

Consequence: a sync leaves each canonical block without its generated
region, and `tools/cairn-specs.mjs --write-views` restores all 25 designs.
That regeneration is idempotent over the canonical document and checks the
requirement identities.

## Preview and merge evidence

| Stage | Delta hash | Expected after | Operations |
|---|---|---|---|
| before fix | `9a5c9b2a779cfa378afbf853fe5c53e72d66075df022a802769eed014c0d1bb6` | `a5515aa621083bdca8564ed52976788d79b77e627e6768a4d8e9395149ef18c4` | 11 modified |
| after fix | `cdb0eae6a0b9d188d8339d16b15e36551fa0ac9f9a673b23b800797a379f7336` | `11d99f29b0b54218d40ad6804bdde0202a8bcf99711c258c9044536c58434940` | 11 modified |

Both `expected_after` values were reproduced independently from the two
documents before being trusted; the preview is a dry run and does not
authorize `--execute`.

Per-requirement delta block hashes changed for every block because the
generated region was removed; the seven identical blocks remain textually
identical to canonical prose.

Known losses if the integrator later runs `sync --execute` and only
regenerates views:

- the two canonical section headings `## Representation and execution` and
  `## Native trust boundaries` sit inside a requirement block (at the end of
  CB-IDENTITY-01 and CB-ORDER-01 respectively) and are not restored by
  regeneration; re-insert them after the sync, or move them out of the
  requirement blocks before syncing;
- the generated scenario regions are dropped by the merge and restored by
  `--write-views`.

No normative obligation is lost: the merged requirement blocks contain every
canonical sentence plus only the reviewed insertions.

## Checks

- `cairn validate --root .`: `valid: true`, zero issues, zero findings.
- `cairn sync c-backend --root .`: `blocked: false`, eleven `modified`
  operations, hashes above.
- `bun tools/cairn-specs.mjs --self-test` and
  `bun tools/check-specs.mjs --self-test`: passing (see tasks.md).

This reconciliation is document review only. It is not owner acceptance, not
`cairn sync --execute` authorization, not native GO, and not case evidence.
