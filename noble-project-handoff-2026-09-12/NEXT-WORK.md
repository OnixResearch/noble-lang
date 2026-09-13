# Noble Next Work

## P0 — Establish the actual implementation state

Before changing architecture again:

1. inspect the real repository/current branch;
2. record compiler/runtime/toolchain versions;
3. run the existing test suite;
4. identify which core acceptance examples are actually implemented and executed;
5. distinguish code that exists from specification-only work.

Do not infer repository state from the design documents.

## P1 — Produce the canonical merged spec revision

Follow `MERGE-PLAN.md` so safety, WIT/WASI, surface renames, and Syndicate concurrency do not overwrite each other.

This is now prerequisite documentation work because implementation decisions otherwise have multiple competing previews.

## P2 — Finish the programs-as-data Wasm vertical slice

The first language milestone should prove in the implementation, with actual tests:

```text
41 [ 1 + ] run                       => 42
20 [ 1 + ] [ 2 * ] compose run      => 42
runtime_n quote [ + ] compose        => runtime Program
20 [ 1 + ] twice run                 => 22
program reflect                      => stable resolved recipe
```

Also demonstrate:

- incompatible composition rejected statically;
- runtime builder operands are not constant-fold-only;
- effectful quotation construction performs no latent effect;
- `run` does not invoke source preparation;
- `reflect` corresponds to the executed program recipe;
- live resource capture/duplication rejection in the supported resource slice.

## P3 — Complete the acceptance-checker slice

Specify/implement the finite candidate/witness acceptance boundary for the supported core:

- stack shapes/tails;
- program interfaces;
- effect bounds;
- recursive `Data`/`Capture` eligibility;
- resource obligations;
- scheme instantiation/generalization for the supported fragment.

Keep the Rust core friendly to Charon/Aeneas: safe sequential Rust, explicit owned data, simple control flow, narrow modeled dependencies.

## P4 — First Component Model vertical slice

Implement the WIT/WASI plan from DEC-W001:

1. parse one WIT world/package and include identity in the build graph;
2. generate typed Noble imports with conservative effects;
3. implement one closed Noble export matching WIT;
4. bridge one owned resource and one scoped borrowed resource;
5. emit/load a Component Model component;
6. run under a pinned WASI 0.3 profile;
7. exercise one native async boundary;
8. execute malformed/authority-denial tests.

## P5 — First proof/refinement slice

Connect the actual checker to Lean through the selected route:

```text
Lean reference semantics
       ^
       | refinement theorem
Aeneas-generated Lean
       ^
       | extraction boundary
Rust acceptance checker
```

Prove one narrow end-to-end checker/representation property before broadening scope.

## P6 — Specify the standard Syndicate concurrency profile

Write the missing normative profile rather than extending the old BEAM-first package.

First vertical example should show why dataspaces are the default coordination mechanism, e.g. service presence/readiness/dependency state:

```text
Noble Program reactions
    -> scoped facet
    -> assertions / interests
    -> automatic retraction on scope termination
    -> Preserves protocol values
    -> WIT boundary for component crossing
```

Include capability denial, malformed wire data, actor/facet failure cleanup, and no-shared-mutable-memory tests.

## P7 — Reconcile choreography and durability

Only after the standard concurrency profile exists:

- project choreography endpoints onto Syndicate actors/facets/dataspaces;
- keep durable execution optional;
- if durability is added, record/replay all concurrency observations that can affect behavior, not merely external RPC results.

## Definition of “next handoff is better”

The next handoff should be able to replace design uncertainty with an executed status table such as:

```text
core checker                       passing / failing / unsupported
runtime quote/compose/run          passing / failing / unsupported
reflect recipe                     passing / failing / unsupported
Wasm backend                       passing / failing / unsupported
WIT import/export                  passing / failing / unsupported
owned/borrowed resource bridge     passing / failing / unsupported
WASI profile                       passing / failing / unsupported
Syndicate profile                  passing / failing / unsupported
Lean core obligations              proved / open
Rust checker refinement            proved / open
backend correspondence             proved / open
```
