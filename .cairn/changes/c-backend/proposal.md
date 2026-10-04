# Opt-in C backend candidate

## Why

The selected M4 target compiles checked Noble computation to managed-linear-memory Wasm. A separately accepted Core-Bootstrap candidate could in principle be emitted as portable C11 and compiled ahead of time, but no selected C backend, native confinement, source/native correspondence or refinement evidence exists. Native ELF can bypass C host-ABI tables via direct syscalls. `C-Backend-Draft` therefore needs its own bounded source, execution and selection contract instead of quietly broadening `Wasm-Draft` or treating a C emitter as safe executable support. r[CB-SCOPE-01] r[CB-SANDBOX-01]

## What Changes

- Add SPEC-BE002 as a distinct *nonselected*, opt-in C11 AOT candidate after independent Core-Bootstrap acceptance. Proposed `noble build SOURCE --target c --out DIR --contract CONTRACT_JSON --policy POLICY_JSON [--native]` takes host-owned source, exact checked expected interface/operation contracts and allowed-effect/limit/tool policy; neither source nor optional pinned x86_64 Linux ELF output executes during build. Only explicit `noble execute --target c --artifact DIR --input INPUT_JSON [--bindings HOST_FILE]` could invoke separately admitted bytes in a pinned native sandbox; current binding grants are checked per request, not inferred from build policy. r[CB-SCOPE-01] r[CB-BUILD-01]
- Specify build-key versus semantic identities, unsigned-word arithmetic for wrapping I64, checked aggregate/program representations, immutable captures, left-to-right sequencing, exact limits and typed `test.emit` ABI. r[CB-IDENTITY-01] r[CB-NUM-01] r[CB-VALUE-01] r[CB-ORDER-01] r[CB-LIMIT-01] r[CB-HOST-01]
- Require independently validated C source and exact corresponding admitted ELF, separately pinned native sandbox with hostile direct-syscall controls, and real emitted-C versus selected Node/V8 Wasm differential evidence before any future explicit selection. r[CB-ADMIT-01] r[CB-SANDBOX-01] r[CB-GATE-01]

## Impact

New native normative family `.cairn/specs/c-backend/spec.md`, this active change's added requirement delta, and unexecuted `specs/conformance/c-backend-cases.json` designs. Shared integration registers the family and scenario file in `specs/spec-family.json`, a separate pending roadmap/status dependency after independent Core-Bootstrap admission, and generated compatibility view/ledger/scenario clauses. This slice changes no compiler, runtime, sandbox, Wasm selection, established language requirement, historical evidence or proof status. No archive, acceptance or production selection occurs. r[CB-GATE-01]
