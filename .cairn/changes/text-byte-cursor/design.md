# Opt-in Text byte cursor design

## Context

Kernel Core-Bootstrap definitions occupy IDs 0..22, ordinary `test.abort` uses ID 23, and selected LIVE operations use IDs 24 and 25. No guest Text byte access exists. The calculator remains absent; errors@1 merely establishes nominal error identities. r[DX-TEXT-01]

## Decisions

### Decision: Disjoint opt-in profile

**Choice:** Only explicit `noble session|compile --text-byte-cursor` selects `Text-Byte-Cursor-v1`. The checking environment keeps IDs 0..22 unchanged, uses three permanently private Reserved rows 23..25 and a pure TextByte row 26. Static source resolution admits only `text.byte` at 26; kernel visibility denies test.emit 22 and reserved 23..25, and independent backend acceptance reconstructs the exact selected 27-row prefix and rejects mismatched profiles or forged rows before emission. The parser rejects combinations with declared modules/bindings and the independent LIVE entry point does not select it. r[DX-TEXT-01]

**Rationale:** No globally overlapping builtin IDs, no silent expansion of ordinary Core-Bootstrap or Declared-Modules-v1 and no changed wrapping I64 arithmetic. Cross-profile preparations bind the selected source and compiler state so stale or forged submissions do not gain this operation. r[DX-TEXT-01]

### Decision: Allocation-free byte-at contract

**Choice:** `text.byte : forall<S:stack> [ S Text I64 -- S Text I64 ! {} ]`. It preserves the original immutable Text handle. At offset 0..len-1 the result is that unsigned UTF-8 byte (0..255); offset==len returns -1 (EOF, including zero-length Text); negative offset returns -2; offset>len returns -3. A caller increments the offset by one only after a nonnegative byte; bounded Text length makes that increment nonwrapping. Validate signed offset before narrowing, verify the text address/length against managed linear memory before `i32.load8_u`, allocate no datum and make no host request. r[DX-TEXT-01]

**Rationale:** Nested tagged Pair/Sum output would allocate five 48-byte guest cells per valid byte. The disjoint negative statuses retain distinct EOF and invalid-offset outcomes with no heap allocation or UTF-8-breaking suffix copy. Byte offsets—not Unicode scalar indexes—match calculator source-span units. r[DX-TEXT-01]

## Risks / Trade-offs

- Status integers are an intentionally scoped low-level API, not an implicit I64-to-BigInt conversion. Consumers must branch on nonnegative byte before incrementing; this does not supply guest iteration or a parser. r[DX-TEXT-01]
- Native backend quotas and traps are distinct from CALC-LIMIT-01's as-yet-unimplemented seven deterministic typed `ResourceLimit` categories. No MA1/CALC acceptance is inferred. r[DX-TEXT-01]

## Finite replay

From the repository root, with a built Noble binary and a **new** output
directory, replay the compiled managed-Wasm child cases:

```sh
bun verification/text-byte-cursor/gate.mjs /absolute/path/to/noble /absolute/new-output-directory
```

The gate writes `observation.json` plus child inputs and raw output under that
directory. It is finite implementation evidence, not a source-bound proof or
promotion of the unselected `TEXT-01` case. r[DX-TEXT-01]
