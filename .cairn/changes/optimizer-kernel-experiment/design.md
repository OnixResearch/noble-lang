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
  whitelist, over-budget nodes/state bytes/work/outputs, shape mismatch, aliasing
  attempt, nonfinite input or output, invalid state transition (step counter
  going backwards, moments of the wrong arity);
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
