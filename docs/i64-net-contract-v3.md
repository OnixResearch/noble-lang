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

## 6. Pinned IVM comparison and mapping decision (#37)

The external comparison is to Vine/Ivy/IVM at
[`cf0c325834d5bf4c8d43aa36f7fb76faf2496988`](https://github.com/VineLang/vine/tree/cf0c325834d5bf4c8d43aa36f7fb76faf2496988),
specifically its [interaction dispatcher](https://github.com/VineLang/vine/blob/cf0c325834d5bf4c8d43aa36f7fb76faf2496988/ivm/src/runtime/interact.rs)
and [Ivy-to-IVM encoder](https://github.com/VineLang/vine/blob/cf0c325834d5bf4c8d43aa36f7fb76faf2496988/ivm/src/program.rs).
This is a comparison of contracts, **not** a translation or equivalence claim.

| Boundary | Pinned IVM path | Selected Noble revision 3 |
|---|---|---|
| Numeric active pair | `ExtFn` meets `ExtVal`; `call` invokes a registered split function on one argument, or a merge function when a second `ExtVal` is already available through the right-hand-side wire. Otherwise it creates a swapped `ExtFn` to await that value. The function and value representations are host registrations. | `op2 k ⋈ num a` creates `op1 k a`; `op1 k a ⋈ num b` computes the *built-in* wrapping `BitVec 64` operation. `Sub` is `a - b` in source-operand port order. No external arithmetic callback, N32 truncation or floating conversion is selected. An IVM merge would need a separately checked argument-order and exact-two-word I64 mapping. |
| Principal and auxiliary ports | `link` queues non-wire active ports on fast/slow worklists; `interact` dispatches principal-port pairs by `Tag` (`Comb`, `ExtFn`, `ExtVal`, `Erase`, `Graft`). The encoder's `Instruction::Binary(tag, label, [reg, ra, rb])` records its root and two children. | inets port 0 is principal and ports 1..arity are auxiliaries. Operand one faces `op2`'s principal, operand two joins auxiliary 1; the result leaves auxiliary 2. A value **must** meet the operation's principal in the first stage. IVM tag/label and register conventions cannot be treated as this port map without a checked adapter. |
| Duplication and erasure | Equal-label combinators annihilate; other `Comb`/`ExtFn` pairs can commute, creating four nodes. `Erase` goes through `copy`, and copying an `ExtVal` checks its registered copyability, setting a flag for non-copyable values. Grafts have their own copy/graft path. | Only `dup ⋈ num v` makes two ordered `num v` copies and only `era ⋈ num v` drops a value. Commutation, equal-label annihilation, grafts, effectful/non-copyable values and all other pairs are **not** equivalent Noble rules. |
| Ordered observations | `Encoder::encode` requires exactly **one** `TreeNet.free` root and constructs one `Register::ROOT`; it does not itself supply Noble's multi-output ordered read-back. | The checked graph declares an ordered output list; every free port at that index must meet a distinct numeric agent and no other agent may remain. Mapping an Ivy root or external result sink to this list would require a separately checked interface and independent ordered-output oracle. |

This decision is **revision 3, not IVM-as-backend and not a silent revision of the old model**.
Revision 1's [orientation counterexamples](../proofs/foundation/NobleFoundation/InteractionNetOrientation.lean)
remain regression subjects: `oriented_active_pair` and
`deeper_minus_top_has_only_the_output_pair` show result-facing arithmetic cannot fire on a
numeric principal partner; `value_into_result_port_is_accepted` shows the old structural validator
does not enforce its intended orientation; `fanout_cannot_feed_an_operand` prevents a shared
value reaching an operand; and `identity_agents_bypass_the_operand_rule` shows input slot
comparison is not source operand correspondence. Revision 2's LIFO operand stacks and
auxiliary-to-auxiliary value delivery were a superseded **dataflow** alternative, not an inets
step rule. Revision 3 instead uses operand-facing `op2`/`op1`, explicit `dup`/`era`, one
producer per wire, source-node/port-order validation and separately checked ordered read-back.
The historical fixtures do not establish a Rust refinement proof; no IVM run is evidence for
Noble reduction, progress or read-back.

## 7. Optional external Ivy/IVM experiment: not pursued here

The optional #37 Ivy/IVM execution comparison is **not pursued in this repository**.
No pinned Vine build, Ivy emitter, registered IVM extrinsics, host runner or IVM output
is part of `noble-ngraph`, its safe-Rust reducer, or the revision-3 proof claim.
The external work belongs to compiler-lab
[#168 (differential lane)](https://github.com/brittonr/compiler-lab/issues/168),
[#169 (runtime failures)](https://github.com/brittonr/compiler-lab/issues/169),
[#170 (resource effects)](https://github.com/brittonr/compiler-lab/issues/170),
[#171 (transform validation)](https://github.com/brittonr/compiler-lab/issues/171), and
[#172 (CPU benchmarks)](https://github.com/brittonr/compiler-lab/issues/172).
Their status is independent of this local backend's correctness and NO-GO decision.

An external comparison could be admitted **only as separate evidence** after pinning
the Vine compiler/runtime and exact source/graph/net bytes; specifying the complete
I64-to-two-N32 bit-pattern and ordered-output adapters; restricting registered extrinsics
and result sinks to the chosen pure profile; imposing process-memory, wall-clock, output
and net-heap caps; and preserving timeout, process failure, malformed output and
unsupported interruption as distinct outcomes. It must compare exact observations
against an independent I64 oracle, Noble's graph reference, packed evaluator and
ordinary Wasm where applicable, with truncation, carry/borrow, operand-swap,
output-permutation and mapping fault controls. It must report whether each case used
Vine N64 or direct Ivy/custom extrinsics, and record construction, validation,
execution, read-back and cleanup separately, including losses and the direct scalar
baseline. IVM's unsafe runtime/compiler would be new **experimental trusted
components**, not a dependency or proof of Noble's safe-Rust path. No external
measurement changes the frozen continuation criteria or authorizes default adoption.

## 8. Finite schedule observations and fuel outcomes

The local [Rust reducer tests](../crates/noble-ngraph/src/net.rs) run the same
validated, shared-subtraction/square/equality graph under LIFO, FIFO and
SplitMix64 seeds `1` and `0x5eed`. These are worklist **selection orders**,
not revision 2's rejected FIFO operand-stack semantics:

| Separate observation | Finite evidence | Limit |
|---|---|---|
| Successful values and count | `schedules_have_identical_values_counts_and_live_bound` uses `(i64::MIN, -1)` and checks four complete results agree, terminate normally in ten interactions and stay under the initial live-agent bound. `reduction_matches_reference_and_exact_interaction_bound` compares the `(11, 4)` result with the normative graph reference. | Agreement on these rows does not prove graph-to-Rust correspondence for arbitrary inputs. |
| Termination and distinct legal order | In [the Lean model](../proofs/foundation/NobleFoundation/NetI64.lean), `parallel_sub_schedules_start_differ` chooses different independent first active pairs; `parallel_sub_schedules_normalize` checks both complete four-step runs; `parallel_sub_schedules_agree` compares ordered positive/wrapping read-back. `parallel_sub_schedules_same_normal_form` instantiates inets' **conditional** normal-form uniqueness and equal-length theorem. | The fixture proves two model runs, not that every checked Rust lowering terminates; the model theorem assumes both normalizations succeed. |
| Fuel boundary and refusal | `schedule_fuel_boundary_refuses_without_partial_readback` runs `(11, 4)` under all four Rust schedules with fuel `K-1`, `K` and `K+1` (`K=10`). At `K-1` each returns only `Exhausted(Fuel)` with nine interactions and required amount ten; at and above `K` each terminates with the normative complete result and ten interactions. The test also checks remaining wire counts. | **No schedule-dependent fuel-exhaustion difference was observed or claimed.** The reducer charges one fuel and removes one wire per firing, and its agent/wire caps validate the input before the schedule runs. A claimed same-net/same-fuel difference in the public exhaustion result would contradict this tested finite profile and the equal-length theorem for normalizing abstract runs. Per-rule transient storage, physical time and interrupted/stuck nets are different comparisons; no schedule-dependent resource-exhaustion receipt exists. |

These tests are executable finite evidence, not retained per-order runtime traces.
The source-bound [NO-GO decision](../verification/ngraph/net-decision.md) and its
timing receipt compare backends, not all four worklist schedules. The old
`fifo_control_schedules_differ` fixture belongs to the rejected revision-2
operand-delivery control; it is **not** a legal revision-3 schedule counterexample.
