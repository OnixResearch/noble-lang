## Context

Baseline `95626e2b`. [MC1 Rules](../../../proofs/mc1/NobleContracts/Rules.lean)
give `Exec` over bottom-to-top stacks, with wrapping bit-vector arithmetic.
The selected CLI's linear-memory `noble_wasm::source::Compiler` inlines runtime
text through [imports.rs](../../../crates/noble-wasm/src/source/emit/imports.rs).
[operations.rs](../../../crates/noble-wasm/src/operations.rs) selects `$op_add`,
`$op_sub` and `$op_mul`. In [operations.wat](../../../crates/noble-wasm/runtime/operations.wat)
each pops right, then left, uses the corresponding `i64` arithmetic opcode,
and calls `$push_i64`. Neither selecting those names nor comparing helper hashes
proves their effects.

The [Talos pilot](../../../verification/wasm-refinement-pilot/README.md) retains
decoder acceptance, spec tests and separate MC1/Talos lemmas. The full native
link failed on a Mathlib symbol, and reducing the full-module decoder exhausted
5,000,000 heartbeats. Its NO-GO did not invalidate the successful observations.
The new route neither imports Talos nor fetches Mathlib or alters old receipts.

## Decisions

### Decision: Structured instruction boundary (owner decision 56)

**Choice:** Prove an opt-off, Mathlib-free, bounded Wasm refinement with a
small-step model and Aeneas-extracted production lowering. Do not start with
a binary decoder.

**Rationale:** Isolate lowering and helper effects from encoding and decoding.
The structured instructions must actually feed the production emitter. A
handwritten second emitter or a parser of the emitted text alone does not
establish the extracted lowering edge.

### Selected first cut and proof plan

Use one checked nonrecursive pure quotation with a monomorphic I64 interface,
I64 literals and wrapping `+`, `-`, `*`. Use the existing arithmetic helpers.
The observation includes the result stack, untouched incoming tail and exact recipe.
Named calls, nested Programs, general branching, resources, components, live slots,
host effects, and opt-on emission do not inherit this cut's theorem.

1. Specify transitions for Wasm operand values, locals, call/return control and
   the linear-memory/global effects of the selected helper closure. Define tagged
   stack contents and bounds independently of the lowering. If pop/push are
   initially abstract, their refinement remains an explicit unproved premise.
2. Relate source stacks and Wasm state at instruction boundaries. Compose the
   helper simulations over the actual emitted order. Give both soundness of
   normal results and existence of runs for well-typed, sufficiently provisioned
   inputs. Treat quotas separately from invalid operands and underflow; an
   instruction-budget refusal does not imply source divergence.
3. Audit the pinned Charon/Aeneas extraction roots and opaque boundaries. Compare
   the exact reconstructed source body, ordered instructions and reflected recipe.
   Distinguish equality of text/bytes on the controls from an encoding theorem.
4. Pin the exact sources, reached helpers, representation and opt-off build.
   Audit named theorems under the strict MC1 axioms. Retain controls for literals,
   arithmetic, wraparound, a nonempty tail and quota boundaries. Independently
   substitute a different well-typed body, swap subtraction operands, change
   a reached helper, and mismatch the recipe without changing the result stack.

### Obligation ownership

| Edge | Selected research | Not closed by the structured result |
|---|---|---|
| O1 / PO-17 | Structured opt-off lowering and reached-helper simulation | WAT/binary encoding and broader profiles |
| O3 / PO-18 | State the exact checked-instruction/recipe premise for the loader | Actual encoder and loaded-export correspondence |
| O7 | Compose justified edges for the same subject | Executed artifact claim without O1/O3/O6, or without O2 in opt-on |
| OWN-PO-4/5, PREUSE C8 | Reuse the semantic route | Read-retention, reclamation and named-call correspondence |

O4 guards, O5 erasure and O6 consumer refinement stay in their existing tasks.
Host-ledger refinement is separate in [ownership](../ownership-lifetime-contract/design.md).

## Risks / Trade-offs

- The helper closure is larger than the subject body. An abstraction of unproved
  helpers yields a narrower theorem and leaves concrete lowering open.
- An extractable structured representation may change emitter internals. Preserve
  canonical output bytes and selected observations against the pinned baseline.
- Review the measured go/no-go before sync. The existing trusted-build boundary
  and open obligations do not change on a failed pilot or a document acceptance.
