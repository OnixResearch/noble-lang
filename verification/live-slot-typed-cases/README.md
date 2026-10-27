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
- The nested chain shape is refused rather than mistyped, so it is outside this coverage.

## Nested chain shape

The typed lane does **not** emit the nested chain shape. A synthetic nested derivation built by its
generator is refused *before pinning* with `root reference has no pinned published slot`, so the
exclusion is deliberate rather than a coverage hole. Nested roots are covered by the canonical
LSLOT-02 lane (`crates/noble-cli/tests/live_slot_nested_roots.mjs`), which was executed this session
and observed both fixtures — `base-parent-child` and `transitive-A-B-C`, each entering at epoch 1 and
committing at epoch 2. That observation is recorded under `inherited_coverage.verified_this_session`
and establishes nothing about the typed generator.
