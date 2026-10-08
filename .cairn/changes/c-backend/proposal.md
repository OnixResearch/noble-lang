# Opt-in C backend candidate

## Why

The selected M4 target compiles checked Noble computation to managed-linear-memory Wasm. A restricted inert C11 source emitter in this isolated WIP accepts only checked effect-free Core-Bootstrap I64/Bool/Unit expressions; it never compiles or executes native ELF. Its source-only path has no ancestor-anchored sealed input/output binding and accepts duplicate JSON members last-wins, so it does not satisfy full C build/admission or limit-integrity requirements. No selected C backend, native confinement, source/native correspondence or refinement evidence exists. Native ELF can bypass C host-ABI tables via direct syscalls. `C-Backend-Draft` therefore needs its own bounded source, execution and selection contract instead of quietly broadening `Wasm-Draft` or treating source emission as safe executable support. r[CB-SCOPE-01] r[CB-BUILD-01] r[CB-ADMIT-01]

## What Changes

- Maintain SPEC-BE002 as a distinct *nonselected*, opt-in C11 AOT candidate after independent Core-Bootstrap acceptance. The isolated-WIP `noble build SOURCE --target c --out DIR --contract CONTRACT_JSON --policy POLICY_JSON` emits checked source and a manifest only for effect-free I64/Bool/Unit; `--native` and `execute --target c` refuse. The broader proposed build takes host-owned source, exact checked expected interface/operation contracts and allowed-effect/limit/tool policy; only explicit `noble execute --target c --artifact DIR --input INPUT_JSON [--bindings HOST_FILE]` could later invoke separately admitted bytes in a pinned native sandbox. Current binding grants would be checked per request, not inferred from build policy. r[CB-SCOPE-01] r[CB-BUILD-01]
- Specify build-key versus semantic identities, unsigned-word arithmetic for wrapping I64, checked aggregate/program representations, immutable captures, left-to-right sequencing, exact limits and typed `test.emit` ABI. r[CB-IDENTITY-01] r[CB-NUM-01] r[CB-VALUE-01] r[CB-ORDER-01] r[CB-LIMIT-01] r[CB-HOST-01]
- Require independently validated C source and exact corresponding admitted ELF, separately pinned native sandbox with hostile direct-syscall controls, and real emitted-C versus selected Node/V8 Wasm differential evidence before any future explicit selection. r[CB-ADMIT-01] r[CB-SANDBOX-01] r[CB-GATE-01]
- Extend only the unexecuted CB-CASE-08 test-only negative source design to namespace escape, vDSO `__vdso_clock_gettime`/`__vdso_time`/`__vdso_gettimeofday`, coarse clocks, direct vvar reads and x86 `rdtsc`/`rdtscp`, without claiming those probes or a benign control ran. r[CB-SANDBOX-01] r[CB-GATE-01]

## Impact

The native normative family `.cairn/specs/c-backend/spec.md` and ten
unexecuted `specs/conformance/c-backend-cases.json` designs already exist.
The document passes **directly edited canonical** requirements outside
generated regions, rather than safely applying a Cairn delta:

- CB-BUILD-01 discloses the narrower, nonadmitted source-only path and
  its duplicate-key last-wins gap. r[CB-BUILD-01]
- CB-LIMIT-01 binds any supplied `limits.list_length` exactly and requires
  rejecting duplicate JSON object members at every nesting level before
  policy validation. CB-CASE-06 now supplies this field in each
  `quota_controls[].policy.limits` and expects exact manifest retention.
  r[CB-LIMIT-01]
- CB-SANDBOX-01 corrects initial ELF load versus post-load no-exec
  ordering; **retains** the prior MUST that direct syscall, inherited-FD
  and namespace/sandbox escape attempts refuse or terminate with zero
  protected operations; and adds vDSO/coarse/vvar/TSC host-time and
  elapsed-time controls. TSC-only isolation is not a complete clock
  barrier. r[CB-SANDBOX-01]
- CB-ADMIT-01 scopes descriptor-bound sealed admission to requests
  seeking native admission and discloses source-only ancestor and output
  re-resolution races without declaring native correspondence.
  r[CB-ADMIT-01]

All eleven already-canonical requirement blocks in the active delta are
MODIFIED, with normative text reconciled against current canonical prose;
independent owner review of these four direct canonical edits and their
case designs is REQUIRED **before integration**, without automatic owner
acceptance. Scenario-link reconciliation and another explicit review
precede any whole-change sync or archive. Generated compatibility views
and the ledger follow canonical text, not an unreviewed sync preview.
The new source-only Unit WIP is intentionally not grafted onto the held
canonical CB-BUILD-01 status prose; the owner must reconcile scope before
approving any integration.

The isolated WIP includes restricted inert source emission only, not
native compilation/execution, sandbox or C/Wasm refinement. `CB-CASE-01`
still supplies `limits.list_length: 2`, which the ordinary present
I64/Bool/Unit-only policy refuses. The duplicate-key bypass remains missing
input validation, not a passed limit control. All CB cases remain
absent/not-run/open/unassessed with empty evidence. No archive, owner
acceptance or production selection occurs. r[CB-GATE-01]
