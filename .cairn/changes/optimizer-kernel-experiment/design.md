## Context

Grounded at worktree HEAD `534f0e02a` (issue #58 cites `main` as inspected at
filing time; no cited line drifted in substance):

- `crates/noble-ngraph/src/train.rs` declares `ADAM = "adam/0"`, whose recorded
  convention is `binary64 RNE, no FMA, running beta products, no pow; no weight
  decay/clipping/clamp`; `AdamState` holds `m`, `v`, running `β₁ᵗ`, `β₂ᵗ` and a
  step counter, all binary64, and is "immutable once published" with a digest
  over exact state bytes.
- `crates/noble-ngraph/src/lib.rs` states that `F32` graphs are authored directly
  in `ngraph-ir/0` and admitted only by `check`, and that source extraction
  exists for `I64` only. No F32 source-level optimizer code exists.
- `crates/noble-ngraph-experiment` is the host-side evidence/measurement
  boundary, not a proposer.
- #14 owns the numeric graph and training contract; #38 owns the Lean-checked
  binary32 reference; #44 numerical-error certificates; #45 the reverse-AD
  reference. This package adds candidate optimizer-program admission plus
  explicit optimizer-state transition semantics, and reuses those obligations
  rather than restating them.

## Decisions

### Decision: a typed transition over host-owned state, not a trainer language

**Choice:** A candidate is a pure, fixed-shape transition
`(params, gradients, optimizer state, bounded step descriptor) → (fresh
parameters, fresh optimizer state)`. All data access is explicit in the
transition's declared inputs; there is no ambient handle to the model graph,
loss, dataset, evaluator or accepted history.

**Rationale:** the host remains the only authority that owns checkpoints,
publication and evaluation. A transition that cannot name the graph cannot
mutate it, so "keep the model immutable" is a type property rather than a
convention.

### Decision: F32 SGD-like reference first; binary64 is a separate algorithm

**Choice:** the pilot's reference update is a deliberately `F32`-only SGD-like
rule over `F32` parameters and gradients. A binary64-moment candidate is a
*distinct algorithm* specified separately, and `adam/0` evidence MUST NOT be
reused to certify host f64 arithmetic.

**Rationale:** `adam/0` converts binary64 moments into binary32 parameters. An
F32-only candidate is not an equivalent lowering of it. Selecting the F32 rule
first keeps the model small enough to check and avoids a silent numeric
substitution. Reusing `adam/0` semantics would require modeling binary64
moments, beta products and the conversion point, which is deferred.

### Decision: separate obligations, one shared admission envelope

**Choice:** model theorems (Lean, kernel-checked), checker obligations and finite
Rust/executed-artifact comparisons are named as three distinct obligation
classes. The admission envelope records which class produced which claim, and
no class may stand in for another.

**Rationale:** type correctness does not prove termination, memory bounds,
finite outputs, descent or generalization. Each selected property needs its own
statement; #38/#44/#45 already provide candidate references for the arithmetic
and derivative slices.

### Decision: budgets refuse rather than truncate

**Choice:** nodes, state bytes, work and outputs are declared bounds. A candidate
that exceeds them is refused with an `exhausted` diagnostic; nothing is
silently truncated or partially applied.

**Rationale:** a truncated candidate would make the published snapshot's
provenance ambiguous. Distinct `unsupported`, `malformed`, `exhausted` and
`internal` outcomes keep a budget refusal from being read as a program error.

### Decision: host-owned rollback, append-only attempts

**Choice:** whole-training rollback is host custody: RNG state, data cursor and
counters are restored together with parameters and optimizer state. Attempt
records are append-only and survive a rollback; an unavailable checkpoint fails
closed rather than resuming from a partial snapshot.

**Rationale:** OPAL's recovery undoes only some later updates and still needs an
available snapshot. A rejected trial must restore the *entire* trial or refuse.

## Minimal candidate IR

This section fixes the complete pilot envelope. A checker MUST reject an
envelope that does not use these types, limits, opcodes, effects or output
shape; none of them are candidate-selected parameters.

### Values and transition interface

`F32` is one `Binary32.Bits` value: exactly 32 bits, interpreted by the
`NobleFoundation.Binary32` model. `F32[n]` is an ordered vector of exactly
`n` such values. `n` is declared in the envelope and MUST satisfy
`1 <= n <= 64`; vector lane order is the serialized order, from lane `0`
through lane `n - 1`. No decimal or host floating-point representation is
accepted: every scalar is an exact binary32 bit pattern.

The hyperparameter `eta : F32` is one exact binary32 bit pattern. The pilot
state type is `State0 := {}` (an empty, owned persistent state); its serialized
size is exactly zero bytes. The transition has the following complete typed
interface:

```
inputs:  p : F32[n], g : F32[n], eta : F32,
         step : StepDescriptor, state : State0
outputs: p' : F32[n], state' : State0
```

`StepDescriptor` is the fixed nine-byte record
`{ version : u8, step_index : u32, batch_index : u32 }`, encoded in that
field order as unsigned little-endian integers. `version` MUST equal `1`.
`step_index` and `batch_index` are bounded by `0 <= value <= 2^32 - 1`.
They are metadata inputs for snapshot binding; the pilot whitelist has no
integer or branching opcode, so the numeric transition cannot inspect or
modify them. `state` and `state'` MUST both have the empty `State0` shape.

### Grammar, whitelist and effects

The canonical envelope is

```
Envelope ::= {
  format: "ngraph-opt/1", n: u32,
  inputs: ["p", "g", "eta", "step", "state"],
  state: "State0", effects: [],
  budgets: {nodes: 16, state_bytes: 0, work: 128, output_lanes: 64},
  nodes: [Node], outputs: {parameters: NodeId, state: "State0"}
}
Node ::= {id: NodeId, op: Op, operands: [NodeId], type: F32 | F32[n]}
Op ::= constant(bits: Bits32)       // result type F32
     | broadcast(NodeId)            // F32 -> F32[n]
     | Binary32.mul(NodeId, NodeId) // F32[n] × F32[n] -> F32[n]
     | Binary32.sub(NodeId, NodeId) // F32[n] × F32[n] -> F32[n]
```

`NodeId` is a unique unsigned 16-bit identifier. Nodes are listed in
topological order; an operand MUST be an earlier node or one of the declared
inputs. `constant` embeds one exact `Bits32` value and `broadcast` replicates
that value in the serialized lane order. The only arithmetic opcodes are the
model functions `NobleFoundation.Binary32.mul` and
`NobleFoundation.Binary32.sub`, applied independently to corresponding lanes.
`constant` and `broadcast` are data constructors, not additional arithmetic.
The whitelist is therefore exactly `{constant, broadcast,
Binary32.mul, Binary32.sub}`. The effect set is exactly `[]` (empty); an
effect field, capability, foreign handle, memory access or host call is not
part of this grammar.

### Fixed budgets

The envelope MUST carry these values verbatim, and the checker MUST measure
the candidate against them rather than trusting a candidate-supplied larger
limit:

| Resource | Pilot budget | Measurement |
|---|---:|---|
| nodes | 16 | total `Node` records, including constructors |
| persistent state bytes | 0 | serialized `State0` bytes per snapshot |
| work | 128 | scalar `Binary32.mul` or `Binary32.sub` lane invocations; a vector node costs `n`, constructors cost 0 |
| outputs | 1 vector, at most 64 lanes | exactly one `p' : F32[n]` and one empty `state'`; no auxiliary output |

The shape bound makes the output-lane budget equivalent to `n <= 64`, but
both checks remain explicit in the refusal record. A candidate over any budget
is refused as `exhausted`; it is never truncated or partially evaluated.

### Reference update and nonfinite rule

For every stored lane `i` in order `0, ..., n - 1`, the reference is exactly

```
p_i' = Binary32.sub p_i (Binary32.mul eta g_i)
```

`Binary32.mul` is evaluated first and rounded once; its result is then passed
to `Binary32.sub`, which rounds once. This is exactly two roundings per lane,
with no FMA or contraction and no reassociation. The stored parameter vector
and the stored gradient vector are consumed in their serialized order, and
the output vector is written in that same order. `state'` is the fresh empty
`State0` value.

The reference delegates signed-zero, NaN and infinity behavior to the #38
model functions without host-float substitutions: `Binary32.mul` uses
`classify`, `isNegative`, `zeroOf`, `infinityOf` and `canonicalNaN`, while
`Binary32.sub` is the model's sign-flipped-right-operand definition
(`add x (y ^^^ 0x80000000)`). Finite results use that model's
round-to-nearest, ties-to-even rounding and gradual underflow. Thus signed
zero signs, canonical NaN (`0x7FC00000`), infinity signs and invalid
infinity operations have the model's exact outcomes whenever the reference is
evaluated.

The pilot's admission rule is **refuse nonfinite inputs and outputs**. Before
evaluating a candidate, the checker applies `Binary32.classify` to every
`p_i`, `g_i` and `eta` and refuses any `infinity` or `notANumber` at the
corresponding input path. `subnormal` is finite and is allowed. After the
ordered transition is evaluated, the checker applies the same test to every
output lane and refuses an `infinity` or `notANumber` at
`$.outputs.parameters[i]`. This keeps the selected experiment's accepted
transition finite while retaining the #38 signed-zero and subnormal boundary
semantics. Refusal is preferable to silently accepting a candidate whose
NaN/overflow behavior is not a useful optimizer comparison; the model remains
the sole definition for diagnostic/reference evaluation. An implementation
MUST NOT replace this rule with host `isfinite` behavior that collapses signed
zero or subnormals.

### Refusal envelope

Every non-admitted result contains one of these classes and a JSON-pointer
`location` into the submitted envelope. The pointer is the first offending
location, not a generic error string.

| Class | Condition | Envelope location |
|---|---|---|
| `malformed` | missing or wrong `format`, `n`, input list, state declaration, budget field, node shape/operand count, node type, output, or `StepDescriptor` field | the exact missing/wrong field, e.g. `$.n`, `$.nodes[3].operands`, or `$.outputs.parameters` |
| `unsupported` | opcode/effect/capability outside the whitelist, a nonfinite input or output, a foreign handle, or a shape/state mismatch | exact offending field, e.g. `$.nodes[2].op`, `$.effects[0]`, `$.inputs.g[7]`, `$.outputs.parameters[7]`, or `$.outputs.state` |
| `exhausted` | node count > 16, state bytes > 0, measured work > 128, or output lanes > 64 | the resource field: `$.nodes`, `$.state`, `$.work`, or `$.outputs` respectively |
| `internal` | checker/reference failure not attributable to candidate content | `$.envelope` (the whole envelope; no candidate field is blamed) |

An internal refusal is still fail-closed and publishes no snapshot. For every
other refusal the exact pointer and observed value are retained in the
append-only attempt record. No refusal class may be converted into an
admission or a partially applied transition.

## Risks / Trade-offs

- A tiny opcode set limits what proposals can express; that is deliberate, and
  widening it is a separate reviewed change rather than a parameter.
- Shape/state interfaces may not fit later optimizer families (e.g. per-layer
  schedules). The design keeps the step descriptor bounded and versioned so a
  wider interface is an added requirement, not an edit to this one.
- `F32`-first selection means the pilot cannot claim parity with `adam/0`
  quality; any comparison against a tuned fixed optimizer is a future
  preregistered experiment.
- Rollback custody depends on the host actually holding RNG and data-cursor
  state. A host that drops them is outside this premise, not kernel-enforced
  safety.

## Fixtures and plan

Phase 2 (future, unchecked here) would add:

- one fixed simple reference update that admits, publishes a fresh parameter
  and state snapshot, and leaves the parent snapshot byte-stable;
- rejected candidates: malformed envelope, effectful operation outside the
  whitelist, over-budget nodes/state bytes/work/outputs, shape mismatch,
  aliasing attempt, nonfinite input, invalid state transition (step counter
  going backwards or a non-`State0` state); the selected pilot rule refuses
  nonfinite inputs and outputs rather than propagating them;
- boundary rounding values and a nonfinite control, tied to the #38 reference
  rather than to finite differences;
- extension and rollback controls: an early-update and a later-update fault must
  each restore the whole trial, with append-only attempt records preserved and
  an unavailable checkpoint failing closed;
- evaluation-tampering controls: a candidate and a proposer that attempt to read
  held-out data, evaluator internals or secrets, and a proposal that attempts to
  influence its own evaluation.

## Proof obligations

Named here, not discharged by this package:

- **Lean model theorems** for the selected F32 SGD-like transition: the
  transition is total on admissible shapes, produces finite outputs where
  declared, preserves the declared shape and leaves untouched slots unchanged.
- **Checker obligations** for admission: envelope bounds, whitelist closure,
  shape/state agreement and the state-transition predicate. Acceptance implies
  the model's well-formedness premises.
- **Finite Rust comparisons** between the ordered reference and the reference
  update, labeled implementation evidence rather than proof.

Arithmetic reference: #38. Numerical-error certificates: #44. Derivative
policy: #45. This package does not restate their obligations and MUST NOT
certify host binary64 arithmetic from the F32 reference.

## Acceptance mapping

| Issue criterion | Where |
|---|---|
| Minimal typed IR, shape/state interface, whitelist, budgets, arithmetic/rounding/nonfinite rules, unsupported diagnostics | NGRAPH-OPT-01, NGRAPH-OPT-02 |
| Lean semantics as reference for the selected slice; separate model/checker/finite classes | NGRAPH-OPT-03, proof obligations above |
| Fixed reference update and rejected malformed/effectful/over-budget candidates; shape/aliasing/nonfinite/boundary/state controls | NGRAPH-OPT-03, fixtures |
| Fresh immutable snapshots binding optimizer code, parents, graph/loss/derivative identities, seed, data order, numeric semantics, tool revisions; byte-stable old snapshots | NGRAPH-OPT-01, NGRAPH-OPT-04 |
| Host-owned whole-training rollback; fail-closed checkpoints; append-only attempts | NGRAPH-OPT-04 |
| Evaluation on frozen artifacts outside proposer authority; tampering controls | NGRAPH-OPT-05 |
| Preregistered tiny CPU comparison kept separate from semantic validation | NGRAPH-OPT-05 |
| Stop/continue recommendation | Phase 2 task |

## Non-claims

- No convergence guarantee, numerical-stability theorem, descent or
  generalization claim. Successful typing and bounded tests supply none.
- No arbitrary Python, in-process candidate execution or automatic trust.
- No claim that an `F32` candidate is an equivalent lowering of `adam/0`.
- No unproved compiler/executed-artifact correspondence, and no promotion of
  `NG-PO-A..H`.
- No production adoption, and no change to ordinary Noble Programs, the `I64`
  slice or `ngraph-ir/0` admission.
