# Opt-in C backend candidate tasks

## Specification

- [x] [serial] Draft distinct nonselected SPEC-BE002 Core-Bootstrap C11 AOT contract and active change delta, without changing selected Wasm/Node/V8, historical evidence or proof status. r[CB-SCOPE-01] r[CB-BUILD-01]
- [x] [serial] Author explicit unexecuted CB-CASE-01 through CB-CASE-10 positive and hostile scenario designs, each with `absent/not-run/open/unassessed` state and no evidence. r[CB-GATE-01]
- [x] [serial] Integrate new family and scenarios in the shared manifest; register roadmap/status pending dependency after independent Core-Bootstrap acceptance; refresh generated compatibility views/scenario clauses and ledger after concurrent live-Wasm source handoff. Shared owner observed local rendering and ledger checks. r[CB-SCOPE-01] r[CB-GATE-01]
- [x] [serial] Run local `tools/cairn-specs.mjs --write-views` and `tools/check-specs.mjs --refresh-ledger --self-test --report` after integration; shared owner reports both passing (13 specs/187 cases, 521 requirements/229 controls). These document checks are not runtime or proof evidence. r[CB-GATE-01]
- [x] [serial] Validate integrated native Cairn documents using the verified direct cached Cairn binary and matching generated policy, not the Nix wrapper: the shared owner observed `valid:true`, `issues:[]`, `findings:[]` over 13 changes and 41 specs after final local view/ledger checks. This is document validation only, not C execution, backend proof, acceptance or production selection. The earlier wrapper invocation unexpectedly triggered Nix auto-GC and is not the successful route. r[CB-GATE-01]

## Remaining native implementation and evidence gates

This isolated WIP adds only an inert, nonselected, checked source emitter for a
restricted effect-free I64/Bool/Unit/Text-result Core subset plus one checked
Pair/Sum/List result, including bounded internal Pair/Sum/List
construction/elimination, exact checked local
type-shape preflights, immutable aggregate captures, quotation/compose/run
and pure `dip`/`if` code. It accepts an optional exact supplied
`limits.list_length` from 0 to 255 (default 255), retaining the effective
bound in the manifest and emitted C; this is not native quota execution.
The development C11 syntax-only diagnostic is not native execution,
compiler correspondence or C/Wasm acceptance. The requirements below
remain open; no CB case is promoted.
Text payloads bind exactly to checked source-body node identities; recursive
contract/policy duplicate member names refuse before source-only emission.
All eleven active MODIFIED delta bodies are reconciled against the current
canonical text and its 25 generated scenario designs in `reconciliation.md`:
seven bodies are byte-identical to canonical, while CB-BUILD-01,
CB-LIMIT-01, CB-SANDBOX-01 and CB-ADMIT-01 are canonical text plus only
reviewed insertions, without relaxing the first-parent bound, clock or
descriptor requirements. The delta omits the generated scenario-link
regions; regeneration restores them on the canonical document. This is not
owner approval, and it does not authorize an unreviewed sync while the eight
remaining gates are undone. A reviewed mid-change sync that applies the
delta without archiving is the owner's decision in the packet; no gate is
closed by it.

Document checks at the reconciled commit on 2026-11-20: `cairn validate`
0 issues and 0 findings over 21 changes; the `cairn sync c-backend` dry run
is unblocked with eleven `modified` operations; `cairn-specs.mjs
--self-test` passes 26 checks and `check-specs.mjs --self-test` passes 230.
These are document checks, not runtime, native or proof evidence.

OWNER DECISION 2026-11-20: c-backend: do the source-only slices (input
integrity, emitter coverage, reconciliation of the eleven MODIFIED blocks,
a review packet). The owner also authorizes a pinned QEMU TCG VM lane from
nixpkgs (icount, fixed clock) and sealed hostile native test execution
inside it. The canonical-edit review ("explicit independent owner review
... before integration") still gets an owner yes/no on the prepared packet
after an independent reviewer agent pass.

This authorizes building and sealed testing of that lane. It does not select
the QEMU release, guest kernel, image, machine model or clock pins (task 4),
does not admit a runner (task 7), and is not native GO: `--native` and
`execute --target c` continue to refuse, and canonical C text and every CB
case remain on HOLD.

CB-CASE-01 still supplies `limits.list_length: 2`; the current exact
I64/Bool/Unit/Text-result source-only policy now accepts and preserves that
field, including for scalar source. The selected native compiler, runner,
complete case inputs and actual C/Wasm differential are absent, so this
cannot count as native runtime or CB-CASE-01 evidence. The supplied bound
cannot be silently dropped or advertised as an unenforced no-op.

- [x] [serial] Obtain explicit independent owner review of all direct canonical CB-BUILD-01/CB-LIMIT-01/CB-SANDBOX-01/CB-ADMIT-01 edits and the aligned CB-CASE-06/08 designs **before integration**; document the reviewed safety obligations and any rejection. This source-only corrective commit is not owner acceptance, native GO or permission for `cairn sync --execute`. r[CB-BUILD-01] r[CB-LIMIT-01] r[CB-SANDBOX-01] r[CB-ADMIT-01]
  Packet prepared 2026-11-20 in `review-packet.md` (canonical edit diffs, case designs, obligations, risks, Accept/Reject/Revise checklist); the box stays unchecked until the owner answers after the independent reviewer pass. Record the item-2 answer together with the packet's reading that ¶1 "candidate instructions" and ¶2 "untrusted instructions" are the same boundary, so the accepted or rejected interpretation is durable.
  Independent review (owner decision 1 of 2026-11-20): a fresh-context reviewer agent that did not write the packet reviewed it in three rounds. Round 1 returned REVISE (two major findings: plain `rdtsc` dropped, and the CB-SANDBOX-01 timing narrowing presented as neutral wording) and round 2 ACCEPT WITH NOTES. Round 3 confirmed `3199fde1` as **ACCEPT**. The record is `review-2026-11-20.md` in this change.
  OWNER ANSWERS 2026-11-20 on `review-packet.md` at `3199fde1`:
  - Item 1, CB-LIMIT-01 supplied-limit obligation: **Accept**. The reworded delta insertion corrects the stale canonical status sentence, which stays in place.
  - Item 2, CB-SANDBOX-01 obligations: **Accept**.
  - Item 2, `e76d8469` timing relaxation: **Accept**, together with its reading. ¶1 "candidate instructions" and ¶2 "untrusted instructions" name the same boundary: after pinned loader/startup setup and before the admitted program's own instructions. Pinned startup/runtime code may run before the post-load filter; no guest computation may.
  - Item 2, FD set: **Revise**. A later, separately reviewed canonical edit aligns ¶1's FD enumeration with ¶2 (protocol and output FDs), because the runtime writes stdout and stderr. Tracked as a task below.
  - Item 3, CB-ADMIT-01 path, snapshot and complete-ELF obligations: **Accept**, as an obligation. The shipped nonselected `noble build --target c` does not yet conform.
  - Item 4, the four delta insertion blocks: **Accept**.
  - Item 5, CB-CASE-06/08 designs: **Accept**. They stay unexecuted, and CB-CASE-06's partial quota policies must be completed before any run.
  - Item 6, non-normative status line: **Revise**. A separate correction is authorized to include Unit/Text results and internal aggregates. Tracked as a task below.
  - Item 7, integration: **Accept**. The integrator ran the intentional mid-change `cairn sync c-backend --execute` (11 requirements applied). The canonical file's BLAKE3 equals the reviewed expected-after hash `e2b67dc4087dbf73b245e91bf4f525e8091bfbb86ad3c7aa10564435ee7493d2`. Views were regenerated, restoring all 25 scenario designs, and the two section headings were put back; a second regeneration is byte-stable. A sentence-level comparison found no pre-sync canonical sentence missing. The change stays active and nothing is archived. Native execution, pins, a runner or VM selection, and any CB case promotion are NOT authorized by these answers.
- [x] [serial] Reconcile each of the eleven active MODIFIED delta blocks against the current canonical requirement and scenario links before any `cairn sync --execute` or archive; a passing structural preview alone does not authorize replacing stronger canonical prose. Review CB-LIMIT-01's exact supplied limit and CB-SANDBOX-01/CB-ADMIT-01's sealed loader, clock and path obligations individually. r[CB-LIMIT-01] r[CB-SANDBOX-01] r[CB-ADMIT-01]
  Reconciliation completed 2026-11-20 in `reconciliation.md`: eleven blocks (seven identical, four canonical plus reviewed insertions), exact differing sentences, CB-LIMIT-01 supplied-bound code facts, CB-SANDBOX-01 clock/vvar/TSC and loader obligations, CB-ADMIT-01 descriptor/sealed-byte obligations, and dry-run preview hashes. The box stays unchecked pending the owner's yes/no on `review-packet.md`; the integrator records it after that.
  Closed 2026-11-20. The reviewer accepted the reconciliation in round 3. Its own reimplementation of the pinned merge reproduced the reviewed delta `ebb57a7a…` and expected-after `e2b67dc4…`. The owner then accepted the packet, and the sync applied exactly that result (see task 1).
- [ ] [serial] Owner packet item 2 "Revise": align CB-SANDBOX-01 ¶1's open-FD enumeration with ¶2 (protocol and output FDs) through a delta edit. Reconcile it against the post-sync canonical text, have it independently reviewed, then sync it. No other canonical sentence may change. r[CB-SANDBOX-01]
  Progress 2026-11-20: the delta edit changes exactly the phrase `except explicitly enumerated private request/response protocol FDs` to `… protocol and output FDs`; a mechanical block comparison reports one changed line in CB-SANDBOX-01 and no other canonical sentence change. The reconciliation is `reconciliation-2026-11-20-fd.md`; the current dry run is unblocked with eleven `modified` operations (`accepted_before 7475077a…baff9`, `delta c01cf771…8f77a`, `expected_after 2cb73f86…ae9d`, `plan deb2da2a…36cd6`). Round 1 of the independent review (`review-2026-11-20-fd.md`) accepted this item's exact-one-phrase property but returned REVISE for other findings; the round-2 fixes are applied and a round-2 review plus the sync are still pending, so the box stays unchecked.
- [ ] [serial] Owner packet item 6: correct the non-normative document status line so it also lists Unit/Text results and internal aggregates. Check every word against `crates/noble-cli/src/c_backend.rs`, have the edit independently reviewed, and change nothing normative. r[CB-BUILD-01]
  Progress 2026-11-20: the canonical status line now lists `I64/Bool/Unit/Text` results and internal `Pair`/`Sum`/`List` aggregates, using the owner's item-6 scope after round 1 found the first wording exceeded it. Every word is checked against `crates/noble-cli/src/c_backend.rs` in `reconciliation-2026-11-20-fd.md`; nothing normative changes. Unit/Text results and internal aggregate construction/elimination exist, `--native`/`execute --target c` refuse and the manifest records no native artifact. Round 1 returned REVISE (the wording was reverted); round-2 review and the sync are pending, so the box stays unchecked.
- [ ] [serial] Bring the restricted source-only build into full input-integrity conformance before counting CB-BUILD-01/CB-LIMIT-01/CB-ADMIT-01: recursive duplicate JSON member rejection and exact supplied `limits.list_length` retention/source-only C enforcement are present, but native admission still needs trusted ancestor-anchored sealed input/output binding and independently verified execution. Until then this emitter remains nonadmitted and `--native`/`execute --target c` refuse. r[CB-BUILD-01] r[CB-LIMIT-01] r[CB-ADMIT-01]
  Progress 2026-11-20 (`8a0ec89e` and its round-2 follow-up, `crates/noble-cli/src/c_backend/inputs.rs`): the descriptor-anchored binding is implemented for this source-only build. Each input path must be lexically absolute with only non-empty, non-`.`, non-`..` UTF-8 components; it is walked from `/` one component at a time with `O_PATH|O_DIRECTORY|O_NOFOLLOW`, so an ancestor or leaf symlink refuses (`is a symlink; C build inputs are opened without following symlinks`), as do a multiply-linked leaf (`multiply-linked input refused by the source-only rule`), a non-regular file, an oversized file and a path deeper than 128 components. The bytes are copied into a memfd sealed with `F_SEAL_WRITE|F_SEAL_SHRINK|F_SEAL_GROW|F_SEAL_SEAL` and read back, and the leaf's device, inode, link count, size and timestamps are rechecked after the snapshot. The output directory is created with `mkdirat` under the walked parent and every artifact is written through the created directory descriptor with `O_CREAT|O_EXCL|O_NOFOLLOW`, never by re-resolving the output path. `INPUTS_SOURCE` joins the build subject and the manifest records `input_integrity_source_sha256`. Measured: `cargo test -p noble-cli --test c_backend` passes 17 tests, including the five `integrity::*` tests; the same probes against the base binary at `6aa08447` are accepted (relative path, `.` component, ancestor symlink, hard link, symlinked output parent), so the controls are failing-first. The box stays unchecked: trusted-anchor selection, in-place-rewrite detection, the `O_RDONLY`-to-`O_PATH` anchor decision and independently verified execution remain open, so the emitter remains nonadmitted and `--native`/`execute --target c` refuse.
- [ ] [serial] Obtain owner-reviewed opt-in C compiler/runtime/runner/filter/mediator pins from the selected `octet/nixpkgs`, a feasible no-clock/no-exec loader boundary (VM or demonstrated equivalent), deterministic complete-ELF correspondence and explicit authorization for sealed native test execution. Cached host bubblewrap and the Lean proof sandbox do not satisfy this prerequisite. r[CB-SANDBOX-01] r[CB-ADMIT-01]
- [ ] [serial] Extend the existing restricted inert C11 source emitter to the complete independently accepted Core-Bootstrap contract, pinned x86_64 Linux compilation, exact C-source and native-ELF admission, with no ordinary CLI/session fallback or source evaluator. r[CB-BUILD-01] r[CB-ADMIT-01]
  Source-only progress 2026-11-20 (`8a0ec89e`): the restricted emitter now accepts exactly one checked `Pair`, `Sum` or `List` result over `I64`/`Bool`/`Unit`/`Text` or nested aggregates and renders it as bounded JSON value text in the selected runtime's value encoding (`Pair` two-element array, `Sum` with its variant, `List` ordered array) through `emit_value` with a depth bound and the output-byte quota; `Syntax` and `Program` results still refuse. The delta's `CB-BUILD-01` insertion records the result forms, the dating sentence and the absence of native or differential evidence. Pinned compilation, complete-ELF correspondence and native admission remain undone, so the box stays unchecked.
- [ ] [serial] Implement checked C11 value/arithmetic/order/runtime, immutable compiled `Program` values and bounded typed `test.emit` ABI; prove unsupported paths and exhaustion classifications through focused controls. r[CB-NUM-01] r[CB-VALUE-01] r[CB-ORDER-01] r[CB-LIMIT-01] r[CB-HOST-01]
  Partial 2026-11-20 (`8a0ec89e`): the source-only runtime has the checked unsigned I64 arithmetic and order primitives, immutable static/dynamic capture recipes, tagged aggregate values and the aggregate result rendering; every quota message contains `exhausted` and every failure is classified as `exhausted` or `trap`, and the control tests exercise the refused interfaces, effects, definitions and bad policies. The bounded typed `test.emit` ABI, mediated host authority and native `Program` values belong to the blocked native lane, so the box stays unchecked.
- [ ] [serial] Pin and verify a genuine native sandbox runner, including hostile direct syscalls, FD, namespace escape and loader controls plus sealed test-only AT_SYSINFO_EHDR vDSO, coarse clock, direct vvar, `rdtsc` and `rdtscp` probes under fixed memory/CPU/wall bounds; a TSC-only barrier is insufficient and absent sandbox blocks execution. r[CB-SANDBOX-01]
- [ ] [serial] Execute source-bound actual C/native versus selected Node/V8 Wasm differential, sanitizer/ABI and artifact-adversary controls. Record the C-specific open PO-17/18/native refinement and make a distinct later acceptance/selection decision only on real evidence. r[CB-GATE-01]
