# Typed live-slot case receipt (issue #49)

`receipt.json` binds one measured run of `crates/noble-cli/tests/live_slot_typed_cases.mjs` to the
exact selected CLI bytes (`sha256 2bbe9f62…fac17`), the lane/runner/helper digests and the retained
foundation receipt. Regenerate with a pinned Node from the repository root:

```
env -i PATH=/run/current-system/sw/bin:<bash-interactive>/bin:<node>/bin:/usr/bin:/bin HOME=$HOME \
  <node>/bin/node verification/live-slot-typed-cases/receipt.mjs <absolute prebuilt noble> /tmp/receipt-NEW
```

The session PATH matters: with only the Node and `/usr/bin` paths on `PATH` the first typed session
times out (the CLI's worker shell never answers), so the bash-interactive and system paths are part
of the recipe rather than decoration.

## Sections and what they do not establish

- `model` — two named `NobleFoundation.LiveDispatch` theorems and the retained foundation receipt
  `d2aa7a382`. They do **not** establish any dispatch/effect count, refusal class or boundary
  measured here.
- `implementation_comparison` — the measured lane output: typed coverage by shape/ceiling/reborrow,
  the controlled negative family, the minimization report, the retention budget and the type
  boundary pairs, plus the exact commands. It does **not** establish the model theorems.
- `host_correspondence` — the selected CLI bytes and the session identifiers. It establishes nothing
  about a differently built CLI or a different fixture authority.
- `inherited_coverage` — coherent maps, revocation/abnormal prefix, stale-CAS/ABA and nested roots
  are measured by their own lanes; only their bytes are bound here.

## Recorded non-claims

- A passing typed case is not a theorem and promotes no LSLOT conformance case.
- Negative cases assert declared semantic classes and stages, never diagnostic wording; the observed
  diagnostics are reported for transparency only.
- Minimality is along the tried witness dimensions (repeat, quota, program index) only.
- `quota` is a declared retention quota field, not an invocation budget, so no exhausted class is
  asserted for it.
- The nested chain case is one fixture assembled through the canonical bootstrap derivation; it does
  not establish that every nested or transitive chain type-checks.

## Nested chain shape

The typed lane **does** now emit the nested chain shape, through its own derivation
(`runChainCase`): the caller's declared formals include a reference to a slot that itself takes a
reference, so the chain cannot be assembled by the sequential publish loop (the nested type has no
pinned published slot yet). It is assembled the canonical way instead — each member installed under
its own selected host source identity, one `bootstrap` publishing both members at epoch 1, then
`reflect` and a single invocation — and the report asserts the declared expectation (the middle
slot dispatches once per reference reaching it: A, B, B, with one pinned epoch) plus an independent
control in which a member that does not match its slot interface is refused before anything is
published.

That is still one fixture, not a general nested-type generator: transitive chains and repeated
nested references are not generated here. The canonical LSLOT-02 lane
(`crates/noble-cli/tests/live_slot_nested_roots.mjs`) remains the owner of the interleaved
bootstrap/checkpoint behaviour of nested roots and is only digest-bound in this receipt.
