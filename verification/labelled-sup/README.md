# Labelled DUP/SUP lane (#105)

**Model:** GO. **Runtime:** HOLD pending the #37 `nets-rust` GO/NO-GO and #105-specific runtime evidence. See [`decision.md`](decision.md) and [`labelled-dup-sup-v1.md`](../../docs/labelled-dup-sup-v1.md).

## Proved

- The labelled rule table is an `INet.System`; its well-formedness and mirror-symmetry obligations are proved in `NobleFoundation/NetLabelled.lean`.
- `labelledStrongConfluent` and `labelled_confluence` instantiate inets' strong-confluence theorem; `labelled_normal_form_unique` proves unique normal forms with equal reduction lengths, and `labelled_reduction_bounded` specializes the conditional reduction bound.
- `validate_sound` proves structural well-formedness, bounded duplicate-free labels, caps for agents/wire records/boundary ports, and label membership. The tree/read-back validators prove declared-label, depth and ordered-output bounds.
- Kernel-checked fixtures cover equal-label projection, different-label nesting, OP1-SUP collapse `[5,7]`, OP2-SUP collapse `[13,15]`, the plain v3 scalar addition path, and refusal when one boundary output cannot be read back (no successful prefix result).
- The four seeded faults (wrong equality, swapped branches, missing nest, erased SUP) are separated from the expected model semantics by theorem.

## Tested

Build the module directly with the repository's pinned Lean 4.35 environment:

```sh
cd proofs/foundation
lake build NobleFoundation.NetLabelled
```

The fixture statements are closed by Lean's kernel-checked `decide` proofs in that module. The module is imported by the foundation root and its public/private theorem lists are registered in `inventory.json`. The repository-wide `lean-proofs.sh` lane builds m3/foundation and audits the inventoried theorem statements and axioms:

```sh
verification/ci/lean-proofs.sh --allow-dirty <output-directory-outside-repository>
```

## Open

- Rust lowering/reduction and a graph-to-net correspondence.
- Differential comparison to the #37 graph reference, packed evaluator, or Wasm; no Rust or runtime tests were run here.
- Runtime scheduling, allocation/scratch limits, cancellation, source/net identity binding, and performance.

This module-only model does not close #105's runtime gate, adopt a backend, or change the default v3 path.
