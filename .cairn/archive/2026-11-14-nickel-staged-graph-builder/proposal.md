## Why

`ngraph-i64/0` is currently described as extraction-only, so a graph constructed in Nickel cannot enter Noble's checked numeric-graph path without an explicit authoring policy. A separate, bounded producer route lets users define reusable arithmetic templates and explicit graph composition without treating Nickel functions as programs to compile or changing Noble's runtime IR.

## What Changes

- Define the `noble-ngraph-nickel/v1` constructor-data envelope and a repository Nickel library for templates, instances, typed handles, explicit wiring, and ordered exposed inputs and outputs.
- Add a distinct `authored-nickel` subject route for only finite acyclic I64 graphs containing concrete I64 inputs, exact I64 constants, and wrapping Add/Sub/Mul. The existing extracted route and F32 `Param` semantics remain unchanged.
- Run pinned Nickel 1.17.0 as an external authoring evaluator, force complete demanded constructor export, lower its JSON to the existing graph IR, and require native `check_with` admission under explicit bounded budgets.
- Retain real evaluation, composition, delayed-contract, provenance, and stale-record controls; document the exact claims and remaining correspondence gaps.

## Impact

- **Files**: `.cairn/changes/nickel-staged-graph-builder/`, `nickel/ngraph/`, existing `noble-ngraph` and `noble-cli` modules/test targets, `tools/`, and the root `README.md`.
- **Testing**: pinned Cairn validation; pinned Nickel evaluation; `noble-ngraph` and `noble-cli` tests; workspace clippy, formatting, Octet, and documents checks.
