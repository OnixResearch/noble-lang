## Why

Issue #8 asks for a small, explicit layer that tells a caller when an already checked behavioral contract applies to actual inputs.

Today, VC-LOGIC-02 and VC-USE-01 separate proof validity from applicability. MC2 delivers three frozen unary-`I64` guard templates (`LtI64Max`, `NeI64Min`, `EqI64Literal`) with strict Lean wrapper theorems.

Several pieces are missing:
- No theorem ties the Rust recognizer's match to the meaning of the contract's own MC1 `requires`. PO-21 is still open.
- Only `I64` inputs are guardable.
- Composition drops guards.
- No rule states which applicability facts survive stack manipulation, substitution or erasure.

This change designs the binding, the correspondence obligations and one bounded first extension without implementing them. r[GUARD-BIND-01] r[GUARD-CORR-01] (`VC-LOGIC-02`) (`VC-USE-01`)

## What Changes

- Select **applicability witnesses** (caller proof, one guarded invocation, or an existing policy premise), not refined input views. Witnesses are bound to the exact whole precondition, contract statement, subject identity/events, interface and semantic/policy revision. Partial, similar and stale checks never establish the whole precondition. r[GUARD-BIND-01] (`VC-ADMIT-04`)
- State the six-part correspondence (recognizer link, guard execution, soundness/exactness, wrapper, renderer layouts, backend lowering) and its outcome classes: refusal, undefined predicates, traps, exhaustion and pre-body rejection. Rejection keeps the existing `inl (I64 1)` schema and consumes the argument. r[GUARD-CORR-01] (`VC-ADMIT-05`)
- Choose one bounded first extension: a versioned v2 template set adding only `NonEmptyList` for whole precondition `(not (is-nil (in x0)))` over a single pure `List A` input, implemented by a `dup`/`list.case`/`if` wrapper. The v1 templates stay frozen. Conjunction is specified as not supported. r[GUARD-LIST-01]
- Specify fact survival under `swap`/`dup`/`dip`/composition/substitution/arithmetic, keeping signed `I64` wrapping distinct from integer bounds. Erasure retains guard code. r[GUARD-FACT-01] (`VC-COMPOSE-01`)
- Provide static and runtime examples, twelve unexecuted GUARD-CASE designs and an evidence ledger, all in `design.md`.

## Impact

- **Files**: only `.cairn/changes/bounded-applicability-guards/` (proposal, design, tasks, metadata and the ADDED program-contracts delta). There are no edits to canonical specs, conformance JSON, obligations, roadmap, status or generated views. An integrator would register the GUARD-CASE designs and PO-21 sub-obligations later.
- **Code**: none. `guard.rs`, `op_invoke`, `build --require-proof`, the Lean MC2 gate and the frozen v1 template set are unchanged.
- **Testing**: document validation only (`cairn validate`, proposal/design/tasks gates). No runtime, proof or conformance evidence is produced or claimed. Resource ownership (#3), generalized composition (#7) and backend correspondence (#10) are coordination dependencies.
