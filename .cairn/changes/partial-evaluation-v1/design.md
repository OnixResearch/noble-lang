## Context

The repository has three pieces of issue #32 already:

1. **A model proof.** `proofs/foundation/NobleFoundation/Specialize.lean` defines `rank`
   (the number of dynamic slots strictly below a slot), `merge σ d` (reconstruct the full
   input assignment), `staticStep`/`staticValue` (the statically known `I64` result of a
   node), `specializeNode`/`specialize` (node-index-preserving substitution and folding) and
   proves `staticValue_sound`, `specialize_wellFormed`, `congruenceWithInputs`,
   `residual_specialize_sound` and `specialize_sound`:
   `∀ i, value (specialize g σ) d i = value g (merge σ d) i`. Its fixtures cover no, all and
   mixed static inputs, a non-prefix static slot, deeper-minus-top subtraction order and the
   `2^64 - 1 + 1` wraparound; `prefix_merge_control` shows that treating static slots as a
   prefix disagrees with `rank` on the non-prefix fixture. The model folds `add`/`sub`/`mul`
   only and has no budgets, no program surface and no effects.
2. **A checked graph pipeline.** `crate::extract` (`admit` then `extract`) produces a
   `CheckedGraph` from an independently kernel-accepted `Submission`; `crate::check` is the
   independent graph checker; `crate::eval::reference` (L3) and `crate::compiled::Compiled`
   (L4) are the two evaluators. Extraction creates no guards and no facts, and the
   `ngraph-i64/0` checker admits only `Input`, `Const`, `I64Add`, `I64Sub`, `I64Mul`,
   `I64Eq` and `Guard` nodes.
3. **A frontend with a `let` sugar, behind a profile.** `crates/noble-contracts/src/source/
   locals.rs` lowers `let name` to checked `dup`/`swap`/`dip` acquisitions. It runs only when
   the session is a live-slot session or a declared-text session
   (`crates/noble-contracts/src/source/preparation.rs:76-82`). Measured on base `fed77835`:
   `noble compile` on `let a a a *` with `--input-type I64` reports `stage: resolve`,
   `outcome: unbound-word`; the same file with `--declared-modules --text-byte-cursor
   --input-type I64` compiles to Wasm. So `let` exists on the **Declared-Text-v1** ordinary
   profile and not on Core-Bootstrap.

What is missing is a Rust entry point, a residual Program with an ordinary acceptance path,
executed differential and negative evidence, and a measurement. This change supplies them
inside the decisions below; it changes no canonical requirement and no existing behavior.

## Decisions

### D1 — Subject slice: checked `ngraph-i64/0` graphs with flat scalar interfaces, no guards and no facts

**Choice:** `specialize` takes a `&CheckedGraph` (never a raw `Graph`) and accepts it only
when all of the following hold; otherwise it refuses before producing any node:

| Condition | Code |
|---|---|
| `profile != ngraph-i64/0` | `E-PROFILE` |
| a `Param` slot exists | `E-PROFILE` |
| `input_shape` or `output_shape` is present | `E-AGGREGATE` |
| any fact record exists | `E-PROFILE` |
| any `Guard` or `F32Index` node exists | `E-PROFILE` |
| static-map arity ≠ input-slot count | `E-INTERFACE` |

**Rationale:** extraction produces no guards, facts, params or shape trees for this profile,
so the accepted slice is exactly the extraction output surface plus directly authored
flat-interface graphs. Guards are refused because a guard is an *observation* (it fails
closed in canonical order) and its fact would have to be re-derived on the residual
interface; pairs/`Unit` are refused because a residual interface with a partially static
subtree has no scalar-constant representation (`Const` is a scalar, and `pair` exists only as
a word, not as a graph op). Refusing explicitly is the honest v1 boundary; silently
specializing a guard graph could drop a failing guard's observation.

### D2 — The static map is over the ordered flattened input leaves, and `merge` reconstructs the original assignment

**Choice:** the map is `&[Option<i64>]` of length equal to the number of `SlotKind::Input`
slots, in slot order. `check` admits exactly one `input k` node per slot, so input slots and
input leaves coincide. For a static slot the residual node is `Const(I64 v)`; for a dynamic
slot it is `Input(rank(k))`, where `rank(k)` counts the dynamic input slots strictly below
`k` — the Lean `rank` verbatim. The same module exposes `merge` so a call site can
reconstruct the original input vector for comparison:
`merge(σ, d)[k] = σ[k]` when static, else `d[rank(k)]`.

`rank` is `O(#inputs)` once per input slot and the substitution is one linear pass.

**Rationale:** this is the model's representation exactly, including the non-prefix case.
`prefix_merge_control` is the reason a prefix shortcut is not allowed.

### D3 — Whitelist: substitute and fold only fully static `add`/`sub`/`mul`/`eq`, with no reassociation

**Choice:** a node is folded to `Const` only when every operand is statically known:

- `I64Add(a, b)` → `Const(I64(a + b))` (wrapping),
- `I64Sub(a, b)` → `Const(I64(a − b))` (wrapping),
- `I64Mul(a, b)` → `Const(I64(a × b))` (wrapping),
- `I64Eq(a, b)` → `Const(Bool(a = b))`.

Every other node keeps its operation and its original operand positions. There is no
reassociation, no commutativity/identity rewrite (`x + 0`, `x × 1`), no common-subexpression
elimination, no constant propagation through `Bool` results back into `I64`, and no change to
node order. Statically known values are computed over the **original** node indices in
canonical order, so a fold can use a folded ancestor's value but a fold never feeds a later
node unless the later node's own operands are also static.

`E` (eq) folding is an extension beyond the model: `specialize_sound` does not fold `eq`.
Its soundness argument is direct (`I64` equality is pure, total and exact on wrapping
values), and it is exercised by differential tests, but it is recorded here as an unproved
extension, not as model-covered.

**Rationale:** the issue asks for a simple static-or-residual baseline first, and the model
fixes the substitution order. Algebraic identities are where "fold more" becomes a proof
obligation; excluding them keeps the residual a literal instance of the model.

### D4 — Prune nodes that reach no output, then renumber; guards and facts are refused, so pruning is unobservable

**Choice:** after substitution and folding, keep the nodes reachable from `outputs` in
canonical order, drop the rest, and renumber nodes densely in that order. Then run
`check::check_with` on the result and return its `CheckedGraph`. Every kept node charges the
`RESIDUAL-NODES` cap once, before pruning; pruning does not refund.

**Rationale:** pruning is not optional — the checker refuses a node that reaches no output
(`E-GRAPH-MALFORMED`/`dangling-node`), and folding creates exactly such nodes. The
observation argument is short because D1 refused every node whose evaluation can *fail* or
have a fact: the remaining ops (`Input`, `Const`, `I64Add/Sub/Mul/Eq`) are pure and total, so
a node that no output depends on cannot change any output or any failure. This argument is
not in the Lean model (which keeps indices fixed); it is a separate, documented step backed
by the whitelist restriction and by differential tests, not by a theorem.

### D5 — The residual is a new identity, never a mutated original

**Choice:** the residual graph's `Subject` is `SubjectKind::Extracted`, derived by
`Subject::derive` from domain parts `"partial-evaluation"`, the accounting revision string
(D7), the original graph's `GraphLogicalId` (32 bytes), and the encoded static map
(one `u8` tag plus an `i64` per input leaf); its `closure` is copied from the original. The
derivation record also carries both graph ids and the residual source digest, so no consumer
has to trust a producer's claim that two graphs are related.

**Rationale:** #33 needs to distinguish "the same Program" from "a residual of this
Program"; a distinct subject plus an explicit record is the only representation that keeps
`GraphLogicalId` honest. A residual can never be mistaken for the original because both its
nodes and its subject differ.

### D6 — The observation relation is `≡obs`, and the claimed relation is per dynamic vector

**Choice:** the checked claim is, for every vector `d` of the dynamic inputs,
`obs(residual, d) ≡obs obs(original, merge(σ, d))`, where `≡obs` is
`eval::Obs::obs_eq` (bitwise outputs except one NaN class; refusals by code and site;
`G-FACT-FALSE` by node and fact; exhaustion by kind; cancellation). Because D1 admits no
guard, no fact and no effect, the two sides are either both `Normal` with equal outputs or
both a preflight refusal of the same code.

The implementation does **not** verify this relation; it reports the residual, and the test
suite exercises the relation on finite vectors. No quantifier, no universal claim and no
refinement proof is asserted by the code.

**Rationale:** #32's acceptance item 2 requires the residual on the ordinary path and item 4
requires a differential; a proof of Program equivalence is a separate obligation and is not
in this change's scope.

### D7 — Accounting revision `ngraph-pe/0` with explicit caps; never claim equal fuel

**Choice:** `specialize::ACCOUNTING_REVISION = "ngraph-pe/0"`. Two caps:

| Cap | Default | Charged | Refusal |
|---|---|---|---|
| residual nodes created, before pruning | `Budgets::default().live_nodes` (262,144) | one per created node | `E-BUDGET-RESIDUAL-NODES` |
| residual variants per request | 1 | one per accepted residual | `E-BUDGET-VARIANTS` |

Both are ordinary `/0` budgets: a caller may lower them and may not raise them
(`validate_policy`, `E-PROFILE`). The residual's own re-check charges the ordinary checking
budgets (`live_nodes`, `inputs`, `outputs`, `layers`, …), so a residual that is larger than
the checker's policy is refused by the checker, not by the specializer.

Per H-LIMIT-03 the two sides are reported separately and no equality is implied: the
original consumes one input per leaf, the residual consumes one input per dynamic leaf, and
the residual's evaluation visits its own subset of nodes. The measurement component reports
`original.nodes`, `residual.nodes`, `folded`, `pruned`, evaluator node visits on each side,
and elapsed time; it never normalizes one side's budget against the other.

**Rationale:** the accounting revision must identify the units a replay claim depends on
(H-LIMIT-02/03), and an honest cap on the work the transformation itself can create is what
makes it bounded rather than "as large as the input".

### D8 — Diagnostics reuse the published vocabulary; stage order is the safety argument

**Choice:** no new diagnostic code prefix. Every refusal is one of the existing D6 codes
listed in D1 plus the two new budget kinds of the published `E-BUDGET-<kind>` family. The
pipeline order is fixed, and the report's existing `stage` field names the reported stage:

1. `specialization` — the `/0` policy, the subject slice (D1), the static-map arity and caps
   (D2, D7), in that order, and then the residual's independent re-check. The code names the
   rule that fired (`E-PROFILE`, `E-INTERFACE`, `E-AGGREGATE`, `E-BUDGET-*`); the stage
   names the phase.
2. `source` — residual emission (D9);
3. `admission` — independent re-acceptance of the emitted Program (D9);
4. `publication` — file publication, only after 3 succeeded.

A refusal in the first three stages publishes nothing. `noble ngraph specialize` reports the
stage in the existing report field, so a test asserts *where* it stopped, not only which code
came out.

Effectful, resource, recursive, aggregate and control subjects are refused by the profile's
**extraction** stage (`noble ngraph extract`, `crate::extract::admit`) exactly as before this
change, so they never become a checked graph and cannot reach `specialize` at all: the entry
point's parameter type is the mechanical guarantee, and the negative controls below check the
extraction stage and the ordinary path's zero-request accounting.

**Rationale:** the published code table is generated into the compatibility view and cited by
registered cases; adding a prefix would force a canonical change for no diagnostic benefit.
Stages are the mechanical form of "refused before any host request".

### D9 — The residual is emitted as `let`-based Declared-Text-v1 source and re-accepted before publication

**Choice:** emission walks the residual graph in canonical order and produces one `let`
binding per node, exactly as the source sugar's own discipline requires:

- the program's stack effect is `dynamic inputs → outputs`, bottom-first;
- inputs are bound first, deepest last, so every input becomes a hidden local;
- for each node in canonical order, the operands are read (the sugar emits the acquisition
  chain), the operation is applied, and the result is bound with `let v<node>`;
- constants are emitted as decimal `I64` literals or `true`/`false` for `Bool`;
- outputs are read in output order after all bindings; the sugar's scope cleanup drops the
  hidden locals beneath them.

The label names are `v<index>` and the caller supplies nothing but the residual's interface
types (derived from `CheckedGraph::ty`). The emitted text is then re-prepared with
`noble_contracts::source::ModuleSession::new_declared_text` under the ordinary source limits,
its `submission()` is re-admitted with `extract::admit` (the kernel independently accepts it),
re-extracted, and only then published. A refusal at re-preparation or re-admission is
reported as that refusal; nothing is written.

**Why Declared-Text-v1 and not Core-Bootstrap:** `let` is only lowered in live-slot and
declared-text sessions (Context), and live-slot dispatch is refused by extraction
(`E-LIVE-SLOT`). Emitting raw `dup`/`swap`/`dip` stack code instead would be a different
representation with worse sharing behavior and a much larger emitter; Declared-Text-v1 is an
ordinary selected profile with a real Wasm compile path, so the residual stays a Program.

**Rationale:** "return the new residual Program" (issue #32) is only true if the residual is
accepted by the same frontend and kernel as any other Program; publication before acceptance
would let a bad residual escape.

### D10 — Public surface: library entry point and `noble ngraph specialize`, no syntax

**Choice:** `noble_ngraph::specialize::{specialize, merge, emit_source, readmit,
ACCOUNTING_REVISION, Limits, Request, Report, Residual}` plus
`noble ngraph specialize GRAPH --static INDEX=VALUE,... --out RESIDUAL [--graph-out GRAPH]
[--budget-* N]...` with the existing `noble-ngraph-report/v1` schema, the existing D8 exit
mapping, existing file bounds and create-new publication. `--static` names 0-based input-leaf
indices, which is the same order as the graph's `input` slots and as the static map. No new
source syntax, no new kernel word, no ambient policy.

**Rationale:** every existing subcommand in this profile is a thin composition root over the
library; a second parsing or evaluation path here would be a second contract.

### D11 — The derivation record carries what #33 needs

**Choice:** `Report` (returned with the residual) records, in stable field order:
`accounting` (`ngraph-pe/0`), `original_graph` (32-byte id), `residual_graph` (32-byte id),
`static_inputs` (the accepted map, indexed by input leaf), `dynamic_inputs` (the residual
interface types), `original_nodes`, `folded`, `pruned`, `residual_nodes`, `variants`, plus, at
the CLI, `source_sha256` of the emitted text and the re-admitted graph id. This is the set #33
binds in its build key: subject identity (D5), static map, accounting revision, both graph
ids and the residual source digest. Nothing in the record is trusted by a consumer: the
consumer recomputes the ids from files.

**Rationale:** a cache keyed on a producer's self-reported relation is exactly the defect #33
must avoid; the record is an index into recomputable facts.

## Effect exclusions

- No effect enters: admission already refuses a non-empty root effect bound (`E-EFFECT`)
  and every effectful word (`test.emit`, `test.abort`) is refused by the kernel's pure
  fragment; `specialize` therefore never sees an effectful subject, and the residual has an
  empty effect bound by construction.
- `quote`/`compose`/`run`/`reflect`/`if`/`case`/`list.case` are outside the `ngraph-i64/0`
  op whitelist and are refused at admission (`E-CONTROL`, `E-DYNAMIC-RUN`, `E-REFLECT`).
- Resources, live references, `Program` values and `Unit`/pair interfaces are refused at
  admission (D1 and `extract`'s own `E-RESOURCE`/`E-PROGRAM-OUTPUT`/`E-DYNAMIC-RUN`).
- The specializer itself is pure: `noble-ngraph` is a `no_std` + `alloc` domain-core crate
  that executes no filesystem, process, environment, clock, thread or output effect, and the
  CLI module spawns no process, executes no Wasm and opens no network connection.

## Budgets and H-LIMIT accounting

- Screening (admission) is the existing D7 `Budget` (`work_units`, `expanded_nodes`,
  `inline_depth`), unchanged and charged by `extract`.
- Transformation is `ngraph-pe/0`: `RESIDUAL-NODES` and `VARIANTS` (D7). Both are charged
  before the residual exists, so an exhausted request returns no graph.
- Residual admission is the ordinary checker's `Budgets` (`live_nodes`, `layers`, `inputs`,
  `outputs`, `facts`, `graph_bytes`, …), with the same `/0` policy and the same
  `E-BUDGET-<kind>` refusals.
- Evaluation is the existing runtime `Fuel` budget and the existing `evaluated_nodes`
  counter, unchanged on both sides.
- `H-LIMIT-01/02/03`: the profile statement, the interruption points and the
  no-equal-budget rule above are the accounting statement. A measurement compares reported
  counts and times; it does not assert that the residual is cheaper in *every* accounting.

## Residual source representation (worked example)

Graph `%0 = input 0; %1 = input 1; %2 = i64.mul %0 %1; %3 = i64.add %2 %0; out %3` with
`σ = [Some 5, None]` becomes `%0 = const 5; %1 = input 0; %2 = i64.mul %0 %1;
%3 = i64.add %2 %0; out %3` (the same indices: substitution folded nothing here), and its
`let` source is

```
let v1
5 let v0
v0 v1 * let v2
v2 v0 + let v3
v3
```

with one dynamic `I64` input. The residual's interface is `[I64] → [I64]`; the original's is
`[I64, I64] → [I64]`.

## Test plan

The frozen protocol (committed before any measurement it governs) is:

- **Fixtures.** (a) the Lean model's five fixtures transcribed to `ngraph-i64/0` graphs
  (no/all/mixed static inputs, non-prefix static slot, deeper-minus-top, wraparound);
  (b) the prefix-merge negative control, which must disagree; (c) seeded random Core
  programs from the existing B2 generator, one static map derived from the program's own
  input positions and one empty map.
- **Differential.** For each fixture and 1,000 seeded vectors per graph: L3 and L4 of the
  original under `merge(σ, d)`, L3 and L4 of the residual under `d`, L3 and L4 of the graph
  re-admitted from the emitted residual source under `d`, and (when `NOBLE_NODE` is set) the
  Node BigInt oracle on the original. Every pair must agree under `≡obs`. Agreement is
  finite differential evidence, not a proof.
- **Negative / zero host requests.** effectful (`E-EFFECT`), resource (`E-RESOURCE`) and
  recursive (`E-RECURSION`) subjects from the existing B3 control set are refused before any
  graph exists, so `specialize` — which takes an admitted graph — is unreachable for them;
  the CLI controls assert the code, the stage and that no output file exists. The zero-host
  claim is a property of the profile, not a counter in its report: `noble-ngraph` executes no
  host effect and the command spawns no process. The Core-Bootstrap profile is *not* the
  same profile — it grants `test.emit`/`test.abort` test hosts and really does execute
  `"hi" test.emit 5` (measured: one guest request) — so a Core session report is never used
  as evidence for this profile, and the controls record that distinction explicitly.
- **Budget ladder.** below/at/above each cap: a `RESIDUAL-NODES` refusal at
  `residual_nodes - 1`, acceptance at exactly `residual_nodes`, and a `VARIANTS` refusal at
  `0`; plus a raised cap refused with `E-PROFILE`.
- **Measurement.** Per fixture: `original_nodes`, `residual_nodes`, `folded`, `pruned`,
  total L3 and L4 node visits on each side over the vectors, retained bytes of each compiled
  graph, and specialization elapsed time measured only in the harness (the library reports
  counts, never time). Reported in a receipt; no threshold and no adoption decision.

## Risks / Trade-offs

- **The `eq` fold is unproved (D3).** Mitigated by keeping it in the same "all operands
  static" rule as the proved folds and by differential tests; reopen if a future obligation
  requires model coverage — the fix is to remove the fold or extend `Specialize.lean`.
- **Pruning is not in the model (D4).** Mitigated by refusing guards and facts, which are the
  only failure points, so the remaining ops are pure and total; reopen if a guard-preserving
  residual is required (it needs fact re-derivation, a separate change).
- **Declared-Text-v1 dependency (D9).** The residual rides a profile whose `let` lowering is
  itself an ordinary checked frontend transform; if that profile's limits
  (65,536 bytes per declared input) are exceeded the residual is refused, not truncated. A
  Core-Bootstrap residual would need a raw `dup`/`swap` emitter; that is a separate change.
- **Re-extraction changes node ids** (pruning renumbers), so nothing may compare the residual
  to the original by node index. The differential compares outputs; the derivation record
  names ids.
- **No pairs/`Unit` support (D1)** means authored graphs with shape trees are refused; only
  extraction's flat interfaces are in scope. #35, which needs `Bool` composition, stays
  blocked and its NO-GO record is untouched.

## Open questions (owner-reserved, recorded not decided)

1. The accounting revision string `ngraph-pe/0` and the two budget-kind names are this
   change's proposal; a different naming convention is a textual change to D7 and the delta.
2. Whether a future revision should preserve the original node indices in the residual
   (making it a literal instance of `specialize_sound`) at the cost of carrying dangling
   nodes in a supergraph instead of a pruned graph.
3. Whether `I64Eq` folding should be added to `Specialize.lean` (with a Bool-valued static
   map) rather than remaining an implementation extension.
