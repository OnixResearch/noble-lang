## Context

Grounded at the slice base `7b289ce2` and extended only by this branch's
`noble-ngraph::belt` files. `PackedSchedule.lean` supplies the finite graph,
I64/Bool reference semantics, and ordered reference-output vocabulary. Rust
`check`, `eval`, and `Compiled` remain the graph admission and comparison
baselines. The belt artifact is a new opt-in representation, not a modification
of those baselines.

## Decisions

### Decision: one target-neutral fragment and explicit per-artifact bounds

Admit only checked `SubProfile::I64` graphs with I64 input slots, I64
constants, `I64Add`, `I64Sub`, `I64Mul`, `I64Eq`, and ordered outputs. Equality
is the only Bool-producing operation. Facts/guards, F32, parameters, other
nodes, and profile mismatches refuse before emission. Each artifact selects a
capacity from 2 through 64; capacity 8 is nominal. At most eight initialized
save slots are available. Graph nodes, outputs, instructions, and bytes have
fixed implementation bounds; exceeding any bound is refusal, never truncation.

### Decision: newest-first belt with explicit preservation

The belt is ordered newest to oldest. Age zero is the newest entry. An
operation reads both operands from the same pre-insertion state and preserves
left/right order; reads do not consume. A definition is inserted at age zero,
and a full belt evicts its oldest value. The lowerer may emit `Save(age, slot)`
before eviction and `Load(slot)` to restore a value. These are the only
preservation operations: there is no implicit spill, and a checker refuses a
live eviction or a live slot overwrite unless another explicit location still
holds the value. Repeated output identities remain repeated reads.

Graph nodes are defined once in canonical node order. After definitions, only
explicit `Output(age)` instructions are accepted, in the graph's ordered output
list. I64 add/subtract/multiply wrap at 64 bits; equality returns Bool. The
backend claims no instruction-set mapping or cycle model.

### Decision: canonical bytes and graph/content binding

Format `ngraph-belt/0` has magic `NGB0`, length-prefixed format/lowerer/checker
revision strings, graph identity, capacity, save-slot count, instruction count,
instruction length, an identity-scheme digest of instruction bytes, a binding
digest, and the instruction bytes. The binding digest covers all revisions, the graph
identity, both machine bounds, count/length, and instruction digest. Integer
fields are little-endian. Opcodes are fixed: input `0`, constant `1`, add `2`,
subtract `3`, multiply `4`, equality `5`, save `6`, load `7`, output `8`.

Encoding and decoding are separate from semantic admission. The lowerer is
untrusted; the checker reparses bytes, validates header/content binding, and
tracks graph-node identities through belt positions and initialized save slots.
It checks each canonical graph operation, operand ages/order, live-value
preservation, completion, and exact output order. The returned token borrows
the exact checked artifact bytes and snapshots the instruction digest and
artifact binding. Execution re-decodes and checks both against those snapshots
before interpretation; a byte change followed by recomputing a valid header is
also refused. No partial output escapes on refusal.

### Decision: evidence classes do not merge

`Belt.lean` is a symbolic identity-machine model whose theorem relates accepted
output identities to `PackedSchedule.referenceOutputs`. It is not a Rust
refinement theorem and does not execute machine timing. Rust tests separately
compare the actual lowerer, independent checker, and interpreter against the
reference and `Compiled`. Extracted-source B2 comparisons add the BigInt oracle
and ordinary Wasm path when configured. Hand-built fixtures are explicitly
labeled as not having a Wasm source run.

The ignored cost lane measures cold admission, lowering, checking, first
execution, and warm execution. The Rust API materializes its output vector
inside execution; a separately reported consumer readback is an extra copy of
that returned vector, not a hidden interpreter stage. Peak payload counts are
logical bounded storage estimates, not allocator/RSS measurements. No timing
threshold is asserted.

## Fixtures and controls

The fixed graph has two inputs, `i64::MIN`, addition, asymmetric subtraction,
multiplication, equality, fanout, and seven ordered outputs including duplicate
nodes. Exact vectors include signed wraparound and subtraction order. Capacity
2, 3, and 4 all force save/reload traffic. Mutations cover wrong opcode,
operand-age swap, stale age, incomplete/extra output, malformed opcode, digest
corruption, invalid and rebinding-inconsistent capacities, wrong graph identity,
uninitialized/bounded slots, live eviction, live slot overwrite, guards/facts,
F32, and parameters. The existing extracted-source B2 suite runs L3/L4/L5 and
compares the belt to BigInt; its configured full campaign also compares to
ordinary Wasm in both optimization modes.

## Proof obligations

`validate_sound` states that a complete accepted belt program's output node
identities are exactly the requested graph outputs, so its model outputs equal
`PackedSchedule.referenceOutputs`. Kernel-checked model controls pin stale-age,
operand-order, uninitialized/overwritten-slot, live-eviction, completion, and
output-order refusals. The theorem does not prove that the Rust encoder,
checker, or interpreter implements the Lean model; finite tests are the evidence
for those source-level correspondences.

## Acceptance mapping

| Issue criterion | Location |
|---|---|
| Target-neutral semantics, bounds, bindings, refusals | `NGRAPH-BELT-01` through `NGRAPH-BELT-03` |
| Lowered artifact checked and executed against graph/packed/exact values | `crates/noble-ngraph/tests/belt_dir/` |
| Extracted source compared with BigInt and configured Wasm | `crates/noble-ngraph-experiment/src/b2i64/` and `tests/extract_b2.rs` |
| Model, correspondence, finite-test and measurement claims separated | `NGRAPH-BELT-04`, `Belt.lean`, `belt_dir/measure.rs` |

## Non-claims

- No Mill ISA compatibility, hardware implementation, schedule, or performance
  improvement is claimed.
- The Lean theorem is a checker-to-reference identity-model theorem, not a
  Rust/Lean refinement proof or source-to-artifact proof.
- Hand-built graph fixtures have no Wasm source comparison. Extracted-source
  Wasm results exist only when the full B2 CLI path is actually run.
- Logical peak payload arithmetic does not report real allocations or general
  RSS, and timing output is descriptive only.
- This is an experimental opt-in module; no existing default execution path or
  accepted architecture profile changes.
