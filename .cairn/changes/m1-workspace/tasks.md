# M1 implementation tasks

All tasks are open. Package validation does not complete an implementation task.
Each phase starts after the previous phase passes its required checks.
All commands use the dedicated implementation worktree as the explicit root.

## Phase 1: Baseline and compatible inputs

- [ ] [serial] 1.1 Run the document baseline and all three Cairn gates. Record the exact commands, outcomes, and independent M1 acceptance scope. r[VT-M1-07]
- [ ] [serial] 1.2 Review published Octet and upstream Charon/Aeneas/Lean contracts. Record exact candidate revisions, consumed interfaces, ownership boundaries, and compatibility constraints. r[VT-M1-01]
- [ ] [serial] 1.3 Select immutable tools and targets through an actual extraction probe. Let Nix generate the lock and reject missing pins or mismatched Charon/backend inputs. r[VT-M1-01]
- [ ] [serial] 1.4 Name the exact extraction, inventory, architecture, and Miri commands in the implementation runbook. Retain failed probes without claiming compatibility. r[VT-M1-01] r[VT-M1-06] r[VT-M1-07]

## Phase 2: Workspace and source ownership

- [ ] [serial] 2.1 Add the kernel and CLI crates with inward dependencies. Add the actual bounded budget transition and tests for positive and exhausted inputs. r[VT-M1-02]
- [ ] [serial] 2.2 Add shell integration tests and architecture rejection controls for kernel effects, unsafe bodies, host types, and reverse dependencies. r[VT-M1-02]
- [ ] [serial] 2.3 Add the compiler-derived source comparison and complete inventory. Classify generated code, dependencies, tests, non-Rust tools, future components, and exact non-kernel exceptions. r[VT-M1-03]
- [ ] [serial] 2.4 Add controls for omitted functions, macros, targets, features, stale mappings, and invalid kernel exceptions. Report extracted, modeled, excepted, proved, and open counts separately. r[VT-M1-03]

## Phase 3: Actual extraction and native boundaries

- [ ] [serial] 3.1 Add the bounded Charon/Aeneas/Lean entry point. Extract the actual workspace function and compile its generated Lean module with source-bound output identities. r[VT-M1-04]
- [ ] [serial] 3.2 Reject missing functions, unsupported bodies, edited output, unexplained models, and stale inputs. Reject proof-required claims with missing proofs or proof holes. r[VT-M1-04]
- [ ] [serial] 3.3 Add native and dependency inventories with exact scope, safety arguments, and target assumptions. Add pinned Miri runs with positive and negative controls. r[VT-M1-06]
- [ ] [serial] 3.4 Reject missing native records and unsupported required Miri configurations. Record an empty owned-unsafe scope only after source accounting establishes it. r[VT-M1-06]

## Phase 4: Quality policy and CI

- [ ] [serial] 4.1 Configure the same immutable Octet revision in Nix and the deny-all hook. Cover the workspace, all targets, and all compatible features without disabled lints or finding budgets. r[VT-M1-05]
- [ ] [serial] 4.2 Add the Nickel architecture policy, checked export, freshness manifest, and compiler-fact coverage for libraries, binaries, and tests. r[VT-M1-05]
- [ ] [serial] 4.3 Add negative controls for lint findings, stale or empty policy, missing facts, unsupported required configurations, and advisory-only results. r[VT-M1-05]
- [ ] [serial] 4.4 Make Nix and CI run the complete Rust and document gate. Include architecture, coverage, extraction, and scoped Miri checks. r[VT-M1-04] r[VT-M1-05] r[VT-M1-06]

## Phase 5: Acceptance and lifecycle completion

- [ ] [serial] 5.1 Add independently bound acceptance records and negative tests for stale subjects, policy, configuration, missing phases, and evidence-role promotion. r[VT-M1-07]
- [ ] [serial] 5.2 Run every required positive and negative control on the declared configuration matrix. Retain exact commands, identities, diagnostics, counts, and phase outcomes in a persistent evidence location. r[VT-M1-07]
- [ ] [serial] 5.3 Review the M1 scope against every mapped roadmap requirement. Update only statuses supported by evidence. Keep refinement, runtime execution, and later milestones open. r[VT-M1-07]
- [ ] [serial] 5.4 After implementation acceptance, inspect the spec sync plan. Execute the accepted plan and regenerate compatibility views and ledgers. Check all seven new IDs. r[VT-M1-07]
- [ ] [serial] 5.5 Run the final post-sync document and Cairn checks. Record persistent evidence and resolve every acceptance blocker before marking this checklist complete. r[VT-M1-07]

## After Task Completion

These lifecycle actions follow the completed implementation checklist. They are not prerequisites for marking their own completion.

1. Run the archive dry-run and inspect its plan.
2. If the plan is unblocked, execute archive with an explicit archive date.
3. Run Cairn validation and commit the completed change.
4. Push the verified branch and integrate it into `origin/main` without a force-push.
5. Before worktree removal, run the drain-evidence guard and preserve any owner-only evidence outside the worktree.
