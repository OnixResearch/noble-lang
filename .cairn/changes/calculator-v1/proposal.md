# Opt-in exact calculator application

## Why

The selected calculator contract requires a real Noble/Wasm parser, exact numeric library and transactional session, not a host-language calculator or a wrapping `I64` demonstration. The source/compiler/runtime boundary also needs an explicit, disjoint application profile so this work cannot silently expand Core-Bootstrap or alter previously selected sessions. r[CALC-SCOPE-01] r[CALC-GATE-01]

## What Changes

- Select `Calculator-v1` only through the calculator entry point. Independently check the Noble module source, every typed guest submission and the selected backend before execution; leave ordinary, Text-byte-cursor, declared-module and LIVE identities and limits unchanged. r[CALC-GATE-01]
- Keep parsing, name resolution, numeric operations, exact display decisions and seven deterministic `ResourceLimit` categories in pure guest library code. The shell supplies bounded terminal bytes and budgets and observes typed results, but never computes a calculator value or injects an opaque guest state as a trusted host result. r[CALC-SCOPE-01] r[CALC-BOUND-01] r[CALC-LIMIT-01]
- Make each terminal line tentative until its guest evaluation and bounded formatting succeed. A fresh candidate worker may replay previously accepted lines, but every checked phase of one candidate line must retain its typed state in one ModuleSession, prospective compiler and Wasm worker. Rejecting the candidate preserves the prior accepted session. r[CALC-STATE-02]
- Apply selected-only 64-page maximum managed memory with 72,362 bounded cells and the unchanged per-invocation allocation ceiling. Before each submission, compact the cells reachable from the operand stack, preserving their order. This is safe because calculator handles never leave the worker: no parked frame, no projection, and the shell ignores report handles. The vacated tail is zeroed (S-MEM-04). Independently bound 512 reachable named bodies and 768 environment rows, including derived operations, without omitting kernel replay. Retain copied signature observations, independent preclone structural guards and cumulative source-preparation work accounting; keep native/host failures distinct from guest `ResourceLimit`. r[CALC-BOUND-01] r[CALC-LIMIT-01]

## Impact

- **Sources:** calculator-only guest modules, terminal shell, independent source/backend admission and selected managed-Wasm runtime. These are application implementation surfaces, not new general Noble syntax or a replacement for the kernel checker.
- **Evidence:** scoped checked-source, backend and real guest execution are implementation observations. Canonical CALC-01 through CALC-13 remain in their existing ledger states until independently run against every recorded input and expected field; this active change neither promotes those cases nor claims proof or AI-authoring acceptance. r[CALC-GATE-01]
- **Lifecycle:** this is an unselected active change, manually authored using the existing `spec-driven` metadata template; `change create` was **not** executed. `tools/cairn.mjs` proposal/design/tasks/validate gates remain **unrun** because the pinned runner invokes Nix and the user's low-space directive forbids further Nix fetch/build. Do not treat local document checks as those gates.
