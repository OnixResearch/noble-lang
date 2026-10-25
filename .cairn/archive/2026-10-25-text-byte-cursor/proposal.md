# Opt-in native Text byte cursor

## Why

Noble's immutable Text literals are opaque to ordinary guest programs. The exact calculator needs a genuine guest-visible UTF-8 byte interface before its library parser can be attempted. An I64 arithmetic demo or host-only parser does not satisfy the selected CALC-GATE-01 scope. r[DX-TEXT-01]

## What Changes

- Specify a separate `Text-Byte-Cursor-v1` CLI/compiler/guest profile selected only by `--text-byte-cursor` in ordinary `session` or `compile`. `text.byte` is otherwise unbound; ordinary Core-Bootstrap, Declared-Modules-v1 and LIVE identities remain unchanged. r[DX-TEXT-01]
- Specify an effect-free `Text I64 -- Text I64` byte-at operation with a retained immutable Text, bytes 0..255, EOF -1, negative offset -2 and past-end offset -3. Native checked Wasm reads original UTF-8 bytes without copying suffixes, heap aggregate allocation or host requests. r[DX-TEXT-01]
- Establish only a bounded text-processing prerequisite, not iteration, BigInt, seven-category numeric budgets, MA1 or any CALC scenario acceptance. r[DX-TEXT-01]

## Impact

- **Files**: scoped kernel/frontend/backend/CLI flag, native runtime helper, an actual compiled-child gate and an active delta under `specs/developer-experience/`.
- **Testing**: full-byte-order Aé (65,195,169), empty/EOF, both invalid offset classes, default-profile refusal, forged private definition refusal and compiled Wasm opt off/on; no selected calculator promotion. r[DX-TEXT-01]
