# Effect explanations prototype (#96)

`crates/noble-contracts/tests/source/causes.rs` is a test-only prototype that explains why a checked Program carries an effect. `run.mjs` runs its report from a clean checkout and retains it:

```sh
env -i HOME="$HOME" PATH=/run/current-system/sw/bin \
  "$(node -p 'require("./policy/tool-selection.json").tool_paths.node.output')/bin/node" \
  verification/effect-causes/run.mjs NEW_EXTERNAL_DIRECTORY
```

## How it works

Facts come only from the kernel's own checked structure. Only the root's `Checked` survives `Session::prepare`, so every body is re-checked with `noble_kernel::acceptance::check`: the root and each specialization in `Submission::definitions`. Each derivation is then joined with its candidate node.

| Node | Fact |
|---|---|
| invocation of a word whose scheme fixes an effect (`test.emit`) | a direct contribution with a host-operation origin |
| invocation of a named row with no body whose scheme fixes an effect (`test.abort`) | a direct contribution with a declared origin. No host operation is claimed for it |
| invocation of a word whose scheme carries an effect variable (`run`, `dip`, `if`, `list.case`) | its instantiated effects, as a latent bound whose originating construction is unavailable |
| invocation of a named word with a specialization | a call edge into that body |
| quotation literal | construction only. Its own context's effects stay latent and do not reach the enclosing context |
| signed self occurrence, live dispatch | unsupported, reported as such |

A bounded fixed point (rules `noble-effect-causes/v1`) computes each context's effects. It charges every edge visit against a budget and records the first reason each effect reached each context. The resulting witness DAG yields a path from the root through call sites to the contributing node. A context that reaches an unknown target is incomplete: its empty set is never a purity claim.

The kernel is the comparison:
- every body's checked effects;
- every call and contributing node's derived effects;
- every quotation's latent bound, which its body's effects must fit.

For each root effect, the root is resubmitted without it. The kernel must refuse it with `EffectInclusion`, and the analysis must explain it.

## Results

All ten fixtures agree with the kernel on every body and checked node:
- a chain;
- a diamond;
- quotations constructed versus run, nested both ways;
- distinct instantiations of one callee;
- composition with a declared `test.abort`;
- branches;
- saved dependencies after rebinding.

The kernel refuses each root effect once it is removed from the bound.

| Requirement | Observed |
|---|---|
| chain `a → b → test.emit` | the path from the root goes through both call sites to the host node |
| diamond sharing an effectful callee | both root call sites keep their own path, into two specializations with one `source_identity` |
| constructing versus running a quotation | `[ … test.emit ] drop` has no effect; its quotation context keeps `test.emit` latent; `run` and nested runs carry it |
| distinct instantiations | `apply` specialized twice carries `test.emit` in its first instance only |
| declared-but-unused effect | inferred latent bounds are exact (`if` branches and `compose` operands keep their own). A declared bound wider than the body is accepted by the kernel and labelled bound-only, with no contributor |
| saved Programs after rebinding | `f` keeps the old `g` and carries its effect; `h`, written the same way after the rebinding, does not. All four bodies have distinct identities |
| synthetic cycles | with a seed: 4 rounds, the effect in every body; without: 1 round, no effect |
| unknown targets, exhaustion | an unknown target makes every context reaching it incomplete; a 3-visit budget stops with `Exhausted` |
| seeded faults | a lost call edge and treating construction as running are each caught by a kernel mismatch |

The cost is small: at most 7 facts and 24 edge visits per fixture.

## Compared with the flat report

`noble-live-inspection/v1` lists the union of the selected interface's and its witnesses' effects. The prototype adds, for each effect:
- which call sites carry it;
- the path to the node that contributes it;
- whether that node is a host operation, a declared row or a latent bound;
- which quotations hold it latent without running it;
- which declared effects are only bounds.

For named calls that is a real improvement: a diamond or a rebinding is visible only this way.

## Limits

- **Higher-order origins are missing.** For a run Program, the path ends at `run` with a latent bound. Which quotation construction flowed to that `run` would need a value-flow relation the kernel does not record. This is the common case for quotations.
- **Specialization derivations are recomputed.** They are not retained, so each body is checked a second time.
- **Identities are session-local.** Explanations do not survive the session.
- **Coverage is plain sessions only.** Module sessions (declared signatures, bound adapters), live dispatch and Programs that are runtime inputs are outside the fixtures.

## Recommendation

Stop at the prototype. A separately scoped integration is worth planning only if the inspection consumer (#76) wants named-call paths. It would need two things first:
- `Prepared` retaining per-specialization derivations;
- a decision on the higher-order origin gap, either a value-flow relation or an explicit "latent, origin unavailable" in the user-facing report.

Without the second, most quotation effects stop at an unexplained bound, and the explanation would promise more than it delivers.
