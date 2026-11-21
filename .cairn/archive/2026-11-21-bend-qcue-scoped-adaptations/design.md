## Context

M3 selected bounded managed-linear-memory Wasm, M6 selected native async
Component Model ownership and retirement, DXM1 selected only finite
two-constructor source variants, and MC2 accepted scoped resource-free pure
contracts. The original ND-56–58 exclusions remain in force. Bend revision
3378e6237ed431d17629efd36d24c96241815b7e and qcue revision
c71183b0a5d3319dde1d611b6e6f77a6eab29418 are comparative sources,
not implementation dependencies or acceptance evidence.

## Decisions

### Decision: Separate contract, optional experiment and excluded mechanisms

**Choice:** Bind owner-approved immutable law identity and independently
verified exact theorem/admission to a release policy without transferring
proof authority to a candidate author. Give Result a schematic ordinary
variant library contract while generic declaration syntax and resource
profiles remain gated. Measure finite ADT lowering only as a separately
recorded backend trial. Gate any pure fork/join study on explicit, fully
owned, bounded, resource-free subprograms and deterministic ordered join.
Use qcue only for finite independent oracle design and reproducible negative
controls in that optional study.

**Rationale:** The existing kernel and component boundary already control
sequential effects, resource retirement and certification; adding another
executor, proof kernel, or CUE refinement semantics would create conflicting
authority. An independent finite model can detect scoped regressions but
cannot discharge a universal Lean theorem.

## Open lifecycle and admission gates

The delta requirements have already been synced into canonical native specs.
Keeping the change active records implementation and measurement work still
open; re-syncing the same added requirements would not be a new selection.
The current case ledgers, not checked specification tasks, determine
implementation, execution, proof and trust outcomes.
Read-only Cairn sync preview now reports `delta_merge.add_conflict` for
already-present `VC-OWNER-01`, `BE-LAYOUT-01` and `DX-PURE-PAR-01`; that is
the expected result of replaying their original `ADDED` deltas, not a
request to change those historical additions to `MODIFIED`. Never execute
this duplicate sync. Cairn native validation and the tasks gate accept
the open change; archive preview cannot complete while implementation and
evidence tasks remain unchecked. Future reviewed spec changes must use genuinely new deltas
against the then-current canonical text rather than reapply this snapshot.

- CONTRACT-16: implemented/passed, proof open. The owner-law records were
  frozen on the owner's instruction (decision 11; commit 12b52f9a; digests
  `9a8eda12…`/`51f2a315…`) before the release consumer (e77c87e5).
  `verification/owner-law/gate.mjs` passed at f1b8ae87
  ([receipt](../../../verification/owner-law/acceptance.json), sha256
  `61378e56…`): it refuses the four hostile variants with distinct codes,
  admits the law-conforming v1 release and the owner-approved v2 revision after
  independent recheck, and leaves ordinary compile/verify unchanged. Owner-law
  independence rests on that custody, not on an observation; the release route
  starts no guest, so its request counters are constant zeros. Ordinary
  proof-free execution is unchanged. OWNER CONFIRMATION 2026-11-21: "accept"
  to review item C2, so this custody-based independence stands.
- DX-04: the separately completed Result library gate covers generic
  schema/versioned signatures and ordinary checked selected-arm callbacks,
  including compiled bypassed effectful callbacks with zero requests.
  Resource-positive execution still needs its own owner/retirement gate.
- DX-13: the optional methodology review passed against pinned qcue source
  `c71183b0a5d3319dde1d611b6e6f77a6eab29418` with ten method controls,
  five exclusions and concrete false-law, false-premise, shrink, identity,
  recipe/effect and proof-authority counterexamples. The immutable
  [review receipt](../../../verification/dx13/acceptance.json) binds its
  prepromotion source inputs. No actual Noble finite oracle ran; no model
  agreement, owner-approved law, Lean theorem or release admission follows.
  A later selected model execution requires its own source-bound evidence.
- ADAPT-17: selected and executed; implemented/passed, proof open. The frozen
  paired trial at clean commit 6b1b1936 recorded 16 workload cells and 52
  controls (2,562 checks passed). The frozen decision is NO-GO:
  runtime-captured-quote-and-composition/opt-off regressed 20.00%, above the 5%
  ceiling. The default backend is unchanged, process memory is reported
  unsupported, and separate WIT/resource-positive evidence remains open.
- ADAPT-18: defer any optional pure join run until branch/schema interfaces,
  DX-04 Result-error prerequisites, independent pure admissibility, exact
  precharged bounds, and deterministic settlement are available. It cannot
  reuse M6 task ownership or count unsettled work as retired.
  **Disposition (owner decision, 2026-11-20): deferred and unexecuted.**
  DX-04 has since passed; no fork/join construct or branch-schema gate exists.
  ADAPT-18 stays `absent`/`not-run` until an owner selects it after those
  gates exist.

None of these conditional experiments changes existing milestone dependency
edges. A selected slice needs real tests, source-bound extraction/refinement
accounting where applicable and independent evidence before its own case or
status may be promoted; unselected slices stay visible and unexecuted. Final
closeout requires an explicit reviewed disposition of all five design lanes,
not a second spec-only archive or a document-validation pass.

## Risks / Trade-offs

- A law edited by the same candidate or stale after schema/policy changes
  could falsely pass: require exact separate law identity and independent
  consumer checks at release.
- A faster layout may change allocation failures or observable recipes:
  compare explicit quotas/traps, semantic identity and WIT owner transitions,
  not just successful output or speed.
- Parallel scheduling may change effect order or hide retained work: only
  explicitly admissible resource-free pure branches enter a measured study.
- qcue's raw source export/reimport can leave an attached live-interface
  conformance proof pending; exclude its live-refinement design entirely.

## Historical receipt accounting

Regenerating `specs/VALIDATION.json` corrects pre-existing count drift:
its prior 78 executed scenarios omitted already-passed DX-03, DX-08 and DX-09
from the Declared-Modules-v1 source-bound case records
(`sha256:e1098bfb81f9d05d4d648705235f31505218a4d789571ba342b9c5209a68ad65`,
`verification/declared-modules-v1/acceptance.json`). That historical refresh
promoted none. The separately recorded DX-13 methodology review is now passed;
at that refresh CONTRACT-16, ADAPT-17 and ADAPT-18 were `absent`/`not-run`.
ADAPT-17 has since executed and passed with a frozen NO-GO result
(`verification/adapt17/receipts/2026-11-13-full.json`); ADAPT-18 is deferred
(above). CONTRACT-16 has since passed its owner-law release gate
(`verification/owner-law/acceptance.json`), proof open. The review has not
executed a Noble oracle.

`verification/dx13/review.mjs` and `verification/result-library/plan.mjs` read
this change's documents as live inputs, so under owner decision 4 they replay
only at their recorded source state. `verification/dx13/postpromotion.mjs` does
not pass at any committed state: its receipt's native-spec (`6301b19e…`) and
design (`27ea7df7…`) inputs were never committed, `specs/DECISIONS.md` changed
at 912aedf9, and STATUS `unexecuted_cases` no longer lists CONTRACT-16 after
62a24094; it is historical-only (integrator decision 23 under the owner's
delegation, 2026-11-20; OWNER CONFIRMATION 2026-11-21: "yes" to review item
C1). The DX-04 receipt's `build.git_head` a46f588a is the
base of an uncommitted worktree, not the built source (its source identity is
only the content hash `sha256:16f9d850…`), and its external raw-artifact
directory is no longer retained.
