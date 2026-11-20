# Opt-in C backend candidate tasks

## Specification

- [x] [serial] Draft distinct nonselected SPEC-BE002 Core-Bootstrap C11 AOT contract and active change delta, without changing selected Wasm/Node/V8, historical evidence or proof status. r[CB-SCOPE-01] r[CB-BUILD-01]
- [x] [serial] Author explicit unexecuted CB-CASE-01 through CB-CASE-10 positive and hostile scenario designs, each with `absent/not-run/open/unassessed` state and no evidence. r[CB-GATE-01]
- [x] [serial] Integrate new family and scenarios in the shared manifest; register roadmap/status pending dependency after independent Core-Bootstrap acceptance; refresh generated compatibility views/scenario clauses and ledger after concurrent live-Wasm source handoff. Shared owner observed local rendering and ledger checks. r[CB-SCOPE-01] r[CB-GATE-01]
- [x] [serial] Run local `tools/cairn-specs.mjs --write-views` and `tools/check-specs.mjs --refresh-ledger --self-test --report` after integration; shared owner reports both passing (13 specs/187 cases, 521 requirements/229 controls). These document checks are not runtime or proof evidence. r[CB-GATE-01]
- [x] [serial] Validate integrated native Cairn documents using the verified direct cached Cairn binary and matching generated policy, not the Nix wrapper: the shared owner observed `valid:true`, `issues:[]`, `findings:[]` over 13 changes and 41 specs after final local view/ledger checks. This is document validation only, not C execution, backend proof, acceptance or production selection. The earlier wrapper invocation unexpectedly triggered Nix auto-GC and is not the successful route. r[CB-GATE-01]

## Remaining native implementation and evidence gates

This isolated WIP adds only an inert, nonselected, checked source emitter for a
restricted effect-free I64/Bool/Unit/Text-result Core subset, including bounded
internal Pair/Sum/List construction/elimination, exact checked local
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

- [ ] [serial] Obtain explicit independent owner review of all direct canonical CB-BUILD-01/CB-LIMIT-01/CB-SANDBOX-01/CB-ADMIT-01 edits and the aligned CB-CASE-06/08 designs **before integration**; document the reviewed safety obligations and any rejection. This source-only corrective commit is not owner acceptance, native GO or permission for `cairn sync --execute`. r[CB-BUILD-01] r[CB-LIMIT-01] r[CB-SANDBOX-01] r[CB-ADMIT-01]
  Packet prepared 2026-11-20 in `review-packet.md` (canonical edit diffs, case designs, obligations, risks, Accept/Reject/Revise checklist); the box stays unchecked until the owner answers after the independent reviewer pass. Record the item-2 answer together with the packet's reading that ¶1 "candidate instructions" and ¶2 "untrusted instructions" are the same boundary, so the accepted or rejected interpretation is durable.
- [ ] [serial] Reconcile each of the eleven active MODIFIED delta blocks against the current canonical requirement and scenario links before any `cairn sync --execute` or archive; a passing structural preview alone does not authorize replacing stronger canonical prose. Review CB-LIMIT-01's exact supplied limit and CB-SANDBOX-01/CB-ADMIT-01's sealed loader, clock and path obligations individually. r[CB-LIMIT-01] r[CB-SANDBOX-01] r[CB-ADMIT-01]
  Reconciliation completed 2026-11-20 in `reconciliation.md`: eleven blocks (seven identical, four canonical plus reviewed insertions), exact differing sentences, CB-LIMIT-01 supplied-bound code facts, CB-SANDBOX-01 clock/vvar/TSC and loader obligations, CB-ADMIT-01 descriptor/sealed-byte obligations, and dry-run preview hashes. The box stays unchecked pending the owner's yes/no on `review-packet.md`; the integrator records it after that.
- [ ] [serial] Bring the restricted source-only build into full input-integrity conformance before counting CB-BUILD-01/CB-LIMIT-01/CB-ADMIT-01: recursive duplicate JSON member rejection and exact supplied `limits.list_length` retention/source-only C enforcement are present, but native admission still needs trusted ancestor-anchored sealed input/output binding and independently verified execution. Until then this emitter remains nonadmitted and `--native`/`execute --target c` refuse. r[CB-BUILD-01] r[CB-LIMIT-01] r[CB-ADMIT-01]
- [ ] [serial] Obtain owner-reviewed opt-in C compiler/runtime/runner/filter/mediator pins from the selected `octet/nixpkgs`, a feasible no-clock/no-exec loader boundary (VM or demonstrated equivalent), deterministic complete-ELF correspondence and explicit authorization for sealed native test execution. Cached host bubblewrap and the Lean proof sandbox do not satisfy this prerequisite. r[CB-SANDBOX-01] r[CB-ADMIT-01]
- [ ] [serial] Extend the existing restricted inert C11 source emitter to the complete independently accepted Core-Bootstrap contract, pinned x86_64 Linux compilation, exact C-source and native-ELF admission, with no ordinary CLI/session fallback or source evaluator. r[CB-BUILD-01] r[CB-ADMIT-01]
- [ ] [serial] Implement checked C11 value/arithmetic/order/runtime, immutable compiled `Program` values and bounded typed `test.emit` ABI; prove unsupported paths and exhaustion classifications through focused controls. r[CB-NUM-01] r[CB-VALUE-01] r[CB-ORDER-01] r[CB-LIMIT-01] r[CB-HOST-01]
- [ ] [serial] Pin and verify a genuine native sandbox runner, including hostile direct syscalls, FD, namespace escape and loader controls plus sealed test-only AT_SYSINFO_EHDR vDSO, coarse clock, direct vvar, `rdtsc` and `rdtscp` probes under fixed memory/CPU/wall bounds; a TSC-only barrier is insufficient and absent sandbox blocks execution. r[CB-SANDBOX-01]
- [ ] [serial] Execute source-bound actual C/native versus selected Node/V8 Wasm differential, sanitizer/ABI and artifact-adversary controls. Record the C-specific open PO-17/18/native refinement and make a distinct later acceptance/selection decision only on real evidence. r[CB-GATE-01]
