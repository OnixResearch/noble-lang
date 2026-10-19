## Why

DX-HOLE-01 and DX-02 are unexecuted because the compiler has source grammar and ordinary closed program admission, but no editor syntax ingress. A hole must be inspected without turning incomplete syntax into a runtime value, effect proof or kernel node. r[DX-HOLE-01]

## What Changes

- Select editor-only `noble-editor/v1` structured JSON and equivalent typed frontend AST, bounded by source byte/node/depth/work limits. No `.noble` hole punctuation is selected.
- Analyze the exact `integer:1, editor-hole, word:+` sequence through the existing resolver and word-scheme inference, retaining the unresolved hole effect and required `I64` suffix. Never execute editor analysis. r[DX-HOLE-01]
- Reject direct, nested-quotation, and serialized hole admissions before accepted preparation or guest/host calls. Admit hole-free source through ordinary parser, independent kernel, managed-Wasm compiler and selected execution. r[DX-HOLE-01]

## Impact

- **Files**: Editor frontend transport and inference, CLI JSON ingress, targeted tests and DX-02 conformance/evidence documents after frozen source-bound acceptance.
- **Testing**: Four DX-02 variants; hole-free `1 2 +` → `3`; malformed/oversized/collision/forged-field input, nested hole, invalid words and independent source/kernel/runtime admission. Rust tests and actual CLI smoke precede source-bound evidence; proof remains open.
