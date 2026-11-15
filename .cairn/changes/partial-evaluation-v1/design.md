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
The dynamic vector carries the residual leaf types: a `Bool` leaf cannot be
static under the I64-valued map, but remains a dynamic `Bool` in slot order;
the typed residual helper validates those values when reconstructing inputs.

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

The original `specialize_sound` theorem does not fold `eq`; D12's
`foldEqualities_sound` covers exact equality folding in the extended graph model.
The Rust fold is exercised by differential tests, but Rust/model correspondence
remains unproved.

**Rationale:** the issue asks for a simple static-or-residual baseline first, and the model
fixes the substitution order. Algebraic identities are where "fold more" becomes a proof
obligation; excluding them keeps the residual a literal instance of the model.

### D4 — Prune unreachable computations, preserve dynamic input slots, then renumber

**Choice:** after substitution and folding, keep the nodes reachable from `outputs` in
canonical order and every remaining dynamic `Input` node, drop other unreachable nodes, and
renumber nodes densely in that order. Preserving unused dynamic inputs keeps every residual
interface slot defined, as required by the graph checker. Then run `check::check_with` on the
result and return its `CheckedGraph`. Every transformed node, including nodes later pruned,
charges the `RESIDUAL-NODES` cap once before pruning; pruning does not refund.

**Rationale:** pruning is not optional — the checker refuses a node that reaches no output
(`E-GRAPH-MALFORMED`/`dangling-node`), and folding creates exactly such computed nodes. The
checker permits unused `Input` nodes but still requires one node for each interface slot, so
dynamic input nodes are retained even when no output uses them; statically substituted inputs
become constants and can be pruned when unreachable. D1 refuses nodes whose evaluation can
*fail* or carry a fact; the remaining ops (`Input`, `Const`, `I64Add/Sub/Mul/Eq`) are pure and
total. D12 proves ordered-output preservation for the model's pruning/renumbering step,
conditional on a valid survivor map. Rust's map construction and its evaluator remain
finite-test evidence, not theorem-covered implementation.

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

Every `noble ngraph` report also carries the existing `guest_requests` counter, which is
exactly zero for this command family: none of these commands enters the guest runtime. The
CLI-representable effectful negative test asserts the counter alongside the refusal code and
stage.

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
  hidden locals beneath them. An empty output vector is valid: cleanup leaves the residual
  with an empty stack effect. A zero-node residual with no inputs or outputs is emitted as
  `0 drop` rather than an empty source module, because the ordinary compiler rejects empty
  source; this effect-free body re-extracts to the empty checked graph.

The label names are `v<index>` and are derived from residual node indices; the caller supplies
the residual's interface types (derived from `CheckedGraph::ty`) and the ordinary source
limits. The emitted text is then re-prepared with
`noble_contracts::source::ModuleSession::new_declared_text` under caller-provided ordinary source limits,
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

### D10 — Public surface: library entry point and `noble ngraph specialize`, no source syntax

**Choice:** this change modifies `NGRAPH-CLI-01` to add `specialize` as its sixth exact
subcommand. It selects `noble_ngraph::specialize::{specialize, merge, emit_source, readmit,
ACCOUNTING_REVISION, Request, Report, Residual}` plus
`noble ngraph specialize GRAPH --static INDEX=VALUE,... --out RESIDUAL [--graph-out GRAPH]
[--budget-* N]...` with the existing `noble-ngraph-report/v1` schema, the existing D8 exit
mapping, `guest_requests: 0`, existing file bounds and create-new publication. `--static`
names 0-based input-leaf indices, which is the same order as the graph's `input` slots and as
the static map. `readmit` receives `noble_contracts::Limits` explicitly so its caller chooses
the ordinary frontend policy rather than a hidden library default. No new source syntax, no
new kernel word, no ambient policy.

**Rationale:** every existing subcommand in this profile is a thin composition root over the
library; a second parsing or evaluation path here would be a second contract.

### D11 — The derivation record carries what #33 needs

**Choice:** `Report` (returned with the residual) records, in stable field order:
`accounting` (`ngraph-pe/0`), `original_graph` (32-byte id), `residual_graph` (32-byte id),
`static_inputs` (the accepted map, indexed by input leaf), `dynamic_inputs` (the residual
interface types), `original_nodes`, `folded`, `pruned`, `residual_nodes`, `variants`, plus, at
the CLI, `source_blake3` (the BLAKE3 digest of the emitted text) and the re-admitted graph
id. This is the set #33 binds in its build key: subject identity (D5), static map, accounting
revision, both graph ids and the residual source digest. Nothing in the record is trusted by
a consumer: it recomputes the ids from files.

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
  re-admitted from the emitted residual source under `d`, and the Node BigInt oracle on the
  original, residual and re-admitted graphs. The BigInt leg is an ignored B2 test run with
  the pinned `NOBLE_NODE`. Every pair must agree under `≡obs`. Agreement is finite
  differential evidence, not a proof. The seeded B2 Core-program sample also tries 16
  programs under two static maps each; map pairs whose emitted `let` source exceeds the
  ordinary Declared-Text-v1 frontend node limit are expected `E-SUBJECT` refusals, not
  mismatches, and at least half of the pairs must be compared through both evaluators and
  re-admission. The ignored Node leg uses 1,000 vectors per accepted pair.
- **Negative / zero host requests.** effectful (`E-EFFECT`), resource (`E-RESOURCE`) and
  recursive (`E-RECURSION`) subjects from the existing B3 control set are refused before any
  graph exists, so `specialize` — which takes an admitted graph — is unreachable for them;
  the extraction controls assert the code and no graph. For the CLI-representable effectful
  refusal, the black-box test asserts `guest_requests: 0` and no output file. The
  resource-input subject and forged recursive subject are library-only:
  `ngraph extract` accepts scalar input types and cannot manufacture the resource subject; the
  recursive `Submission` is forged from accepted source then mutated before extraction, not
  constructed through public recursion syntax. The B3 resource row marks its ordinary Core path
  not expressible, and the recursive row separately records the ordinary-source frontend
  refusal; neither ordinary control is treated as evidence for the extraction refusal's
  `guest_requests` count.
  `noble-ngraph` executes no host effect and the command spawns no process. The
  Core-Bootstrap profile is *not* the same profile — it grants `test.emit`/`test.abort` test
  hosts and really does execute `"hi" test.emit 5` (measured: one guest request) — so a Core
  session report is never used as evidence for this profile, and the controls record that
  distinction explicitly.
- **Budget ladder.** below/at/above each cap: a `RESIDUAL-NODES` refusal at
  `residual_nodes - 1`, acceptance at exactly `residual_nodes` and one above; a `VARIANTS`
  refusal at `0`, acceptance at `1` and a raised cap at `2` refused with `E-PROFILE`. A cap
  above the default residual-node value is also refused with `E-PROFILE`.
- **Measurement.** Per fixture: `original_nodes`, `residual_nodes`, `folded`, `pruned`, total
  L3 and L4 node visits on each side over the vectors, retained bytes of each compiled graph,
  `specialization_ns` timed around only the checked-graph specialization call,
  residualization/readmission pipeline elapsed time, and evaluator time. `cold_eval_ns` is the first
  single-vector evaluation after graph construction; `warm_eval_ns_total` is one 1,000-vector
  batch after one untimed warm-up. Both have distinct L3/L4 rows for original, residual and
  re-admitted graphs. Timing is measured only in the harness (the library reports counts,
  never time); it is descriptive and has no threshold or adoption decision. The receipt binds
  the clean source commit and records the seed and vector count.

## Risks / Trade-offs

- **The model proves the equality-fold step but not Rust correspondence.** The residual model
  folds `eq` only when its two operands are exact I64 constants, and its evaluator uses exact
  BitVec equality. The paired Rust/Lean fixture covers both outcomes. The implementation's
  static-value table and the model definitions remain separate; no Rust-to-model or backend
  theorem follows.
- **Pruning is conditional on a valid survivor map.** The model proves preservation under
  explicit closure, output-retention, dynamic-input-retention, order and inverse-map premises.
  It does not derive those premises from the Rust keep-walk/map-building loops. The checked
  graph verifier and pinned fixtures remain finite implementation evidence. Reopen if the
  requirement is to establish the Rust map builder's universal validity or if a failing/effectful
  node enters the admitted slice; the latter also needs observation/fact preservation.
- **Declared-Text-v1 dependency (D9).** The residual rides a profile whose `let` lowering is
  itself an ordinary checked frontend transform; if that profile's limits
  (65,536 bytes per declared input) are exceeded the residual is refused, not truncated. A
  Core-Bootstrap residual would need a raw `dup`/`swap` emitter; that is a separate change.
- **Re-extraction changes node ids** (pruning renumbers), so nothing may compare the residual
  to the original by node index. The differential compares outputs; the derivation record
  names ids.
- **No directly authored graph shapes (D1):** authored graphs with shape trees still refuse.
  The separately selected checked-Program path (D13) can reconstruct bounded active
  Pair/Unit shapes after the full original Program is independently admitted. This
  distinction does not add Bool composition; #35's NO-GO record is untouched.

## Open questions (owner-reserved, recorded not decided)

1. The accounting revision string `ngraph-pe/0` and the two budget-kind names are this
   change's proposal; a different naming convention is a textual change to D7 and the delta.

## Remaining issue #32 boundary decisions (2026-11-14)

### D12 — Prove equality folding and pruning under an explicit survivor-map contract

**Choice:** `NobleFoundation.SpecializeResidual` models the post-substitution graph. It extends
the existing graph semantics with a Boolean constant, folds `eq` only when both source
operands are exact I64 constants, and defines dead-node removal plus dense renumbering from a
`PruneMap`. The map premises require output and operand closure, retention of every dynamic
input node, dense two-way index correspondence and topological operand order. The theorem
`residualize_outputs_sound` proves the ordered output values after the composition agree with
the original values under `merge`; it reuses the existing theorem for static input mapping and
wrapping add/sub/mul folding. A valid map can keep extra pure nodes: this theorem establishes
preservation, not minimality of the Rust reachability walk.

**Boundary:** this is a model theorem conditional on a valid `PruneMap`. It does not prove that
the Rust reachability and numbering loops construct a map satisfying every premise for every
admitted graph, nor does it establish Rust/backend correspondence. The frozen true/false Rust
fixture and the Lean fixture pin the same graph, static maps, live-node set, output order and
renumbered operands as finite evidence only.

**Reopen criteria:** extend the bridge only when #32 requires universal Rust-map construction
or actual backend refinement. A change that introduces guards/facts/effects must also prove
preservation of their observations before pruning; the current total I64/Bool whitelist does
not grant that claim.

### D13 — Opt-in checked-Program residualization with an opaque stack tail

**Choice:** `noble ngraph specialize SOURCE --source --interface 'TYPE ... --' --tail N`
selects the Program path; without `--source` the existing graph path is unchanged. The
library accepts the *entire* original Core submission and every reached definition through
the kernel, recomputes its exact closure and compares the supplied claim before extraction.
The first `N` bottom-first root-stack values are opaque, resource-free tail values. The
original input/output type prefixes must match; each is represented by a distinct symbolic
marker. Any operation that consumes, copies into an output, moves or otherwise changes a
marker refuses before any residual Program is returned. The active suffix may contain
scalar I64/Bool items or bounded nested `Pair`/`Unit` items. Its supported scalar leaves
are flattened bottom-first by item, left-to-right inside pairs; `Unit` contributes no
binding position. Only I64 leaves may be static. Static positions in the CLI count the
opaque tail *items* followed by those flattened leaves, and cannot bind the opaque prefix;
the library map is relative to the active leaves. Directly authored graph mode still
refuses shape trees under D1.

**Reconstruction and observation:** for a supported, independently accepted original,
`merge(tail, sigma, dynamic) = tail ++ reconstruct_active(sigma, dynamic)` in bottom-first
stack-item order. `reconstruct_active` rebuilds every pair/Unit value in the checked
original input shape from static I64 leaves and ordered dynamic scalar leaves; it rejects
a wrong arity or type rather than silently changing an item. The residual receives
`tail ++ dynamic_scalar_leaves` and must yield the original output types, including
the unchanged tail items. `emit_source`
binds only dynamic suffix leaves and emits checked `unit`/`pair` words for the output
shapes; its source is freshly prepared as a Declared-Text-v1 expression at that full
interface, then independently kernel-admitted, closure-checked and graph-re-extracted
before publication.
The Program emitter spells singly used total expressions at their use and retains
lexical bindings for input leaves and shared nodes. This avoids redundant `let`
quotations that exhaust the ordinary compiler's fixed work ledger on an otherwise
accepted nested output; it does not change graph-mode emission or bypass the
ordinary compiler's limits.
Ordinary `noble compile` and Core `session` do not invoke specialization; the residual
is separately selectable through ordinary declared-text `compile` or `session`.
The finite differential compares the typed stack values, not session-local value handles,
source spelling, fuel, timing, identity, or arbitrary reflection contexts.

**Closure and identity:** the original subject digest binds the exact full-stack request,
environment, root body/texts and each reached immutable definition identity, contract and
body. The residual source contains no named calls: its independently recomputed closure is
empty, while its derivation records the original closure separately. A new domain-separated
residual Program subject binds the original subject identity, accounting revision, full-stack
tail count and static map, and the independently re-admitted residual Program subject.
`--subject-id` can pin an externally retained original digest; a stale pin refuses.
Core definition identities are session-local `u64`s, not portable global names. The original
Program is not mutated or re-published.

**Limits and exclusions:** the original full root and all dependencies are checked before
selecting the graph whitelist. The extractor still refuses effects, resources, reflection,
recursion, unresolved dispatch and active types other than bounded I64/Bool/Pair/Unit shapes;
a tail containing a resource also refuses. The kernel, source frontend, extraction and
specialization caps remain in force; exhaustion publishes nothing. The active suffix
permits I64/Bool scalar leaves and shape-only `Pair`/`Unit` structure; static Bool
values, out-of-bound pair depth and effectful residualization are not admitted by
implication. A tail at the frontend's stack or source-type bound may refuse, rather
than being silently shortened.

**Evidence boundary and reopen criteria:** CLI differential tests cover no/all/mixed and
non-prefix maps, Text/Bool/Unit/Pair tails, nested active Pair/Unit shapes, named dependencies
after rebinding, repeated calls and captures on Core-original versus Declared-Text residual
ordinary execution. Direct-library refusal controls cover malformed closure, effects,
resources, reflection, recursion, unsupported active types and reachable budget boundaries;
only the CLI-representable cases have an instrumented zero-guest-request counter.
The clean-source receipt
`verification/ngraph/pe-cases/17461eb/acceptance.json` binds 18/18 registered
cases and seven auxiliary tests to commit
`17461ebf59d0348ae609427d07237e1162b9a2d7`; CASE-10 reviews the older
measurement instead of repeating it. The registered cases have explicit
execution evidence, while their proof fields remain open except that
measurement review is not a proof obligation.
The Lean model proves the graph transformation under a valid survivor map; the MC1
frame lemma is model-only. This does **not** prove that (1) every admitted Core Program
extracts to the same ordered-value semantics under the exact tail and closure,
(2) Rust's keep-walk and renumbering construct a valid survivor map for every graph,
(3) the source emitter and its pair/Unit reconstruction preserve that Program-level
semantics for every accepted shape, or (4) L3/L4 and ordinary compiled execution refine
those semantics under compatible limits and effects. The retained fixtures and case receipts
are finite witnesses, not a universal extraction/Program, Rust/model, emitter or backend
correspondence theorem. Reopen any specific obligation on an admitted counterexample,
on a change to the value/effect whitelist or source emission, or when a consumer requires
universal refinement instead of finite regression evidence. Transparent/default
specialization needs a separate adoption decision; the ordinary unspecialized path is
not changed.

### D14 — Frozen comparison protocol for partially-static offsets (issue #32 owner follow-up)

**Experiment contract, frozen before measuring:** This is an explicitly selected
checked-graph experiment, not a change to D3, the default specializer, Core
execution, or an already reflected Program. Its domain is D1's pure,
total, flat `ngraph-i64/0` slice. Represent an add/sub expression by its
wrapping `i64` offset and an **ordered** sequence of signed, shared symbolic
node identities. A shared expression is materialized once; an unshared
linear chain may be flattened. Reconstruct only a new checked residual,
with the D2 ordered input map and a distinct derivation identity. Never
cross multiplication, equality, guards, effects or a shape boundary.
The existing D7 residual-node cap applies to both intermediate and final
growth; exceeding it must refuse rather than publish a partial graph.

**Frozen comparison:** Compare the exact same checked graph and static map
through the current D3-based static-or-residual baseline (including the
separately approved #34 total identity rules) and the opt-in offset path. Fixed
families are chained addition `(x + 2) + 3`, chained subtraction
`(x - 2) - 3`, wrapping boundary `x + i64::MAX + 1`, a shared linear
subexpression used twice, a non-prefix static input, and a total expression
made dead by folding/forwarding. Check every output under the same merged
dynamic inputs at `0`, `1`, `-1`, `i64::MAX`, `i64::MIN`, and 64 fixed-seed
vectors. Include below/at/above a reduced growth cap, and distinguish
refusal from mismatched values. A nested partially known pair remains an
explicit **unsupported graph experiment**: D1 rejects authored shape
trees; D13's Program path independently supports bounded pair/Unit
reconstruction but is not automatically routed through this new offset
transform. Do not count a scalar flattening as pair evidence.

Record **separately**: residual graph node count; L3/L4 runtime node visits
on both sides; specialization and source emission/re-admission elapsed time;
first and repeated replay time; compiled retained bytes, checked-graph
retained bytes, and emitted-source bytes. Use a source-committed runner and
a clean committed source identity; timings on a shared host are descriptive
and never a pass condition. A positive structural result requires every
finite vector to agree, a checked/re-admitted residual, at least one family
with fewer graph nodes *and* runtime visits than baseline, no family with
more than 125% of baseline runtime visits or retained compiled bytes, and
both growth-limit refusals and input-order controls to fire. An adoption
**GO** additionally requires a universal Rust-to-wrapping-model and
emitter/replay correspondence result, a reviewed intentional code-inspection
semantics for the *new* Program identity, and repeatable cost benefit under
equivalent budgets. Without any of these, **NO-GO** for default adoption
even if the finite experiment improves a fixture. Reopen on a completed
proof/bridge and a new frozen matched-budget comparison, or on an admitted
counterexample (which first requires a correctness repair).

The Wei/Tan/Zhong let-insertion observation is tested here as preservation
of graph-node identity and lexical sharing in emitted source, not as
permission to move an effect or alter the result of code inspection. An
inspector may distinguish old and new code; only an explicitly requested
new residual identity is returned. The Yallop et al. partially-static
suggestion is restricted to wrapping-I64 offsets and symbolic atoms; no
F32, effectful value, or general recursive pair algebra is implied.

### D15 — Measured result and default-adoption decision (issue #32 owner follow-up)

**NO-GO for default adoption.** D14's source-committed, opt-in producer and
the raw comparison in `verification/ngraph/pe-offset-2026-11-15.json` retain
69 ordered dynamic vectors for each of seven fixed families. Every L3/L4
result and every independently re-admitted emitted residual agrees on those
finite vectors. Counts below are *totals* over 69 vectors for each graph's
runtime visits; retained bytes are from the compiled graph, not a claim about
the total process's memory.

| Frozen family | Baseline → offset nodes | Baseline → offset L3/L4 visits | Baseline → offset compiled bytes |
|---|---:|---:|---:|
| chained add | 5 → 3 | 345 → 207 | 124 → 76 |
| chained subtract | 5 → 3 | 345 → 207 | 124 → 76 |
| wrapping boundary | 5 → 3 | 345 → 207 | 124 → 76 |
| shared subexpression | 4 → 4 | 276 → 276 | 108 → 108 |
| non-prefix static input | 9 → 5 | 621 → 345 | 216 → 120 |
| dead pure total forward | 3 → 3 | 207 → 207 | 76 → 76 |
| growth cap control | 5 → 7 | 345 → 483 | 120 → 168 |

The growth control consumes **140%** of baseline visits and compiled bytes,
exceeding D14's pre-registered 125% maximum: the positive structural result
is **not** met even though four families have smaller graphs. An attempted
growth beyond the lowered `residual_nodes` cap refuses without publishing;
the unchanged D3 baseline and equal-cap exact boundary are tested. On
reduced-node cases the checked-graph retained bytes can still equal baseline
because the separate new store reserves the baseline node capacity; the
compiled bytes, emitted-source bytes and all elapsed-time categories are
recorded separately in the receipt. Timing numbers are one shared-host debug
sample per fixture, not a matched-budget speedup or cold-cache comparison.
The opt-in comparison invokes D3 first; its total specialization time is
therefore not a standalone replacement cost.

The selected normalization laws over `BitVec 64`, ordered signed atom
identities, wrapping offset folding, rank/merge, and add/sub step semantics
are kernel-checked in `NobleFoundation.PartialStatic` and inventoried by the
foundation lane. They are **model laws**, not a theorem that the Rust
normalizer, graph emitter, compiler, ordinary execution or the original
Program's stack tail refine that model for all admitted inputs. Finite tests
assert emitted `let` sharing, distinct residual identity and admission; the
old reflected Program, source, subject and unspecialized/default execution
remain unchanged. A code inspector can distinguish the *explicitly
requested new Program*: this is not a license for silent optimization of a
reflected object. The current experiment rejects authored nested partially
known pairs at D1's graph boundary (`E-AGGREGATE`); D13 separately supports
bounded pair/Unit Program reconstruction but does not route it through
offset normalization. No effectful or potentially failing operation is
moved or eliminated.

| Owner follow-up | Disposition and condition for changing it |
|---|---|
| Wei/Tan/Zhong binding store, sharing, let insertion, dead total nodes | **DONE for this bounded experiment:** shared graph identities are materialized once, re-emitted as lexical `let`s; dead pure total nodes and repeated uses are fixed cases. More general DCE/let-motion is **NO-GO** without a separate whitelist and semantic bridge. |
| Yallop partially-static mixed data and wrapping normalization | **DONE for I64 add/sub offsets with ordered signed atoms**; partial recursive pairs/F32 are **NO-GO** here. Reopen on admitted typed aggregate cases and an explicit normalization proof for them. |
| Nested pairs, repeated subexpressions, non-prefix maps, dead nodes, growth cap | **DONE or DECIDED:** scalar controls and cap boundaries are executed; nested pair is a tested refusal, not misrepresented as supported. Reopen pairs only with a D1/D13-compatible representation and proof. |
| Recipe/identity and intensional code inspection | **DECIDED:** explicit new checked residual and readmitted Program; no silent modification of the original. Reopen default adoption only after an owner-reviewed observational contract explicitly covers inspector-visible code identities. |
| #42 certificates, #33 manifest/endpoint replay, static merge and Program bridge | **DECIDED:** the #42 layer and #33 binding do not establish this Rust/model or Program bridge. Do not claim they do. Reopen on a universally checked correspondence through rank/merge, residual source emission/re-admission and ordinary replay, with #33 binding exact endpoints when this representation is used. |
| E-graphs, self-application, effectful code motion | **DECIDED not prerequisite** for the pure selected experiment; reopen only under a separately authorized enlarged domain and proof/effect policy. |

Adoption requires **all** D14 criteria: repair the 140% growth failure under
the frozen structural gate, prove universal Rust-to-model plus
emitter/replay correspondence with the static-input merge relation,
review inspector-visible identity semantics, and demonstrate repeatable
matched-budget cost benefit under a new pre-registered comparison. Reopen
on a completed proof/bridge and newly frozen measured gate, or immediately
on an admitted counterexample (first repair correctness). Until then D3
remains the only default.
