# I64 interaction-net contract, revision 3 (`ngraph-net-i64/3`)

**Status: selected.** The owner re-based the interaction-net work (#37, #105, #109, #110, #117) on
[OnixResearch/inets](https://github.com/OnixResearch/inets) on 2026-11-13. inets formalises
Lafont's interaction nets in Lean 4.35 without Mathlib, generically over the symbol type, and proves
for every interaction system: strong confluence (`INet.Step.diamond`), unique normal forms reached in
the same number of steps by every reduction (`INet.normal_form_unique`), the reduction bound
(`INet.reduction_bounded`), observational equivalence and compaction, simultaneous firing of
independent pairs (`INet.Indep.*`), and a shared-memory thread machine refined to the abstract
reduction with exact action, work and time bounds (`INet.MRun.*`, `INet.XMach.*`, `INet.IMach.*`).

Revision 2 ([`i64-net-contract-v2.md`](i64-net-contract-v2.md), a dataflow rule that is not an
interaction rule) is **superseded** and kept only as a recorded comparison. Its Lean models in
`proofs/m3/NobleM2/InteractionNet{V2,Reduce}.lean` describe revision 2 and say nothing about
revision 3.

The vendored library is `proofs/inets/InteractionNets.lean`, byte-identical to upstream commit
`98bbb01070664e4462a35efa5d62de6f27143572` (provenance and digest: `proofs/inets/UPSTREAM.json`),
compiled by `proofs/foundation` as the library `InteractionNets`.

## 1. Admitted graphs

The input is a graph admitted by `noble-ngraph` under profile `ngraph-i64/0`
(`crates/noble-ngraph/src/check.rs`), restricted to the net whitelist:

| Graph node | Admitted |
|---|---|
| program input (I64 slot) | yes |
| I64 constant | yes |
| `I64Add`, `I64Sub`, `I64Mul` | yes |
| `I64Eq` (Bool result) | yes |
| output list, including repeated outputs, Unit/pair shapes flattened in order | yes |
| guards, facts, F32, parameters, any other operation | **refused** before lowering (`unsupported`), nothing is produced |

The Lean counterpart is `NobleFoundation.PackedSchedule.Graph` extended to an ordered output list,
under `WellFormed` (operands strictly earlier) and a well-typedness premise (add/sub/mul/eq take I64
operands). A net is built **per input vector**: inputs become values (below).

## 2. The interaction system `NetI64`

Symbols (inets `S`), with arities (number of auxiliary ports; port 0 is principal, ports `1..ar`
auxiliary, as in inets):

| Symbol | Arity | Principal port faces | Auxiliary ports |
|---|---|---|---|
| `num v` (`v : BitVec 64`) | 0 | its consumer | — |
| `op2 k` (`k ∈ {add, sub, mul, eq}`) | 2 | the producer of operand 1 | 1: operand 2's producer; 2: the result's consumer |
| `op1 k a` (`a : BitVec 64`) | 1 | the producer of operand 2 | 1: the result's consumer |
| `dup` | 2 | the producer of the value | 1, 2: the two consumers (copy order is the canonical order of §3) |
| `era` | 0 | the producer of an unused value | — |

Rules (each given in both orientations; the reverse orientation is the inets mirror `Rhs.swap`):

| Active pair | Right-hand side |
|---|---|
| `op2 k ⋈ num a` | one new `op1 k a`; its principal port joins `op2`'s auxiliary 1, its auxiliary 1 joins `op2`'s auxiliary 2 |
| `op1 k a ⋈ num b` | one new `num (k a b)`; its principal port joins `op1`'s auxiliary 1. `add`/`sub`/`mul` wrap modulo 2^64 (`sub` is `a - b`: operand 1 minus operand 2); `eq` gives `1` if `a = b`, else `0` |
| `dup ⋈ num v` | two new `num v`; they join `dup`'s auxiliary 1 and 2 |
| `era ⋈ num v` | nothing |

There is no other rule. Any other pair whose principal ports meet (for example `num ⋈ num`,
`op2 ⋈ op2`) is **stuck**; a translated well-typed graph never produces one (to be proved), and a
reducer that meets one reports `internal-invalid`, never a result.

### Revision-2 operand-stack multiplicity (from #124)

Revision 2's `Operands : List Value` could hold multiple values if two producers delivered to one
operand port, making LIFO consumption schedule-dependent; revision 2 never stated single-producer
admission. This question is **decided: superseded by revision 3**. In revision 3 there are no
operand stacks: every inets wire is one edge between exactly two port endpoints, so each consumer
port has one producer endpoint (and fanout is represented by an explicit `dup` agent).

## 3. Translation and read-back

- Each program input slot `i` becomes `num (inputs i)`; each constant `c` becomes `num c`.
- Each operation node becomes `op2 k`, with operand 1's value wired to its principal port and
  operand 2's value wired to its auxiliary 1 (`x - y` keeps `x` on the principal port).
- A value with `n ≥ 2` uses (operand positions and output positions, in the canonical order:
  consumers by ascending node index, operand 1 before operand 2, then outputs in output order) is
  fanned out by a left-leaning chain of `n - 1` `dup` agents; a value with no use gets one `era`.
- The net's interface (free ports) is the ordered output list; output `j` is free port `j`.
- **Read-back** of a normal form: every interface port must be joined to the principal port of a
  `num v`, and no other agent may remain; output `j` reads `v`, presented as Bool `v = 1` when the
  output node is `eq`. Anything else (a remaining agent, a stuck pair, an interface port joined to
  something other than a `num`) is a refusal (`internal-invalid`), never a partial result.

## 4. Observation relation and counts

- **Observation:** for an admitted graph and an input vector, the read-back of the net's normal form
  equals the graph reference outputs (`eval::reference` in Rust; `PackedSchedule.value` in Lean).
- **Interaction count:** every reduction of the translated net to its normal form has exactly
  `K = 2·#ops + #dups + #eras` steps (by inets' unique-normal-form theorem the count does not depend
  on the schedule).
- **Live agents** never exceed the initial count `#inputs + #constants + #ops + #dups + #eras`
  (no rule increases the number of agents; `dup ⋈ num` keeps it).
- **Budgets** (Rust): interaction fuel, agent and wire capacities, charged before growth; a refusal
  below the limit, success at the limit, and exhaustion reported as `exhausted`, never as a partial
  result.

## 5. Trust and identities

- Proof authority: the Lean kernel (pinned Lean 4.35.0) over the vendored inets file and the noble
  modules; axioms audited per theorem (`propext`, `Classical.choice`, `Quot.sound` only).
- Trusted: the vendored file's provenance (`proofs/inets/UPSTREAM.json`), the Rust lowering and
  reducer until a correspondence is established (finite differential tests and a kernel-checked
  corpus of real lowered nets are evidence, not a refinement proof), the host for timing.
- Identities: the contract revision `ngraph-net-i64/3`, the graph identity, the lowering revision
  and the input vector bind every retained net and result.
