# Conditional one-call linking pilot — issue #52

This lane studies one pure I64 caller and one immutable, acyclic callee on the
**existing same-artifact named-call path**. It adds no language syntax, production
checker, module-loading ABI, proof admission flag or release permission.

The formal reference is the pinned `NobleContracts.NamedV1` semantics imported by
[`LinkBoundary.lean`](../../proofs/foundation/NobleFoundation/LinkBoundary.lean).
The [foundation checker](../foundation/check.mjs) inventories the exact theorem
statements and their transitive axioms. The executable controller independently
compiles and runs the fixture; its receipt establishes identity and finite
observations, not semantic preservation of Rust, Wasm or the loader.

## Selected subject

The exact fixture files are listed by [`fixtures/subject.json`](fixtures/subject.json):

```noble
module LinkCallee@1 [ export split def split [ - 7 ] ]
```

A separate import unit is `import LinkCallee@1 as c`. The caller root is
`c.split -`, with complete bottom-first input `Bool I64 I64`; the Bool is a
protected prefix. The callee returns `[wrap64(x-y), 7]` in that order, and the
caller returns `[prefix, wrap64(wrap64(x-y)-7)]`. The second root, `c.split`, is a
boundary probe exposing both callee results before the caller continuation.

The caller is a compiled root, **not** a separately declared `LinkCaller@1`
module. `LinkCaller@1/run` is the Lean fixture's provenance label for those exact
root bytes. It does not authenticate source acceptance. The executable identity
binds the root bytes, source revision/tree and import-resolution snapshot; the
callee additionally binds `LinkCallee@1`, `split`, its lexical definition slot,
exact body bytes and dependency closure. A lexical definition slot, a NamedV1
specialization slot and a runtime table index are distinct identities.

Six fixed vectors cover positive, negative, equal, signed-endpoint wrapping and
wide noncommutative inputs. Both roots run with optimization off and on: 24
nominal executions. This is a bounded roster, not exhaustive I64 testing.

## Real boundary and model map

`noble compile --declared-modules --module callee --module import` accepts the
source through the existing frontend/kernel and emits **one** WAT artifact
containing the root and its instantiated callee. It does not import a separately
loaded callee Wasm module.

| Existing implementation | Lean boundary |
|---|---|
| `source/plan/operations.rs::named_call` resolves a selected immutable definition to `Action::Call(root, identity)` | `splitUse`, `splitEntry`, `pilotEnvironment`, `pilot_closure`; `ProgramBinding` records provenance, specialization, body, dependency edges/rank, runtime entry/environment and artifact bytes. Accepted-source projection remains a premise. |
| `source/emit/functions.rs::entry` enqueues the caller's next entry before executing the call; `Action::Call` enqueues the callee Program | `scheduleNamedCall`, `return_target_exact`, `horizontal_runtime_link`: callee dispatch occurs before the exact pending caller continuation. |
| `runtime/stack.wat` stores operand tag:i32, padding:i32, payload:i64 at byte `16*i`; `$sp` counts cells | `OperandCell`, `operandAddress`, `decodeCell`, `runtimeRepresents`. Physical bottom-first memory corresponds to the reverse of the model's top-first operand list. Only Bool/I64 cells are in this runtime boundary. |
| Continuation slots contain index:i32 and environment:i32 at `4096+8*i`; `$cp` counts slots; `$dispatch` pops one and uses `call_indirect` | `ContinuationSlot`, `continuationAddress`, `dispatchNext`. Exact table target/environment and unambiguous table binding are explicit premises. |
| `$op_sub` pops right then left, computes `left-right`, and pushes one I64 | `RuntimeStep.sub`, `pilot_runtime_callee_run`, `pilot_runtime_caller_post_run`; source semantics uses the existing NamedV1 subtraction constructor. |
| Emitted modules import private engine memory, table and mutable globals | `ExclusiveEngineOwnership`; successful scalar runs preserve the protected operand suffix, pending continuations and table. No arbitrary external alias/mutation is admitted. |

`EngineState` is a **projection**, not the complete engine or Wasm memory:
operand cells, continuation cells, table bindings and the `$sp`/`$cp` counts.
Step/peak/quota accounting, heap preparation and other imported globals remain
part of the real engine and helper trust boundary. They are not asserted
unchanged by this model. Quota, abort, traps, divergence, cleanup, timings and
resource accounting are outside the initial normal-return observation relation.
The concrete model witnesses show ordered scalar results and a typed Bool
prefix; source-level resource calls, live dispatch, async, recursion, concurrency
and arbitrary adversarial contexts are excluded.

## Named proof obligations

- `scalar_call_entry_return`, `caller_after_named_call`, `pilot_caller_executes`:
  the concrete NamedV1 call and caller law with an arbitrary preserved tail.
- `horizontal_compose`: independent source pre/callee/post normal-return
  segments compose through exact entry lookup and ordered-interface matching.
- `horizontal_runtime_link`: conditional same-artifact composition additionally
  requires exact source/body/dependency binding, the caller's acyclic closure,
  compatible boundary representations, independently related scalar component
  runs, exact table/continuation targets and exclusive engine ownership. It
  derives the dispatch/return protocol and preserved frame/control state; it does
  not assume one combined caller-result theorem.
- `vertical_simulation_compose`: a separate rule composing two local forward
  simulations through finite intermediate steps. It is not a proof that the
  actual frontend, compiler, optimizer or loader supplies either simulation.
- Identity/order/refusal lemmas distinguish an identical displayed signature
  from exact source, dependency, body, table target and artifact identity.

[`inventory.json`](../../proofs/foundation/inventory.json) and the generated
`foundation-check.json` carry the exact statements and axiom sets. Only the
foundation's established `propext`, `Classical.choice`, `Quot.sound` whitelist is
permitted. No custom axioms, proof holes, external SMT acceptance or native
proof evaluation are introduced. The pinned Mathlib `.olean` cache remains the
foundation's existing separate trust assumption.

## Reproduce

Run from a clean committed checkout with the existing foundation Lake cache
and the policy-selected tools available. Output directories must be new and
outside the source checkout.

```sh
NODE=/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node
OUT="$HOME/.cache/noble-linking-$(date +%s)"; mkdir "$OUT"
env -i "$NODE" verification/foundation/check.mjs --output "$OUT/check"
env -i "$NODE" --no-liftoff --no-wasm-lazy-compilation --no-wasm-tier-up \
  --stack-size=1024 --max-old-space-size=256 verification/linking/run.mjs \
  --foundation "$OUT/check/foundation-check.json" --output "$OUT/linking"
env -i "$NODE" --no-liftoff --no-wasm-lazy-compilation --no-wasm-tier-up \
  --stack-size=1024 --max-old-space-size=256 verification/linking/run.mjs \
  --verify "$OUT/linking/linking.json"
```

Select `tool_paths.node.output` from `policy/tool-selection.json` in another
checkout. The exact Node flags are required by the runtime configuration. The
controller builds the selected checkout with the reviewed offline Cargo vendor,
then compiles each subject through the real CLI and loads it through
`CoreEngine.prepare` in `Declared-Modules-v1`. Empty bindings grant no host
operation. `verification/foundation/ci.sh` includes this lane and verification.

The controller retains raw command argv/status/stdout/stderr, exact source and
import bytes, checked WAT, assembled/optimized Wasm, preparation and execution
reports, vectors, limits and the proof inventory. `linking.json` binds those
artifacts, compiler/helper/ABI/tool identities, optimization mode and loaded-byte
hashes. Its own SHA-256 is printed separately. `--verify` requires the same clean
source revision/tree and refuses altered subjects, dependencies, reports or
artifacts. These are same-UID procedural receipts, not signed attestations or
protection against a malicious verifier/controller.

## Controls

Both optimization modes exercise a same-signature wrong callee body, reversed
inputs, reversed callee results, a corrupted continuation table target, changed
module version, changed dependency bytes and substituted embedded-callee WAT.
The same-signature identity-reuse check is recorded separately. Version and
whitespace-only dependency changes retain the finite result while invalidating
reuse, so semantic coincidence cannot substitute for identity.

Post-check substitution deliberately executes the wrong embedded body through
the ordinary engine and shows why the caller source hash alone is insufficient.
The evidence verifier rejects reuse of the original subject/artifact binding.
This experiment does **not** claim ordinary `CoreEngine.prepare` refuses every
unproved WAT or that a production proof-to-loader admission path was added.

## Trust and coverage

| Edge | What is established here | What remains assumed/open |
|---|---|---|
| Component simulation | Lean source semantics and successful scalar helper transcription; explicit pre/callee/post premises | Accepted source to model, actual emitted Wasm and helper semantics |
| Horizontal linked model | Conditional exact-identity/frame/continuation theorem | Real compiler and loader supplying its premises |
| Vertical composition | Generic local forward-simulation composition rule | Concrete simulations across Rust/IR/Wasm/loader layers |
| Real lowering, including optimization | Finite actual CLI/Wasm comparisons in both modes | C8 / O1 and optimizer preservation; no universal backend theorem |
| Final-byte loading | Retained engine load hashes match emitted final bytes for the exercised artifacts; substitution is distinguished | C8 / O3 loader/ABI correspondence theorem |
| Identity to semantics | Exact source/dependency/artifact reuse checks plus conditional model composition | O7 still needs real lowering/loading and admission-refinement evidence |

This is bounded progress on **#7 C8 and #10 O1/O3/O7 only**. The
[reusable-call design](../../.cairn/changes/reusable-contract-calls/design.md) and
[artifact-correspondence design](../../.cairn/changes/artifact-correspondence-extension/design.md)
remain completed designs; this pilot neither reopens them nor implements their
full profiles. PAA-1 still excludes definitions/imports. PO-17/18, C8 and the
corresponding real implementation obligations remain open; no conformance case
or proof-ledger status is promoted from these finite receipts.
