# M1 scope review

Task 5.3 review of the mapped M1 requirements. Statuses below are only those supported by retained evidence.
Refinement, runtime execution, M2 and later milestones stay open.

## Requirement status

| Requirement | Scope | Status | Evidence |
|---|---|---|---|
| VT-M1-01 | Immutable, compatible tool inputs | **Met** | `tool-selection` check: 96 pure cases and 19 command rejections; executed extraction, Lean, and Miri compatibility observations; `octet-controls` pin agreement |
| VT-M1-02 | Kernel/shell boundary | **Met** | `boundary-controls`: 2 positive baselines, 16 rejected fixtures, 70 harness self-tests; five workspace Rust tests and the CLI shell tests |
| VT-M1-03 | Compiler-derived source comparison and inventory | **Met** | `source-coverage`: 16 reviewed subjects over 17 compiler items, 29 self-tests, zero diagnostics; exception rules and kernel-exception rejection |
| VT-M1-04 | Bounded extraction and invalid-artifact rejection | **Met** | Entry point executed against the offline proof root with `verify` exit 0 from a fresh inventory (`.pi/m1-extraction/run8`); 13 rejections observed; the acceptance phase records the run. No refinement proof exists |
| VT-M1-05 | Full pinned Octet catalog, policy, facts | **Met** | 13 `flake check` checks; policy structure and export/manifest freshness; six observed negative control families (`.pi/m1-octet-task4*/`) |
| VT-M1-06 | Native and dependency assurance; scoped Miri | **Met** | `native-assurance` check passes; the scoped-Miri tool run executed the pinned positive (2 tests) and the strict-provenance negative with the offline sysroot input (`.pi/m1-native/toolrun/`) |
| VT-M1-07 | Independent acceptance and control matrix | **Met** | Expectation, comparator, 16 negative controls, and the matrix record exist; the full evaluation on tree `4420049e` **accepts** fifteen of fifteen phases (`.pi/m1-acceptance/`). The sync executed after acceptance and the post-sync checks pass (`.pi/m1-matrix/`) |

## Roadmap position

The engine and gate work for the roadmap M1 exit criteria is complete: the extraction and scoped-Miri phases executed and acceptance accepts all fifteen phases. The spec sync (5.4) and the final post-sync checks (5.5) remain before the checklist completes.

## Explicitly open

- Refinement: zero proofs. Extraction and generated-Lean compilation do not establish refinement or Wasm correspondence.
- Runtime execution: the boundary and native fixtures compile; their effects are never executed. Miri runs only the kernel budget tests.
- M2 and later milestones: untouched.
- CI: `flake check` runs thirteen checks locally; no external CI job consumes them yet.
- Independent authentication: pins, digests, and replays remain useful evidence without authenticating the toolchain, cache, or host.
- Task 2.2 remainder already mapped to other tasks: the unsafe/native ownership record belongs to NAT-SITE, and the relocated-body exception belongs to SRC-EXCEPTION; neither is claimed here.

## Lifecycle

The implementation checklist is complete. Archive, validation, commit, push, and integration complete the change.
