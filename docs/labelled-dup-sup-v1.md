# Labelled DUP/SUP profile `labelled-dup-sup/1`

**Status:** selected for the model-only #105 lane. This is an opt-in extension of the selected I64 interaction-net contract [`ngraph-net-i64/3`](i64-net-contract-v3.md); it does not change that contract's default backend or claim a Rust implementation.

The owner re-based #37 and this interaction-net family on the vendored OnixResearch/inets library on 2026-11-13. The prerequisite is recorded in the current #37 contract and issue decision. Inets supplies the generic strong-confluence, normal-form uniqueness, and reduction-bound theorems; this profile proves that its rule table is a well-formed `INet.System`, then instantiates those theorems.

## Symbols and ports

Port `0` is principal; an agent with arity `n` has auxiliary ports `1..n`.

| Symbol | Arity | Meaning |
|---|---:|---|
| `num v` | 0 | wrapping I64 value, represented by `BitVec 64` |
| `op2 k` | 2 | first operand arrives at the principal; auxiliary 1 is the second operand; auxiliary 2 is the result continuation |
| `op1 k a` | 1 | first operand `a` is stored in the agent; the principal accepts the second operand; auxiliary 1 is the result continuation |
| `dup` | 2 | the unlabelled v3 fanout agent |
| `dupL L` | 2 | labelled fanout agent |
| `sup L` | 2 | labelled ordered alternative |
| `era` | 0 | eraser |

`L` is a natural-number label. A net declares a finite duplicate-free label list; every `dupL L` and `sup L` must use a declared label. Labels are not inferred, renamed, or truncated. `dup` remains the v3 unlabelled agent; it is not an alias for `dupL L`. There is no implicit `dup ⋈ sup` rule: an unsupported active pair is stuck, and complete read-back will refuse it rather than publish a partial value.

## Rule table

Each row has its mirrored orientation as required by `INet.System.rule_symm`. Auxiliaries are listed in their agent-local order.

| Active pair | Right-hand side |
|---|---|
| `dup ⋈ num v`; `dupL L ⋈ num v` | Two `num v` agents, one on each fanout auxiliary. |
| `dupL L ⋈ era` | Two `era` agents, one on each fanout auxiliary. |
| `era ⋈ num v` | No new agent. |
| `era ⋈ sup M` | Two `era` agents, one on each SUP auxiliary. |
| `dupL L ⋈ sup L` | Projection: DUP auxiliary 1 joins SUP auxiliary 1; auxiliary 2 joins auxiliary 2. |
| `dupL L ⋈ sup M`, `L ≠ M` | Commute and nest: the old DUP auxiliaries lead to two new `sup M` principals. The old SUP auxiliaries lead to new `dupL L` principals. The first new SUP's auxiliary 1/2 receive copy 1 from the DUPs of the old SUP's auxiliary 1/2; the second new SUP's auxiliary 1/2 receive copy 2. |
| `op2 k ⋈ num a` | One `op1 k a`; its principal joins the old `op2` auxiliary 1 and its auxiliary 1 joins the old `op2` auxiliary 2, as in v3. |
| `op1 k a ⋈ num b` | One `num (k a b)`; its principal joins the old `op1` auxiliary 1. `add`, `sub`, and `mul` wrap modulo 2^64; `sub` is first minus second; `eq` is 1 exactly when the operands are equal, otherwise 0. |
| `op2 k ⋈ sup L` | OP-SUP distribution: a new `sup L` principal joins old `op2` auxiliary 2 (the result continuation); a new `dupL L` principal joins old auxiliary 1 (the other operand); the old SUP auxiliaries join the principals of two `op2 k` copies; the DUP copies join those copies' auxiliary 1 ports; the copies' auxiliary 2 ports join the new SUP auxiliaries 1 and 2, respectively. |
| `op1 k a ⋈ sup L` | OP-SUP distribution: a new `sup L` principal joins the old `op1` auxiliary 1; the old SUP auxiliaries join the principals of two `op1 k a` copies; the copies' auxiliary 1 result ports join the new SUP auxiliaries 1 and 2, respectively. |

Every other pair is unruled. The arithmetic rule table is the v3 table; `sup_free_rule_table_exact` proves it is unchanged on SUP-free symbol pairs, including the label-insensitive scalar fanout rules for `dupL`.

## Validation and observation

`NobleFoundation.NetLabelled.validate` combines inets' executable structural check with a duplicate-free declared label list, label membership for every labelled agent, and these frozen caps:

| Limit | Value | Enforced by |
|---|---:|---|
| Declared labels per net | 16 | `validate` |
| Agents | 64 | `validate` |
| Stored directed wire entries | 192 | `validate`; `XNet.ofWires` stores both directions |
| Ordered free/boundary ports | 16 | `validate` |
| Rewrite steps supplied to one observation | 256 | `observe` |
| SUP nesting depth along a value-tree path | 8 | `validateTree` and bounded read-back fuel |
| Collapsed scalar values across the complete boundary | 256 | `validateReadback` |

The limits were frozen before any measurements; no performance or runtime measurement is claimed. There is no host request, allocator, or scratch-memory implementation in this Lean model, so those runtime quantities are not represented as measured or enforced bytes.

A completed normal form is read from its entire ordered free-port list. Each root must lead through principal ports to `num` or recursively to a `sup L` whose two auxiliary ports produce the left and right trees. `readback` is fuel-bounded; cycles, excessive nesting, unsupported agents, and incomplete paths refuse. `observe` also requires the input validator, the rewrite cap, a complete tree for every boundary port, and a total represented-agent count equal to the normal form's agent count; this refuses hidden normal-form agents. `validateTree` checks declared labels, depth, and per-tree collapse size; `validateReadback` checks boundary and aggregate collapse caps.

Ordered collapse is defined by:

```text
collapse(num v)       = [v]
collapse(sup L a b)    = collapse(a) ++ collapse(b)
```

Thus the label controls interaction, while observation preserves the tree's left-to-right branch order and ignores labels when producing scalar results. `opSupFixture` checks `add 2` over `sup 7{3,5}` collapses to `[5,7]`; `op2SupFixture` checks `add` over `sup 7{3,5}` with the other operand `10` collapses to `[13,15]`.

## Trust boundary

Proofs are checked by the pinned Lean 4.35 kernel against the byte-pinned vendored `proofs/inets/InteractionNets.lean` (provenance in `proofs/inets/UPSTREAM.json`) and this module. The generic inets results establish strong confluence for well-formed nets over this system, uniqueness of normal forms when reached, and the conditional reduction bound. They do **not** prove termination for every input, a source compiler, graph-to-net correspondence, Rust/Wasm behavior, allocator/scratch limits, or runtime scheduling. HVM3 `INTERS.md` is prior art for the labelled table only; its WIP status and all external performance claims are not Noble evidence.

The runtime arm remains gated on #37's Rust backend decision and on #105's own runtime evidence. This model does not enable a backend or widen #37's admitted source subset.
