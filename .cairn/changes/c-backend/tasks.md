# Opt-in C backend candidate tasks

## Specification

- [x] [serial] Draft distinct nonselected SPEC-BE002 Core-Bootstrap C11 AOT contract and active added-requirement delta, without changing selected Wasm/Node/V8, historical evidence or proof status. r[CB-SCOPE-01] r[CB-BUILD-01]
- [x] [serial] Author explicit unexecuted CB-CASE-01 through CB-CASE-10 positive and hostile scenario designs, each with `absent/not-run/open/unassessed` state and no evidence. r[CB-GATE-01]
- [x] [serial] Integrate new family and scenarios in the shared manifest; register roadmap/status pending dependency after independent Core-Bootstrap acceptance; refresh generated compatibility views/scenario clauses and ledger after concurrent live-Wasm source handoff. Shared owner observed local rendering and ledger checks. r[CB-SCOPE-01] r[CB-GATE-01]
- [x] [serial] Run local `tools/cairn-specs.mjs --write-views` and `tools/check-specs.mjs --refresh-ledger --self-test --report` after integration; shared owner reports both passing (13 specs/187 cases, 521 requirements/229 controls). These document checks are not runtime or proof evidence. r[CB-GATE-01]
- [x] [serial] Validate integrated native Cairn documents using the verified direct cached Cairn binary and matching generated policy, not the Nix wrapper: the shared owner observed `valid:true`, `issues:[]`, `findings:[]` over 13 changes and 41 specs after final local view/ledger checks. This is document validation only, not C execution, backend proof, acceptance or production selection. The earlier wrapper invocation unexpectedly triggered Nix auto-GC and is not the successful route. r[CB-GATE-01]

## Future implementation and evidence gates (not undertaken by this spec-only change)

- [ ] [serial] Independently accept bounded Core-Bootstrap candidates, implement actual opt-in C11 source emission and pinned x86_64 Linux compilation, exact C-source and native-ELF admission, with no ordinary CLI/session fallback or source evaluator. r[CB-BUILD-01] r[CB-ADMIT-01]
- [ ] [serial] Implement checked C11 value/arithmetic/order/runtime, immutable compiled `Program` values and bounded typed `test.emit` ABI; prove unsupported paths and exhaustion classifications through focused controls. r[CB-NUM-01] r[CB-VALUE-01] r[CB-ORDER-01] r[CB-LIMIT-01] r[CB-HOST-01]
- [ ] [serial] Pin and verify a genuine native sandbox runner, including hostile direct syscalls, FD and loader controls, under fixed memory/CPU/wall bounds; absent sandbox blocks execution. r[CB-SANDBOX-01]
- [ ] [serial] Execute source-bound actual C/native versus selected Node/V8 Wasm differential, sanitizer/ABI and artifact-adversary controls. Record the C-specific open PO-17/18/native refinement and make a distinct later acceptance/selection decision only on real evidence. r[CB-GATE-01]
