# I64 interaction-net contract: revision 2 (superseded)

Status: **superseded on 2026-11-13 by [revision 3](i64-net-contract-v3.md)**, an interaction system over
the vendored, kernel-checked [OnixResearch/inets](https://github.com/OnixResearch/inets) library. Revision
2 is a dataflow rule, not an interaction rule; it is kept only as a recorded comparison, and nothing new
builds on it. The text below is the revision as it was selected.

Former status: selected for #37. The four semantic questions left open in the candidate are decided
below, each with the reason that decided it. This document claims no proof of its own: the vocabulary
is `proofs/m3/NobleM2/InteractionNetV2.lean` and the local rule and stack reducer are
`proofs/m3/NobleM2/InteractionNetReduce.lean`, with the limits stated at the end of this document.

## The four decisions

| Question | Decision | Reason |
|---|---|---|
| Operand stacks LIFO or FIFO | **LIFO** | "deeper operand first" must be a property of the stack, not of source order; FIFO would revive the counterexample that an identity agent reorders operands |
| `Erase` eager or only on a value | **only on a value** | eager erase would drop a pending computation, which no budget or read-back check could detect afterwards |
| Does `Eq` share the I64 value domain | **yes, as 0/1** | a second wire domain reintroduces revision 1's principal-port rule; encoding `Bool` as `I64 0/1` keeps one value type on every wire and is checkable in read-back |
| Fanout synchronous or lazy | **lazy** | a synchronous copy is a global barrier: it changes the progress bound and forbids interleaving with reduction, which revision 2 would otherwise allow |

## What revision 1 gets wrong

`proofs/foundation/NobleFoundation/InteractionNetOrientation.lean` records three counterexamples that
any successor must answer:

| # | Revision-1 behaviour | Why it fails |
|---|---|---|
| 1 | a value meets an arithmetic agent only when the *principal* ports face each other | arithmetic agents have no value at their principal port, so arithmetic never fires; values reach operands only as auxiliaries |
| 2 | operand order is decided by comparing an input's *source slot* with the operand position | a value passing through `add 0` changes its source slot and bypasses the check (`identity_agents_bypass_the_operand_rule`) |
| 3 | fanout copies cannot reach an operand, because auxiliary-to-auxiliary links are refused | a shared value can never feed both operands of the same agent (`fanout_cannot_feed_an_operand`) |

## Candidate revision 2

### Agents and ports

| Agent | Principal | Auxiliary |
|---|---|---|
| `Value(v)` | the value | — |
| `Add`, `Sub`, `Mul`, `Eq` | the result wire | `1`: first operand stack, `2`: second operand stack |
| `Fanout` | the value to copy | `1`, `2`: the two copies |
| `Erase` | the value to discard | — |
| `Input(i)` / `Output(i)` | boundary endpoint | — |

### Wires

Every wire carries a value at the moment it is consumed. A wire may join any two ports except two
principals of arithmetic agents (which have no value to offer). Auxiliary-to-auxiliary value wires are
therefore **legal** — this is fix 3.

### Reduction

1. **Arithmetic fires on operands, not principals.** A binary agent with a value on *both* operand
   stacks reduces and pushes its result onto its result wire. `Add` and `Mul` are
   wrapping I64 operations with no overflow-mode dependence; `Eq` yields I64 `0`/`1` in the single
   value domain; `Sub` is `first - second` in **port order**, never in source order — this is fix 2.
2. **Deeper operand first** is a property of each operand *stack*: values pushed later are consumed
   first (LIFO). Because order comes from the port and the stack discipline, an identity agent on a
   wire cannot reorder anything.
3. **Fanout** duplicates a `Value` it receives. Its two copies are ordinary values and may be consumed
   by operands — this is fix 1 and 3 together.
4. **Erase** consumes a value and disappears. No other agent is entitled to an erasure.
5. **Output** consumes its value in boundary order; read-back fails if any output is missing or if any
   agent remains with a consumable value after the last output.

### Invariants a rewrite must preserve

- exactly one value per wire at consumption;
- the multiset of values entering is the multiset consumed by outputs, fanouts and erasures;
- the ordered output vector equals the checked graph's declared outputs, under exact wrapping I64;
- no active-pair count grows without a charged rewrite (progress bound, #37 item 5).

## What the local model covers

`proofs/m3/NobleM2/InteractionNetReduce.lean` models the rule of point 1 for the four arithmetic
agents (`fire`, `stepNode`) and states the fanout and erase effects of points 3 and 4 as formulas it
never fires; point 2 is a property of the operand stacks in the type. It does **not** model delivery:
a firing drops the value it computes instead of pushing it onto its result wire, so no agent's result
reaches another agent's operand stack. Points 3, 4 and 5 are decisions about the contract, not
behaviours of a whole-net run, and `readBack` models only the count check of point 5 — it does not
check that any agent is left holding a consumable value. Of the invariants above, the model proves
that the waiting-value count does not grow (`stepList_stackValues_le`) and that no step increases the
space potential (`stepList_spacePotential_le`); the multiset invariant, the read-back-against-graph
invariant and a charged rewrite bound need delivery, a bound graph and per-step charging, none of
which the model has. `proofs/foundation/NobleFoundation/InteractionNet.lean` remains the revision-1
structural validator.
