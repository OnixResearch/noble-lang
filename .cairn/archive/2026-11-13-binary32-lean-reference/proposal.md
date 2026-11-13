## Why

Numeric-Graph-v1 selects binary32 graph arithmetic, and its evaluator is
covered by finite differential tests only. No Lean-kernel-checked reference
names the arithmetic the graph claims: the existing foundation models signed
I64 (`proofs/foundation`, issue #15) and nothing else. A real-number identity or
a native FPU evaluation does not establish the selected machine arithmetic, so
the F32 obligations of #40/#44/#58 and the F32 parts of #41/#45 stay open until
a pinned model carries kernel-checked expectations. This change is that missing
reference layer (issue #38), not another graph implementation or GPU worker.

## What Changes

- `proofs/foundation/NobleFoundation/Binary32.lean` selects one explicit,
  kernel-reducible binary32 model: raw 32-bit encodings, classification, an
  exact scaled-integer interpretation, round-to-nearest/ties-to-even with
  gradual underflow, and add/subtract/multiply with signed-zero, infinity, NaN
  and overflow rules. r[NGV-F32-01]
- `proofs/foundation/NobleFoundation/Binary32Expectations.lean` states the
  proved properties and kernel-checks 26 retained expectations that an
  independent external tool proposes. r[NGV-F32-02]
- `verification/binary32/propose.py` is the external proposer, and
  `verification/foundation/receipts/4be2a68/` retains the clean-source checker
  run: 205 theorems, nine of them binary32, axiom sets inside the allow-list.
  r[NGV-F32-03]

## Impact

- **Files**: `proofs/foundation/NobleFoundation/Binary32.lean`,
  `proofs/foundation/NobleFoundation/Binary32Expectations.lean`,
  `proofs/foundation/NobleFoundation.lean`, `proofs/foundation/inventory.json`,
  `proofs/foundation/README.md`, `verification/binary32/propose.py`,
  `verification/binary32/proposals.json`,
  `verification/foundation/receipts/4be2a68/`,
  `verification/foundation/receipts/README.md`
- **Testing**: `lake build NobleFoundation`, `verification/foundation/check.mjs`
  (inventory/axiom audit, clean source, no warnings),
  `verification/binary32/propose.py`, and the affected flake checks
  (`documents`, `cairn`, `policy`, `format`). No Rust, Wasm, optimizer or
  runtime claim is made or tested here.
- **Not changed**: `crates/noble-ngraph` semantics, acceptance, kernel types,
  identity, ABI, the numeric-graph conformance states, and the existing APFloat
  differential lane.
