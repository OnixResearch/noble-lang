# `ngraph-cert-scoped/0`: guard-scoped I64 rewrites

This document specifies the separately versioned scoped-rewrite extension requested by issue #108. It does not widen `ngraph-cert/0`; the existing format retains its three-rule vocabulary and meaning. The implementation is `crates/noble-ngraph/src/cert/scoped.rs`, and the independent fragment model is `proofs/foundation/NobleFoundation/GuardedRewrite.lean`.

## Certified relation

The only accepted relation kind is **observation equality**. For each input admitted by the unchanged input domain, and assuming sufficient execution budgets and no cancellation, the original and rewritten endpoints must have equal observations under `Obs::obs_eq`:

- normal observations agree on all ordered outputs;
- a preflight refusal agrees on its refusal code and site;
- a guard failure agrees on the identity and fact of the **first** failing guard;
- exhaustion is outside the theorem's sufficient-budget premise, and checker exhaustion is never an acceptance;
- cancellation is outside the relation's no-cancellation premise.

`ScopedEquality` (agreement only when the scope holds) and `DirectedRefinement` are named relation kinds but are unsupported, not substitutes for equality. A scoped fold may change a value on an input where its premise is false; that input is safe only when the unchanged preceding guard is itself the first observed failure. A later guard cannot justify an earlier rewrite.

## Supported rule and scope

A step may replace node `m = op(a, b)` **in place** with `Const(claimed)`, for wrapping `I64` add, subtract or multiply, only if each operand has one checked value from one of these sources:

1. the operand node is an `I64` graph constant;
2. it is an interface input with an exact, single-value `InputValidated` fact (preflight scope); or
3. a guard at a node index strictly below `m` names that operand and has an exact, single-value `Range` fact (guard scope).

Every available single-valued justification for an operand must agree. A stale/derived fact is not an input-domain premise. The checker re-derives the selected justification, operand pair, operand values and wrapping result; it does not trust the witness's claims.

The witness binds the format revision, relation kind, left and right graph identities, ordered step sequence, rule, rewritten node, ordered operand identities, per-operand scope records, per-operand values and claimed result. Step nodes must be strictly increasing. Node and guard identities are unchanged; there is no old-to-new map.

The endpoints must preserve profile, slots, input/output shapes, outputs, fact records and subject. Every guard remains at its original node with its original fact and value. A changed input domain, removed/reordered/retuned guard, or substituted endpoint is refused.

## Outcomes and work bounds

The checker distinguishes:

- `Accepted`: the exact endpoints and witness establish the supported equality relation;
- `Refused`: a witness or endpoint field is invalid or its premise is unavailable;
- `Unsupported`: the relation or operation is outside this revision;
- `Exhausted`: the derivation exceeds 64 steps or a witness scan exceeds 4096 nodes;
- `Cancelled`: a caller's `check_with_cancel` probe stops before acceptance.

A refusal is not a claim that the transformation is wrong on every input. Unsupported, exhausted and cancelled work never count as acceptance. `check` uses no cancellation probe; `check_with_cancel` polls after bounded endpoint preflight and between steps. The observation theorem excludes cancellation and assumes enough budget.

## Required negative controls

The version's acceptance tests include all of the following:

- a future guard cannot justify an earlier rewrite;
- a wrong guarded value or result is refused;
- a derived/stale fact cannot stand in for an admitted input-domain fact;
- removing a guard is refused;
- changing input facts/domain is refused;
- substituting the claimed result is refused;
- scoped-only and directed-refinement relation claims are unsupported, including a reversed
  directed-refinement endpoint pair;
- dropping the guard assumption makes the equality witness unavailable;
- work bounds and cancellation produce explicit non-accepted outcomes.

The guarded and preflight pilots use an `Input` operand so their guard or input fact is load-bearing. The two-guard execution fixture compares complete reference and compiled observations for inputs that satisfy both predicates, violate either predicate, or violate both, including cases where the rewritten value is shared.

## Evidence boundary

`GuardedRewrite.lean` proves the stated observation theorem for its own small input-fact/guard fragment and kernel-checks the named controls. `GuardedRewriteCorpus.lean` is generated from actual Rust derivations; its `decide` theorems establish acceptance by the model checker for those encoded certificates. This is **model proof**, not a theorem connecting Rust bytes or execution to Lean.

The Rust tests separately establish concrete behavior for the named fixtures by running both endpoints through `eval::reference` and the compiled evaluator and comparing full `Obs::obs_eq` observations. Those are **finite concrete tests**, not a universal proof. No implementation-correspondence theorem binds the Rust checker or either evaluator to the Lean fragment. The packed-schedule proof in `PackedSchedule.lean` / `PackedScheduleCorpus.lean` is a separate claim and is not extended by this certificate format.
