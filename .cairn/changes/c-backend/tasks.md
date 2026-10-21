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
and pure `dip`/`if` code. The fixed 255-element List ceiling is not an
accepted supplied `limits.list_length` policy. The development C11
syntax-only diagnostic is not native execution, compiler correspondence or
C/Wasm acceptance. The requirements below remain open; no CB case is promoted.
Text payloads bind exactly to checked source-body node identities; recursive
contract/policy duplicate member names refuse before source-only emission.
All eleven active MODIFIED delta bodies retain the planned first-parent
canonical obligations and linked scenario designs. Seven bodies match its
normative text; CB-BUILD-01, CB-LIMIT-01, CB-SANDBOX-01 and CB-ADMIT-01
add scoped source-only disclosures or hostile controls without relaxing
the first-parent bound, clock or descriptor requirements. This is not owner
approval or legal sync while the eight remaining gates are undone.

CB-CASE-01 still supplies `limits.list_length: 2`; the current exact
I64/Bool/Unit/Text-only policy refuses that field, so the case cannot be counted as
emitter admission or native runtime evidence. A later accepted list-capable
profile must store and enforce the supplied list bound. Even an extended
restricted source-only route accepting that policy must retain the same supplied bound in
its manifest, not silently drop it or advertise a no-op limit.

- [ ] [serial] Obtain explicit independent owner review of all direct canonical CB-BUILD-01/CB-LIMIT-01/CB-SANDBOX-01/CB-ADMIT-01 edits and the aligned CB-CASE-06/08 designs **before integration**; document the reviewed safety obligations and any rejection. This source-only corrective commit is not owner acceptance, native GO or permission for `cairn sync --execute`. r[CB-BUILD-01] r[CB-LIMIT-01] r[CB-SANDBOX-01] r[CB-ADMIT-01]
- [ ] [serial] Reconcile each of the eleven active MODIFIED delta blocks against the current canonical requirement and scenario links before any `cairn sync --execute` or archive; a passing structural preview alone does not authorize replacing stronger canonical prose. Review CB-LIMIT-01's exact supplied limit and CB-SANDBOX-01/CB-ADMIT-01's sealed loader, clock and path obligations individually. r[CB-LIMIT-01] r[CB-SANDBOX-01] r[CB-ADMIT-01]
- [ ] [serial] Bring the restricted source-only build into full input-integrity conformance before counting CB-BUILD-01/CB-LIMIT-01/CB-ADMIT-01: recursive duplicate JSON member rejection is present, but any accepted `limits.list_length` must be preserved exactly in the manifest and enforced when lists are supported, and any native admission request still needs trusted ancestor-anchored sealed input/output binding. Until then this emitter remains nonadmitted and `--native`/`execute --target c` refuse. r[CB-BUILD-01] r[CB-LIMIT-01] r[CB-ADMIT-01]
- [ ] [serial] Obtain owner-reviewed opt-in C compiler/runtime/runner/filter/mediator pins from the selected `octet/nixpkgs`, a feasible no-clock/no-exec loader boundary (VM or demonstrated equivalent), deterministic complete-ELF correspondence and explicit authorization for sealed native test execution. Cached host bubblewrap and the Lean proof sandbox do not satisfy this prerequisite. r[CB-SANDBOX-01] r[CB-ADMIT-01]
- [ ] [serial] Extend the existing restricted inert C11 source emitter to the complete independently accepted Core-Bootstrap contract, pinned x86_64 Linux compilation, exact C-source and native-ELF admission, with no ordinary CLI/session fallback or source evaluator. r[CB-BUILD-01] r[CB-ADMIT-01]
- [ ] [serial] Implement checked C11 value/arithmetic/order/runtime, immutable compiled `Program` values and bounded typed `test.emit` ABI; prove unsupported paths and exhaustion classifications through focused controls. r[CB-NUM-01] r[CB-VALUE-01] r[CB-ORDER-01] r[CB-LIMIT-01] r[CB-HOST-01]
- [ ] [serial] Pin and verify a genuine native sandbox runner, including hostile direct syscalls, FD, namespace escape and loader controls plus sealed test-only AT_SYSINFO_EHDR vDSO, coarse clock, direct vvar, `rdtsc` and `rdtscp` probes under fixed memory/CPU/wall bounds; a TSC-only barrier is insufficient and absent sandbox blocks execution. r[CB-SANDBOX-01]
- [ ] [serial] Execute source-bound actual C/native versus selected Node/V8 Wasm differential, sanitizer/ABI and artifact-adversary controls. Record the C-specific open PO-17/18/native refinement and make a distinct later acceptance/selection decision only on real evidence. r[CB-GATE-01]
