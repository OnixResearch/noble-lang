## Why

Core-Bootstrap expressly excludes user-defined schemas and imports. DX-03, DX-08 and DX-09 remain unexecuted designs. The next bounded post-M4 slice needs real guest declarations, checked nonstructural identities, and a module linker whose required operation is not host authority. A structured type fixture or a host-side simulation of guest calls would not meet this goal.

## What Changes

- Select `Declared-Modules-v1`, a finite source and runtime profile outside Core-Bootstrap: nonrecursive nominal opaque wrappers, exactly two-constructor tagged variants, visibility-controlled typed construction and elimination, and compiled Wasm execution. r[DX-TYPE-01] r[DX-TYPE-02]
- Select explicit source module import/export and a checked, independently supplied `test.emit : Text -- ! {test.emit}` binding. Freeze semantic dependencies and adapter identity at resolution/link; reject absent, mistyped or effect-incompatible bindings before guest code runs. Do not treat linking as a credential or execute module code on import. r[DX-MODULE-01] r[DX-MODULE-02]
- Exercise DX-03 as a static zero-guest-call gate, DX-08 as a zero-call/zero-authority link gate, and DX-09 through a genuinely compiled Noble/Wasm request denied by the host after display-name rebinding. Existing case ledgers remain `absent`/`not-run` until separately executed with actual evidence. r[DX-TYPE-01] r[DX-TYPE-02] r[DX-MODULE-01] r[DX-MODULE-02]

## Scope and impact

Production implementation, verification evidence, and status promotion are separate lifecycle work. Expected affected owners are `noble-contracts` source scanner/parser/resolution/inference/emission and module linker, `noble-kernel` types/schema/acceptance/effects, `noble-wasm` type descriptors/lowering/Wasm data operations, `noble-cli` session/host dispatch, and real CLI/compiled-Wasm acceptance gates. This change alone alters no production code, generated spec views, conformance state, or prior acceptance receipt. Core-Bootstrap and accepted M4–M8 remain unaffected profiles.

## Explicit exclusions

No recursive schemas/definitions, generic user variants, structural coercion, pattern guards, unbounded constructors, portable canonical package format, package registry or network transport, automatic module initializers, host authority from a declaration, arbitrary new host interfaces, or universal compiler/backend proof. No guest program is interpreted by host code.
