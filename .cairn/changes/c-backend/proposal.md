# Opt-in C backend candidate

## Why

The selected M4 target compiles checked Noble computation to managed-linear-memory Wasm. A restricted inert C11 source emitter in this isolated WIP accepts checked effect-free Core-Bootstrap expressions with one I64/Bool/Unit/Text result and internal Pair/Sum/List construction/elimination; it never compiles or executes native ELF. Its source-only path rejects duplicate JSON members recursively, but has no ancestor-anchored sealed input/output binding and refuses supplied `limits.list_length` even though its List arena has a fixed ceiling, so it does not satisfy full C build/admission or limit-integrity requirements. No selected C backend, native confinement, source/native correspondence or refinement evidence exists. Native ELF can bypass C host-ABI tables via direct syscalls. `C-Backend-Draft` therefore needs its own bounded source, execution and selection contract instead of quietly broadening `Wasm-Draft` or treating source emission as safe executable support. r[CB-SCOPE-01] r[CB-BUILD-01] r[CB-ADMIT-01]

## What Changes

- Maintain SPEC-BE002 as a distinct *nonselected*, opt-in C11 AOT candidate after independent Core-Bootstrap acceptance. The isolated-WIP `noble build SOURCE --target c --out DIR --contract CONTRACT_JSON --policy POLICY_JSON` emits checked source and a manifest only for effect-free I64/Bool/Unit/Text results, with bounded internal Pair/Sum/List values and exact local Core type-shape checks but no final aggregate output or supplied list-length policy; `--native` and `execute --target c` refuse. The broader proposed build takes host-owned source, exact checked expected interface/operation contracts and allowed-effect/limit/tool policy; only explicit `noble execute --target c --artifact DIR --input INPUT_JSON [--bindings HOST_FILE]` could later invoke separately admitted bytes in a pinned native sandbox. Current binding grants would be checked per request, not inferred from build policy. r[CB-SCOPE-01] r[CB-BUILD-01]
- Specify build-key versus semantic identities, unsigned-word arithmetic for wrapping I64, checked aggregate/program representations, immutable captures, left-to-right sequencing, exact limits and typed `test.emit` ABI. r[CB-IDENTITY-01] r[CB-NUM-01] r[CB-VALUE-01] r[CB-ORDER-01] r[CB-LIMIT-01] r[CB-HOST-01]
- Require independently validated C source and exact corresponding admitted ELF, separately pinned native sandbox with hostile direct-syscall controls, and real emitted-C versus selected Node/V8 Wasm differential evidence before any future explicit selection. r[CB-ADMIT-01] r[CB-SANDBOX-01] r[CB-GATE-01]
- Extend only the unexecuted CB-CASE-08 test-only negative source design to namespace escape, vDSO `__vdso_clock_gettime`/`__vdso_time`/`__vdso_gettimeofday`, coarse clocks, direct vvar reads and x86 `rdtsc`/`rdtscp`, without claiming those probes or a benign control ran. r[CB-SANDBOX-01] r[CB-GATE-01]

## Impact

The native normative family `.cairn/specs/c-backend/spec.md` and ten
unexecuted `specs/conformance/c-backend-cases.json` designs already exist.
The donor canonical contains direct edits outside generated regions; the
planned first-parent canonical retains obligations that an eventual Cairn
sync MUST NOT erase. The open delta records, without syncing:

- CB-BUILD-01 discloses the narrower, nonadmitted source-only path,
  exact ordered checked Core interface matching and recursive
  duplicate-key refusal. The active delta additionally records internal
  Pair/Sum/List WIP without claiming final aggregate results or native
  quota-policy admission. r[CB-BUILD-01]
- CB-LIMIT-01 preserves the first-parent CB-CASE-01 supplied-bound OPEN
  sentence, binds any supplied `limits.list_length` exactly and requires
  rejecting duplicate JSON object members at every nesting level before
  policy validation. CB-CASE-06 now supplies this field in each
  `quota_controls[].policy.limits` and expects exact manifest retention.
  r[CB-LIMIT-01]
- CB-SANDBOX-01 preserves the first-parent VM/virtualized-clock-or-equivalent
  barrier and initial ELF load versus post-load no-exec ordering; **retains**
  the prior MUST that direct syscall, inherited-FD
  and namespace/sandbox escape attempts refuse or terminate with zero
  protected operations; and adds vDSO/coarse/vvar/TSC host-time and
  elapsed-time controls. TSC-only isolation is not a complete clock
  barrier. r[CB-SANDBOX-01]
- CB-ADMIT-01 retains the first-parent descriptor-bound obligation for
  every build and execution request, while disclosing the source-only
  ancestor and output re-resolution gaps without claiming conformance.
  r[CB-ADMIT-01]

All eleven requirement blocks in the active delta are MODIFIED: seven
normative bodies match the planned first-parent canonical, while CB-BUILD-01,
CB-LIMIT-01, CB-SANDBOX-01 and CB-ADMIT-01 retain its obligations and add
explicit source-only disclosures or narrower hostile controls.
The delta retains all 25 canonical linked scenario designs, with paths
relative to its own location. Independent owner review of the direct
canonical edits, the expanded source-only scope and case designs remains
REQUIRED **before integration**, without automatic owner acceptance.
The complete native implementation/evidence tasks remain undone. A
structurally nonblocked sync preview is not permission to replace the
held canonical requirements or to archive. Generated compatibility views
and the ledger follow canonical text until legal sync and regeneration.

The isolated WIP includes restricted inert source emission only, not
native compilation/execution, sandbox or C/Wasm refinement. `CB-CASE-01`
still supplies `limits.list_length: 2`, which the ordinary present
three-key source-only policy refuses. Recursive duplicate-key rejection
fixes that narrow input-integrity defect but is not a passed limit control.
All CB cases remain
absent/not-run/open/unassessed with empty evidence. No archive, owner
acceptance or production selection occurs. r[CB-GATE-01]
