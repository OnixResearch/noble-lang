# Reusable contract-based named calls and law bundles tasks

## Design (this change)

- [x] [serial] Re-verify the #7 baseline at `ffcce53` (origin/main `44329dc` plus format/pin) against the `0ea10d3` links, and record the drift and the absence of README MC1/MC2 sections. Record design-time observations O7-1 to O7-4 with the prebuilt `noble-44329dc-release` binary. These are explicitly not evidence. r[PREUSE-EVID-01]
- [x] [serial] Select the opt-in `Named-Contract-Calls-v3` grammar, the bounded vocabulary and budgets, and the refusal list. r[PREUSE-GRAMMAR-01] r[PREUSE-CHECK-01]
- [x] [serial] Specify the exact subject and dependency envelope, the contract-based call rule, the separation between lemmas and calls, the bounded checking and replay algorithm, and invalidation and diagnostics. r[PREUSE-ENVELOPE-01] r[PREUSE-CALL-01] r[PREUSE-LEMMA-01] r[PREUSE-CHECK-01]
- [x] [serial] Write the worked three-stage example (`steps@1.inc` twice, `offsets@2.add10`, `arith@1.add-assoc-wrap`) and the unexecuted PREUSE-S01 to S22 scenario designs inside `design.md`. r[PREUSE-CALL-01] r[PREUSE-CHECK-01]
- [x] [serial] Specify the correspondence obligations C1-C8 separately from finite tests, and the additive migration plan. r[PREUSE-EVID-01] r[PREUSE-GRAMMAR-01]
- [x] [serial] Specify the explicit law-bundle schema, the pure `seq@1.code-monoid` example mapped to existing rules, the worked reuse, the negative cases, the ambiguous two-bundle case and import-stability tests, and the distinction between packaging, deriving and correspondence. r[PREUSE-BUNDLE-01]
- [x] [serial] Run `cairn validate` and the proposal, design and tasks gates for this change. These are document checks only. r[PREUSE-EVID-01]

## Future implementation and evidence gates (not undertaken by this design)

- [ ] [serial] Prove reviewed Lean theorems T1-T6 (`NamedV3.exec_append_iff`, `callFrame`, `embed`, `liftMc1Claim`, `chainClaim`, `Holds₂` evaluation lemmas) in a separate `NamedV3` module under the gate's axiom list. Whitelist `List.nil_append`, `List.append_nil`, `BitVec.add_assoc`, `BitVec.add_comm` and `BitVec.mul_add` as exact reviewed rule identities. r[PREUSE-CALL-01] r[PREUSE-LEMMA-01]
- [ ] [serial] Implement the revision-3 grammar, frozen-snapshot `use`, envelope reconstruction, the symbolic checker, `Eq` over bounded scalar terms, and isolated dependency replay. Leave revisions 1 and 2 byte-for-byte unchanged in behavior. r[PREUSE-GRAMMAR-01] r[PREUSE-ENVELOPE-01] r[PREUSE-CHECK-01]
- [ ] [serial] Implement the law-bundle record, explicit selection, ambiguity refusal, checked specialization and bundle invalidation. Execute the negative and import-stability cases. r[PREUSE-BUNDLE-01]
- [ ] [serial] At sync, register PREUSE-S01 to S22 and the bundle cases as conformance designs, add a `PREUSE` evidence route to `tools/check-specs.mjs` `ROUTES`, refresh generated views and the ledger, then execute and retain source-bound receipts. Promote no status beyond its receipts. r[PREUSE-EVID-01]
- [ ] [serial] Discharge or explicitly keep open correspondence obligations C1-C8 (PO-15/16/17/18/19/20) through their own routes, with artifact correspondence coordinated with #10. Consider archive only after implementation, evidence and separately reviewed gates. r[PREUSE-EVID-01]

## Progress notes (not acceptance)

- T1 and T6 are landed in a separate `NamedV3` module (`proofs/mc1/NobleContracts/NamedV3.lean`) and are pinned by `ContractGate.lean` under the strict axiom list (46 required theorems): T1 `exec_append_iff` carries its own `run_append_iff` factoring lemma, and T6 is `holds_true₂` plus `holds_eq₂_iff` (equality holds exactly when both operands evaluate to the same named value — the named `.eq` compares values directly, not their `toInt`). Revisions 1 and 2 are untouched.
- T2, T3, T4 and T5 are **blocked on vocabulary, not proofs**: an entry embedding, step certificates, bridges, a join and the injective slot renaming do not exist in Lean today, so their statements cannot be written without inventing a second convention for the caller/callee boundary. `exportedClaim` (MC1) and `liftCode`/`lift_run` exist for T4, but T4 is stated over an *entry* claim, and no entry-level named claim exists; adding one beside `exportedNamedClaim` would duplicate it.
- The lemma whitelist (`List.nil_append`, `List.append_nil`, `BitVec.add_assoc`, `BitVec.add_comm`, `BitVec.mul_add`) is not yet registered: the landed proofs use `List.nil_append` structurally but no BitVec rule, so the whitelist addition stays with the remaining four theorems.
