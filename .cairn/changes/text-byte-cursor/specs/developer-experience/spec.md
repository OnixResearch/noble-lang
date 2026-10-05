# Guest-native Text byte cursor extension

## ADDED Requirements

### Requirement: DX-TEXT-01
r[DX-TEXT-01]

`Text-Byte-Cursor-v1` MUST be an explicitly selected, pure, independently checked Noble source/managed-Wasm profile, not an addition to ordinary `Core-Bootstrap`, accepted `Declared-Modules-v1`, or LIVE. Only opt-in `session` and `compile` MAY resolve `text.byte` as the globally disjoint Definition(26); reserved identities 23–25 and test.emit 22 MUST NOT be invocable in this profile, including by a forged direct submission. Ordinary and declared/LIVE behavior, wrapping I64 arithmetic and existing accepted evidence MUST remain unchanged.

The exact scheme MUST be `forall<S:stack> [ S Text I64 -- S Text I64 ! {} ]`. It MUST preserve the original immutable Text value with no substring copy or guest heap allocation and use no host request. For signed byte offset `i`, return unsigned UTF-8 byte 0..255 when `0 <= i < len(Text)`, -1 only when `i == len(Text)` (including empty Text), -2 when `i < 0`, and -3 when `i > len(Text)`. It MUST validate memory bounds and signed offset before narrowing/loading; a consumer MUST increment the offset only after a nonnegative byte. This byte-at facility does not establish iteration, arbitrary precision, deterministic calculator budget accounting, MA1 or any CALC case acceptance.

#### Scenario: TEXT-01 compiled cursor boundary

- GIVEN the unpromoted `TEXT-01` design in [`text-byte-cursor-cases.json`](../../../../../specs/conformance/text-byte-cursor-cases.json), with its UTF-8, empty, negative, large-positive, ordinary-profile and forged-private-definition inputs
- WHEN both optimization modes compile and run genuine managed-Wasm children and the independent admission boundary checks the forged candidate
- THEN the observed byte/status, original Text, zero host requests, no native trap and negative controls must match every `expected` field; the case remains absent/not-run until selected conformance assessment, not MA1/CALC acceptance
