# I64 interaction-net contract: revision 2, selected

Status: **selected for #37**. The four semantic questions left open in the candidate are decided
below, each with the reason that decided it. Dependent issues (#105, #107, #108, #109, #110, #112–#115)
may now reference revision 2 by name; they must still say which parts of it they implement, and no
proof is claimed by this document — `InteractionNet.lean` remains a structural validator and
reduction semantics are not implemented.

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
   wrapping I64 operations with no overflow-mode dependence; `Eq` yields `Bool`; `Sub` is
   `first - second` in **port order**, never in source order — this is fix 2.
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

## What needs ratification before this is selected

| Question | Candidate answer | Why it matters |
|---|---|---|
| Are operand stacks LIFO or FIFO? | LIFO | determines whether "deeper operand first" is a stack law or a source-order rule; FIFO would revive counterexample 2 |
| Does `Erase` fire eagerly or only when its wire carries a value? | only on a value | eager erase would silently drop a pending computation |
| Are `Eq` results `Bool` values on the same wires as I64? | yes, one value domain | otherwise `Eq` needs a second wire type and revision 1's principal rule reappears |
| Is fanout synchronous (both copies before either is consumed)? | no, lazy | synchronous copying forces a barrier and changes the progress bound |

Selecting revision 2 (or amending these four answers) is the decision #105, #107–#110 and #112–#115
are waiting on. Nothing here is claimed as proved; `InteractionNet.lean` remains a structural
validator, and reduction semantics are not implemented.
